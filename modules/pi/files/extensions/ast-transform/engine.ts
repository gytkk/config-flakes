import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { constants, type Stats } from "node:fs";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { TextDecoder } from "node:util";
import { withFileMutationQueue } from "@earendil-works/pi-coding-agent";

export const LIMITS = { files: 32, fileBytes: 1024 * 1024, totalBytes: 8 * 1024 * 1024, matches: 200, outputBytes: 40_000, outputLines: 1500, previews: 4 };
const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
export type Query = { files: string[]; language: string; pattern: string; maxMatches?: number };
export type RewriteQuery = Query & { replacement: string };
type Snapshot = { path: string; name: string; bytes: Buffer; stat: Stats };
type Scope = { root: string; rootStat: Stats; files: Snapshot[] };
export type Match = { path: string; startByte: number; endByte: number; start: { line: number; byteColumn: number }; end: { line: number; byteColumn: number }; text: string };
type Edit = { start: number; end: number; replacement: string };
type ChangedFile = { snapshot: Snapshot; after: Buffer };
type Preview = { scope: Scope; changes: ChangedFile[]; expires: number };
type Stage = { path: string; stat: Stats; change: ChangedFile };

function abort(signal?: AbortSignal) {
  if (signal?.aborted) throw new Error("AST operation cancelled; preview tokens invalidated.");
}
function message(error: unknown): string { return error instanceof Error ? error.message : String(error); }
function identity(a: Stats, b: Stats) { return a.dev === b.dev && a.ino === b.ino; }
function same(a: Stats, b: Stats) {
  return identity(a, b) && a.size === b.size && a.mode === b.mode && a.mtimeMs === b.mtimeMs && a.ctimeMs === b.ctimeMs && a.nlink === b.nlink;
}
function inside(root: string, file: string) {
  const relative = path.relative(root, file);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}
function utf8(bytes: Buffer, name: string): string {
  let text: string;
  try { text = decoder.decode(bytes); } catch { throw new Error(`Unsupported encoding: valid UTF-8 required: ${name}`); }
  if (text.includes("\0") || text.startsWith("\uFEFF")) throw new Error(`Unsupported encoding: NUL and BOM files are not supported: ${name}`);
  return text;
}
function unicode(text: unknown, label: string, maximum = 16_384): asserts text is string {
  if (typeof text !== "string" || Buffer.byteLength(text) > maximum || text.includes("\0") || Buffer.from(text).toString("utf8") !== text) {
    throw new Error(`${label} must be valid Unicode without NUL, at most ${maximum} UTF-8 bytes.`);
  }
}
function display(text: string) { return text.replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, "0")}`); }
export function bounded(text: string): string {
  const visible = display(text);
  if (Buffer.byteLength(visible) > LIMITS.outputBytes || visible.split("\n").length > LIMITS.outputLines) {
    throw new Error("AST output exceeds 40 KB / 1500 lines. Narrow the explicit file scope or pattern; no preview token was issued.");
  }
  return visible;
}

// Reject symlinks in every component, not just the final file. This is a
// best-effort path check, NOT a sandbox against hostile external rename races.
async function safePath(root: string, file: string) {
  if (!inside(root, file)) throw new Error("AST paths must be files inside cwd (no path escapes).");
  let current = root;
  const parts = path.relative(root, file).split(path.sep);
  for (let i = 0; i < parts.length; i++) {
    current = path.join(current, parts[i]);
    const stat = await fs.lstat(current);
    if (stat.isSymbolicLink()) throw new Error(`Symlinks are not supported: ${current}`);
    if (i < parts.length - 1 && !stat.isDirectory()) throw new Error(`Not a directory: ${current}`);
  }
  if (await fs.realpath(file) !== file) throw new Error(`Path escaped canonical cwd: ${file}`);
}
async function snapshot(root: string, file: string, signal?: AbortSignal): Promise<Snapshot> {
  abort(signal);
  await safePath(root, file);
  const entry = await fs.lstat(file);
  if (!entry.isFile() || entry.nlink !== 1) throw new Error(`Only regular, non-hardlinked files are supported: ${file}`);
  const handle = await fs.open(file, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const before = await handle.stat();
    if (!same(entry, before) || !before.isFile()) throw new Error(`File changed while opening: ${file}`);
    if (before.size > LIMITS.fileBytes) throw new Error(`File exceeds 1 MiB: ${file}`);
    // Fixed allocation bounds memory even when an external writer grows the file.
    const buffer = Buffer.alloc(before.size + 1);
    let length = 0;
    while (length < buffer.length) {
      abort(signal);
      const { bytesRead } = await handle.read(buffer, length, buffer.length - length, length);
      if (!bytesRead) break;
      length += bytesRead;
    }
    const after = await handle.stat();
    if (!same(before, after) || length !== before.size) throw new Error(`File changed while reading: ${file}`);
    const bytes = buffer.subarray(0, length);
    utf8(bytes, file);
    await safePath(root, file);
    if (!same(after, await fs.lstat(file))) throw new Error(`File changed while reading: ${file}`);
    return { path: file, name: path.relative(root, file), bytes, stat: after };
  } finally { await handle.close(); }
}
async function scope(cwd: string, files: string[], signal?: AbortSignal): Promise<Scope> {
  if (!Array.isArray(files) || files.length < 1 || files.length > LIMITS.files) throw new Error("Supply 1–32 explicit file paths; directories/globs are not supported.");
  const root = await fs.realpath(cwd);
  const rootStat = await fs.lstat(root);
  const paths = files.map((file) => {
    unicode(file, "File path", 4096);
    const name = file.replace(/^@/, "");
    if (!name || /[\x00-\x1f\x7f]/.test(name) || name.split(/[\\/]/).includes("..")) throw new Error("Unsafe file path: control characters and '..' components are not supported.");
    // Accept absolute paths under either the lexical cwd or its canonical form.
    const absolute = path.resolve(cwd, name);
    const target = inside(path.resolve(cwd), absolute) ? path.resolve(root, path.relative(path.resolve(cwd), absolute)) : absolute;
    if (!inside(root, target)) throw new Error("AST paths must be files inside cwd (no path escapes).");
    return target;
  }).sort();
  if (new Set(paths).size !== paths.length) throw new Error("Duplicate file paths are not supported.");
  const snapshots: Snapshot[] = [];
  let bytes = 0;
  for (const file of paths) {
    const item = await snapshot(root, file, signal);
    bytes += item.bytes.length;
    if (bytes > LIMITS.totalBytes) throw new Error("Explicit scope exceeds 8 MiB. Narrow the file list.");
    snapshots.push(item);
  }
  return { root, rootStat, files: snapshots };
}

// Parse exactly the bytes we snapshotted via stdin; ast-grep never receives a
// target filename, --update-all, or interactive rewrite permission.
export async function runAstGrep(root: string, bytes: Buffer, query: Query, replacement: string | undefined, signal: AbortSignal): Promise<unknown[]> {
  abort(signal);
  const args = ["run", "--stdin", `--lang=${query.language}`, `--pattern=${query.pattern}`, "--json=compact", "--color=never"];
  if (replacement !== undefined) args.push(`--rewrite=${replacement}`);
  return new Promise((resolve, reject) => {
    const child = spawn("ast-grep", args, { cwd: root, stdio: ["pipe", "pipe", "pipe"] });
    const chunks: Buffer[] = [];
    const errors: Buffer[] = [];
    let size = 0;
    let failure: Error | undefined;
    const stop = (error: Error) => { failure ??= error; child.kill("SIGKILL"); };
    const cancel = () => stop(new Error("AST operation cancelled; preview tokens invalidated."));
    signal.addEventListener("abort", cancel, { once: true });
    if (signal.aborted) cancel();
    const timer = setTimeout(() => stop(new Error("ast-grep timed out after 15 seconds. Narrow the query.")), 15_000);
    const collect = (target: Buffer[]) => (chunk: Buffer) => {
      size += chunk.length;
      if (size > 4 * 1024 * 1024) stop(new Error("ast-grep JSON exceeds 4 MiB. Narrow the query."));
      else target.push(chunk);
    };
    child.stdout.on("data", collect(chunks));
    child.stderr.on("data", collect(errors));
    child.stdin.on("error", (error: NodeJS.ErrnoException) => { if (error.code !== "EPIPE") stop(error); });
    child.on("error", (error: NodeJS.ErrnoException) => {
      failure = new Error(error.code === "ENOENT" ? "ast-grep binary unavailable. Install pkgs.ast-grep (not the Unix sg command)." : `ast-grep failed: ${error.message}`);
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      signal.removeEventListener("abort", cancel);
      if (failure) return reject(failure);
      if (code !== 0 && code !== 1) return reject(new Error(`ast-grep failed (${code}): ${display(Buffer.concat(errors).toString("utf8").slice(0, 2000))}`));
      try {
        const output = Buffer.concat(chunks).toString("utf8").trim();
        const data = output === "" && code === 1 ? [] : JSON.parse(output);
        if (!Array.isArray(data)) throw new Error("Expected ast-grep JSON array.");
        resolve(data);
      } catch (error) { reject(new Error(`Invalid ast-grep output: ${message(error).slice(0, 500)}`)); }
    });
    child.stdin.end(bytes);
  });
}
function record(value: unknown): value is Record<string, unknown> { return value !== null && typeof value === "object" && !Array.isArray(value); }
function bytePosition(bytes: Buffer, offset: number) {
  const before = bytes.subarray(0, offset);
  let line = 1;
  for (const byte of before) if (byte === 10) line++;
  return { line, byteColumn: offset - before.lastIndexOf(10) };
}
function range(bytes: Buffer, start: unknown, end: unknown): { start: number; end: number } {
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || (start as number) < 0 || (end as number) <= (start as number) || (end as number) > bytes.length) throw new Error("Invalid or empty ast-grep byte range.");
  for (const offset of [start as number, end as number]) {
    if (offset < bytes.length && (bytes[offset] & 0xc0) === 0x80) throw new Error("ast-grep range splits a UTF-8 character.");
  }
  return { start: start as number, end: end as number };
}
export function applyEdits(bytes: Buffer, edits: Edit[]): Buffer {
  const sorted = [...edits].sort((a, b) => a.start - b.start || a.end - b.end);
  const chunks: Buffer[] = [];
  let cursor = 0;
  let size = bytes.length;
  for (const edit of sorted) {
    range(bytes, edit.start, edit.end);
    unicode(edit.replacement, "ast-grep replacement", LIMITS.fileBytes);
    if (edit.start < cursor) throw new Error("Overlapping AST replacements are not supported. Narrow the pattern.");
    size += Buffer.byteLength(edit.replacement) - (edit.end - edit.start);
    if (size > LIMITS.fileBytes) throw new Error("Rewritten file exceeds 1 MiB.");
    chunks.push(bytes.subarray(cursor, edit.start), Buffer.from(edit.replacement));
    cursor = edit.end;
  }
  chunks.push(bytes.subarray(cursor));
  return Buffer.concat(chunks);
}
function parseMatches(file: Snapshot, rows: unknown[], rewrite: boolean): { matches: Match[]; edits: Edit[] } {
  const matches: Match[] = [];
  const edits: Edit[] = [];
  for (const row of rows) {
    if (!record(row) || !record(row.range) || !record(row.range.byteOffset) || typeof row.text !== "string") throw new Error("Unsupported ast-grep JSON match format.");
    const { start, end } = range(file.bytes, row.range.byteOffset.start, row.range.byteOffset.end);
    if (file.bytes.subarray(start, end).toString("utf8") !== row.text) throw new Error("ast-grep match does not match the file snapshot.");
    matches.push({ path: file.name, startByte: start, endByte: end, start: bytePosition(file.bytes, start), end: bytePosition(file.bytes, end), text: row.text });
    if (rewrite) {
      unicode(row.replacement, "ast-grep replacement", LIMITS.fileBytes);
      // Newer ast-grep emits replacementOffsets; never silently apply a fix at
      // the match range if the CLI explicitly supplied a different edit range.
      const offsets = row.replacementOffsets;
      const editRange = offsets === undefined ? { start, end } : record(offsets) ? range(file.bytes, offsets.start, offsets.end) : (() => { throw new Error("Invalid ast-grep replacement offsets."); })();
      edits.push({ ...editRange, replacement: row.replacement });
    }
  }
  return { matches, edits };
}
function diff(file: ChangedFile): string {
  const lines = (bytes: Buffer) => bytes.toString("utf8").match(/[^\n]*\n|[^\n]+$/g) ?? [];
  const a = lines(file.snapshot.bytes), b = lines(file.after);
  let prefix = 0, suffix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) prefix++;
  while (suffix < a.length - prefix && suffix < b.length - prefix && a[a.length - suffix - 1] === b[b.length - suffix - 1]) suffix++;
  const start = Math.max(0, prefix - 3);
  const aEnd = Math.min(a.length, a.length - suffix + 3), bEnd = Math.min(b.length, b.length - suffix + 3);
  const line = (sign: string, text: string) => sign + text + (text.endsWith("\n") ? "" : "\n\\ No newline at end of file\n");
  return `--- ${JSON.stringify(`a/${file.snapshot.name}`)}\n+++ ${JSON.stringify(`b/${file.snapshot.name}`)}\n@@ -${aEnd === start ? start : start + 1},${aEnd - start} +${bEnd === start ? start : start + 1},${bEnd - start} @@\n`
    + a.slice(start, prefix).map((s) => line(" ", s)).join("")
    + a.slice(prefix, a.length - suffix).map((s) => line("-", s)).join("")
    + b.slice(prefix, b.length - suffix).map((s) => line("+", s)).join("")
    + a.slice(a.length - suffix, aEnd).map((s) => line(" ", s)).join("");
}
async function validateScope(scope: Scope, signal?: AbortSignal) {
  if (await fs.realpath(scope.root) !== scope.root || !identity(scope.rootStat, await fs.lstat(scope.root))) throw new Error("cwd changed since preview.");
  for (const old of scope.files) {
    const current = await snapshot(scope.root, old.path, signal);
    if (!same(old.stat, current.stat) || !old.bytes.equals(current.bytes)) throw new Error(`File changed since preview: ${old.name}. No stale preview can be applied.`);
  }
}
async function locks<T>(files: Snapshot[], action: () => Promise<T>, signal: AbortSignal, index = 0): Promise<T> {
  abort(signal);
  return index === files.length ? action() : withFileMutationQueue(files[index].path, () => locks(files, action, signal, index + 1));
}
async function stage(change: ChangedFile): Promise<Stage> {
  const temporary = path.join(path.dirname(change.snapshot.path), `.pi-ast-${randomUUID()}.tmp`);
  const handle = await fs.open(temporary, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
  try {
    await handle.writeFile(change.after);
    await handle.chmod(change.snapshot.stat.mode & 0o777);
    await handle.sync();
    return { path: temporary, stat: await handle.stat(), change };
  } catch (error) {
    await fs.unlink(temporary).catch(() => {});
    throw error;
  } finally { await handle.close(); }
}
async function cleanupStage(item: Stage) {
  const current = await fs.lstat(item.path).catch(() => undefined);
  if (current && identity(current, item.stat)) await fs.unlink(item.path);
}
async function commit(preview: Preview, signal: AbortSignal) {
  return locks(preview.scope.files, async () => {
    const staged: Stage[] = [], written: Stage[] = [];
    try {
      // ALL scoped files (including zero-match files) are checked before any
      // staging or target mutation. Staging is followed by a second full check.
      await validateScope(preview.scope, signal);
      for (const change of preview.changes) {
        abort(signal);
        staged.push(await stage(change));
      }
      await validateScope(preview.scope, signal);
      for (const item of staged) {
        abort(signal);
        const old = item.change.snapshot;
        const current = await snapshot(preview.scope.root, old.path, signal);
        if (!same(old.stat, current.stat) || !old.bytes.equals(current.bytes)) throw new Error(`File changed during apply: ${old.name}`);
        if (!same(item.stat, await fs.lstat(item.path))) throw new Error("Staged replacement changed during apply.");
        abort(signal);
        await fs.rename(item.path, old.path);
        written.push(item);
      }
      abort(signal);
    } catch (error) {
      const unrestored: string[] = [];
      for (const item of written.reverse()) {
        const old = item.change.snapshot;
        let rollback: Stage | undefined;
        try {
          const current = await snapshot(preview.scope.root, old.path);
          if (!identity(current.stat, item.stat) || !current.bytes.equals(item.change.after)) throw new Error("External change");
          rollback = await stage({ snapshot: old, after: old.bytes });
          const check = await snapshot(preview.scope.root, old.path);
          if (!same(current.stat, check.stat) || !check.bytes.equals(item.change.after)) throw new Error("External change");
          await fs.rename(rollback.path, old.path);
        } catch { unrestored.push(old.name); }
        finally { if (rollback) await cleanupStage(rollback).catch(() => {}); }
      }
      throw new Error(`${message(error)}${written.length ? unrestored.length ? ` Guarded rollback incomplete; inspect these files: ${unrestored.join(", ")}` : " All committed replacements were rolled back." : " No target files changed."}`);
    } finally {
      await Promise.all(staged.map((item) => cleanupStage(item).catch(() => {})));
    }
  }, signal);
}

export class AstTransform {
  private previews = new Map<string, Preview>();
  private active = new Set<AbortController>();
  private pending = new Set<Promise<unknown>>();
  invalidate() {
    this.previews.clear();
    for (const controller of this.active) controller.abort();
  }
  async shutdown() {
    this.invalidate();
    await Promise.allSettled([...this.pending]);
  }
  private prune() {
    for (const [id, item] of this.previews) if (item.expires < Date.now()) this.previews.delete(id);
  }
  private operation<T>(signal: AbortSignal | undefined, fn: (signal: AbortSignal) => Promise<T>): Promise<T> {
    if (this.active.size >= 4) return Promise.reject(new Error("At most four AST operations may run concurrently."));
    const controller = new AbortController();
    const cancel = () => this.invalidate();
    this.active.add(controller);
    signal?.addEventListener("abort", cancel, { once: true });
    if (signal?.aborted) cancel();
    const work = Promise.resolve().then(() => { abort(controller.signal); return fn(controller.signal); });
    this.pending.add(work);
    return work.finally(() => {
      signal?.removeEventListener("abort", cancel);
      this.active.delete(controller);
      this.pending.delete(work);
    });
  }
  private async query(cwd: string, query: Query, replacement: string | undefined, signal: AbortSignal) {
    unicode(query.language, "Language", 64);
    if (!/^[a-zA-Z][a-zA-Z0-9_-]*$/.test(query.language)) throw new Error("Supply a built-in ast-grep language name.");
    unicode(query.pattern, "Pattern");
    if (!query.pattern.trim()) throw new Error("Pattern must not be empty.");
    if (replacement !== undefined) unicode(replacement, "Replacement");
    const maximum = query.maxMatches ?? LIMITS.matches;
    if (!Number.isInteger(maximum) || maximum < 1 || maximum > LIMITS.matches) throw new Error("maxMatches must be 1–200.");
    const captured = await scope(cwd, query.files, signal);
    const matches: Match[] = [], changes: ChangedFile[] = [];
    let total = 0;
    for (const file of captured.files) {
      const rows = await runAstGrep(captured.root, file.bytes, query, replacement, signal);
      if (matches.length + rows.length > maximum) throw new Error(`Match limit (${maximum}) exceeded. Narrow the pattern; no partial preview is retained.`);
      const parsed = parseMatches(file, rows, replacement !== undefined);
      matches.push(...parsed.matches);
      if (replacement !== undefined) {
        const after = applyEdits(file.bytes, parsed.edits);
        utf8(after, file.name);
        total += after.length;
        if (total > LIMITS.totalBytes) throw new Error("Rewritten scope exceeds 8 MiB.");
        if (!after.equals(file.bytes)) changes.push({ snapshot: file, after });
      }
    }
    abort(signal);
    await validateScope(captured, signal);
    return { scope: captured, matches, changes };
  }
  search(cwd: string, query: Query, signal?: AbortSignal) {
    return this.operation(signal, async (signal) => {
      const result = await this.query(cwd, query, undefined, signal);
      const text = bounded(JSON.stringify({ coordinateSystem: "startByte/endByte: zero-based UTF-8 bytes, end exclusive; line/byteColumn: one-based", matches: result.matches }, null, 2));
      return { text, matches: result.matches };
    });
  }
  preview(cwd: string, query: RewriteQuery, signal?: AbortSignal) {
    return this.operation(signal, async (signal) => {
      unicode(query.replacement, "Replacement");
      const result = await this.query(cwd, query, query.replacement, signal);
      if (!result.changes.length) return { text: "No changes; no preview token issued.", previewId: undefined, files: [] as string[] };
      const id = randomUUID();
      const text = bounded(`Preview only; no files written. ${result.matches.length} matches, ${result.changes.length} changed files.\npreviewId: "${id}"\nReview before ast_apply. Token is single-use, expires in 10 minutes, and is invalidated by cancellation/session navigation/reload. Control characters are escaped for display.\n\n${result.changes.map(diff).join("\n")}`);
      abort(signal);
      this.prune();
      while (this.previews.size >= LIMITS.previews) this.previews.delete(this.previews.keys().next().value!);
      this.previews.set(id, { scope: result.scope, changes: result.changes, expires: Date.now() + 600_000 });
      return { text, previewId: id, files: result.changes.map((file) => file.snapshot.name) };
    });
  }
  apply(cwd: string, previewId: string, signal?: AbortSignal) {
    // Consume synchronously, before any await: concurrent replay cannot enter.
    this.prune();
    const item = this.previews.get(previewId);
    this.previews.delete(previewId);
    if (!item) return Promise.reject(new Error("Unknown, expired, discarded, or already consumed AST preview token."));
    return this.operation(signal, async (signal) => {
      if (await fs.realpath(cwd) !== item.scope.root) throw new Error("Preview belongs to a different cwd.");
      await commit(item, signal);
      return { text: `Applied exact stored preview to ${item.changes.length} files. Token consumed. External writers do not share Pi locks; per-file renames are not a crash-safe cross-file transaction.`, files: item.changes.map((file) => file.snapshot.name) };
    });
  }
  discard(previewId: string) {
    this.prune();
    if (!this.previews.delete(previewId)) throw new Error("Unknown, expired, discarded, or already consumed AST preview token.");
    return { text: "AST preview discarded; no files changed." };
  }
}
