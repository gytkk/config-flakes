import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { truncateHead } from "@earendil-works/pi-coding-agent";
import { Type, type Static } from "typebox";
import { PythonKernel, type PythonImage } from "./kernel.ts";

export const PythonCellSchema = Type.Object({
  code: Type.Optional(Type.String({ maxLength: 65536, description: "Python code (at most 64 KiB UTF-8); final expression is displayed. Omit only for reset." })),
  timeout: Type.Optional(Type.Integer({ minimum: 1, maximum: 120, description: "Execution timeout in seconds, default 30; timeout destroys the kernel." })),
  reset: Type.Optional(Type.Boolean({ description: "Discard all state before this cell; may be used without code." })),
}, { additionalProperties: false });
export type PythonCellInput = Static<typeof PythonCellSchema>;

export default function pythonKernelExtension(pi: ExtensionAPI) {
  let kernel: PythonKernel | undefined;
  let owner: string | undefined;
  let available = false;

  const sessionId = (ctx: ExtensionContext) => ctx.sessionManager.getSessionId();
  const current = (ctx: ExtensionContext) => {
    if (!available || !kernel || owner !== sessionId(ctx)) throw new Error("Python kernel is unavailable for this session");
    return kernel;
  };

  pi.on("session_start", async (event, ctx) => {
    available = false;
    await kernel?.close("session replaced");
    owner = sessionId(ctx);
    kernel = new PythonKernel(ctx.cwd, `${event.reason}; memory-only state`);
    available = true;
  });
  pi.on("session_shutdown", async () => {
    available = false;
    owner = undefined;
    await kernel?.close();
    kernel = undefined;
  });
  pi.on("session_tree", async (_event, ctx) => {
    await current(ctx).reset("session tree changed; historical Python state cannot be restored");
  });

  pi.registerTool({
    name: "python_cell",
    label: "Python cell",
    description: "Execute Python in a persistent session-local python3 kernel with the user's privileges. Variables/functions and top-level await persist between serialized cells; stdout, stderr, final expression (_), tracebacks and display_image(PNG/JPEG bytes, path, or matplotlib Figure) are returned. Runtime errors preserve earlier mutations. State is memory-only: reset, abort, timeout, session switch, resume, fork, reload or tree navigation discard it; code is never replayed. Default timeout 30s, maximum 120s. Text is capped at 48 KiB/1800 lines (individual streams 16 KiB); excess is discarded, not saved. Up to 4 images, 2 MiB each/4 MiB total. Use builtin edit/write for coordinated file edits; arbitrary Python I/O does not share Pi's file locks. Background work is not supported; await/join work before returning.",
    parameters: PythonCellSchema,
    async execute(_toolCallId, params, signal, _onUpdate, ctx) {
      const result = await current(ctx).run(params, signal);
      const sections = [
        result.notice,
        result.stdout && `stdout:\n${result.stdout}`,
        result.stderr && `stderr:\n${result.stderr}`,
        result.value && `result:\n${result.value}`,
        result.traceback && `Python error (earlier mutations preserved):\n${result.traceback}`,
      ].filter(Boolean);
      const bounded = truncateHead(sections.join("\n\n") || "Cell completed (no output).", { maxBytes: 48 * 1024, maxLines: 1800 });
      const text = bounded.content + (bounded.truncated ? "\n[Text truncated; excess discarded, not saved.]" : "");
      if (!result.ok) throw new Error(text);
      const content: ({ type: "text"; text: string } | PythonImage)[] = [{ type: "text", text }, ...result.images];
      return {
        content,
        details: { cell: result.id, kernelState: "live", truncated: bounded.truncated, imageCount: result.images.length },
      };
    },
  });

  pi.registerCommand("python", {
    description: "Python kernel: /python status or /python reset (interrupts and discards state)",
    getArgumentCompletions: (prefix) => ["status", "reset"].filter((value) => value.startsWith(prefix)).map((value) => ({ value, label: value })),
    handler: async (args, ctx) => {
      const action = args.trim() || "status";
      if (action !== "status" && action !== "reset") {
        ctx.ui.notify("Usage: /python status | /python reset", "warning");
        return;
      }
      const active = current(ctx);
      if (action === "reset") await active.reset();
      // Do not use a stale context after an asynchronous session transition.
      if (available && kernel === active && owner === sessionId(ctx)) ctx.ui.notify(active.status(), "info");
    },
  });
}
