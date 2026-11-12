import { afterEach, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { applyEdit, prepareEdit } from "../files/extensions/lsp-refactor/workspace-edit";

const roots: string[] = [];
afterEach(async () => {
  await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true })));
});
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "pi-lsp-edit-"));
  roots.push(root);
  return root;
}
const lock = async (_file: string, run: () => Promise<void>) => run();
const uri = (file: string) => pathToFileURL(file).href;
const edit = (line: number, start: number, end: number, newText: string) => ({
  range: { start: { line, character: start }, end: { line, character: end } }, newText,
});

test("preview preserves files; apply honors UTF-16 columns and CRLF across files", async () => {
  const root = await fixture();
  const a = join(root, "한글.ts"), b = join(root, "use.ts");
  await writeFile(a, 'const emoji = "😀"; const old = 1;\r\n');
  await writeFile(b, "old();\r\n");
  const plan = await prepareEdit(root, { changes: {
    [uri(a)]: [edit(0, 26, 29, "renamed")],
    [uri(b)]: [edit(0, 0, 3, "renamed")],
  } }, new Map());
  expect(await readFile(a, "utf8")).toBe('const emoji = "😀"; const old = 1;\r\n');
  expect(await readFile(b, "utf8")).toBe("old();\r\n");
  await applyEdit(plan, lock);
  expect(await readFile(a, "utf8")).toBe('const emoji = "😀"; const renamed = 1;\r\n');
  expect(await readFile(b, "utf8")).toBe("renamed();\r\n");
});

test("a stale file blocks the entire multi-file apply without overwriting user work", async () => {
  const root = await fixture();
  const a = join(root, "a.ts"), b = join(root, "b.ts");
  await writeFile(a, "old"); await writeFile(b, "old");
  const plan = await prepareEdit(root, { changes: {
    [uri(a)]: [edit(0, 0, 3, "new")], [uri(b)]: [edit(0, 0, 3, "new")],
  } }, new Map());
  await writeFile(b, "user edit");
  await expect(applyEdit(plan, lock)).rejects.toThrow();
  expect(await readFile(a, "utf8")).toBe("old");
  expect(await readFile(b, "utf8")).toBe("user edit");
});

test("versioned edits reject a mismatched document version", async () => {
  const root = await fixture(), file = join(root, "a.ts");
  await writeFile(file, "old");
  await expect(prepareEdit(root, { documentChanges: [{
    textDocument: { uri: uri(file), version: 2 }, edits: [edit(0, 0, 3, "new")],
  }] }, new Map([[uri(file), 3]]))).rejects.toThrow();
  expect(await readFile(file, "utf8")).toBe("old");
});

test("same-position inserts retain LSP array order", async () => {
  const root = await fixture(), file = join(root, "a.ts");
  await writeFile(file, "end");
  const plan = await prepareEdit(root, { changes: {
    [uri(file)]: [edit(0, 0, 0, "first "), edit(0, 0, 0, "second ")],
  } }, new Map());
  await applyEdit(plan, lock);
  expect(await readFile(file, "utf8")).toBe("first second end");
});

test("overlapping replacements and surrogate-splitting ranges cannot produce a preview", async () => {
  const root = await fixture(), file = join(root, "a.ts");
  await writeFile(file, "😀 old");
  for (const edits of [
    [edit(0, 3, 6, "new"), edit(0, 4, 6, "bad")],
    [edit(0, 1, 2, "bad")],
  ]) {
    await expect(prepareEdit(root, { changes: { [uri(file)]: edits } }, new Map())).rejects.toThrow();
  }
  expect(await readFile(file, "utf8")).toBe("😀 old");
});

test("symlink escape and a file replaced by a symlink after preview never edit the target", async () => {
  const root = await fixture(), outside = await fixture();
  const file = join(root, "a.ts"), target = join(outside, "outside.ts");
  await writeFile(file, "old"); await writeFile(target, "old");
  const change = { changes: { [uri(file)]: [edit(0, 0, 3, "new")] } };
  const plan = await prepareEdit(root, change, new Map());
  await rm(file); await symlink(target, file);
  await expect(applyEdit(plan, lock)).rejects.toThrow();
  await expect(prepareEdit(root, change, new Map())).rejects.toThrow();
  expect(await readFile(target, "utf8")).toBe("old");
});

test("a mixed edit with resource operations is rejected without partial application", async () => {
  const root = await fixture(), file = join(root, "a.ts");
  await writeFile(file, "old");
  await expect(prepareEdit(root, { documentChanges: [
    { textDocument: { uri: uri(file), version: null }, edits: [edit(0, 0, 3, "new")] },
    { kind: "delete", uri: uri(file) },
  ] }, new Map())).rejects.toThrow();
  expect(await readFile(file, "utf8")).toBe("old");
});

test("closed documents reported at version zero remain guarded by disk snapshots", async () => {
  const root = await fixture(), file = join(root, "closed.go");
  await writeFile(file, "old");
  const plan = await prepareEdit(root, { documentChanges: [{
    textDocument: { uri: uri(file), version: 0 }, edits: [edit(0, 0, 3, "new")],
  }] }, new Map());
  await applyEdit(plan, lock);
  expect(await readFile(file, "utf8")).toBe("new");
});
