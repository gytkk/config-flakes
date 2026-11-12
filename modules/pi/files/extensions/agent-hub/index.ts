import { randomUUID } from "node:crypto";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { HubRpc, PackageRejection } from "./rpc";
import { branchOrigins, childKey, completionObservation, foregroundRows, identity, mergeChild, OBSERVATION, record, RESUME, restoreRecords, resumedRun, sameOwner, snapshotObservation, terminalStatus, type Child, type Owner, type ResumeReceipt } from "./records";
import { displayText, showText } from "./view";
import { completionText } from "./completion";

const FOLLOW_UP = "Request follow-up (detached background resume)…";
const COVERAGE = "Current-branch foreground children + Hub resume receipts; not the entire Fleet.";

export default function agentHub(pi: ExtensionAPI): void {
  if (process.env.PI_SUBAGENT_CHILD === "1") return;
  let owner: Owner | undefined;
  let lifetime = new AbortController();
  const live = new Map<string, Child>();
  let refresh = { running: false, dirty: false, warned: false };
  const toolRequests = new Map<string, unknown>();
  const slashRequests = new Map<string, unknown>();
  let subscriptions: Array<() => void> = [];
  let open: object | undefined;

  const current = (ctx: ExtensionContext) => !!owner && !lifetime.signal.aborted && sameOwner({ sessionId: ctx.sessionManager.getSessionId(), sessionFile: ctx.sessionManager.getSessionFile() ?? null }, owner);
  const stateFor = (ctx: ExtensionContext) => {
    const state = restoreRecords(ctx.sessionManager.getBranch(), owner!, !!ctx.sessionManager.getHeader()?.parentSession);
    for (const [key, child] of live) state.children.set(key, mergeChild(state.children.get(key), child));
    return state;
  };
  const saveObservation = (ctx: ExtensionContext, child: Child) => {
    if (!current(ctx)) return;
    const previous = stateFor(ctx).children.get(childKey(child));
    const next = mergeChild(previous, child);
    if (JSON.stringify(previous) === JSON.stringify(next)) return;
    pi.appendEntry(OBSERVATION, { owner, runId: next.runId, child: next });
    live.set(childKey(next), next);
  };
  const observe = (ctx: ExtensionContext, details: unknown, terminal: boolean, origin?: unknown) => {
    if (!current(ctx)) return;
    for (const child of foregroundRows(details, terminal, origin)) saveObservation(ctx, child);
  };
  const applySnapshot = (ctx: ExtensionContext, data: unknown) => {
    for (const child of stateFor(ctx).children.values()) {
      const update = snapshotObservation(child, data);
      if (update) saveObservation(ctx, update);
    }
  };
  // Events are invalidation hints, not lifecycle proof. Coalesce refreshes and
  // query ONLY current-branch exact Hub receipts. No timers or fleet discovery.
  const refreshOwned = (ctx: ExtensionContext) => {
    if (!current(ctx)) return;
    const work = refresh;
    work.dirty = true;
    if (work.running) return;
    work.running = true;
    const signal = lifetime.signal;
    const sessionOwner = owner!;
    const valid = () => !signal.aborted && owner === sessionOwner && current(ctx);
    void (async () => {
      try {
        while (valid() && work.dirty) {
          work.dirty = false;
          const children = [...stateFor(ctx).children.values()].filter(child => child.source === "hub-resume");
          if (!children.length) continue;
          const rpc = new HubRpc(pi.events, signal);
          await rpc.checkOwner(sessionOwner);
          for (const child of children) {
            if (!valid()) return;
            const data = await rpc.request("status", { id: child.runId, index: child.index });
            if (!valid()) return;
            if (data.isError === true) throw new Error(String(data.text || "Status unavailable"));
            applySnapshot(ctx, data.asyncSnapshot);
          }
          work.warned = false;
        }
      } catch (error) {
        if (valid() && !work.warned) {
          work.warned = true;
          ctx.ui.notify(`Agent Hub refresh failed; recorded rows may be stale. Use /agent-hub → Read package status to retry the read.\n${displayText(String(error), 500)}`, "warning");
        }
      } finally { work.running = false; }
    })();
  };
  const shutdown = () => {
    lifetime.abort();
    subscriptions.splice(0).forEach(dispose => dispose());
    live.clear(); toolRequests.clear(); slashRequests.clear(); owner = undefined; open = undefined;
    refresh = { running: false, dirty: false, warned: false };
  };
  pi.on("session_shutdown", shutdown);
  pi.on("session_start", (_event, ctx) => {
    shutdown();
    if (ctx.mode !== "tui") return;
    lifetime = new AbortController();
    owner = { sessionId: ctx.sessionManager.getSessionId(), sessionFile: ctx.sessionManager.getSessionFile() ?? null };
    // Slash progress lacks a runId in 0.84.0: only its terminal response is targetable.
    const sessionOwner = owner;
    subscriptions = [
      pi.events.on("subagent:slash:started", raw => {
        const id = identity(record(raw)?.requestId);
        if (id && owner === sessionOwner && current(ctx)) slashRequests.set(id, branchOrigins(ctx.sessionManager.getBranch()).slash.get(id));
      }),
      pi.events.on("subagent:slash:response", raw => {
        const response = record(raw);
        const id = identity(response?.requestId);
        if (!id || !slashRequests.has(id) || owner !== sessionOwner || !current(ctx)) return;
        const origin = slashRequests.get(id);
        slashRequests.delete(id);
        observe(ctx, record(response?.result)?.details, true, origin);
      }),
      pi.events.on("subagent:jobs-changed", () => { if (owner === sessionOwner) refreshOwned(ctx); }),
      pi.events.on("subagent:async-complete", () => { if (owner === sessionOwner) refreshOwned(ctx); }),
      pi.events.on("subagents:rpc:v1:ready", () => { if (owner === sessionOwner) refreshOwned(ctx); }),
    ];
    refreshOwned(ctx);
  });
  pi.on("tool_execution_start", (event, ctx) => {
    if (event.toolName === "subagent" && !event.parentToolCallId && current(ctx)) toolRequests.set(event.toolCallId, event.args);
  });
  pi.on("tool_execution_update", (event, ctx) => {
    if (event.toolName === "subagent" && current(ctx) && toolRequests.has(event.toolCallId)) observe(ctx, record(event.partialResult)?.details, false, toolRequests.get(event.toolCallId));
  });
  pi.on("tool_execution_end", (event, ctx) => {
    if (event.toolName !== "subagent" || !current(ctx) || !toolRequests.has(event.toolCallId)) return;
    const origin = toolRequests.get(event.toolCallId);
    toolRequests.delete(event.toolCallId);
    observe(ctx, record(event.result)?.details, true, origin);
  });
  pi.on("message_end", (event, ctx) => {
    if (!current(ctx)) return;
    const message = record(event.message);
    if (message?.role !== "custom" || message.customType !== "subagent-notify") return;
    for (const child of stateFor(ctx).children.values()) {
      const update = completionObservation(child, message.details);
      if (update) saveObservation(ctx, update);
    }
  });
  pi.on("session_tree", (_event, ctx) => {
    lifetime.abort();
    lifetime = new AbortController();
    live.clear(); toolRequests.clear(); slashRequests.clear(); open = undefined;
    refresh = { running: false, dirty: false, warned: false };
    refreshOwned(ctx);
  });

  pi.registerCommand("agent-hub", {
    description: "Browse this parent's foreground children; inspect or explicitly request a detached follow-up",
    handler: async (_args, ctx) => {
      if (ctx.mode !== "tui" || !current(ctx)) {
        ctx.ui.notify("Agent Hub requires an interactive parent Pi session.", "warning"); return;
      }
      if (open) { ctx.ui.notify("Agent Hub is already open.", "info"); return; }
      const dialog = {};
      open = dialog;
      const signal = lifetime.signal;
      const sessionOwner = { ...owner! };
      const rpc = new HubRpc(pi.events, signal);
      const valid = () => !signal.aborted && current(ctx) && sameOwner(owner, sessionOwner);
      const check = () => { if (!valid()) throw new Error("Parent session changed; reopen Agent Hub."); };
      const notify = (message: string, kind: "info" | "warning" | "error" = "info") => {
        if (valid()) ctx.ui.notify(displayText(message, 2000), kind);
      };
      const snapshot = () => {
        check();
        return stateFor(ctx);
      };
      const dispatchBlock = (child: Child) => {
        const currentChild = snapshot().children.get(childKey(child));
        if (!currentChild) return "Child is no longer on this branch.";
        // Execution cannot be undone by tree navigation. Only the guard reads
        // all owner entries; abandoned receipts never become roster rows.
        const receipt = restoreRecords(ctx.sessionManager.getEntries(), sessionOwner, true).receipts.get(childKey(child));
        if (receipt) return `A Hub resume was already dispatched (${receipt.requestId}, ${receipt.state}). ${receipt.error ?? ""} ${receipt.resumedRunId ? `Continue from new run ${receipt.resumedRunId} / index 0.` : "Inspect package status/Fleet; Hub will not redispatch this target."}`;
        if (["running", "queued", "pending", "detached", "stopped"].includes(currentChild.status) || (currentChild.source === "hub-resume" && !terminalStatus(currentChild.status))) return `Follow-up unavailable for recorded state: ${currentChild.status}. Inspect package status.`;
      };
      const inspect = async (child: Child, transcript = false) => {
        check();
        notify(transcript ? "Reading package transcript tail…" : "Reading package status…");
        await rpc.checkOwner(sessionOwner);
        check();
        const data = await rpc.request("status", { id: child.runId, index: child.index, ...(transcript ? { view: "transcript", lines: 500 } : {}) });
        check();
        if (data.isError === true) throw new Error(String(data.text || "Status unavailable"));
        applySnapshot(ctx, data.asyncSnapshot);
        return typeof data.text === "string" ? data.text : "Package returned no status text.";
      };
      try {
        while (valid()) {
          const state = snapshot();
          const children = [...state.children.values()].reverse();
          if (!children.length) {
            notify(`${COVERAGE}\nNo addressable native children recorded here. Use /subagents-fleet for other activity. Copied fork history without matching owner observations is excluded.`);
            return;
          }
          const labels = children.map((child, i) => `${i + 1}. ${displayText(child.agent, 100)} · execution: ${displayText(child.status, 80)} · ${displayText(child.model, 100)}${child.agent === "worker" || child.completion ? ` · worker verdict: ${child.completion?.status ?? "unknown"}` : ""}`.replace(/\n/g, " "));
          const selected = await ctx.ui.select(`Agent Hub — ${COVERAGE}`, labels, { signal });
          if (!valid() || selected === undefined) return;
          let child = children[labels.indexOf(selected)];
          if (!child) return;
          while (valid()) {
            child = snapshot().children.get(childKey(child)) ?? child;
            const blocked = dispatchBlock(child);
            const actions = ["Inspect task, model, status and recorded result", "Read package status", "Read transcript tail (up to 500 lines)", ...(blocked ? ["Why follow-up is unavailable"] : [FOLLOW_UP]), "Back to children"];
            const action = await ctx.ui.select(`${displayText(child.agent, 100)} · ${displayText(child.status, 80)} · index ${child.index}`, actions, { signal });
            if (!valid() || action === undefined || action === "Back to children") break;
            if (action === actions[0]) {
              await showText(ctx, "Child record (read-only)", `Agent: ${child.agent}\nModel: ${child.model}\nRecorded status: ${child.status} (execution)\nSource: ${child.source}\nParent: ${sessionOwner.sessionId}\nRun: ${child.runId}\nChild index: ${child.index}\nChild session: ${child.sessionFile ?? "not recorded"}\nEvidence: ${child.evidence ?? "recorded"}\nPrevious: ${child.previous ? `${child.previous.runId} / index ${child.previous.index}` : "none"}\nLineage root: ${child.root ? `${child.root.runId} / index ${child.root.index}` : `${child.runId} / index ${child.index}`}\n\nTask\n${child.task || "not recorded"}\n\n${completionText(child)}\n\nRecorded result\n${child.result || "No result text recorded. Read package status or transcript."}\n\nCompletion does not prove resume eligibility. This is a snapshot, not a live child session.`, signal);
            } else if (action === actions[1] || action === actions[2]) {
              try { await showText(ctx, action, await inspect(child, action === actions[2]), signal); }
              catch (error) { notify(String(error), "error"); }
            } else if (action === "Why follow-up is unavailable") {
              notify(blocked!, "warning");
            } else if (action === FOLLOW_UP) {
              try {
                // Read-only inspection and capability/owner checks precede user input.
                const status = await inspect(child);
                if (!(await rpc.checkOwner(sessionOwner))) throw new Error("Installed pi-subagents does not advertise resume. Update/reload the package before requesting a follow-up.");
                check();
                await showText(ctx, "Package status before follow-up", status, signal);
                check();
                const message = await ctx.ui.input("Follow-up for this child (plain text; no execution until confirmed)", "What should this child do next?", { signal });
                if (!valid() || !message?.trim()) continue;
                if (message.length > 4000) { notify("Follow-up exceeds 4,000 characters. Shorten the request.", "warning"); continue; }
                const confirmed = await ctx.ui.confirm("Start detached background follow-up?", displayText(`Run: ${child.runId}\nChild index: ${child.index}\nAgent: ${child.agent}\nModel: ${child.model}\n\n${message}\n\nPackage resume starts a NEW detached background child from stored context, not a parked live session. Its stored model/tools/budgets and package eligibility checks apply. Closing Hub or timing out does NOT cancel it. Package completion may wake the parent. No automatic retry.`, 6000), { signal });
                if (!valid() || !confirmed) continue;
                // Recheck after all user waits. Never let a stale dialog send work.
                if (!(await rpc.checkOwner(sessionOwner))) throw new Error("Resume capability is no longer available.");
                check();
                const block = dispatchBlock(child);
                if (block) throw new Error(block);
                const receipt: ResumeReceipt = { owner: sessionOwner, requestId: randomUUID(), target: { ...child, result: "" }, followUp: message.trim(), state: "requested" };
                pi.appendEntry(RESUME, receipt);
                notify(`Requesting detached resume (${receipt.requestId})…`);
                let accepted = false;
                try {
                  const response = await rpc.request("resume", { id: child.runId, index: child.index, message: message.trim() }, receipt.requestId);
                  check();
                  if (response.isError === true) throw new PackageRejection(String(response.text || "Package reported resume failure."));
                  const resumedRunId = resumedRun(response);
                  if (!resumedRunId || resumedRunId === child.runId) throw new Error("Resume reply has no unambiguous new single-run identity; outcome unknown.");
                  pi.appendEntry(RESUME, { ...receipt, state: "accepted", resumedRunId });
                  accepted = true;
                  refreshOwned(ctx);
                  await showText(ctx, "Detached background resume accepted (not completed)", `Request: ${receipt.requestId}\nSource: ${child.runId} / index ${child.index}\nNew run: ${resumedRunId}\n\n${String(response.text ?? "")}\n\nThis receipt acknowledges detached execution, not completion. Return to the roster to inspect the new run.`, signal);
                  break;
                } catch (error) {
                  const rejected = error instanceof PackageRejection;
                  if (valid() && !accepted) pi.appendEntry(RESUME, { ...receipt, state: rejected ? "rejected" : "unknown", error: displayText(String(error), 2000) });
                  notify(`${String(error)}\nRequest ${receipt.requestId}: ${accepted ? "detached execution was accepted; return to the new receipt row" : rejected ? "package reported failure; no successful launch acknowledged (not proof of rollback)" : "execution may have started"}. Hub will not retry this target, including after reload. Check /subagents-fleet and package status.`, "error");
                }
              } catch (error) { notify(String(error), "error"); }
            }
          }
        }
      } catch (error) { notify(String(error), "error"); }
      finally { if (open === dialog) open = undefined; }
    },
  });
}
