import { afterEach, expect, test } from "bun:test";
import { chmod, link, mkdir, mkdtemp, readFile, realpath, rename, rm, stat, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type ExtensionAPI, withFileMutationQueue } from "@earendil-works/pi-coding-agent";
import astTransform from "../files/extensions/ast-transform";
import { applyEdits, AstTransform, LIMITS } from "../files/extensions/ast-transform/engine";

const roots: string[] = [];
const engines: AstTransform[] = [];
afterEach(async () => {
  await Promise.all(engines.splice(0).map((engine) => engine.shutdown()));
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});
async function fixture(contents = ["console.log('one');\n", "console.log('two');\n"]) {
  const root = await realpath(await mkdtemp(join(tmpdir(), "pi-ast-test-")));
  roots.push(root);
  const files = contents.map((_, i) => `${i + 1}.ts`);
  await Promise.all(files.map((file, i) => writeFile(join(root, file), contents[i])));
  const engine = new AstTransform();
  engines.push(engine);
  const query = { files, language: "typescript", pattern: "console.log($$$ARGS)", replacement: "logger.info($$$ARGS)" };
  const read = () => Promise.all(files.map((file) => readFile(join(root, file), "utf8")));
  return { root, files, engine, query, read };
}
function requireCli() {
  if (!Bun.which("ast-grep")) throw new Error("Real CLI tests require pkgs.ast-grep on PATH. /usr/bin/sg is not ast-grep; do not skip these tests for integration.");
}

test("real CLI: two-file search, no-write preview, exact apply and replay rejection", async () => {
  requireCli();
  const f = await fixture();
  await chmod(join(f.root, f.files[0]), 0o751);
  const before = await f.read();
  const search = await f.engine.search(f.root, f.query);
  expect(search.matches.map((match) => match.path)).toEqual(f.files);
  expect(search.matches[0]).toMatchObject({ startByte: 0, endByte: 18, start: { line: 1, byteColumn: 1 }, end: { line: 1, byteColumn: 19 }, text: "console.log('one')" });
  const preview = await f.engine.preview(f.root, f.query);
  expect(preview.text).toContain('a/1.ts');
  expect(preview.text).toContain('a/2.ts');
  expect(preview.text).toContain("-console.log('one');");
  expect(preview.text).toContain("+logger.info('one');");
  expect(await f.read()).toEqual(before);
  // Neither query object nor subsequent searches may change a stored preview.
  f.query.replacement = "wrong($$$ARGS)";
  await f.engine.search(f.root, { ...f.query, pattern: "missing($A)" });
  await f.engine.apply(f.root, preview.previewId!);
  expect(await f.read()).toEqual(["logger.info('one');\n", "logger.info('two');\n"]);
  expect((await stat(join(f.root, f.files[0]))).mode & 0o777).toBe(0o751);
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("consumed");
});

test("real CLI: UTF-8 byte ranges, CRLF, metavariables and empty replacement", async () => {
  requireCli();
  const f = await fixture(["const 한 = '😀'; console.log(한);\r\n"]);
  const text = (await f.read())[0];
  const found = await f.engine.search(f.root, f.query);
  const start = Buffer.byteLength(text.slice(0, text.indexOf("console")));
  expect(found.matches[0].startByte).toBe(start);
  expect(found.matches[0].start).toEqual({ line: 1, byteColumn: start + 1 });
  expect(found.matches[0].text).toBe("console.log(한)");
  const preview = await f.engine.preview(f.root, f.query);
  await f.engine.apply(f.root, preview.previewId!);
  expect((await f.read())[0]).toBe("const 한 = '😀'; logger.info(한);\r\n");
  const deletion = await f.engine.preview(f.root, { ...f.query, pattern: "logger.info($A)", replacement: "" });
  await f.engine.apply(f.root, deletion.previewId!);
  expect((await f.read())[0]).toBe("const 한 = '😀'; ;\r\n");
});

test("real CLI: stale second file rejects all writes and consumes token", async () => {
  requireCli();
  const f = await fixture();
  const preview = await f.engine.preview(f.root, f.query);
  await writeFile(join(f.root, f.files[1]), "user edit\n");
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("changed since preview");
  expect(await f.read()).toEqual(["console.log('one');\n", "user edit\n"]);
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("consumed");
});

test("real CLI: zero-match scoped files also participate in all-or-none preflight", async () => {
  requireCli();
  const f = await fixture(["console.log('one');\n", "const untouched = 1;\n"]);
  const preview = await f.engine.preview(f.root, f.query);
  await writeFile(join(f.root, f.files[1]), "const untouched = 2;\n");
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("changed since preview");
  expect((await f.read())[0]).toBe("console.log('one');\n");
});

test("real CLI: replacement inode with identical bytes invalidates preview", async () => {
  requireCli();
  const f = await fixture();
  const preview = await f.engine.preview(f.root, f.query);
  const temporary = join(f.root, "replacement.ts");
  await writeFile(temporary, (await f.read())[1]);
  await rename(temporary, join(f.root, f.files[1]));
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("changed since preview");
  expect((await f.read())[0]).toBe("console.log('one');\n");
});

test("overlapping, duplicate and invalid UTF-8 byte edits are rejected; adjacent edits work", () => {
  const bytes = Buffer.from("f(f(x))");
  expect(() => applyEdits(bytes, [{ start: 0, end: 7, replacement: "a" }, { start: 2, end: 6, replacement: "b" }])).toThrow("Overlapping");
  expect(() => applyEdits(bytes, [{ start: 0, end: 1, replacement: "a" }, { start: 0, end: 1, replacement: "b" }])).toThrow("Overlapping");
  expect(() => applyEdits(Buffer.from("한"), [{ start: 0, end: 1, replacement: "a" }])).toThrow("UTF-8");
  expect(() => applyEdits(bytes, [{ start: 0, end: 1, replacement: "\ud800" }])).toThrow("Unicode");
  expect(applyEdits(Buffer.from("ab"), [{ start: 0, end: 1, replacement: "x" }, { start: 1, end: 2, replacement: "y" }]).toString()).toBe("xy");
});

test("paths reject traversal, outside cwd, directories, duplicate aliases, symlinks and hardlinks before invoking CLI", async () => {
  const f = await fixture();
  const outside = await fixture();
  await mkdir(join(f.root, "sub"));
  await symlink(join(outside.root, outside.files[0]), join(f.root, "escape.ts"));
  await symlink(join(f.root, f.files[0]), join(f.root, "alias.ts"));
  await symlink(outside.root, join(f.root, "linked"));
  for (const files of [["../outside.ts"], [join(outside.root, outside.files[0])], ["sub"], ["escape.ts"], ["alias.ts"], ["linked/1.ts"], ["1.ts", "./1.ts"]]) {
    await expect(f.engine.search(f.root, { ...f.query, files })).rejects.toThrow();
  }
  await link(join(f.root, f.files[0]), join(f.root, "hard.ts"));
  await expect(f.engine.search(f.root, { ...f.query, files: ["hard.ts"] })).rejects.toThrow("hardlinked");
  expect(await outside.read()).toEqual(["console.log('one');\n", "console.log('two');\n"]);
});

test("unsupported encoding and oversized files fail before invoking CLI", async () => {
  const f = await fixture(["ok"]);
  for (const bytes of [Buffer.from([0xff, 0xfe, 0x61, 0]), Buffer.from([0xc0, 0xaf]), Buffer.from("\uFEFFhello"), Buffer.from("a\0b")]) {
    await writeFile(join(f.root, f.files[0]), bytes);
    await expect(f.engine.search(f.root, f.query)).rejects.toThrow("Unsupported encoding");
  }
  await writeFile(join(f.root, f.files[0]), Buffer.alloc(LIMITS.fileBytes + 1, 0x61));
  await expect(f.engine.search(f.root, f.query)).rejects.toThrow("1 MiB");
});

test("real CLI: symlink introduced after preview rejects before touching any targets", async () => {
  requireCli();
  const f = await fixture();
  const outside = await fixture();
  const preview = await f.engine.preview(f.root, f.query);
  await rm(join(f.root, f.files[1]));
  await symlink(join(outside.root, outside.files[1]), join(f.root, f.files[1]));
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("Symlinks");
  expect((await f.read())[0]).toBe("console.log('one');\n");
  expect(await outside.read()).toEqual(["console.log('one');\n", "console.log('two');\n"]);
});

test("real CLI: mutation queue waits for an existing writer then revalidates every snapshot", async () => {
  requireCli();
  const f = await fixture();
  const preview = await f.engine.preview(f.root, f.query);
  const entered = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  const writer = withFileMutationQueue(join(f.root, f.files[0]), async () => {
    entered.resolve();
    await release.promise;
    await writeFile(join(f.root, f.files[0]), "queued writer\n");
  });
  await entered.promise;
  const application = f.engine.apply(f.root, preview.previewId!);
  const rejected = application.catch((error: unknown) => error);
  release.resolve();
  await writer;
  expect(String(await rejected)).toContain("changed since preview");
  expect(await f.read()).toEqual(["queued writer\n", "console.log('two');\n"]);
});

test("real CLI: cancellation while waiting for mutation locks prevents all writes", async () => {
  requireCli();
  const f = await fixture();
  const preview = await f.engine.preview(f.root, f.query);
  const entered = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  const holder = withFileMutationQueue(join(f.root, f.files[0]), async () => {
    entered.resolve();
    await release.promise;
  });
  await entered.promise;
  const controller = new AbortController();
  const application = f.engine.apply(f.root, preview.previewId!, controller.signal);
  const rejected = application.catch((error: unknown) => error);
  controller.abort();
  release.resolve();
  await holder;
  expect(String(await rejected)).toContain("cancelled");
  expect(await f.read()).toEqual(["console.log('one');\n", "console.log('two');\n"]);
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("consumed");
});

test("real CLI: tokens cannot be used from a different cwd", async () => {
  requireCli();
  const f = await fixture();
  const other = await fixture();
  const preview = await f.engine.preview(f.root, f.query);
  await expect(f.engine.apply(other.root, preview.previewId!)).rejects.toThrow("different cwd");
  expect(await f.read()).toEqual(["console.log('one');\n", "console.log('two');\n"]);
  await expect(f.engine.apply(f.root, preview.previewId!)).rejects.toThrow("consumed");
});

test("real CLI: leading-hyphen pattern/replacement values are data, not CLI options", async () => {
  requireCli();
  const f = await fixture(["const n = -value;\n"]);
  const preview = await f.engine.preview(f.root, { ...f.query, pattern: "-$A", replacement: "-other($A)" });
  await f.engine.apply(f.root, preview.previewId!);
  expect(await f.read()).toEqual(["const n = -other(value);\n"]);
});

test("real CLI: simultaneous replay allows exactly one apply", async () => {
  requireCli();
  const f = await fixture();
  const preview = await f.engine.preview(f.root, f.query);
  const attempts = await Promise.allSettled([f.engine.apply(f.root, preview.previewId!), f.engine.apply(f.root, preview.previewId!)]);
  expect(attempts.map((item) => item.status).sort()).toEqual(["fulfilled", "rejected"]);
  expect(await f.read()).toEqual(["logger.info('one');\n", "logger.info('two');\n"]);
});

test("real CLI: cancellation and discard invalidate tokens without writes", async () => {
  requireCli();
  const f = await fixture();
  const a = await f.engine.preview(f.root, f.query);
  const b = await f.engine.preview(f.root, f.query);
  f.engine.discard(a.previewId!);
  await expect(f.engine.apply(f.root, a.previewId!)).rejects.toThrow("discarded");
  const controller = new AbortController();
  controller.abort();
  await expect(f.engine.search(f.root, f.query, controller.signal)).rejects.toThrow("cancelled");
  await expect(f.engine.apply(f.root, b.previewId!)).rejects.toThrow("expired");
  expect(await f.read()).toEqual(["console.log('one');\n", "console.log('two');\n"]);
});

test.each(["session_start", "session_before_switch", "session_before_fork", "session_before_tree", "session_tree", "session_shutdown", "agent_settled"])("real CLI: %s invalidates tool-issued tokens", async (event) => {
  requireCli();
  const f = await fixture();
  type CapturedTool = Parameters<ExtensionAPI["registerTool"]>[0];
  const tools = new Map<string, CapturedTool>();
  const events = new Map<string, Function>();
  astTransform({ registerTool: (tool: CapturedTool) => tools.set(tool.name, tool), on: (name: string, fn: Function) => events.set(name, fn) } as unknown as ExtensionAPI);
  const ctx = { cwd: f.root };
  const result = await tools.get("ast_preview").execute("preview", f.query, undefined, undefined, ctx);
  await events.get(event)!({ aborted: true });
  await expect(tools.get("ast_apply").execute("apply", { previewId: result.details.previewId }, undefined, undefined, ctx)).rejects.toThrow("expired");
  expect(await f.read()).toEqual(["console.log('one');\n", "console.log('two');\n"]);
});

test("real CLI: match/output limits fail closed, and no-op previews have no token", async () => {
  requireCli();
  const f = await fixture();
  await expect(f.engine.preview(f.root, { ...f.query, maxMatches: 1 })).rejects.toThrow("Match limit");
  const noOp = await f.engine.preview(f.root, { ...f.query, replacement: f.query.pattern });
  expect(noOp.previewId).toBeUndefined();
  const large = await fixture([`console.log('${"x".repeat(24_000)}');\n`]);
  await expect(large.engine.preview(large.root, large.query)).rejects.toThrow("output exceeds");
  expect((await large.read())[0]).toContain("console.log");
  const previews = [];
  for (let i = 0; i < 5; i++) previews.push(await f.engine.preview(f.root, f.query));
  await expect(f.engine.apply(f.root, previews[0].previewId!)).rejects.toThrow("expired");
  await f.engine.apply(f.root, previews[4].previewId!);
});
