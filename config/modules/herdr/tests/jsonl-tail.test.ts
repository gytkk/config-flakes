import { expect, test } from "bun:test";
import { appendFile, mkdtemp, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTail } from "../files/jsonl-tail";

test("JSONL tail handles partial Unicode, append, replacement, truncation and missing files", async () => {
  const root = await mkdtemp(join(tmpdir(), "herdr-jsonl-"));
  const path = join(root, "child.jsonl");
  try {
    const tail = new JsonlTail();
    expect(await tail.read(path)).toEqual([]);
    expect(tail.available).toBeFalse();
    const record = Buffer.from(JSON.stringify({ text: "한🙂" }) + "\n");
    const split = record.indexOf(Buffer.from("한")) + 1;
    await writeFile(path, record.subarray(0, split));
    expect(await tail.read(path)).toEqual([]);
    await appendFile(path, record.subarray(split));
    expect(await tail.read(path)).toEqual([{ text: "한🙂" }]);
    expect(await tail.read(path)).toEqual([]);
    await writeFile(`${path}.new`, '{"replacement":true}\n');
    await rename(`${path}.new`, path);
    expect(await tail.read(path)).toEqual([{ replacement: true }]);
    expect(tail.reset).toBeTrue();
    await writeFile(path, '{}\n');
    expect(await tail.read(path)).toEqual([{}]);
    expect(tail.reset).toBeTrue();
    await rm(path);
    expect(await tail.read(path)).toEqual([]);
    expect(tail.available).toBeFalse();
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("malformed records are diagnosed without exposing record contents", async () => {
  const root = await mkdtemp(join(tmpdir(), "herdr-jsonl-invalid-"));
  try {
    const path = join(root, "child.jsonl");
    const diagnostics: string[] = [];
    const tail = new JsonlTail(message => diagnostics.push(message));
    await writeFile(path, 'private malformed content\n{"valid":true}\n');
    expect(await tail.read(path)).toEqual([{ valid: true }]);
    expect(tail.hasGaps).toBeTrue();
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics[0]).toContain("byte 0");
    expect(diagnostics[0]).not.toContain("private malformed content");
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("reads bounded chunks while preserving a record across the read boundary", async () => {
  const root = await mkdtemp(join(tmpdir(), "herdr-jsonl-bounded-"));
  try {
    const path = join(root, "child.jsonl");
    const text = "x".repeat(1024 * 1024);
    await writeFile(path, JSON.stringify({ text }) + '\n{"next":true}\n');
    const tail = new JsonlTail();
    expect(await tail.read(path)).toEqual([]);
    expect(tail.available).toBeTrue();
    expect(tail.caughtUp).toBeFalse();
    expect(await tail.read(path)).toEqual([{ text }, { next: true }]);
    expect(tail.caughtUp).toBeTrue();
    expect(tail.reset).toBeFalse();
    expect(await tail.read(path)).toEqual([]);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("invalid UTF-8 and oversized records recover without exposing record contents", async () => {
  const root = await mkdtemp(join(tmpdir(), "herdr-jsonl-recovery-"));
  try {
    const path = join(root, "child.jsonl");
    const diagnostics: string[] = [];
    const tail = new JsonlTail(message => diagnostics.push(message));
    const invalid = Buffer.concat([Buffer.from('{"private":"'), Buffer.from([0xff]), Buffer.from('"}\n')]);
    await writeFile(path, Buffer.concat([invalid, Buffer.alloc(8 * 1024 * 1024 + 1, 0x78), Buffer.from('\n{"valid":true}\n')]));
    const records: unknown[] = [];
    let sawGap = false;
    for (let reads = 0; reads < 10; reads++) {
      records.push(...await tail.read(path));
      sawGap ||= tail.hasGaps;
      if (tail.caughtUp) break;
    }
    expect(tail.caughtUp).toBeTrue();
    expect(records).toEqual([{ valid: true }]);
    expect(sawGap).toBeTrue();
    expect(diagnostics).toEqual([
      `skipping malformed JSONL record in ${path} at byte 0`,
      `skipping oversized JSONL record in ${path} at byte ${invalid.length}`,
    ]);
    expect(await tail.read(path)).toEqual([]);
    expect(tail.hasGaps).toBeFalse();
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("keeps partial records and offsets independent for each path", async () => {
  const root = await mkdtemp(join(tmpdir(), "herdr-jsonl-paths-"));
  try {
    const first = join(root, "first.jsonl");
    const second = join(root, "second.jsonl");
    const tail = new JsonlTail();
    await writeFile(first, '{"first":');
    await writeFile(second, '{"second":true}\n');
    expect(await tail.read(first)).toEqual([]);
    expect(await tail.read(second)).toEqual([{ second: true }]);
    expect(tail.reset).toBeTrue();
    await appendFile(first, 'true}\n');
    expect(await tail.read(first)).toEqual([{ first: true }]);
    expect(tail.reset).toBeFalse();
    expect(await tail.read(second)).toEqual([]);
    await rm(first);
    expect(await tail.read(first)).toEqual([]);
    await appendFile(second, '{"appended":true}\n');
    expect(await tail.read(second)).toEqual([{ appended: true }]);
    expect(tail.available).toBeTrue();
    expect(tail.reset).toBeFalse();
  } finally { await rm(root, { recursive: true, force: true }); }
});
