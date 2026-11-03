import * as fs from "node:fs";
import { expect, spyOn, test } from "bun:test";
import terminalTitleReset, { CLEAR_TITLE } from "../files/extensions/terminal-title-reset";

function exitListenersAfterShutdown(reason: string, mode = "tui") {
  const listeners: Function[] = [];
  const once = process.once;
  process.once = ((event: string, listener: Function) => {
    expect(event).toBe("exit");
    listeners.push(listener);
    return process;
  }) as never;
  try {
    terminalTitleReset({
      on: (event: string, handler: Function) => {
        expect(event).toBe("session_shutdown");
        handler({ type: "session_shutdown", reason }, { mode });
      },
    } as never);
  } finally {
    process.once = once;
  }
  return listeners;
}

test("clears the terminal title at exit only when the TUI quits", () => {
  const [listener, ...rest] = exitListenersAfterShutdown("quit");
  expect(rest).toEqual([]);
  const writeSync = spyOn(fs, "writeSync").mockImplementation(() => 0);
  try {
    listener();
    expect(writeSync.mock.calls).toEqual([[1, CLEAR_TITLE]]);
  } finally {
    writeSync.mockRestore();
  }
  for (const reason of ["reload", "new", "resume", "fork"]) expect(exitListenersAfterShutdown(reason)).toEqual([]);
  for (const mode of ["rpc", "print", "json"]) expect(exitListenersAfterShutdown("quit", mode)).toEqual([]);
});
