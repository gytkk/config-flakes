import { randomUUID } from "node:crypto";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { Type, type Static } from "typebox";
import { Check } from "typebox/value";
import {
  withFileMutationQueue,
  type ExtensionAPI,
  type ExtensionContext,
} from "@earendil-works/pi-coding-agent";
import { applyEdit, prepareEdit, type EditPlan } from "./workspace-edit";

export interface RefactorTarget {
  file: string;
  root: string;
  capabilities: {
    renameProvider?: boolean | { prepareProvider?: boolean };
    codeActionProvider?: boolean | { resolveProvider?: boolean };
  };
  versions: Map<string, number>;
  diagnostics: unknown[];
  request(method: string, params: unknown, signal?: AbortSignal): Promise<unknown>;
  sync(files: string[], signal?: AbortSignal): Promise<void>;
}

export interface RefactorBackend {
  target(ctx: ExtensionContext, path: string): Promise<RefactorTarget>;
}

const actionSchema = Type.Object({
  title: Type.String(),
  kind: Type.Optional(Type.String()),
  disabled: Type.Optional(Type.Object({ reason: Type.String() })),
  edit: Type.Optional(Type.Unknown()),
  command: Type.Optional(Type.Unknown()),
  data: Type.Optional(Type.Unknown()),
});
type Action = Static<typeof actionSchema>;

function parseAction(value: unknown): Action {
  if (!Check(actionSchema, value)) throw new Error("The server returned an invalid code action.");
  return value;
}

function canResolve(target: RefactorTarget): boolean {
  const provider = target.capabilities.codeActionProvider;
  return typeof provider === "object" && provider.resolveProvider === true;
}
type OfferedAction = { action: Action; target: RefactorTarget; source: string };
type Preview = { plan: EditPlan; target: RefactorTarget };
const TOOL_NAMES = ["lsp_rename", "lsp_code_actions", "lsp_apply_edit"];
const MAX_PREVIEW_BYTES = 40_000;
const MAX_PREVIEWS = 8;
const MAX_ACTIONS = 100;
const position = {
  line: Type.Integer({ minimum: 0, description: "Zero-based line" }),
  character: Type.Integer({ minimum: 0, description: "Zero-based UTF-16 character offset" }),
};

function result(text: string) {
  return { content: [{ type: "text" as const, text }], details: {} };
}

function checkSignal(signal?: AbortSignal): void {
  signal?.throwIfAborted();
}

function requireCapability(target: RefactorTarget, name: keyof RefactorTarget["capabilities"]): void {
  if (!target.capabilities[name]) throw new Error(`The selected LSP server does not support ${name}.`);
}

async function unchanged(file: string, source: string): Promise<void> {
  if (await readFile(file, "utf8") !== source) {
    throw new Error("The source changed while requesting this refactoring. Request a fresh preview.");
  }
}

function unsupportedAction(action: Action): string | undefined {
  if (action.disabled) return action.disabled.reason;
  if (action.command !== undefined) {
    return "Command-bearing actions cannot be safely previewed; use the editor for this action.";
  }
  return undefined;
}

export function registerRefactorTools(pi: ExtensionAPI, backend: RefactorBackend): void {
  const previews = new Map<string, Preview>();
  const actions = new Map<string, OfferedAction>();
  let generation = 0;
  const clear = () => {
    generation++;
    previews.clear();
    actions.clear();
  };
  pi.on("session_start", clear);
  pi.on("session_shutdown", clear);

  function assertCurrent(start: number, signal?: AbortSignal): void {
    checkSignal(signal);
    if (start !== generation) throw new Error("The session changed. Request a fresh preview.");
  }

  async function preview(target: RefactorTarget, edit: unknown, start: number, signal?: AbortSignal) {
    const plan = await prepareEdit(target.root, edit, target.versions, signal);
    assertCurrent(start, signal);
    if (Buffer.byteLength(plan.diff, "utf8") > MAX_PREVIEW_BYTES) {
      throw new Error("The complete diff exceeds 40 KB. Narrow the refactoring or use your editor; no apply token was created.");
    }
    const token = randomUUID();
    if (previews.size >= MAX_PREVIEWS) previews.delete(previews.keys().next().value!);
    previews.set(token, { plan, target });
    return result([
      "Preview only; no files changed. Review the complete diff before applying.",
      `Apply with lsp_apply_edit({ previewId: ${JSON.stringify(token)} }). Tokens are session-local and single-use.`,
      plan.diff,
    ].join("\n\n"));
  }

  pi.registerCommand("lsp-refactor", {
    description: "Enable semantic rename and code-action preview/apply tools for this session",
    handler: async (args, ctx) => {
      if (args.trim()) {
        ctx.ui.notify("Usage: /lsp-refactor", "warning");
        return;
      }
      pi.setActiveTools([...new Set([...pi.getActiveTools(), ...TOOL_NAMES])]);
      ctx.ui.notify("LSP refactoring enabled. Ask for a rename or code action; review its preview before requesting apply. Positions are zero-based UTF-16.", "info");
    },
  });

  pi.registerTool({
    name: "lsp_rename",
    label: "LSP Rename Preview",
    defaultActive: false,
    description: "Preview a semantic symbol rename across files using the existing LSP connection. Use only for an explicitly requested refactoring. Does not write files. Review the returned diff, then call lsp_apply_edit only when applying is authorized.",
    parameters: Type.Object({
      path: Type.String(),
      ...position,
      newName: Type.String({ minLength: 1 }),
    }),
    async execute(_id, params, signal, _update, ctx) {
      const start = generation;
      checkSignal(signal);
      const target = await backend.target(ctx, params.path);
      requireCapability(target, "renameProvider");
      const source = await readFile(target.file, "utf8");
      const at = {
        textDocument: { uri: pathToFileURL(target.file).href },
        position: { line: params.line, character: params.character },
      };
      if (typeof target.capabilities.renameProvider === "object" && target.capabilities.renameProvider.prepareProvider) {
        const prepared = await target.request("textDocument/prepareRename", at, signal);
        if (prepared === null) throw new Error("The server cannot rename the symbol at this position.");
      }
      const edit = await target.request("textDocument/rename", { ...at, newName: params.newName }, signal);
      await unchanged(target.file, source);
      assertCurrent(start, signal);
      return preview(target, edit, start, signal);
    },
  });

  pi.registerTool({
    name: "lsp_code_actions",
    label: "LSP Code Actions",
    defaultActive: false,
    description: "For an explicitly requested fix/refactoring, list LSP code actions with path and a zero-based UTF-16 range (end defaults to start). Pass an actionId alone to resolve and preview one action. Never writes files or executes server commands. Command-bearing actions must be applied through an editor instead.",
    parameters: Type.Object({
      path: Type.Optional(Type.String()),
      line: Type.Optional(position.line),
      character: Type.Optional(position.character),
      endLine: Type.Optional(Type.Integer({ minimum: 0 })),
      endCharacter: Type.Optional(Type.Integer({ minimum: 0 })),
      only: Type.Optional(Type.Array(Type.String(), { description: "Optional LSP kinds, e.g. quickfix or source.organizeImports" })),
      actionId: Type.Optional(Type.String()),
    }),
    async execute(_id, params, signal, _update, ctx) {
      const start = generation;
      checkSignal(signal);
      if (params.actionId) {
        if (params.path !== undefined || params.line !== undefined || params.character !== undefined || params.endLine !== undefined || params.endCharacter !== undefined || params.only !== undefined) {
          throw new Error("Pass actionId alone to preview a listed action.");
        }
        const offered = actions.get(params.actionId);
        if (!offered) throw new Error("Unknown or expired actionId. List code actions again.");
        await unchanged(offered.target.file, offered.source);
        let action = offered.action;
        const reason = unsupportedAction(action);
        if (reason) throw new Error(reason);
        if (!action.edit && canResolve(offered.target)) {
          action = parseAction(await offered.target.request("codeAction/resolve", action, signal));
        }
        const resolvedReason = unsupportedAction(action);
        if (resolvedReason) throw new Error(resolvedReason);
        if (!action.edit) throw new Error("This action has no previewable WorkspaceEdit.");
        await unchanged(offered.target.file, offered.source);
        assertCurrent(start, signal);
        return preview(offered.target, action.edit, start, signal);
      }
      if (params.path === undefined || params.line === undefined || params.character === undefined) {
        throw new Error("Listing code actions requires path, line, and character.");
      }
      if ((params.endLine === undefined) !== (params.endCharacter === undefined)) {
        throw new Error("Provide both endLine and endCharacter, or neither.");
      }
      const end = { line: params.endLine ?? params.line, character: params.endCharacter ?? params.character };
      if (end.line < params.line || (end.line === params.line && end.character < params.character)) {
        throw new Error("The code-action range ends before it starts.");
      }
      const target = await backend.target(ctx, params.path);
      requireCapability(target, "codeActionProvider");
      const source = await readFile(target.file, "utf8");
      const found = await target.request("textDocument/codeAction", {
        textDocument: { uri: pathToFileURL(target.file).href },
        range: { start: { line: params.line, character: params.character }, end },
        context: { diagnostics: target.diagnostics, only: params.only, triggerKind: 1 },
      }, signal);
      await unchanged(target.file, source);
      assertCurrent(start, signal);
      if (found !== null && !Array.isArray(found)) throw new Error("The server returned an invalid code-action list.");
      if ((found?.length ?? 0) > MAX_ACTIONS) throw new Error("More than 100 actions returned. Narrow the range or supply only kinds.");
      actions.clear();
      const rows = (found ?? []).map((value: unknown) => {
        const action = parseAction(value);
        const reason = unsupportedAction(action) ?? (!action.edit && !canResolve(target)
          ? "No previewable edit or resolve support" : undefined);
        if (reason) return { title: action.title, kind: action.kind, unavailable: reason };
        const actionId = randomUUID();
        actions.set(actionId, { action, target, source });
        return { actionId, title: action.title, kind: action.kind };
      });
      return result(rows.length ? JSON.stringify(rows, null, 2) : "No code actions at this range.");
    },
  });

  pi.registerTool({
    name: "lsp_apply_edit",
    label: "Apply LSP Preview",
    defaultActive: false,
    description: "Apply exactly one previously reviewed lsp_rename/lsp_code_actions preview, only when the user authorized applying it. Refuses files changed since preview. A token cannot be reused. Does not execute server commands or create, rename, or delete files.",
    parameters: Type.Object({ previewId: Type.String() }),
    async execute(_id, params, signal) {
      checkSignal(signal);
      const stored = previews.get(params.previewId);
      if (!stored) throw new Error("Unknown, expired, or already consumed previewId. Request a fresh preview.");
      previews.delete(params.previewId);
      const versions = stored.target.versions;
      for (const file of stored.plan.files) {
        if (file.version !== undefined && versions.get(pathToFileURL(file.path).href) !== file.version) {
          throw new Error(`Document version changed since preview: ${file.path}`);
        }
      }
      const files = await applyEdit(stored.plan, withFileMutationQueue, signal);
      // A synchronization error must not disguise a completed filesystem edit as an unapplied preview.
      try {
        await stored.target.sync(files, signal);
      } catch (error) {
        return result(`Applied to ${files.join(", ")}. LSP synchronization failed: ${error instanceof Error ? error.message : String(error)}. Do not retry apply; refresh diagnostics before further refactoring.`);
      }
      return result(`Applied reviewed edit to:\n${files.join("\n")}`);
    },
  });
}
