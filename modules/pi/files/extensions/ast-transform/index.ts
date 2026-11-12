import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import { AstTransform, LIMITS } from "./engine";

const query = {
  files: Type.Array(Type.String({ minLength: 1, maxLength: 4096 }), { minItems: 1, maxItems: LIMITS.files, description: "Explicit existing files inside cwd; no directories, globs, symlinks, hardlinks, or '..' components. Leading @ is accepted." }),
  language: Type.String({ minLength: 1, maxLength: 64, description: "Built-in ast-grep language, e.g. typescript, javascript, python, rust." }),
  pattern: Type.String({ minLength: 1, maxLength: 16384, description: "ast-grep structural pattern; $NAME captures one node and $$$ARGS captures multiple nodes." }),
  maxMatches: Type.Optional(Type.Integer({ minimum: 1, maximum: LIMITS.matches, description: "Maximum total matches; exceeding it rejects the whole request (default 200)." })),
};
const token = Type.Object({ previewId: Type.String({ minLength: 1, maxLength: 100 }) }, { additionalProperties: false });

export default function astTransform(pi: ExtensionAPI) {
  const engine = new AstTransform();
  // Tokens are deliberately never restored from transcript details. A resume,
  // fork, reload, or tree navigation requires a fresh preview of current disk.
  pi.on("session_start", () => engine.invalidate());
  pi.on("session_before_switch", () => engine.invalidate());
  pi.on("session_before_fork", () => engine.invalidate());
  pi.on("session_before_tree", () => engine.invalidate());
  pi.on("session_tree", () => engine.invalidate());
  pi.on("session_shutdown", () => engine.shutdown());
  pi.on("agent_settled", (event) => { if (event.aborted) engine.invalidate(); });

  pi.registerTool({
    name: "ast_search",
    label: "AST Search",
    description: "Read-only ast-grep structural search over 1–32 explicit UTF-8 files inside cwd. Complements LSP semantic rename; this matches syntax, not symbol identity. Returns exact zero-based UTF-8 byte ranges (end exclusive), plus one-based lines/byte columns. Limits: 1 MiB/file, 8 MiB/scope, 200 matches, 40 KB/1500 output lines; narrow scope on overflow. Requires ast-grep on PATH.",
    parameters: Type.Object(query, { additionalProperties: false }),
    annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false },
    async execute(_id, params, signal, _update, ctx) {
      const result = await engine.search(ctx.cwd, params, signal);
      return { content: [{ type: "text", text: result.text }], details: { matches: result.matches } };
    },
  });
  pi.registerTool({
    name: "ast_preview",
    label: "AST Preview",
    description: "Preview an ast-grep structural rewrite without writing any files. Supply explicit files, language, pattern, and replacement (ast-grep metavariables supported). Returns the complete multi-file diff and a single-use previewId for ast_apply; never applies an undisplayed/truncated diff. Same bounds as ast_search. Only the latest four previews survive, for at most 10 minutes; cancellation, session navigation, resume/fork/reload discard them. This is syntax rewriting, not LSP symbol rename.",
    parameters: Type.Object({ ...query, replacement: Type.String({ maxLength: 16384, description: "ast-grep replacement template; empty string deletes matched nodes." }) }, { additionalProperties: false }),
    annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: false, openWorldHint: false },
    async execute(_id, params, signal, _update, ctx) {
      const result = await engine.preview(ctx.cwd, params, signal);
      return { content: [{ type: "text", text: result.text }], details: { previewId: result.previewId, files: result.files } };
    },
  });
  pi.registerTool({
    name: "ast_apply",
    label: "AST Apply",
    description: "Apply an already reviewed ast_preview token EXACTLY once, without rerunning ast-grep. Checks ALL scoped file identities/content under sorted Pi mutation locks before writing; rejects stale previews, symlinks, escapes, and unsupported encodings. Preserves permission bits and attempts guarded rollback on failure/cancellation. External editors do not share Pi locks: final check-to-rename races and process crashes prevent cross-file atomicity. Tokens are consumed even on failure. Runs with the user's privileges, not in a sandbox.",
    parameters: token,
    annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: false },
    async execute(_id, params, signal, _update, ctx) {
      const result = await engine.apply(ctx.cwd, params.previewId, signal);
      return { content: [{ type: "text", text: result.text }], details: { files: result.files } };
    },
  });
  pi.registerTool({
    name: "ast_discard",
    label: "AST Discard",
    description: "Discard an unused AST preview token without changing files.",
    parameters: token,
    annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: false, openWorldHint: false },
    async execute(_id, params, signal) {
      if (signal?.aborted) { engine.invalidate(); throw new Error("AST operation cancelled."); }
      const result = engine.discard(params.previewId);
      return { content: [{ type: "text", text: result.text }], details: {} };
    },
  });
}
