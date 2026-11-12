import { afterEach, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import pythonExtension from "../files/extensions/python-kernel/index.ts";
import { PythonKernel } from "../files/extensions/python-kernel/kernel.ts";

const roots: string[] = [];
const kernels: PythonKernel[] = [];
const cleanups: (() => Promise<void>)[] = [];
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a2ioAAAAASUVORK5CYII=";

afterEach(async () => {
  await Promise.all(cleanups.splice(0).map((close) => close()));
  await Promise.all(kernels.splice(0).map((kernel) => kernel.close()));
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "pi-python-test-"));
  roots.push(root);
  const kernel = new PythonKernel(root);
  kernels.push(kernel);
  return { root, kernel };
}

async function waitFile(path: string): Promise<string> {
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) {
    try { const value = await readFile(path, "utf8"); if (value) return value; } catch {}
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error(`Timed out waiting for disposable worker marker: ${path}`);
}

function assertGone(pid: number) {
  expect(() => process.kill(pid, 0)).toThrow();
}

test("real Python preserves functions, mutations after errors, and top-level await", async () => {
  const { kernel } = await fixture();
  const first = await kernel.run({ code: "x = 40\ndef plus(n):\n    return x + n\nplus(2)" });
  expect(first.value).toBe("42");
  expect(first.notice).toContain("Fresh Python kernel");
  const failed = await kernel.run({ code: "x += 1\nprint('before failure')\nraise ValueError('expected failure')" });
  expect(failed.ok).toBe(false);
  expect(failed.stdout).toBe("before failure\n");
  expect(failed.traceback).toContain("ValueError: expected failure");
  expect(failed.traceback).toContain("<python_cell 2>");
  expect((await kernel.run({ code: "plus(1)" })).value).toBe("42");
  expect((await kernel.run({ code: "import asyncio\nasync def answer():\n    await asyncio.sleep(0)\n    return plus(2)\nawait answer()" })).value).toBe("43");
  expect((await kernel.run({ code: "_" })).value).toBe("43");
  const syntax = await kernel.run({ code: "x = 0\nif :" });
  expect(syntax.ok).toBe(false);
  expect((await kernel.run({ code: "x" })).value).toBe("41");
});

test("future imports persist and a persistent event loop supports objects across cells", async () => {
  const { kernel } = await fixture();
  await kernel.run({ code: "from __future__ import annotations\nimport asyncio\nasync def make():\n    return asyncio.get_running_loop().create_future()\nf = await make()" });
  const result = await kernel.run({ code: "def typed(x: NotDefined):\n    return x\nf.set_result(9)\nawait f" });
  expect(result.ok).toBe(true);
  expect(result.value).toBe("9");
});

test("stdout, stderr, native fd writes and child output cannot corrupt control frames", async () => {
  const { kernel } = await fixture();
  const result = await kernel.run({ code: `import os, sys, subprocess
print('{"id":1,"ok":false}')
os.write(1, b'\\x00\\x00\\x00\\xffraw\\n')
sys.stderr.write('stderr marker\\n')
subprocess.run([sys.executable, '-c', "print('child marker')"], check=True)
42` });
  expect(result.ok).toBe(true);
  expect(result.value).toBe("42");
  expect(result.stdout).toContain('"ok":false');
  expect(result.stdout).toContain("raw");
  expect(result.stdout).toContain("child marker");
  expect(result.stderr).toBe("stderr marker\n");
  expect((await kernel.run({ code: "6 * 7" })).value).toBe("42");
});

test("bounds noisy streams, repr and traceback without losing subsequent execution", async () => {
  const { kernel } = await fixture();
  const result = await kernel.run({ code: "import os\nfor i in range(128):\n    os.write(1, b'x' * 8192)\n    os.write(2, b'y' * 8192)\n'z' * 1000000" });
  expect(result.stdout.length).toBeLessThan(17000);
  expect(result.stderr.length).toBeLessThan(17000);
  expect(result.stdout).toContain("truncated");
  expect(result.stderr).toContain("truncated");
  expect(result.value.length).toBeLessThan(8300);
  const error = await kernel.run({ code: "raise ValueError('e' * 1000000)" });
  expect(error.traceback.length).toBeLessThan(8300);
  expect(error.traceback).toContain("truncated");
  expect((await kernel.run({ code: "1 + 1" })).value).toBe("2");
});

test("real image helper returns identical PNG bytes and path images, rejects unsupported/oversized images", async () => {
  const { root, kernel } = await fixture();
  await writeFile(join(root, "pixel.png"), Buffer.from(PNG, "base64"));
  const result = await kernel.run({ code: `import base64\ndisplay_image(base64.b64decode('${PNG}'))\ndisplay_image('pixel.png')` });
  expect(result.ok).toBe(true);
  expect(result.images).toEqual([
    { type: "image", mimeType: "image/png", data: PNG },
    { type: "image", mimeType: "image/png", data: PNG },
  ]);
  const invalid = await kernel.run({ code: "display_image(b'not an image')" });
  expect(invalid.ok).toBe(false);
  expect(invalid.traceback).toContain("Only PNG and JPEG");
  expect((await kernel.run({ code: "display_image(b'x' * (2 * 1024 * 1024 + 1))" })).traceback).toContain("2 MiB");
  const tooMany = await kernel.run({ code: "for i in range(5):\n    display_image('pixel.png')" });
  expect(tooMany.ok).toBe(false);
  expect(tooMany.images).toHaveLength(4);
  expect((await kernel.run({ code: "None" })).images).toEqual([]);
});

test("concurrent calls are ordered and cancelling a queued cell does not kill its sibling", async () => {
  const { root, kernel } = await fixture();
  const first = kernel.run({ code: "import time\nfrom pathlib import Path\nx = 1\nPath('ready').write_text('ready')\ntime.sleep(0.2)\nx" });
  await waitFile(join(root, "ready"));
  const controller = new AbortController();
  const skipped = kernel.run({ code: "x = 100" }, controller.signal).catch((error) => error);
  const third = kernel.run({ code: "x += 1\nx" });
  controller.abort();
  expect((await skipped).message).toContain("cancelled while queued");
  expect((await first).value).toBe("1");
  expect((await third).value).toBe("2");
});

test.each(["abort", "timeout"])("%s kills the process, rejects queued work, and never replays side effects", async (mode) => {
  const { root, kernel } = await fixture();
  const controller = new AbortController();
  const running = kernel.run({ code: `import os, time
from pathlib import Path
Path('effect').write_text('once')
Path('pid').write_text(str(os.getpid()))
while True: time.sleep(1)`, timeout: 1 }, controller.signal).catch((error) => error);
  const pid = Number(await waitFile(join(root, "pid")));
  const queued = kernel.run({ code: "Path('unexpected').write_text('bad')" }).catch((error) => error);
  if (mode === "abort") controller.abort();
  const stopped = await running;
  const skipped = await queued;
  expect(stopped.message).toContain(mode === "abort" ? "cancelled" : "timed out");
  expect(skipped.message).toContain("changed while queued");
  assertGone(pid);
  const recovered = await kernel.run({ code: "'os' in globals()" });
  expect(recovered.notice).toContain("kernel lost");
  expect(recovered.value).toBe("False");
  expect(await readFile(join(root, "effect"), "utf8")).toBe("once");
  await expect(readFile(join(root, "unexpected"))).rejects.toThrow();
});

test("reset interrupts active work and removes variables; reset-only stays lazy", async () => {
  const { root, kernel } = await fixture();
  await kernel.run({ code: "x = 17" });
  const work = kernel.run({ code: "import os, time\nfrom pathlib import Path\nPath('pid').write_text(str(os.getpid()))\ntime.sleep(30)" }).catch((error) => error);
  const pid = Number(await waitFile(join(root, "pid")));
  await kernel.reset();
  expect((await work).message).toContain("reset");
  assertGone(pid);
  expect(kernel.status()).toContain("not running");
  const reset = await kernel.run({ reset: true });
  expect(reset.notice).toContain("reset");
  expect(kernel.status()).toContain("not running");
  expect((await kernel.run({ code: "'x' in globals()" })).value).toBe("False");
});

test("unexpected exit is reported, a later call starts clean, and missing python is actionable", async () => {
  const { root, kernel } = await fixture();
  await expect(kernel.run({ code: "import os\nos._exit(7)" })).rejects.toThrow("Kernel lost");
  const fresh = await kernel.run({ code: "40 + 2" });
  expect(fresh.notice).toContain("kernel lost");
  expect(fresh.value).toBe("42");
  const missing = new PythonKernel(root, "test", join(root, "missing-python"));
  kernels.push(missing);
  await expect(missing.run({ code: "42" })).rejects.toThrow("Unable to run python3");
});

test("rejects invalid bounds before execution and pre-aborted requests leave state untouched", async () => {
  const { kernel } = await fixture();
  await expect(kernel.run({ code: "42", timeout: 0 })).rejects.toThrow("1 to 120");
  await expect(kernel.run({ code: "42", timeout: 121 })).rejects.toThrow("1 to 120");
  await expect(kernel.run({ code: "42", timeout: NaN })).rejects.toThrow("1 to 120");
  await expect(kernel.run({ code: "x".repeat(65537) })).rejects.toThrow("64 KiB");
  await expect(kernel.run({})).rejects.toThrow("Provide code");
  expect(kernel.status()).toContain("not running");
  await kernel.run({ code: "x = 42" });
  await expect(kernel.run({ reset: true }, AbortSignal.abort())).rejects.toThrow("cancelled");
  expect((await kernel.run({ code: "x" })).value).toBe("42");
});

async function extensionFixture() {
  const { root } = await fixture();
  const events = new Map<string, Function>();
  type CapturedTool = Parameters<ExtensionAPI["registerTool"]>[0];
  type CapturedCommand = Parameters<ExtensionAPI["registerCommand"]>[1];
  const tools = new Map<string, CapturedTool>();
  const commands = new Map<string, CapturedCommand>();
  const notices: string[] = [];
  let id = "session-A";
  const ctx = { cwd: root, sessionManager: { getSessionId: () => id }, ui: { notify: (text: string) => notices.push(text) } };
  pythonExtension({
    on: (name: string, handler: Function) => events.set(name, handler),
    registerTool: (tool: CapturedTool) => tools.set(tool.name, tool),
    registerCommand: (name: string, command: CapturedCommand) => commands.set(name, command),
  } as unknown as ExtensionAPI);
  cleanups.push(async () => { await events.get("session_shutdown")!(); });
  const start = (reason = "startup") => events.get("session_start")!({ reason }, ctx);
  const call = (params: unknown) => tools.get("python_cell").execute("cell", params, undefined, undefined, ctx);
  return { events, commands, ctx, notices, start, call, setId: (value: string) => { id = value; } };
}

test("tool surfaces Python errors as failures and returns images from successful cells", async () => {
  const f = await extensionFixture();
  await f.start();
  await expect(f.call({ code: "x = 42\nraise ValueError('visible')" })).rejects.toThrow("earlier mutations preserved");
  expect((await f.call({ code: "x" })).content[0].text).toContain("42");
  const result = await f.call({ code: `import base64\ndisplay_image(base64.b64decode('${PNG}'))` });
  expect(result.content[1]).toEqual({ type: "image", mimeType: "image/png", data: PNG });
  const noisy = await f.call({ code: "print('line\\n' * 10000)" });
  expect(noisy.content[0].text.split("\n").length).toBeLessThanOrEqual(1802);
  expect(noisy.content[0].text).toContain("truncated");
});

test.each(["resume", "fork", "reload", "new"])("%s starts without inherited state and shutdown rejects old calls", async (reason) => {
  const f = await extensionFixture();
  await f.start();
  await f.call({ code: "secret = 42" });
  await f.events.get("session_shutdown")!();
  await expect(f.call({ code: "42" })).rejects.toThrow("unavailable");
  f.setId("session-B");
  await f.start(reason);
  const fresh = await f.call({ code: "'secret' in globals()" });
  expect(fresh.content[0].text).toContain("False");
  expect(fresh.content[0].text).toContain(reason);
});

test("tree navigation invalidates state and slash status/reset stay lazy", async () => {
  const f = await extensionFixture();
  await f.start();
  await f.commands.get("python").handler("status", f.ctx);
  expect(f.notices.at(-1)).toContain("not running");
  await f.call({ code: "x = 1" });
  await f.events.get("session_tree")!({}, f.ctx);
  const fresh = await f.call({ code: "'x' in globals()" });
  expect(fresh.content[0].text).toContain("False");
  expect(fresh.content[0].text).toContain("tree changed");
  await f.commands.get("python").handler("reset", f.ctx);
  expect(f.notices.at(-1)).toContain("not running");
});
