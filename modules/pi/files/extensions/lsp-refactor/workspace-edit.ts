import { constants } from "node:fs";
import * as fs from "node:fs/promises";
import type { Stats } from "node:fs";
import { randomUUID } from "node:crypto";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { TextDecoder } from "node:util";

export type EditPlan = {
  root: string;
  files: Array<{ path: string; before: string; after: string; version?: number }>;
  diff: string;
};

type PlannedFile = EditPlan["files"][number];
type RecordValue = Record<string, unknown>;
type RawFile = { uri: string; edits: unknown[]; version?: number };
type Snapshot = { text: string; stat: Stats };
type FileState = { aliases: string[]; snapshot: Snapshot };
type PlanState = {
  rootPath: string;
  rootStat: Stats;
  files: Map<string, FileState>;
  versions: Map<string, number>;
  versionChecks: Array<{ uri: string; version: number }>;
};
type StagedFile = { file: PlannedFile; temporary: string; stat: Stats };

// Plans are capabilities, not a serialization format. Keep filesystem provenance private.
const planStates = new WeakMap<EditPlan, PlanState>();
const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });

function record(value: unknown): value is RecordValue {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function abort(signal?: AbortSignal): void {
  if (signal?.aborted) {
    const error = new Error("Workspace edit aborted.");
    error.name = "AbortError";
    throw error;
  }
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function inside(root: string, file: string): boolean {
  const relative = path.relative(root, file);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

function localPath(uri: unknown): string {
  if (typeof uri !== "string") throw new Error("Workspace edit URI must be a string.");
  let url: URL;
  try {
    url = new URL(uri);
  } catch {
    throw new Error(`Invalid workspace edit URI: ${uri}`);
  }
  if (url.protocol !== "file:" || url.hostname !== "" || url.search !== "" || url.hash !== "") {
    throw new Error(`Only local file URIs without query or fragment are supported: ${uri}`);
  }
  try {
    return fileURLToPath(url);
  } catch {
    throw new Error(`Invalid local file URI: ${uri}`);
  }
}

function versionNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 2_147_483_647;
}

function parseEdit(edit: unknown, versions: Map<string, number>): RawFile[] {
  if (!record(edit)) throw new Error("Expected an LSP WorkspaceEdit object.");
  const hasChanges = Object.hasOwn(edit, "changes");
  const hasDocuments = Object.hasOwn(edit, "documentChanges");
  if (hasChanges && hasDocuments) throw new Error("Ambiguous WorkspaceEdit: changes and documentChanges cannot be combined.");
  const files: RawFile[] = [];
  if (hasChanges) {
    if (!record(edit.changes)) throw new Error("WorkspaceEdit.changes must be an object.");
    for (const [uri, edits] of Object.entries(edit.changes)) {
      if (!Array.isArray(edits)) throw new Error(`Text edits must be an array: ${uri}`);
      files.push({ uri, edits });
    }
  } else if (hasDocuments) {
    if (!Array.isArray(edit.documentChanges)) throw new Error("WorkspaceEdit.documentChanges must be an array.");
    for (const change of edit.documentChanges) {
      if (!record(change) || Object.hasOwn(change, "kind") || !record(change.textDocument) || !Array.isArray(change.edits)) {
        throw new Error("Only TextDocumentEdit operations are supported; resource operations cannot be applied.");
      }
      const { uri, version } = change.textDocument;
      if (typeof uri !== "string") throw new Error("TextDocumentEdit requires a file URI.");
      if (version !== null && !versionNumber(version)) throw new Error(`Invalid document version: ${uri}`);
      // Some servers (including gopls) use version 0 for unopened, on-disk files.
      // Such files have no client version; the before snapshot guards their apply.
      const unversioned = version === null || (version === 0 && !versions.has(uri));
      if (!unversioned && versions.get(uri) !== version) throw new Error(`Document version is stale or unavailable: ${uri}`);
      files.push({ uri, edits: change.edits, ...(unversioned ? {} : { version: version as number }) });
    }
  }
  if (!files.some((file) => file.edits.length > 0)) throw new Error("WorkspaceEdit contains no text edits.");
  return files;
}

function sameIdentity(a: Stats, b: Stats): boolean {
  return a.dev === b.dev && a.ino === b.ino;
}

function sameSnapshot(a: Stats, b: Stats): boolean {
  return sameIdentity(a, b) && a.size === b.size && a.mtimeMs === b.mtimeMs && a.ctimeMs === b.ctimeMs && a.mode === b.mode;
}

async function snapshot(file: string, signal?: AbortSignal): Promise<Snapshot> {
  abort(signal);
  const entry = await fs.lstat(file);
  if (!entry.isFile()) throw new Error(`Workspace edit target is not a regular file: ${file}`);
  // O_NONBLOCK also prevents a raced-in FIFO from hanging before fstat can reject it.
  const handle = await fs.open(file, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const before = await handle.stat();
    if (!before.isFile()) throw new Error(`Workspace edit target is not a regular file: ${file}`);
    if (!sameSnapshot(entry, before)) throw new Error(`File changed while being opened: ${file}`);
    const bytes = await handle.readFile({ signal });
    const after = await handle.stat();
    if (!sameSnapshot(before, after)) throw new Error(`File changed while being read: ${file}`);
    abort(signal);
    let text: string;
    try {
      text = decoder.decode(bytes);
    } catch {
      throw new Error(`Workspace edits require valid UTF-8 text: ${file}`);
    }
    return { text, stat: after };
  } finally {
    await handle.close();
  }
}

function lineOffsets(text: string): Array<{ start: number; end: number }> {
  const lines: Array<{ start: number; end: number }> = [];
  let start = 0;
  for (let index = 0; index < text.length; index++) {
    const code = text.charCodeAt(index);
    if (code !== 10 && code !== 13) continue;
    lines.push({ start, end: index });
    if (code === 13 && text.charCodeAt(index + 1) === 10) index++;
    start = index + 1;
  }
  lines.push({ start, end: text.length });
  return lines;
}

function offset(position: unknown, text: string, lines: Array<{ start: number; end: number }>): number {
  if (!record(position) || !Number.isSafeInteger(position.line) || !Number.isSafeInteger(position.character)) {
    throw new Error("Text edit position requires integer line and UTF-16 character offsets.");
  }
  const line = position.line as number;
  const character = position.character as number;
  if (line < 0 || line >= lines.length || character < 0 || character > lines[line].end - lines[line].start) {
    throw new Error("Text edit position is outside the document or line.");
  }
  const result = lines[line].start + character;
  const previous = text.charCodeAt(result - 1);
  const next = text.charCodeAt(result);
  if (previous >= 0xd800 && previous <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
    throw new Error("Text edit position splits a UTF-16 surrogate pair.");
  }
  return result;
}

function validUnicode(text: string): boolean {
  for (let index = 0; index < text.length; index++) {
    const code = text.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = text.charCodeAt(++index);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function editedText(before: string, rawEdits: unknown[]): string {
  const lines = lineOffsets(before);
  const edits = rawEdits.map((edit, index) => {
    if (!record(edit) || !record(edit.range) || typeof edit.newText !== "string" || !validUnicode(edit.newText)) {
      throw new Error("Invalid text edit: expected a range and valid Unicode newText.");
    }
    const start = offset(edit.range.start, before, lines);
    const end = offset(edit.range.end, before, lines);
    if (end < start) throw new Error("Text edit range ends before it starts.");
    return { start, end, text: edit.newText, index };
  });
  // Inserts at a shared position retain server order and precede a replacement there.
  edits.sort((a, b) => a.start - b.start || a.end - b.end || a.index - b.index);
  const parts: string[] = [];
  let cursor = 0;
  for (const edit of edits) {
    if (edit.start < cursor) throw new Error("Overlapping text edits are not supported.");
    parts.push(before.slice(cursor, edit.start), edit.text);
    cursor = edit.end;
  }
  parts.push(before.slice(cursor));
  return parts.join("");
}

function diffLines(text: string): string[] {
  return text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
}

function diffLine(prefix: string, text: string): string {
  return text.endsWith("\n") ? prefix + text : `${prefix}${text}\n\\ No newline at end of file\n`;
}

function fileDiff(root: string, file: PlannedFile): string {
  const before = diffLines(file.before);
  const after = diffLines(file.after);
  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) prefix++;
  let suffix = 0;
  while (suffix < before.length - prefix && suffix < after.length - prefix && before[before.length - suffix - 1] === after[after.length - suffix - 1]) suffix++;
  const start = Math.max(0, prefix - 3);
  const beforeEnd = Math.min(before.length, before.length - suffix + 3);
  const afterEnd = Math.min(after.length, after.length - suffix + 3);
  const beforeCount = beforeEnd - start;
  const afterCount = afterEnd - start;
  const name = path.relative(root, file.path).split(path.sep).join("/");
  const result = [
    `--- ${JSON.stringify(`a/${name}`)}\n`,
    `+++ ${JSON.stringify(`b/${name}`)}\n`,
    `@@ -${beforeCount === 0 ? start : start + 1},${beforeCount} +${afterCount === 0 ? start : start + 1},${afterCount} @@\n`,
  ];
  for (let index = start; index < prefix; index++) result.push(diffLine(" ", before[index]));
  for (let index = prefix; index < before.length - suffix; index++) result.push(diffLine("-", before[index]));
  for (let index = prefix; index < after.length - suffix; index++) result.push(diffLine("+", after[index]));
  for (let index = before.length - suffix; index < beforeEnd; index++) result.push(diffLine(" ", before[index]));
  return result.join("");
}

/** Validate the complete edit and prepare an in-memory preview without writing files. */
export async function prepareEdit(root: string, edit: unknown, versions: Map<string, number>, signal?: AbortSignal): Promise<EditPlan> {
  abort(signal);
  if (!path.isAbsolute(root)) throw new Error("Workspace root must be an absolute path.");
  const rawFiles = parseEdit(edit, versions);
  const canonicalRoot = await fs.realpath(root);
  const rootStat = await fs.stat(canonicalRoot);
  if (!rootStat.isDirectory()) throw new Error("Workspace root must be a directory.");
  const state: PlanState = { rootPath: root, rootStat, files: new Map(), versions, versionChecks: [] };
  const files: EditPlan["files"] = [];
  for (const raw of rawFiles) {
    abort(signal);
    const requested = localPath(raw.uri);
    const canonical = await fs.realpath(requested);
    if (!inside(canonicalRoot, canonical)) throw new Error(`Workspace edit target escapes the project root: ${requested}`);
    if (state.files.has(canonical)) throw new Error(`Multiple edit entries refer to the same file: ${canonical}`);
    const current = await snapshot(canonical, signal);
    const after = editedText(current.text, raw.edits);
    state.files.set(canonical, { aliases: [requested], snapshot: current });
    if (raw.version !== undefined) state.versionChecks.push({ uri: raw.uri, version: raw.version });
    if (after !== current.text) files.push({ path: canonical, before: current.text, after, ...(raw.version === undefined ? {} : { version: raw.version }) });
  }
  if (files.length === 0) throw new Error("WorkspaceEdit does not change any file contents.");
  files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  const plan: EditPlan = { root: canonicalRoot, files, diff: files.map((file) => fileDiff(canonicalRoot, file)).join("\n") };
  // Prevent a consumer from changing what an approved preview will apply.
  for (const file of files) Object.freeze(file);
  Object.freeze(files);
  Object.freeze(plan);
  planStates.set(plan, state);
  await preflight(plan, state, signal);
  return plan;
}

async function checkRoot(plan: EditPlan, state: PlanState): Promise<void> {
  if (await fs.realpath(state.rootPath) !== plan.root || await fs.realpath(plan.root) !== plan.root) {
    throw new Error("Workspace root changed since preview.");
  }
  const current = await fs.stat(plan.root);
  if (!current.isDirectory() || !sameIdentity(current, state.rootStat)) throw new Error("Workspace root was replaced since preview.");
}

async function checkPath(plan: EditPlan, file: PlannedFile, state: PlanState): Promise<void> {
  await checkRoot(plan, state);
  if (!inside(plan.root, file.path) || await fs.realpath(file.path) !== file.path) throw new Error(`Workspace edit path changed: ${file.path}`);
  for (const alias of state.files.get(file.path)!.aliases) {
    if (await fs.realpath(alias) !== file.path) throw new Error(`Workspace edit symlink target changed: ${alias}`);
  }
}

async function unchanged(plan: EditPlan, file: PlannedFile, state: PlanState, signal?: AbortSignal): Promise<void> {
  abort(signal);
  await checkPath(plan, file, state);
  const current = await snapshot(file.path, signal);
  const original = state.files.get(file.path)!.snapshot;
  if (current.text !== file.before || !sameSnapshot(current.stat, original.stat)) throw new Error(`File changed since preview: ${file.path}`);
  // realpath after reading also catches a directory being swapped during the read.
  await checkPath(plan, file, state);
}

async function preflight(plan: EditPlan, state: PlanState, signal?: AbortSignal): Promise<void> {
  abort(signal);
  for (const check of state.versionChecks) {
    if (state.versions.get(check.uri) !== check.version) throw new Error(`Document version changed since preview: ${check.uri}`);
  }
  for (const file of plan.files) await unchanged(plan, file, state, signal);
  abort(signal);
}

async function stage(file: PlannedFile, text: string, mode: number, signal?: AbortSignal): Promise<StagedFile> {
  abort(signal);
  const temporary = path.join(path.dirname(file.path), `.pi-lsp-edit-${randomUUID()}.tmp`);
  const handle = await fs.open(temporary, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
  try {
    await handle.writeFile(text, { encoding: "utf8", signal });
    await handle.chmod(mode & 0o7777);
    await handle.sync();
    const stat = await handle.stat();
    abort(signal);
    await handle.close();
    return { file, temporary, stat };
  } catch (error) {
    const failures: string[] = [];
    try { await handle.close(); } catch (closeError) { failures.push(message(closeError)); }
    try { await fs.unlink(temporary); } catch (unlinkError) { failures.push(message(unlinkError)); }
    throw new Error(`${message(error)}${failures.length ? `; temporary file cleanup failed: ${failures.join("; ")}` : ""}`);
  }
}

async function removeTemporary(staged: StagedFile): Promise<void> {
  try {
    await fs.unlink(staged.temporary);
  } catch (error) {
    if (!record(error) || error.code !== "ENOENT") throw error;
  }
}

async function rollback(plan: EditPlan, committed: StagedFile[], state: PlanState): Promise<string[]> {
  const errors: string[] = [];
  for (const written of [...committed].reverse()) {
    let restore: StagedFile | undefined;
    try {
      await checkPath(plan, written.file, state);
      const current = await snapshot(written.file.path);
      if (current.text !== written.file.after || !sameIdentity(current.stat, written.stat) || current.stat.mtimeMs !== written.stat.mtimeMs || current.stat.mode !== written.stat.mode) {
        throw new Error("Intervening changes detected; refusing to overwrite them");
      }
      restore = await stage(written.file, written.file.before, state.files.get(written.file.path)!.snapshot.stat.mode);
      await checkPath(plan, written.file, state);
      const rechecked = await snapshot(written.file.path);
      if (rechecked.text !== current.text || !sameSnapshot(rechecked.stat, current.stat)) throw new Error("File changed during rollback; refusing to overwrite it");
      await fs.rename(restore.temporary, written.file.path);
    } catch (error) {
      errors.push(`${written.file.path}: ${message(error)}`);
    } finally {
      if (restore) {
        try { await removeTemporary(restore); } catch (error) { errors.push(`Rollback temporary file ${restore.temporary}: ${message(error)}`); }
      }
    }
  }
  return errors;
}

/**
 * Acquire every caller-provided file lock in canonical path order before preflight.
 * Atomic per-file renames prevent truncation, not a filesystem-wide transaction.
 * External editors do not share these locks: there remains an unavoidable race
 * between the last snapshot check and rename, including during rollback. Crash
 * recovery and a race-free compare-and-swap need filesystem/editor cooperation.
 */
export async function applyEdit(plan: EditPlan, withFileLock: (file: string, fn: () => Promise<void>) => Promise<void>, signal?: AbortSignal): Promise<string[]> {
  const state = planStates.get(plan);
  if (!state) throw new Error("Unknown workspace edit plan; prepare it in this process first.");
  const acquire = async (index: number): Promise<void> => {
    abort(signal);
    if (index < plan.files.length) {
      await withFileLock(plan.files[index].path, () => acquire(index + 1));
      return;
    }
    const staged: StagedFile[] = [];
    const committed: StagedFile[] = [];
    let failure: unknown;
    const cleanupErrors: string[] = [];
    try {
      await preflight(plan, state, signal);
      for (const file of plan.files) staged.push(await stage(file, file.after, state.files.get(file.path)!.snapshot.stat.mode, signal));
      // Staging can take time. Recheck the whole set before mutating any target.
      await preflight(plan, state, signal);
      for (const pending of staged) {
        await unchanged(plan, pending.file, state, signal);
        abort(signal);
        await fs.rename(pending.temporary, pending.file.path);
        committed.push(pending);
        abort(signal);
      }
    } catch (error) {
      const rollbackErrors = await rollback(plan, committed, state);
      failure = new Error(`${message(error)}${rollbackErrors.length ? `; rollback incomplete: ${rollbackErrors.join("; ")}` : committed.length ? "; all written files rolled back" : ""}`);
    } finally {
      for (const pending of staged) {
        try { await removeTemporary(pending); } catch (error) { cleanupErrors.push(`${pending.temporary}: ${message(error)}`); }
      }
    }
    if (failure || cleanupErrors.length) {
      throw new Error(`${failure ? message(failure) : "Edits applied"}${cleanupErrors.length ? `; temporary file cleanup failed: ${cleanupErrors.join("; ")}` : ""}`);
    }
  };
  await acquire(0);
  return plan.files.map((file) => file.path);
}
