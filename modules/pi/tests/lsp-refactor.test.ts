import { afterEach, expect, mock, test } from "bun:test";
import { mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { registerRefactorTools, type RefactorTarget } from "../files/extensions/lsp-refactor";

const roots: string[] = [];
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function fixture() {
  const root = await realpath(await mkdtemp(join(tmpdir(), "pi-refactor-test-")));
  roots.push(root);
  const files = [join(root, "first.ts"), join(root, "second.ts")];
  await Promise.all(files.map((file) => writeFile(file, "foo\n")));
  const edit = { changes: Object.fromEntries(files.map((file) => [pathToFileURL(file).href, [{
    range: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } }, newText: "bar",
  }]])) };
  const request = mock(async (method: string, _params: unknown, _signal?: AbortSignal): Promise<unknown> => {
    switch (method) {
      case "textDocument/prepareRename": return { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } };
      case "textDocument/rename": return edit;
      case "textDocument/codeAction": return [{ title: "Fix", data: { id: 1 } }];
      case "codeAction/resolve": return { title: "Fix", edit };
      default: throw new Error(`Unexpected request: ${method}`);
    }
  });
  const sync = mock(async (_files: string[], _signal?: AbortSignal) => {});
  const target: RefactorTarget = {
    root, file: files[0], versions: new Map(), diagnostics: [], request, sync,
    capabilities: { renameProvider: { prepareProvider: true }, codeActionProvider: { resolveProvider: true } },
  };
  const tools = new Map<string, { execute: Function }>();
  const events = new Map<string, Function>();
  const ctx = { cwd: root, ui: { notify: mock(() => {}) } };
  registerRefactorTools({
    on: (name: string, handler: Function) => events.set(name, handler),
    registerTool: (tool: { name: string; execute: Function }) => tools.set(tool.name, tool),
  } as unknown as ExtensionAPI, { target: async () => target });
  const call = async (name: string, params: unknown, signal?: AbortSignal): Promise<string> => {
    const result = await tools.get(name)!.execute("call", params, signal, undefined, ctx);
    return result.content.map((part: { text: string }) => part.text).join("\n");
  };
  const rename = (signal?: AbortSignal) => call("lsp_rename", { path: files[0], line: 0, character: 0, newName: "bar" }, signal);
  const list = () => call("lsp_code_actions", { path: files[0], line: 0, character: 0 });
  const contents = () => Promise.all(files.map((file) => readFile(file, "utf8")));
  return { files, events, ctx, target, request, sync, call, rename, list, contents };
}

function previewId(text: string): string {
  const match = /previewId: "([^"]+)"/.exec(text);
  if (!match) throw new Error(`No preview token in result: ${text}`);
  return match[1];
}

test("rename previews both files, applies once, and syncs only after writing", async () => {
  const f = await fixture();
  const controller = new AbortController();
  const preview = await f.rename(controller.signal);
  expect(preview).toContain("Preview only");
  expect(preview).toContain("first.ts");
  expect(preview).toContain("second.ts");
  expect(await f.contents()).toEqual(["foo\n", "foo\n"]);
  expect(f.request.mock.calls.map(([method]) => method)).toEqual(["textDocument/prepareRename", "textDocument/rename"]);
  expect(f.request.mock.calls[1][1]).toMatchObject({ position: { line: 0, character: 0 }, newName: "bar" });
  expect(f.request.mock.calls[1][2]).toBe(controller.signal);
  expect(f.sync).not.toHaveBeenCalled();
  f.sync.mockImplementation(async () => { expect(await f.contents()).toEqual(["bar\n", "bar\n"]); });
  const params = { previewId: previewId(preview) };
  expect(await f.call("lsp_apply_edit", params)).toContain("Applied reviewed edit");
  expect(f.sync).toHaveBeenCalledTimes(1);
  expect(f.sync.mock.calls[0][0]).toEqual(f.files);
  await expect(f.call("lsp_apply_edit", params)).rejects.toThrow("already consumed");
});

test("lists and resolves an action before preview and apply", async () => {
  const f = await fixture();
  const [action] = JSON.parse(await f.list());
  expect(action.title).toBe("Fix");
  const preview = await f.call("lsp_code_actions", { actionId: action.actionId });
  expect(f.request.mock.calls.map(([method]) => method)).toEqual(["textDocument/codeAction", "codeAction/resolve"]);
  expect(await f.contents()).toEqual(["foo\n", "foo\n"]);
  await f.call("lsp_apply_edit", { previewId: previewId(preview) });
  expect(await f.contents()).toEqual(["bar\n", "bar\n"]);
});

test.each(["session_start", "session_shutdown"])("%s invalidates previews and offered actions", async (event) => {
  const f = await fixture();
  const id = previewId(await f.rename());
  const [action] = JSON.parse(await f.list());
  f.events.get(event)!();
  await expect(f.call("lsp_apply_edit", { previewId: id })).rejects.toThrow("expired");
  await expect(f.call("lsp_code_actions", { actionId: action.actionId })).rejects.toThrow("expired");
  expect(await f.contents()).toEqual(["foo\n", "foo\n"]);
});

test.each(["cancel", "session"])("rejects a pending rename after %s", async (change) => {
  const f = await fixture();
  const controller = new AbortController();
  const started = Promise.withResolvers<void>();
  const pending = Promise.withResolvers<unknown>();
  f.target.capabilities.renameProvider = true;
  f.request.mockImplementationOnce(async () => { started.resolve(); return pending.promise; });
  const work = f.rename(controller.signal);
  await started.promise;
  if (change === "cancel") controller.abort();
  else f.events.get("session_shutdown")!();
  pending.resolve({ changes: { [pathToFileURL(f.files[0]).href]: [{
    range: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } }, newText: "bar",
  }] } });
  await expect(work).rejects.toThrow(change === "cancel" ? "abort" : "session changed");
  expect(await f.contents()).toEqual(["foo\n", "foo\n"]);
  expect(f.sync).not.toHaveBeenCalled();
});

test("rejects stale previews without writing other files or syncing", async () => {
  const f = await fixture();
  const params = { previewId: previewId(await f.rename()) };
  await writeFile(f.files[1], "user edit\n");
  await expect(f.call("lsp_apply_edit", params)).rejects.toThrow("changed since preview");
  expect(await f.contents()).toEqual(["foo\n", "user edit\n"]);
  expect(f.sync).not.toHaveBeenCalled();
});

test("reports a sync failure as applied and does not allow retrying the token", async () => {
  const f = await fixture();
  const params = { previewId: previewId(await f.rename()) };
  f.sync.mockRejectedValueOnce(new Error("server disconnected"));
  const result = await f.call("lsp_apply_edit", params);
  expect(result).toContain("Applied to");
  expect(result).toContain("LSP synchronization failed: server disconnected");
  expect(await f.contents()).toEqual(["bar\n", "bar\n"]);
  await expect(f.call("lsp_apply_edit", params)).rejects.toThrow("already consumed");
});

test("rejects commands introduced by action resolution without writing", async () => {
  const f = await fixture();
  const [action] = JSON.parse(await f.list());
  f.request.mockResolvedValueOnce({ title: "Fix", command: { command: "unsafe" } });
  await expect(f.call("lsp_code_actions", { actionId: action.actionId })).rejects.toThrow("Command-bearing");
  expect(await f.contents()).toEqual(["foo\n", "foo\n"]);
});
