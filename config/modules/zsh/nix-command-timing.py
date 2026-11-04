"""Stream a Nix command's output and report elapsed time between stage messages."""

import argparse
import errno
import os
import pty
import re
import selectors
import signal
import subprocess
import sys
import termios
import time
import unicodedata
from contextlib import ExitStack

ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


def stage_for(tool, line):
    line = ANSI.sub("", line).strip()
    if tool == "home-manager":
        if line == "Starting Home Manager activation":
            return "Activation 준비"
        match = re.fullmatch(r"Activating ([A-Za-z0-9_-]+)", line)
        if match:
            return "Activation: " + match[1]
    else:
        if line == "building the system configuration...":
            return "시스템 평가·다운로드·빌드"
        if line.startswith("stopping the following units: "):
            return "서비스 중지"
        if line == "activating the configuration...":
            return "시스템 activation"
        if (
            re.fullmatch(r"reloading user units for .+\.\.\.", line)
            or line == "restarting sysinit-reactivation.target"
            or line.startswith(
                (
                    "reloading the following units: ",
                    "restarting the following units: ",
                    "starting the following units: ",
                )
            )
        ):
            return "서비스 적용 (Home Manager 포함)"
        if line == "Done. The new configuration is" or line.startswith(
            "Done. The new configuration is "
        ):
            return "마무리"
    return None


class Timings:
    def __init__(self, tool, started):
        self.tool = tool
        self.started = self.boundary = started
        self.current = "준비·평가·다운로드·빌드" if tool == "home-manager" else "준비"
        self.rows = []
        self.saw_stage = False

    def line(self, line, now):
        stage = stage_for(self.tool, line)
        if stage is not None:
            self.saw_stage = True
            if stage != self.current:
                self.rows.append((self.current, now - self.boundary))
                self.current, self.boundary = stage, now

    def finish(self, now):
        total = now - self.started
        rows = (
            self.rows + [(self.current, now - self.boundary)]
            if self.saw_stage
            else [("전체 (단계 로그 없음)", total)]
        )
        return rows + [("합계", total)]


def duration(seconds):
    tenths = round(max(0, seconds) * 10)
    minutes, remainder = divmod(tenths, 600)
    return f"{minutes}분 {remainder / 10:.1f}초"


def width(text):
    return sum(
        0
        if unicodedata.combining(c)
        else 2
        if unicodedata.east_asian_width(c) in "WF"
        else 1
        for c in text
    )


def report(tool, rows, returncode, output):
    cells = [("단계 (로그 기준)", "소요 시간")] + [
        (label, duration(elapsed)) for label, elapsed in rows
    ]
    widths = [max(width(row[index]) for row in cells) for index in range(2)]
    border = "+-" + "-+-".join("-" * size for size in widths) + "-+"
    print(f"\n[{tool}] 단계별 소요 시간 (종료 코드: {returncode})", file=output)
    print(border, file=output)
    for index, row in enumerate(cells):
        print(
            "| "
            + " | ".join(
                cell + " " * (size - width(cell))
                for cell, size in zip(row, widths, strict=True)
            )
            + " |",
            file=output,
        )
        if index == 0 or index == len(cells) - 2:
            print(border, file=output)
    print(border, file=output, flush=True)


def run(tool, command):
    with ExitStack() as streams:
        return run_with_streams(tool, command, streams)


def run_with_streams(tool, command, streams):
    timings = Timings(tool, time.monotonic())
    outputs = []
    terminals = {}
    try:
        for output in (sys.stdout.buffer, sys.stderr.buffer):
            is_terminal = output.isatty()
            read_fd, write_fd = pty.openpty() if is_terminal else os.pipe()
            reader = streams.enter_context(os.fdopen(read_fd, "rb", buffering=0))
            writer = streams.enter_context(os.fdopen(write_fd, "wb", buffering=0))
            if is_terminal:
                # Keep the PTY's newline translation: sudo puts the real terminal
                # in raw mode while the command runs, so it cannot add the CR.
                termios.tcsetwinsize(read_fd, termios.tcgetwinsize(output.fileno()))
                terminals[read_fd] = output
            outputs.append((reader, writer, output))
        process = subprocess.Popen(command, stdout=outputs[0][1], stderr=outputs[1][1])
        for _, writer, _ in outputs:
            writer.close()
    except OSError as error:
        print(f"{command[0]}: {error}", file=sys.stderr)
        result = 127 if isinstance(error, FileNotFoundError) else 126
        report(tool, timings.finish(time.monotonic()), result, sys.stderr)
        return result

    def forward_signal(number, frame):
        if process.poll() is None:
            try:
                process.send_signal(number)
            except ProcessLookupError:
                pass

    def resize(number, frame):
        for fd, output in terminals.items():
            termios.tcsetwinsize(fd, termios.tcgetwinsize(output.fileno()))
        forward_signal(number, frame)

    previous = {
        number: signal.signal(number, forward_signal)
        for number in (signal.SIGINT, signal.SIGTERM)
    }
    if terminals:
        previous[signal.SIGWINCH] = signal.signal(signal.SIGWINCH, resize)
    buffers = {}
    try:
        with selectors.DefaultSelector() as selector:
            for pipe, _, output in outputs:
                selector.register(pipe, selectors.EVENT_READ, output)
                buffers[pipe] = b""
            while selector.get_map():
                events = selector.select(timeout=0.1)
                # Detached descendants can retain a pipe after the command exits.
                if not events and process.poll() is not None:
                    break
                for key, _ in events:
                    try:
                        data = os.read(key.fd, 65536)
                    except OSError as error:
                        # Linux PTY masters report EIO when the slave closes.
                        if key.fd not in terminals or error.errno != errno.EIO:
                            raise
                        data = b""
                    now = time.monotonic()
                    if not data:
                        if buffers[key.fileobj]:
                            timings.line(
                                buffers[key.fileobj].decode(errors="replace"), now
                            )
                        selector.unregister(key.fileobj)
                        terminals.pop(key.fd, None)
                        key.fileobj.close()
                        continue
                    key.data.write(data)
                    key.data.flush()
                    lines = re.split(rb"[\r\n]", buffers[key.fileobj] + data)
                    buffers[key.fileobj] = lines.pop()[-65536:]
                    for line in lines:
                        timings.line(line.decode(errors="replace"), now)
            for key in list(selector.get_map().values()):
                if buffers[key.fileobj]:
                    timings.line(
                        buffers[key.fileobj].decode(errors="replace"), time.monotonic()
                    )
                terminals.pop(key.fd, None)
                key.fileobj.close()
        code = process.wait()
    except BaseException:
        if process.poll() is None:
            process.terminate()
        process.wait()
        raise
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)
    result = code if code >= 0 else 128 - code
    report(tool, timings.finish(time.monotonic()), result, sys.stderr)
    return result


def main():
    signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tool", choices=("home-manager", "nixos-rebuild"), required=True
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required after --")
    return run(args.tool, command)


if __name__ == "__main__":
    sys.exit(main())
