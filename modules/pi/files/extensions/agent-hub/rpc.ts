import { randomUUID } from "node:crypto";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { record, sameOwner, type Owner } from "./records";

export const REQUEST = "subagents:rpc:v1:request";
export const REPLY = "subagents:rpc:v1:reply:";
type Method = "ping" | "status" | "resume";

// A package failure reply is evidence of rejection/failure, not a transport
// timeout. It does not prove rollback of every package side effect.
export class PackageRejection extends Error {}

// The documented event contract is usable even when independently installed
// pi-subagents is not a resolvable Node dependency of this extension.
export class HubRpc {
  constructor(private events: ExtensionAPI["events"], private signal: AbortSignal) {}

  request(method: Method, params: Record<string, unknown> = {}, requestId = randomUUID(), timeoutMs = method === "ping" ? 2000 : method === "resume" ? 30000 : 10000): Promise<Record<string, unknown>> {
    return new Promise((resolve, reject) => {
      if (this.signal.aborted) { reject(new Error("Agent Hub session ended.")); return; }
      let settled = false;
      let unsubscribe = () => {};
      const finish = (error?: Error, data?: Record<string, unknown>) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        unsubscribe();
        this.signal.removeEventListener("abort", abort);
        if (error) reject(error); else resolve(data!);
      };
      const abort = () => finish(new Error("Agent Hub session ended; a dispatched resume is not cancelled."));
      const timer = setTimeout(() => finish(new Error(method === "ping"
        ? "pi-subagents API unavailable or timed out. Check package loading, then reopen /agent-hub."
        : `${method} reply timed out (request ${requestId}). A dispatched resume may still be running; do not redispatch without checking package status.`)), timeoutMs);
      unsubscribe = this.events.on(`${REPLY}${requestId}`, (raw: unknown) => {
        const reply = record(raw);
        if (reply?.version !== 1 || reply.requestId !== requestId || (reply.method !== undefined && reply.method !== method)) return;
        if (reply.success === false) {
          const error = record(reply.error);
          finish(new PackageRejection(`pi-subagents ${String(error?.code ?? "error")}: ${String(error?.message ?? "Request failed")}`));
        } else if (reply.success === true && record(reply.data)) {
          finish(undefined, record(reply.data));
        } else finish(new Error("Malformed pi-subagents reply; no automatic retry."));
      });
      this.signal.addEventListener("abort", abort, { once: true });
      try {
        this.events.emit(REQUEST, { version: 1, requestId, method, params, source: { extension: "agent-hub" } });
      } catch (error) {
        finish(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  async checkOwner(owner: Owner): Promise<boolean> {
    const ping = await this.request("ping");
    if (ping.version !== 1 || !sameOwner(ping.session, owner)) throw new Error("pi-subagents belongs to a different or unavailable parent session. Reopen Agent Hub after the session is ready.");
    return record(ping.capabilities)?.resume === true;
  }
}
