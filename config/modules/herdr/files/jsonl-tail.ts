import { open } from "node:fs/promises";

const READ_LIMIT = 1024 * 1024;
const MAX_LINE = 8 * 1024 * 1024;

type TailState = {
  identity: string;
  offset: number;
  line: Buffer;
  lineOffset: number;
  dropping: boolean;
};

function context(error: unknown, action: string): Error {
  const detail = error instanceof Error ? error.message : String(error);
  return new Error(`${action}: ${detail}`, { cause: error });
}

function isCode(error: unknown, code: string): boolean {
  return typeof error === "object" && error !== null && "code" in error
    && (error as NodeJS.ErrnoException).code === code;
}

export class JsonlTail {
  readonly #states = new Map<string, TailState>();
  readonly #diagnose: (message: string) => void;
  caughtUp = true;
  available = false;
  reset = false;
  hasGaps = false;

  constructor(diagnose: (message: string) => void = console.warn) {
    this.#diagnose = diagnose;
  }

  async read(path: string): Promise<unknown[]> {
    this.available = false;
    this.reset = false;
    this.hasGaps = false;
    let file;
    try {
      file = await open(path, "r");
    } catch (error) {
      if (isCode(error, "ENOENT")) {
        this.#states.delete(path);
        this.caughtUp = true;
        return [];
      }
      throw context(error, `cannot open JSONL file ${path}`);
    }

    try {
      const stat = await file.stat();
      if (!stat.isFile()) throw new Error("path is not a regular file");
      this.available = true;

      const identity = `${stat.dev}:${stat.ino}`;
      let state = this.#states.get(path);
      if (!state || state.identity !== identity || stat.size < state.offset) {
        this.reset = true;
        state = { identity, offset: 0, line: Buffer.alloc(0), lineOffset: 0, dropping: false };
        this.#states.set(path, state);
      }

      const length = Math.min(READ_LIMIT, Math.max(0, stat.size - state.offset));
      if (length === 0) {
        this.caughtUp = true;
        return [];
      }

      const chunk = Buffer.allocUnsafe(length);
      const chunkOffset = state.offset;
      const { bytesRead } = await file.read(chunk, 0, length, state.offset);
      state.offset += bytesRead;
      this.caughtUp = state.offset >= stat.size;
      return this.#records(path, state, chunk.subarray(0, bytesRead), chunkOffset);
    } catch (error) {
      this.available = false;
      throw context(error, `cannot read JSONL file ${path}`);
    } finally {
      await file.close();
    }
  }

  #records(path: string, state: TailState, chunk: Buffer, chunkOffset: number): unknown[] {
    const records: unknown[] = [];
    let cursor = 0;
    while (cursor < chunk.length) {
      const newline = chunk.indexOf(0x0a, cursor);
      const end = newline < 0 ? chunk.length : newline;
      const segment = chunk.subarray(cursor, end);

      if (!state.dropping) {
        if (state.line.length + segment.length > MAX_LINE) {
          state.line = Buffer.alloc(0);
          state.dropping = true;
          this.hasGaps = true;
          this.#diagnose(`skipping oversized JSONL record in ${path} at byte ${state.lineOffset}`);
        } else if (segment.length > 0) {
          state.line = Buffer.concat([state.line, segment]);
        }
      }

      if (newline < 0) break;
      if (state.dropping) {
        state.dropping = false;
      } else {
        try {
          const text = new TextDecoder("utf-8", { fatal: true }).decode(state.line);
          records.push(JSON.parse(text));
        } catch {
          this.hasGaps = true;
          this.#diagnose(`skipping malformed JSONL record in ${path} at byte ${state.lineOffset}`);
        }
        state.line = Buffer.alloc(0);
      }
      cursor = newline + 1;
      state.lineOffset = chunkOffset + cursor;
    }
    return records;
  }
}
