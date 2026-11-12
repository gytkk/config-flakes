import { afterEach, expect, test } from "bun:test";
import { chmod, link, lstat, mkdtemp, rm, symlink, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createBashTool, type ExtensionAPI, type ExtensionContext } from "@earendil-works/pi-coding-agent";
import { KeybindingsManager, TUI_KEYBINDINGS, visibleWidth, type Component } from "@earendil-works/pi-tui";
import extension, { automaticSource, safeDisplay } from "../files/extensions/session-artifacts/index.ts";
import { ArtifactStore, ENTRY_TYPE, LIMITS, page, restoreArtifacts, search, type Artifact } from "../files/extensions/session-artifacts/store.ts";
import { showPage } from "../files/extensions/session-artifacts/view.ts";

const cleanup: string[] = [];
afterEach(async () => { for (const path of cleanup.splice(0)) await rm(path, { recursive: true, force: true }); });
async function fixture() {
  const dir = await mkdtemp(join(tmpdir(), "artifact-test-")); cleanup.push(dir);
  return { dir, store: new ArtifactStore(join(dir, "store")), source: join(dir, "source.txt") };
}
type CapturedTool = Parameters<ExtensionAPI["registerTool"]>[0];
type CapturedCommand = Parameters<ExtensionAPI["registerCommand"]>[1];
type BranchEntry = { type: string; customType: string; data: Artifact };
function harness(root: string, cwd: string, initial: BranchEntry[] = [], session = "session-a") {
  let branch = [...initial];
  const handlers = new Map<string, Function>();
  const messages: Parameters<ExtensionAPI["sendMessage"]>[0][] = [], notifications: string[] = [];
  const tools: { name: string; sourceInfo: { source: string } }[] = [];
  let tool!: CapturedTool, command!: CapturedCommand;
  const ctx = {
    cwd, mode: "json", hasUI: false,
    sessionManager: { getSessionId: () => session, getBranch: () => branch },
    ui: { notify: (text: string) => notifications.push(text), select: () => { throw new Error("Unexpected dialog"); }, custom: () => { throw new Error("Unexpected custom screen"); } },
  } as unknown as ExtensionContext;
  extension({
    on: (name: string, handler: Function) => handlers.set(name, handler),
    registerTool: (value: CapturedTool) => { tool = value; },
    registerCommand: (_name: string, value: CapturedCommand) => { command = value; },
    appendEntry: (customType: string, data: Artifact) => branch.push({ type: "custom", customType, data: structuredClone(data) }),
    sendMessage: (message: Parameters<ExtensionAPI["sendMessage"]>[0]) => messages.push(message), getAllTools: () => tools,
  } as never, { storeRoot: root });
  handlers.get("session_start")!({}, ctx);
  return { ctx, messages, notifications, tools, entries: () => [...branch],
    call: (args: Parameters<CapturedTool["execute"]>[1], signal?: AbortSignal) => tool.execute("call", args, signal, undefined, ctx),
    command: (args = "") => command.handler(args, ctx),
    event: (name: string, event: unknown = {}) => handlers.get(name)?.(event, ctx),
    branch: (entries: BranchEntry[]) => { branch = [...entries]; handlers.get("session_tree")!({}, ctx); },
  };
}

test("register snapshots selected text privately; source deletion does not affect page/search", async () => {
  const f = await fixture(); await writeFile(f.source, "first\nNeedle.* literal\nlast\n");
  const a = await f.store.register("session", f.source, "sample", { offset: 2, limit: 2 });
  expect((await lstat(f.store.path(a))).mode & 0o777).toBe(0o600);
  expect((await lstat(join(f.store.root, a.owner))).mode & 0o777).toBe(0o700);
  await unlink(f.source);
  const text = await f.store.text(a);
  expect(text).toBe("Needle.* literal\nlast");
  expect(page(text, 1, 1)).toMatchObject({ text: "1:0 Needle.* literal", nextOffset: 2, nextColumn: 0 });
  expect(page(text, 2, 1).nextOffset).toBeUndefined();
  expect(search(text, ".*").matches).toBe(1);
  expect(search(text, "needle", 1, 50, true).matches).toBe(1);
  expect(search(text, "^Needle").matches).toBe(0);
});

test("paging can reconstruct very long Unicode lines without losing characters and stays byte bounded", () => {
  const text = "😀한".repeat(8000) + "\nsecond";
  let offset = 1, column = 0, reconstructed = "";
  for (let count = 0; count < 20; count++) {
    const p = page(text, offset, 1, column);
    expect(Buffer.byteLength(p.text)).toBeLessThan(LIMITS.pageBytes);
    reconstructed += p.text.replace(/^\d+:\d+ /, "");
    if (!p.nextOffset) break;
    if (p.nextOffset > offset) reconstructed += "\n";
    offset = p.nextOffset; column = p.nextColumn;
  }
  expect(reconstructed).toBe(text);
  expect(() => page("short", 1, 1, 10)).toThrow("cursor");
  expect(() => search("text", "")).toThrow("literal");
  const result = search(("😀".repeat(100) + "\n").repeat(100), "😀");
  expect(Buffer.byteLength(result.text)).toBeLessThan(LIMITS.pageBytes);
  expect(result.nextOffset).toBeDefined();
});

test("real installed native bash truncation contract is retained without changing its result", async () => {
  const f = await fixture(), h = harness(f.store.root, f.dir);
  h.tools.push({ name: "bash", sourceInfo: { source: "builtin" } });
  const bash = createBashTool(f.dir, { operations: { exec: async (_command, _cwd, options) => {
    options.onData(Buffer.from("line output\n".repeat(2200))); return { exitCode: 0 };
  } } });
  const result = await bash.execute("bash-call", { command: "fixture" }, undefined, undefined);
  const path = result.details!.fullOutputPath!; cleanup.push(path);
  expect(result.details!.truncation!.truncated).toBe(true);
  const before = structuredClone(result);
  expect(await h.event("tool_result", { toolName: "bash", ...result })).toBeUndefined();
  expect(result).toEqual(before);
  const entry = h.entries()[0]; expect(entry.customType).toBe(ENTRY_TYPE);
  await unlink(path);
  const read = await h.call({ action: "read", id: entry.data.id, limit: 1 });
  expect(read.content[0].text).toContain("line output");
});

test("codemode details require builtin provenance and exact temp contract, never free text paths", async () => {
  const f = await fixture(), h = harness(f.store.root, f.dir);
  const path = join(tmpdir(), "pi-codemode-0123456789abcdef.txt");
  // A random private fixture directory is not the native output directory.
  expect(automaticSource("codemode", { calls: [], fullOutputPath: join(f.dir, "pi-codemode-0123456789abcdef.txt") }, "builtin")).toBeUndefined();
  expect(automaticSource("codemode", { calls: [], fullOutputPath: path }, "builtin")).toBe(path);
  expect(automaticSource("codemode", { calls: [], fullOutputPath: path }, "extension")).toBeUndefined();
  expect(automaticSource("codemode", { fullOutputPath: path }, "builtin")).toBeUndefined();
  expect(automaticSource("bash", { truncation: { truncated: true }, fullOutputPath: f.source }, "builtin")).toBeUndefined();
  await h.event("tool_result", { toolName: "bash", content: [{ type: "text", text: `Full output: ${f.source}` }] });
  expect(h.entries()).toEqual([]);
});

test("resume, fork and tree only restore indexed active branch snapshots; new registrations have unique IDs", async () => {
  const f = await fixture(); await writeFile(f.source, "one\ntwo");
  const h = harness(f.store.root, f.dir);
  const first = (await h.call({ action: "register", path: f.source })).details.artifact;
  const ancestor = h.entries();
  const second = (await h.call({ action: "register", path: f.source })).details.artifact;
  expect(first.id).not.toBe(second.id);
  const resumed = harness(f.store.root, f.dir, h.entries());
  expect((await resumed.call({ action: "list" })).details.count).toBe(2);
  h.branch(ancestor);
  await expect(h.call({ action: "read", id: second.id })).rejects.toThrow("Unknown artifact ID");
  expect((await h.call({ action: "list" })).details.count).toBe(1);
  const fork = harness(f.store.root, f.dir, ancestor, "fork-session");
  expect((await fork.call({ action: "read", id: first.id })).content[0].text).toContain("one");
  const third = (await fork.call({ action: "register", path: f.source })).details.artifact;
  expect(third.owner).not.toBe(first.owner);
  await h.event("session_shutdown");
  expect(await f.store.text(first)).toBe("one\ntwo");
  expect(restoreArtifacts([{ type: "custom", customType: ENTRY_TYPE, data: { ...first, id: "../../outside" } }]).size).toBe(0);
});

test("reject missing, oversized, binary, non-UTF8, symlink, hardlink, directory and unsafe private paths", async () => {
  const f = await fixture();
  await expect(f.store.register("s", f.source, "missing")).rejects.toThrow();
  await writeFile(f.source, Buffer.alloc(LIMITS.sourceBytes + 1, 65));
  await expect(f.store.register("s", f.source, "large")).rejects.toThrow("exceeds");
  await writeFile(f.source, Buffer.from([65, 0, 66]));
  await expect(f.store.register("s", f.source, "binary")).rejects.toThrow("binary");
  await writeFile(f.source, Buffer.from([0xff, 0xfe]));
  await expect(f.store.register("s", f.source, "encoding")).rejects.toThrow("UTF-8");
  await writeFile(f.source, "ok");
  const alias = join(f.dir, "alias"); await symlink(f.source, alias);
  await expect(f.store.register("s", alias, "link")).rejects.toThrow("Symlink");
  const aliasDir = join(f.dir, "alias-dir"); await symlink(f.dir, aliasDir);
  await expect(f.store.register("s", join(aliasDir, "source.txt"), "parent link")).rejects.toThrow("Symlink");
  await expect(f.store.register("s", f.dir, "directory")).rejects.toThrow("regular");
  await chmod(f.source, 0o644);
  await expect(f.store.register("s", f.source, "untrusted output", { privateFile: true })).rejects.toThrow("private");
  await link(f.source, join(f.dir, "hardlink"));
  await expect(f.store.register("s", f.source, "hardlink")).rejects.toThrow("hardlinked");
});

test("tampered or deleted snapshots fail closed and reads cannot address arbitrary paths", async () => {
  const f = await fixture(); await writeFile(f.source, "original");
  const h = harness(f.store.root, f.dir);
  const a = (await h.call({ action: "register", path: f.source })).details.artifact;
  await expect(h.call({ action: "read", id: f.source })).rejects.toThrow("Unknown artifact ID");
  await writeFile(f.store.path(a), "modified");
  await expect(h.call({ action: "read", id: a.id })).rejects.toThrow("corrupt");
  await unlink(f.store.path(a));
  await expect(h.call({ action: "read", id: a.id })).rejects.toThrow();
});

test("parallel registration, cancellation and shutdown leave retained files but no stale branch entry", async () => {
  const f = await fixture(); await writeFile(f.source, "data");
  const h = harness(f.store.root, f.dir);
  await Promise.all(Array.from({ length: 4 }, () => h.call({ action: "register", path: f.source })));
  expect(new Set(h.entries().map(e => e.data.id)).size).toBe(4);
  const abort = new AbortController(); abort.abort();
  await expect(h.call({ action: "register", path: f.source }, abort.signal)).rejects.toThrow();
  const work = h.call({ action: "register", path: f.source });
  const rejected = work.catch((error: unknown) => error);
  h.branch([]);
  expect(await rejected).toBeInstanceOf(Error);
  expect(h.entries()).toEqual([]);
  await h.event("session_shutdown");
});

test("non-TUI commands return text without prompting and retention help describes manual cleanup", async () => {
  const f = await fixture(), h = harness(f.store.root, f.dir);
  await h.command();
  expect(h.messages[0].content).toContain("No artifacts");
  await h.command("help");
  expect(h.messages[1].content).toContain("Session deletion does not delete snapshots");
  await writeFile(f.source, "sample\nmore");
  await h.command(`register ${f.source}`);
  const id = h.entries()[0].data.id;
  await h.command(`read ${id} 2`);
  expect(h.messages.at(-1).content).toContain("2:0 more");
  h.ctx.mode = "rpc"; h.ctx.hasUI = true;
  await h.command();
  expect(h.messages.at(-1).content).toContain(id);
});

test("native content viewer is read-only, width bounded, keyboard pageable and abortable", async () => {
  const keys = new KeybindingsManager(TUI_KEYBINDINGS);
  const controller = new AbortController();
  let component!: Component & { handleInput(data: string): void; dispose(): void };
  const ctx = { mode: "tui", ui: { custom: (factory: Function) => {
    const { promise, resolve } = Promise.withResolvers<unknown>();
    component = factory({ terminal: { rows: 20 }, requestRender() {} }, { fg: (_: string, s: string) => s }, keys, resolve);
    return promise;
  } } } as unknown as ExtensionContext;
  const shown = showPage(ctx, "Artifact", "한😀\n".repeat(40), true, false, controller.signal);
  for (const width of [1, 8, 60]) for (const line of component.render(width)) expect(visibleWidth(line)).toBeLessThanOrEqual(width);
  component.handleInput("n"); expect(await shown).toBe("next"); component.dispose();
  const next = showPage(ctx, "Artifact", "body", false, true, controller.signal);
  controller.abort(); expect(await next).toBe("back"); component.dispose();
  expect(safeDisplay("safe\x1b]52;c;secret\x07\x1b[31mred\x1b[0m")).toBe("safered");
});

test("/artifacts selects a registered row then shows content and returns to the list without writes", async () => {
  const f = await fixture(), h = harness(f.store.root, f.dir);
  await writeFile(f.source, "inspect this snapshot");
  await h.call({ action: "register", path: f.source });
  h.ctx.mode = "tui"; h.ctx.hasUI = true;
  let selections = 0;
  const screens: string[] = [];
  h.ctx.ui.select = async (_title: string, labels: string[]) => selections++ === 0 ? labels[0] : undefined;
  h.ctx.ui.custom = (factory: Function) => new Promise(resolve => {
    const component = factory({ terminal: { rows: 20 }, requestRender() {} }, { fg: (_: string, s: string) => s }, new KeybindingsManager(TUI_KEYBINDINGS), resolve);
    screens.push(component.render(120).join("\n"));
    component.handleInput("\x1b"); component.dispose();
  });
  await h.command();
  expect(screens.join("\n")).toContain("inspect this snapshot");
  expect(selections).toBe(2);
  expect(h.entries()).toHaveLength(1);
  expect(h.messages).toHaveLength(0);
});

test("line and owned-store quotas reject rather than silently lose source data", async () => {
  const f = await fixture(); await writeFile(f.source, "x\n".repeat(LIMITS.lines));
  await expect(f.store.register("s", f.source, "too many lines")).rejects.toThrow("lines");
  const a = await f.store.register("s", f.source, "selected", { limit: 2 });
  expect(await f.store.text(a)).toBe("x\nx");
  const folder = join(f.store.root, a.owner);
  for (let i = 0; i < LIMITS.files - 1; i++) await writeFile(join(folder, `${i}.pending`), "", { mode: 0o600 });
  await expect(f.store.register("s", f.source, "quota", { limit: 2 })).rejects.toThrow("quota");
});
