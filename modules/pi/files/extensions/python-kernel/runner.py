"""Session-local Python worker. Control frames use fd 3, never stdout/stderr.

This is execution with the user's privileges, not a sandbox. Limits bound the
transport/capture buffers, not the memory or side effects of arbitrary Python.
"""
import ast
import asyncio
import base64
import __future__
import inspect
import io
import json
import os
import reprlib
import struct
import sys
import threading
import traceback

MAX_REQUEST = 400_000
TEXT_LIMIT = 16_384
IMAGE_LIMIT = 2 * 1024 * 1024
IMAGE_TOTAL = 4 * 1024 * 1024


def read_exact(size):
    chunks = bytearray()
    while len(chunks) < size:
        chunk = os.read(3, size - len(chunks))
        if not chunk:
            raise EOFError()
        chunks.extend(chunk)
    return bytes(chunks)


def send(value):
    data = json.dumps(value, ensure_ascii=True).encode("ascii")
    frame = memoryview(struct.pack(">I", len(data)) + data)
    while frame:
        frame = frame[os.write(3, frame):]


class BoundedText:
    def __init__(self, limit=TEXT_LIMIT):
        self.data = bytearray()
        self.limit = limit
        self.truncated = False

    def append(self, data):
        remaining = self.limit - len(self.data)
        self.data.extend(data[:remaining])
        self.truncated |= len(data) > remaining

    def write(self, text):
        # Avoid encoding an arbitrarily large user string just for capture.
        self.append(text[:self.limit].encode("utf-8", "replace"))
        self.truncated |= len(text) > self.limit
        return len(text)

    def text(self):
        return self.data.decode("utf-8", "replace") + (
            "\n[output truncated; excess discarded]" if self.truncated else ""
        )


class Capture:
    def __init__(self, fd):
        self.fd = fd
        self.output = BoundedText()
        self.reader, writer = os.pipe()
        os.dup2(writer, fd)
        os.close(writer)
        self.thread = threading.Thread(target=self.drain, daemon=True)
        self.thread.start()

    def drain(self):
        try:
            while True:
                chunk = os.read(self.reader, 8192)
                if not chunk:
                    break
                self.output.append(chunk)
        finally:
            os.close(self.reader)

    def finish(self):
        os.dup2(null_fd, self.fd)
        # An unjoined child holding stdout open is covered by the cell timeout.
        self.thread.join()
        return self.output.text()


class ImageBuffer(io.BytesIO):
    def write(self, data):
        if self.tell() + len(data) > IMAGE_LIMIT:
            raise ValueError("Image exceeds 2 MiB")
        return super().write(data)


class Images:
    def __init__(self):
        self.items = []
        self.size = 0
        self.active = False

    def display(self, source):
        """display_image(PNG/JPEG bytes, path, or a matplotlib Figure)."""
        if not self.active:
            raise RuntimeError("display_image is only available during a cell")
        if len(self.items) >= 4:
            raise ValueError("At most 4 images per cell")
        if isinstance(source, (str, os.PathLike)):
            with open(source, "rb") as image:
                data = image.read(IMAGE_LIMIT + 1)
        elif isinstance(source, (bytes, bytearray, memoryview)):
            if len(source) > IMAGE_LIMIT:
                raise ValueError("Image exceeds 2 MiB")
            data = bytes(source)
        elif callable(getattr(source, "savefig", None)):
            output = ImageBuffer()
            source.savefig(output, format="png")
            data = output.getvalue()
        else:
            raise TypeError("Expected PNG/JPEG bytes, a path, or a matplotlib Figure")
        if len(data) > IMAGE_LIMIT or self.size + len(data) > IMAGE_TOTAL:
            raise ValueError("Image limit: 2 MiB each, 4 MiB per cell")
        if data.startswith(b"\x89PNG\r\n\x1a\n"):
            mime = "image/png"
        elif data.startswith(b"\xff\xd8\xff"):
            mime = "image/jpeg"
        else:
            raise ValueError("Only PNG and JPEG images are supported")
        self.items.append({"type": "image", "mimeType": mime,
                           "data": base64.b64encode(data).decode("ascii")})
        self.size += len(data)


# stdin cannot consume the control channel or wait for interactive input.
null_fd = os.open(os.devnull, os.O_RDWR)
os.dup2(null_fd, 0)
os.dup2(null_fd, 1)
os.dup2(null_fd, 2)
os.set_inheritable(3, False)
loop = asyncio.new_event_loop()
asyncio.set_event_loop(loop)
# Match an interactive Python session rather than importing from the runner's directory.
sys.path.insert(0, os.getcwd())
images = Images()
namespace = {"__name__": "__main__", "display_image": images.display}
future_mask = 0
for feature in __future__.all_feature_names:
    future_mask |= getattr(__future__, feature).compiler_flag
flags = ast.PyCF_ALLOW_TOP_LEVEL_AWAIT
printer = reprlib.Repr()
printer.maxstring = 8192
printer.maxother = 8192


def evaluate(compiled):
    value = eval(compiled, namespace, namespace)
    if compiled.co_flags & inspect.CO_COROUTINE:
        return loop.run_until_complete(value)
    return value


def cell(request):
    global flags
    captures = [Capture(1), Capture(2)]
    old_stdout, old_stderr = sys.stdout, sys.stderr
    # closefd=False allows restoring fd targets without dangling writer copies.
    sys.stdout = io.TextIOWrapper(io.FileIO(1, "w", closefd=False), encoding="utf-8", write_through=True)
    sys.stderr = io.TextIOWrapper(io.FileIO(2, "w", closefd=False), encoding="utf-8", write_through=True)
    images.items = []
    images.size = 0
    images.active = True
    result = {"id": request["id"], "ok": True, "value": "", "traceback": ""}
    try:
        filename = "<python_cell %s>" % request["id"]
        tree = compile(request["code"], filename, "exec", flags | ast.PyCF_ONLY_AST, dont_inherit=True)
        expression = None
        if tree.body and isinstance(tree.body[-1], ast.Expr):
            expression = ast.Expression(tree.body.pop().value)
        compiled = compile(tree, filename, "exec", flags, dont_inherit=True)
        flags |= compiled.co_flags & future_mask
        # Compile both parts before execution, just like an ordinary cell.
        final = compile(expression, filename, "eval", flags, dont_inherit=True) if expression else None
        evaluate(compiled)
        if final:
            value = evaluate(final)
            if value is not None:
                namespace["_"] = value
                output = BoundedText(8192)
                output.write(printer.repr(value))
                result["value"] = output.text()
    except BaseException as error:
        result["ok"] = False
        output = BoundedText(8192)
        traceback.print_exception(type(error), error, error.__traceback__, file=output)
        result["traceback"] = output.text()
    finally:
        images.active = False
        for stream in (sys.stdout, sys.stderr):
            try:
                stream.flush()
            except BaseException:
                pass
        sys.stdout, sys.stderr = old_stdout, old_stderr
        result["stdout"], result["stderr"] = [capture.finish() for capture in captures]
        result["images"] = images.items
        images.items = []
    return result


while True:
    try:
        length = struct.unpack(">I", read_exact(4))[0]
        if length > MAX_REQUEST:
            raise ValueError("Request frame too large")
        request = json.loads(read_exact(length))
        send(cell(request))
    except EOFError:
        break
