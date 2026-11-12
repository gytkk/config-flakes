import { StringEnum } from "@earendil-works/pi-ai";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { Text, stripTerminalSequences } from "@earendil-works/pi-tui";
import { Type } from "typebox";
import {
  MAX_NAME, MAX_PHASES, MAX_TASKS, SNAPSHOT_TYPE, cloneState, currentTask,
  decodeState, emptyState, summary, transition, treeText,
} from "./state";
import { createTaskViewer, widgetLine } from "./view";

const Name = Type.String({ minLength: 1, maxLength: MAX_NAME });
const Tasks = Type.Array(Name, { minItems: 1, maxItems: MAX_TASKS });
const Phases = Type.Array(Type.Object({ name: Name, tasks: Tasks }, { additionalProperties: false }), { minItems: 1, maxItems: MAX_PHASES });
export const TaskParams = Type.Object({
  action: StringEnum(["init", "start", "done", "drop", "append", "remove", "view"] as const),
  phases: Type.Optional(Phases),
  phase: Type.Optional(Type.String({ minLength: 1, maxLength: MAX_NAME, description: "Phase id or unambiguous exact name; scopes target or receives appended tasks" })),
  tasks: Type.Optional(Tasks),
  target: Type.Optional(Type.String({ minLength: 1, maxLength: MAX_NAME, description: "Task id (preferred) or unambiguous exact name" })),
  replace: Type.Optional(Type.Boolean({ description: "Explicitly replace an existing plan with init" })),
}, { additionalProperties: false });

export default function phaseTasks(pi: ExtensionAPI): void {
  let state = emptyState();
  let live = false;
  let restoreError: string | undefined;
  let dialog: AbortController | undefined;

  const closeDialog = () => { dialog?.abort(); dialog = undefined; };
  const refresh = (ctx: ExtensionContext) => {
    if (!ctx.hasUI) return;
    if (!currentTask(state) || restoreError) { ctx.ui.setWidget("phase-tasks", undefined); return; }
    const snapshot = cloneState(state);
    if (ctx.mode !== "tui") {
      ctx.ui.setWidget("phase-tasks", [widgetLine(snapshot, 120)!]);
      return;
    }
    ctx.ui.setWidget("phase-tasks", (_tui, theme) => ({
      render(width) {
        const line = widgetLine(snapshot, width);
        return line === undefined ? [] : [theme.fg("accent", line)];
      },
      invalidate() {},
    }));
  };
  const restore = (ctx: ExtensionContext) => {
    closeDialog();
    state = emptyState();
    restoreError = undefined;
    live = true;
    // Custom snapshots are authoritative and appended synchronously at commit.
    // Tool results can be persisted later in assistant order, not execution order;
    // replaying those as well could resurrect stale state after parallel calls.
    const branch = ctx.sessionManager.getBranch();
    for (let i = branch.length - 1; i >= 0; i--) {
      const entry = branch[i];
      if (entry.type !== "custom" || entry.customType !== SNAPSHOT_TYPE) continue;
      try { state = decodeState(entry.data); }
      catch { restoreError = "Cannot restore the latest task snapshot. Use task_list init with replace:true to explicitly create a new plan, or navigate to an earlier branch."; }
      break;
    }
    refresh(ctx);
    if (restoreError && ctx.hasUI) ctx.ui.notify(restoreError, "warning");
  };
  pi.on("session_start", (_event, ctx) => restore(ctx));
  pi.on("session_tree", (_event, ctx) => restore(ctx));
  pi.on("session_shutdown", (_event, ctx) => {
    live = false;
    closeDialog();
    state = emptyState();
    restoreError = undefined;
    if (ctx.hasUI) ctx.ui.setWidget("phase-tasks", undefined);
  });

  pi.registerTool({
    name: "task_list",
    label: "Tasks",
    description: "Manage this session branch's phased plan. init requires phases:[{name,tasks:[names]}], creates pending tasks; replacing a plan requires replace:true. start takes optional target (default first pending), optional phase scope; only one task may be in_progress. done takes optional target (default active), requires in_progress. drop(target) marks pending/active abandoned; remove(target) deletes any task and empty phases. Finishing/dropping/removing the active task promotes the first pending task in phase/task order. append accepts either phases or phase+tasks; new tasks stay pending. view returns the full tree. Targets are stable ids or unambiguous exact names (optional phase scope); ids take precedence over names. Names must be single-line, control-free, at most 160 characters. Limits: 32 phases, 200 tasks, 32 KB snapshot. No files or automatic model continuations.",
    parameters: TaskParams,
    executionMode: "sequential",
    async execute(_id, params, signal, _onUpdate, ctx) {
      if (!live) throw new Error("task_list: session is not active");
      signal?.throwIfAborted();
      if (restoreError && !(params.action === "init" && params.replace === true)) throw new Error(restoreError);
      const candidate = transition(state, params);
      signal?.throwIfAborted();
      if (params.action !== "view") {
        // No direct filesystem mutations: Pi owns session persistence. There is
        // no await in this validate/append/publish transaction, so cancellation
        // and session replacement cannot interleave a half-applied mutation.
        pi.appendEntry(SNAPSHOT_TYPE, cloneState(candidate));
        state = candidate;
        restoreError = undefined;
        closeDialog();
        refresh(ctx);
      }
      return {
        content: [{ type: "text", text: treeText(candidate) }],
        details: { action: params.action, snapshot: cloneState(candidate) },
      };
    },
    renderCall(args, theme) {
      const action = ["init", "start", "done", "drop", "append", "remove", "view"].includes(args.action ?? "") ? args.action : "";
      return new Text(theme.fg("toolTitle", `task_list ${action}`), 0, 0);
    },
    renderResult(result, { expanded }, theme) {
      try {
        const snapshot = decodeState(result.details?.snapshot);
        return new Text(expanded ? treeText(snapshot) : summary(snapshot), 0, 0);
      } catch {
        const content = result.content.find(item => item.type === "text");
        const message = content?.type === "text" ? content.text : "Task operation failed.";
        const safe = stripTerminalSequences(message.slice(0, 2000)).replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g, "");
        return new Text(theme.fg("error", safe), 0, 0);
      }
    },
  });

  pi.registerCommand("tasks", {
    description: "Show the full phased task tree on the current branch (read-only)",
    handler: async (args, ctx) => {
      if (args.trim()) {
        if (ctx.hasUI) ctx.ui.notify("Usage: /tasks (read-only). Use task_list to change the plan.", "warning");
        return;
      }
      if (!live) return;
      const snapshot = cloneState(state);
      if (ctx.mode !== "tui") {
        // In RPC this is a notification; headless clients use task_list view.
        if (ctx.hasUI) ctx.ui.notify(restoreError ?? treeText(snapshot), "info");
        return;
      }
      closeDialog();
      const controller = new AbortController();
      dialog = controller;
      try {
        await ctx.ui.custom<void>((tui, theme, keys, done) => {
          const abort = () => done();
          controller.signal.addEventListener("abort", abort, { once: true });
          if (controller.signal.aborted) done();
          return {
            ...createTaskViewer(snapshot, theme, keys, () => tui.terminal.rows, () => tui.requestRender(), done, restoreError),
            dispose() { controller.signal.removeEventListener("abort", abort); },
          };
        });
      } finally {
        if (dialog === controller) dialog = undefined;
      }
    },
  });
}
