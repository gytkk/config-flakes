import { createHash, randomUUID } from "node:crypto";
import { constants } from "node:fs";
import { lstat, mkdir, open, opendir, rename, unlink } from "node:fs/promises";
import { dirname, isAbsolute, join, parse, resolve } from "node:path";
import { withFileMutationQueue } from "@earendil-works/pi-coding-agent";

export const ENTRY_TYPE = "session-artifacts.v1";
export const LIMITS = { sourceBytes: 2 * 1024 * 1024, lines: 20_000, files: 64, storeBytes: 32 * 1024 * 1024, pageBytes: 16_384, pageLines: 200 };
export interface Artifact {
  version: 1; id: string; owner: string; title: string; source: string;
  bytes: number; lines: number; sha256: string; created: string;
}
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const hash = /^[0-9a-f]{64}$/;
export const digest = (value: string | Buffer) => createHash("sha256").update(value).digest("hex");
export function validArtifact(value: unknown): value is Artifact {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const a = value as Record<string, unknown>;
  return a.version === 1 && typeof a.id === "string" && uuid.test(a.id)
    && typeof a.owner === "string" && hash.test(a.owner) && typeof a.sha256 === "string" && hash.test(a.sha256)
    && typeof a.title === "string" && a.title.length <= 160 && typeof a.source === "string" && a.source.length <= 4096
    && typeof a.bytes === "number" && Number.isInteger(a.bytes) && a.bytes >= 0 && a.bytes <= LIMITS.sourceBytes
    && typeof a.lines === "number" && Number.isInteger(a.lines) && a.lines >= 1 && a.lines <= LIMITS.lines && typeof a.created === "string" && a.created.length <= 40;
}
export function restoreArtifacts(entries: readonly unknown[]): Map<string, Artifact> {
  const result = new Map<string, Artifact>();
  for (const entry of entries) {
    if (!entry || typeof entry !== "object" || !("type" in entry) || !("customType" in entry) || !("data" in entry)) continue;
    if (entry.type === "custom" && entry.customType === ENTRY_TYPE && validArtifact(entry.data)) {
      if (result.size < LIMITS.files || result.has(entry.data.id)) result.set(entry.data.id, entry.data);
    }
  }
  return result;
}
export function integer(value: number | undefined, fallback: number, max: number, min = 1): number {
  const n = value ?? fallback;
  if (!Number.isInteger(n) || n < min || n > max) throw new Error(`Expected integer ${min}–${max}`);
  return n;
}
export function check(signal?: AbortSignal) { signal?.throwIfAborted(); }

// Reject links in every component, not just the final file. This is validation,
// not a sandbox against a concurrently hostile process with the same privileges.
export async function noLinks(path: string): Promise<void> {
  const absolute = resolve(path);
  let current = parse(absolute).root;
  for (const part of absolute.slice(current.length).split("/").filter(Boolean)) {
    current = join(current, part);
    if ((await lstat(current)).isSymbolicLink()) throw new Error("Symlink paths are not supported");
  }
}
async function privateDirectory(path: string): Promise<void> {
  // Validate existing ancestors before creating anything through them.
  await noLinks(dirname(path));
  try { await mkdir(path, { mode: 0o700 }); }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error; }
  await noLinks(path);
  const st = await lstat(path);
  if (!st.isDirectory() || (st.mode & 0o077) !== 0 || (process.getuid && st.uid !== process.getuid())) throw new Error("Artifact store must be a private user-owned directory (0700)");
}
export async function readRegular(path: string, signal?: AbortSignal, privateFile = false): Promise<Buffer> {
  check(signal);
  await noLinks(path);
  const fd = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const st = await fd.stat();
    if (!st.isFile() || st.nlink !== 1) throw new Error("Expected a regular, non-hardlinked file");
    if (privateFile && ((st.mode & 0o077) !== 0 || (process.getuid && st.uid !== process.getuid()))) throw new Error("Expected private user-owned output");
    if (st.size > LIMITS.sourceBytes) throw new Error(`Source exceeds ${LIMITS.sourceBytes} bytes`);
    const buffer = Buffer.alloc(Math.min(st.size + 1, LIMITS.sourceBytes + 1));
    let used = 0;
    while (used < buffer.length) {
      check(signal);
      const { bytesRead } = await fd.read(buffer, used, buffer.length - used, used);
      if (!bytesRead) break;
      used += bytesRead;
    }
    const after = await fd.stat();
    if (after.size !== st.size || after.mtimeMs !== st.mtimeMs || used !== st.size) throw new Error("Source changed during snapshot");
    return buffer.subarray(0, used);
  } finally { await fd.close(); }
}
export function decode(buffer: Buffer): string {
  let text: string;
  try { text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(buffer); }
  catch { throw new Error("Artifact must be UTF-8 text, not binary"); }
  if (/[\x00-\x08\x0b\x0c\x0e-\x1a\x1c-\x1f]/.test(text)) throw new Error("Artifact must be text, not binary");
  return text;
}

export class ArtifactStore {
  constructor(readonly root: string) {}
  path(a: Artifact) {
    if (!validArtifact(a)) throw new Error("Invalid artifact record");
    return join(this.root, a.owner, `${a.id}.txt`);
  }
  async register(session: string, source: string, title: string, options: { offset?: number; limit?: number; privateFile?: boolean; signal?: AbortSignal } = {}): Promise<Artifact> {
    if (!isAbsolute(source) || source.length > 4096 || source.includes("\0")) throw new Error("Invalid source path");
    const { signal } = options;
    const offset = integer(options.offset, 1, LIMITS.sourceBytes);
    const limit = integer(options.limit, LIMITS.lines, LIMITS.lines);
    return withFileMutationQueue(join(this.root, ".mutation"), async () => {
      check(signal);
      const buffer = await readRegular(source, signal, options.privateFile);
      const all = decode(buffer).split("\n");
      if (offset > all.length) throw new Error("Snapshot offset is beyond end of file");
      if (options.limit === undefined && all.length - offset + 1 > LIMITS.lines) throw new Error(`Source exceeds ${LIMITS.lines} lines; select a range with limit`);
      const text = all.slice(offset - 1, offset - 1 + limit).join("\n");
      const a: Artifact = { version: 1, id: randomUUID(), owner: digest(session), title: title.slice(0, 160), source,
        bytes: Buffer.byteLength(text), lines: text.split("\n").length, sha256: digest(text), created: new Date().toISOString() };
      await privateDirectory(this.root);
      const folder = dirname(this.path(a));
      await privateDirectory(folder);
      // Enumerate only this extension's own session store, never source directories.
      let files = 0, bytes = 0;
      const dir = await opendir(folder);
      for await (const item of dir) {
        if (++files >= LIMITS.files + 1) throw new Error("Session artifact file quota reached; manual cleanup required");
        const st = await lstat(join(folder, item.name));
        if (!st.isFile() || st.isSymbolicLink()) throw new Error("Unexpected entry in artifact store");
        bytes += st.size;
      }
      if (files >= LIMITS.files || bytes + a.bytes > LIMITS.storeBytes) throw new Error("Session artifact quota reached; manual cleanup required");
      return withFileMutationQueue(this.path(a), async () => {
        const temporary = join(folder, `${a.id}.pending`);
        let fd;
        try {
          check(signal);
          fd = await open(temporary, "wx", 0o600);
          await fd.writeFile(text, "utf8");
          await fd.sync();
          await fd.close(); fd = undefined;
          check(signal);
          await rename(temporary, this.path(a));
          return a;
        } finally {
          await fd?.close();
          await unlink(temporary).catch(() => {});
        }
      });
    });
  }
  async text(a: Artifact, signal?: AbortSignal): Promise<string> {
    const data = await readRegular(this.path(a), signal, true);
    if (data.length !== a.bytes || digest(data) !== a.sha256) throw new Error("Artifact snapshot changed or is corrupt");
    check(signal);
    return decode(data);
  }
}

export function page(text: string, offset = 1, limit = LIMITS.pageLines, column = 0) {
  integer(offset, 1, LIMITS.sourceBytes); integer(limit, LIMITS.pageLines, LIMITS.pageLines); integer(column, 0, LIMITS.sourceBytes, 0);
  const lines = text.split("\n");
  if (offset > lines.length || column > lines[offset - 1].length) throw new Error("Page cursor beyond end of artifact");
  const output: string[] = [];
  let budget = LIMITS.pageBytes - 512, nextOffset = offset, nextColumn = column;
  for (let i = offset - 1; i < lines.length && output.length < limit && budget > 32; i++) {
    const start = i === offset - 1 ? column : 0;
    const line = lines[i].slice(start);
    // Codepoint iteration avoids splitting surrogate pairs or UTF-8 characters.
    let chunk = "", used = 0;
    for (const char of line) { const size = Buffer.byteLength(char); if (used + size > budget - 32) break; chunk += char; used += size; }
    output.push(`${i + 1}:${start} ${chunk}`); budget -= used + 32;
    if (chunk.length < line.length) { nextOffset = i + 1; nextColumn = start + chunk.length; break; }
    nextOffset = i + 2; nextColumn = 0;
  }
  return { text: output.join("\n"), nextOffset: nextOffset <= lines.length ? nextOffset : undefined, nextColumn, totalLines: lines.length };
}
export function search(text: string, query: string, offset = 1, limit = 50, ignoreCase = false) {
  if (!query || query.length > 256 || query.includes("\n") || query.includes("\r")) throw new Error("Search needs a single-line literal of 1–256 characters");
  integer(offset, 1, LIMITS.sourceBytes); integer(limit, 50, 50);
  const lines = text.split("\n"), matches: string[] = [];
  const needle = ignoreCase ? query.toLowerCase() : query;
  let nextOffset: number | undefined;
  let budget = LIMITS.pageBytes - 512;
  for (let i = offset - 1; i < lines.length; i++) {
    const at = (ignoreCase ? lines[i].toLowerCase() : lines[i]).indexOf(needle);
    if (at < 0) continue;
    if (matches.length >= limit) { nextOffset = i + 1; break; }
    const start = Math.max(0, at - 40);
    const snippet = `${i + 1}: ${start ? "…" : ""}${lines[i].slice(start, start + 100)}${lines[i].length > start + 100 ? "…" : ""}`;
    const bytes = Buffer.byteLength(snippet) + 1;
    if (bytes > budget) { nextOffset = i + 1; break; }
    matches.push(snippet); budget -= bytes;
  }
  return { text: matches.join("\n") || "No matches.", matches: matches.length, nextOffset };
}
