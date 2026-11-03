import { writeSync } from "node:fs";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

// OSC 0 with an empty title.
export const CLEAR_TITLE = "\x1b]0;\x07";

export default function (pi: ExtensionAPI) {
  // Pi never clears its "π - <dir>" title and rewrites it after session_shutdown, so clear it at process exit.
  pi.on("session_shutdown", (event, ctx) => {
    if (event.reason !== "quit" || ctx.mode !== "tui") return;
    process.once("exit", () => {
      try {
        writeSync(1, CLEAR_TITLE);
      } catch {
        // The terminal is already gone, so there is no title left to clear.
      }
    });
  });
}
