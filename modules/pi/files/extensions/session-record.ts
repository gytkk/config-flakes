import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join, relative } from "node:path";

export default function (pi: ExtensionAPI) {
  pi.on("session_shutdown", (_event, ctx) => {
    const path = ctx.sessionManager.getSessionFile();
    if (!path) return;
    const header = ctx.sessionManager.getHeader();
    if (!header || ("private" in header && header.private !== false)) return;
    const home = process.env.HOME ?? homedir();
    const parts = relative(join(home, ".pi/agent/sessions"), path).split("/");
    if (parts.length !== 2 || !/^--.*--$/.test(parts[0]) || !existsSync(path)) return;
    const result = spawnSync(join(home, ".local/bin/agent-session-record"), ["hook", "pi", "session-shutdown"], {
      input: JSON.stringify({ session_id: ctx.sessionManager.getSessionId(), transcript_path: path }) + "\n",
      stdio: ["pipe", "ignore", "ignore"], timeout: 2000,
    });
    if (result.status !== 0) console.error("Pi session recording hook did not complete");
  });
}
