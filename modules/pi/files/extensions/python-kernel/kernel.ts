import { spawn, type ChildProcess } from "node:child_process";
import type { Duplex } from "node:stream";
import { fileURLToPath } from "node:url";

export const MAX_CODE_BYTES = 64 * 1024;
export const MAX_FRAME_BYTES = 6 * 1024 * 1024;
export const DEFAULT_TIMEOUT = 30;
export const MAX_TIMEOUT = 120;
const MAX_PENDING = 16;

export interface PythonImage {
  type: "image";
  mimeType: "image/png" | "image/jpeg";
  data: string;
}
export interface CellResult {
  id: number;
  ok: boolean;
  stdout: string;
  stderr: string;
  value: string;
  traceback: string;
  images: PythonImage[];
  notice?: string;
}
export interface CellInput { code?: string; timeout?: number; reset?: boolean }

interface Worker {
  child: ChildProcess;
  channel: Duplex | null;
  exited: Promise<void>;
  pending?: { id: number; resolve: (result: CellResult) => void; reject: (error: Error) => void };
}

function validateResult(value: unknown, id: number): value is CellResult {
  if (!value || typeof value !== "object") return false;
  const r = value as Record<string, unknown>;
  return r.id === id && typeof r.ok === "boolean" &&
    [r.stdout, r.stderr, r.value, r.traceback].every((v) => typeof v === "string" && v.length <= 40_000) &&
    Array.isArray(r.images) && r.images.length <= 4 && r.images.every((image: unknown) => {
      if (!image || typeof image !== "object" || !("type" in image) || !("mimeType" in image) || !("data" in image)) return false;
      return image.type === "image" && (image.mimeType === "image/png" || image.mimeType === "image/jpeg") &&
        typeof image.data === "string" && image.data.length <= 2_800_000 && /^[A-Za-z0-9+/]*={0,2}$/.test(image.data);
    });
}

/** Owns one ephemeral namespace. Neither history nor interrupted code is replayed. */
export class PythonKernel {
  private worker?: Worker;
  private tail: Promise<unknown> = Promise.resolve();
  private cleanup: Promise<void> = Promise.resolve();
  private epoch = 0;
  private pending = 0;
  private sequence = 0;
  private closed = false;
  private notice: string;
  private lastReason: string;

  constructor(private readonly cwd: string, reason = "fresh session", private readonly executable = "python3") {
    this.lastReason = reason;
    this.notice = `Fresh Python kernel (${reason}); no prior state restored and no code replayed.`;
  }

  status(): string {
    return `Python kernel: ${this.closed ? "closed" : this.worker ? (this.worker.pending ? "busy" : "ready") : "not running"}; ` +
      `${this.pending} cell(s) pending. ${this.lastReason}. State is memory-only; never replayed.`;
  }

  async reset(reason = "explicit reset"): Promise<void> {
    this.epoch++;
    this.lastReason = reason;
    this.notice = `Python kernel reset (${reason}); all variables lost. No code was replayed.`;
    await this.stop(new Error(`Python kernel reset (${reason}); pending code was not replayed.`));
  }

  async close(reason = "session shutdown"): Promise<void> {
    this.closed = true;
    await this.reset(reason);
  }

  run(input: CellInput, signal?: AbortSignal): Promise<CellResult> {
    if (this.closed) return Promise.reject(new Error("Python session is closed"));
    if (signal?.aborted) return Promise.reject(new Error("Python cell cancelled before execution"));
    if (input.code !== undefined && (typeof input.code !== "string" || Buffer.byteLength(input.code) > MAX_CODE_BYTES)) {
      return Promise.reject(new Error("Python code must be a string of at most 64 KiB"));
    }
    if (input.reset !== undefined && typeof input.reset !== "boolean") return Promise.reject(new Error("reset must be boolean"));
    if (!input.reset && !input.code?.trim()) return Promise.reject(new Error("Provide code or reset: true"));
    const timeout = input.timeout ?? DEFAULT_TIMEOUT;
    if (!Number.isInteger(timeout) || timeout < 1 || timeout > MAX_TIMEOUT) {
      return Promise.reject(new Error("Python timeout must be an integer from 1 to 120 seconds"));
    }
    if (this.pending >= MAX_PENDING) return Promise.reject(new Error("Python cell queue is full (16 cells)"));
    this.pending++;
    const epoch = this.epoch;
    let executing = false;
    const task = this.tail.then(async () => {
      executing = true;
      if (signal?.aborted) throw new Error("Python cell cancelled before execution");
      if (this.closed || epoch !== this.epoch) throw new Error("Python kernel changed while queued; cell not executed. Submit again only after reviewing state loss.");
      await this.cleanup;
      if (signal?.aborted) throw new Error("Python cell cancelled before execution");
      if (this.closed || epoch !== this.epoch) throw new Error("Python kernel changed while queued; cell not executed.");
      if (input.reset) {
        // A tool reset is ordered with other cells, unlike /python reset which interrupts.
        await this.stop(new Error("Python kernel explicitly reset"));
        if (signal?.aborted) throw new Error("Python cell cancelled before execution");
        if (this.closed || epoch !== this.epoch) throw new Error("Python session changed during reset; cell not executed.");
        this.lastReason = "explicit reset";
        this.notice = "Python kernel reset; all variables lost. No code was replayed.";
      }
      const notice = this.notice;
      if (!input.code?.trim()) {
        return { id: 0, ok: true, stdout: "", stderr: "", value: "", traceback: "", images: [], notice } satisfies CellResult;
      }
      try {
        const worker = this.worker ?? this.start();
        const result = await this.execute(worker, input.code, timeout, signal);
        this.notice = "";
        return { ...result, notice: notice || undefined };
      } catch (error) {
        // The namespace is uncertain after *any* transport failure. Never retry.
        this.epoch++;
        this.lastReason = error instanceof Error ? error.message : "kernel failure";
        this.notice = `Python kernel lost (${this.lastReason}); all variables lost. No code was replayed.`;
        await this.stop(error instanceof Error ? error : new Error("Kernel failure"));
        throw new Error(`${this.lastReason}. Kernel lost; side effects may already have happened. Code was NOT replayed.`, { cause: error });
      }
    });
    this.tail = task.catch(() => {}).finally(() => { this.pending--; });
    // A queued cancellation returns promptly without touching an active sibling.
    if (!signal) return task;
    return new Promise((resolve, reject) => {
      const abort = () => {
        if (!executing) reject(new Error("Python cell cancelled while queued; code not executed"));
      };
      signal.addEventListener("abort", abort, { once: true });
      task.then(resolve, reject).finally(() => signal.removeEventListener("abort", abort));
      if (signal.aborted) abort();
    });
  }

  private start(): Worker {
    const child = spawn(this.executable, ["-u", "-B", fileURLToPath(new URL("./runner.py", import.meta.url))], {
      cwd: this.cwd,
      detached: process.platform !== "win32",
      stdio: ["ignore", "pipe", "pipe", "pipe"],
      env: { ...process.env, PYTHONIOENCODING: "utf-8", MPLBACKEND: process.env.MPLBACKEND ?? "Agg" },
    });
    // Startup diagnostics and arbitrary fd writes never enter the control parser.
    child.stdout?.resume();
    child.stderr?.resume();
    let finishExit!: () => void;
    const exited = new Promise<void>((resolve) => { finishExit = resolve; });
    const worker: Worker = { child, channel: child.stdio[3] as Duplex | null, exited };
    this.worker = worker;
    let buffer = Buffer.alloc(0);
    const failed = (message: string) => {
      const error = new Error(message);
      worker.pending?.reject(error);
      if (this.worker === worker) {
        this.epoch++;
        this.lastReason = message;
        this.notice = `Python kernel lost (${message}); variables lost. No code replayed.`;
        void this.stop(error);
      }
    };
    child.once("error", (error) => { finishExit(); failed(`Unable to run python3: ${error.message}`); });
    child.once("spawn", () => { if (!worker.channel) failed("Unable to run python3: control channel unavailable"); });
    child.once("exit", (code, signal) => {
      finishExit();
      failed(`Python process exited (${signal ?? code})`);
    });
    worker.channel?.on("error", () => failed("Python control channel failed"));
    worker.channel?.on("end", () => failed("Python control channel closed"));
    worker.channel?.on("data", (chunk: Buffer) => {
      if (this.worker !== worker) return;
      if (buffer.length + chunk.length > MAX_FRAME_BYTES + 4) return failed("Python IPC limit exceeded");
      buffer = Buffer.concat([buffer, chunk]);
      if (buffer.length < 4) return;
      const length = buffer.readUInt32BE(0);
      if (length > MAX_FRAME_BYTES) return failed("Python IPC frame too large");
      if (buffer.length < length + 4) return;
      const pending = worker.pending;
      if (!pending || buffer.length !== length + 4) return failed("Unexpected Python control frame");
      try {
        const result: unknown = JSON.parse(buffer.subarray(4).toString("utf8"));
        if (!validateResult(result, pending.id)) return failed("Invalid Python control result");
        buffer = Buffer.alloc(0);
        worker.pending = undefined;
        pending.resolve(result);
      } catch {
        failed("Invalid Python control JSON");
      }
    });
    return worker;
  }

  private execute(worker: Worker, code: string, timeout: number, signal?: AbortSignal): Promise<CellResult> {
    return new Promise((resolve, reject) => {
      const id = ++this.sequence;
      const finish = (error?: Error, result?: CellResult) => {
        clearTimeout(timer);
        signal?.removeEventListener("abort", abort);
        if (worker.pending?.id === id) worker.pending = undefined;
        if (error) reject(error);
        else resolve(result!);
      };
      const abort = () => finish(new Error("Python cell cancelled"));
      const timer = setTimeout(() => finish(new Error(`Python cell timed out after ${timeout}s`)), timeout * 1000);
      worker.pending = { id, resolve: (result) => finish(undefined, result), reject: (error) => finish(error) };
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) { abort(); return; }
      const data = Buffer.from(JSON.stringify({ id, code }));
      const frame = Buffer.allocUnsafe(data.length + 4);
      frame.writeUInt32BE(data.length, 0);
      data.copy(frame, 4);
      worker.channel?.write(frame, (error) => { if (error) finish(new Error("Python request write failed")); });
    });
  }

  private stop(error: Error): Promise<void> {
    const worker = this.worker;
    this.worker = undefined;
    if (!worker) return this.cleanup;
    worker.pending?.reject(error);
    worker.pending = undefined;
    // SIGKILL also handles Python code that catches KeyboardInterrupt/SIGTERM.
    // Descendants in the group are killed; deliberately detached children are not contained.
    try {
      if (worker.child.pid && process.platform !== "win32") process.kill(-worker.child.pid, "SIGKILL");
      else worker.child.kill("SIGKILL");
    } catch { worker.child.kill("SIGKILL"); }
    worker.channel?.destroy();
    worker.child.stdout?.destroy();
    worker.child.stderr?.destroy();
    this.cleanup = Promise.all([this.cleanup, worker.exited]).then(() => {});
    return this.cleanup;
  }
}
