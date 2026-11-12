import { getAgentDir, type ExtensionAPI, type ExtensionContext } from "@earendil-works/pi-coding-agent";
import { StringEnum } from "@earendil-works/pi-ai";
import { stripTerminalSequences } from "@earendil-works/pi-tui";
import { Type } from "typebox";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { ArtifactStore, ENTRY_TYPE, LIMITS, check, page, restoreArtifacts, search, type Artifact } from "./store.ts";
import { showPage } from "./view.ts";

export const artifactSchema = Type.Object({
  action: StringEnum(["register", "list", "read", "search"] as const),
  path: Type.Optional(Type.String({ maxLength: 4096, description: "Explicit source path for register only; no symlinks" })),
  title: Type.Optional(Type.String({ maxLength: 160 })),
  id: Type.Optional(Type.String({ maxLength: 64, description: "Registered artifact ID, required for read/search" })),
  offset: Type.Optional(Type.Integer({ minimum: 1, description: "1-based source/page/search starting line" })),
  limit: Type.Optional(Type.Integer({ minimum: 1, maximum: LIMITS.lines, description: "register: max 20000 lines; read: max 200; search: max 50 matching lines" })),
  column: Type.Optional(Type.Integer({ minimum: 0, description: "read only: 0-based UTF-16 continuation column from previous page" })),
  query: Type.Optional(Type.String({ minLength: 1, maxLength: 256, description: "search only: literal single-line text, never regex" })),
  ignoreCase: Type.Optional(Type.Boolean()),
}, { additionalProperties: false });

export function safeDisplay(text: string): string {
  return stripTerminalSequences(text).replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f\u202a-\u202e\u2066-\u2069]/g, "");
}
// Pi 1.5.2: bash details.truncation + fullOutputPath; codemode details.calls +
// fullOutputPath. Only native providers and their exact private temp naming
// contracts qualify. External shell logs require explicit registration.
export function automaticSource(toolName: string, details: unknown, source: string | undefined): string | undefined {
  if (source !== "builtin" || !details || typeof details !== "object") return;
  const d = details as { fullOutputPath?: unknown; truncation?: { truncated?: unknown }; calls?: unknown };
  if (typeof d.fullOutputPath !== "string" || d.fullOutputPath.length > 4096) return;
  const path = d.fullOutputPath;
  if (dirname(path) !== tmpdir() || resolve(path) !== path) return;
  if (toolName === "bash" && d.truncation?.truncated === true && /^pi-bash-[a-f0-9]{16}\.log$/.test(basename(path))) return path;
  if (toolName === "codemode" && Array.isArray(d.calls) && /^pi-codemode-[a-f0-9]{16}\.txt$/.test(basename(path))) return path;
}

export default function sessionArtifacts(pi: ExtensionAPI, options?: { storeRoot: string }) {
  const store = new ArtifactStore(options?.storeRoot ?? join(getAgentDir(), "session-artifacts"));
  let artifacts = new Map<string, Artifact>();
  let lifetime = new AbortController();
  const pending = new Set<Promise<unknown>>();
  let browsing = false;
  const retention = `Snapshots retained under ${store.root}. No shutdown or automatic deletion. Close Pi and manually remove unneeded owner directories (including abandoned branches); forks may still reference them. Session deletion does not delete snapshots. Runs with your user privileges; no credential scanning or network upload.`;
  function reset(ctx?: ExtensionContext) {
    lifetime.abort(); lifetime = new AbortController();
    artifacts = ctx ? restoreArtifacts(ctx.sessionManager.getBranch()) : new Map();
  }
  pi.on("session_start", (_event, ctx) => reset(ctx));
  pi.on("session_tree", (_event, ctx) => reset(ctx));
  pi.on("session_shutdown", async () => {
    lifetime.abort(); artifacts.clear();
    await Promise.allSettled([...pending]);
  });
  function signalFor(signal?: AbortSignal) {
    return signal ? AbortSignal.any([signal, lifetime.signal]) : lifetime.signal;
  }
  function track<T>(task: Promise<T>): Promise<T> {
    pending.add(task); task.then(() => pending.delete(task), () => pending.delete(task)); return task;
  }
  async function register(ctx: ExtensionContext, path: string, title?: string, offset?: number, limit?: number, automatic = false, signal?: AbortSignal) {
    const bound = signalFor(signal);
    const index = artifacts;
    if (index.size >= LIMITS.files) throw new Error("Active branch artifact limit reached (64)");
    const source = resolve(ctx.cwd, path.replace(/^@/, ""));
    const session = ctx.sessionManager.getSessionId();
    return track((async () => {
      const a = await store.register(session, source, safeDisplay(title ?? basename(source)), { offset, limit, privateFile: automatic, signal: bound });
      check(bound);
      // No await between the ownership check, persistent entry and live index.
      if (index.size >= LIMITS.files) throw new Error("Active branch artifact limit reached (64)");
      pi.appendEntry(ENTRY_TYPE, a);
      index.set(a.id, a);
      return a;
    })());
  }
  function get(id?: string): Artifact {
    const a = id ? artifacts.get(id) : undefined;
    if (!a) throw new Error("Unknown artifact ID on the active branch; use artifact list");
    return a;
  }
  function list() {
    return [...artifacts.values()].map(a => `${a.id}  ${safeDisplay(a.title)}  (${a.bytes} bytes, ${a.lines} lines)`).join("\n") || "No artifacts on this branch. Use artifact register or /artifacts register PATH.";
  }
  pi.on("tool_result", async (event, ctx) => {
    const source = pi.getAllTools().find(t => t.name === event.toolName)?.sourceInfo?.source;
    const path = automaticSource(event.toolName, event.details, source);
    if (!path) return;
    const bound = signalFor(ctx.signal);
    try { await register(ctx, path, `${event.toolName} full output`, undefined, undefined, true, bound); }
    catch (error) {
      if (!bound.aborted && ctx.hasUI) ctx.ui.notify(`Artifact not retained: ${safeDisplay(String(error)).slice(0, 300)}`, "warning");
    }
    // Observation only: retain all original content/details/structuredContent.
  });
  pi.registerTool({
    name: "artifact", label: "Session artifact",
    description: "Register an explicit UTF-8 text file snapshot, list current-branch IDs, read pages, or search one registered ID using literal text (no regex). Sources: regular non-symlink/non-hardlinked files up to 2 MiB; snapshots up to 20000 lines. 64 files / 32 MiB per originating session. Reads: max 200 lines / 16 KiB; use returned nextOffset/nextColumn for long lines. Search: max 50 matching-line snippets. Private snapshots survive source deletion and session shutdown; manual cleanup only. Does not scan for files or credentials or upload data.",
    parameters: artifactSchema,
    async execute(_callId, args, signal, _update, ctx) {
      const bound = signalFor(signal); check(bound);
      if (args.action === "list") return { content: [{ type: "text", text: list() }], details: { count: artifacts.size } };
      if (args.action === "register") {
        if (!args.path) throw new Error("register requires path");
        const a = await register(ctx, args.path, args.title, args.offset, args.limit, false, signal);
        return { content: [{ type: "text", text: `Registered ${a.id}: ${a.title} (${a.bytes} bytes, ${a.lines} lines). Private snapshot retained; manual cleanup only.` }], details: { artifact: a } };
      }
      const a = get(args.id);
      const text = await track(store.text(a, bound)); check(bound);
      const result = args.action === "read" ? page(text, args.offset, args.limit, args.column)
        : search(text, args.query ?? "", args.offset, args.limit, args.ignoreCase);
      return { content: [{ type: "text", text: safeDisplay(`${result.text}\n${result.nextOffset ? `Continue: offset=${result.nextOffset}${"nextColumn" in result ? ` column=${result.nextColumn}` : ""}` : "[End]"}`) }], details: { ...result, text: undefined, id: a.id } };
    },
  });
  pi.registerCommand("artifacts", {
    description: "Browse retained text snapshots; /artifacts register PATH | read ID [OFFSET] | search ID TEXT | help",
    handler: async (args, ctx) => {
      const bound = lifetime.signal;
      const reply = (text: string) => {
        if (bound.aborted) return;
        if (ctx.mode === "tui") ctx.ui.notify(safeDisplay(text), "info");
        else pi.sendMessage({ customType: "session-artifacts-response", content: safeDisplay(text), display: true }, { triggerTurn: false });
      };
      try {
        const input = args.trim();
        if (input === "help") { reply(`/artifacts: list then read-only content viewer. /artifacts register PATH (entire remaining argument, no shell quoting). /artifacts read ID [OFFSET]. /artifacts search ID LITERAL. Model tool: artifact.\n${retention}`); return; }
        if (input.startsWith("register ")) {
          const a = await register(ctx, input.slice(9).trim(), undefined, undefined, undefined, false, bound);
          reply(`Registered ${a.id}: ${a.title}\n${retention}`); return;
        }
        const read = /^read (\S+)(?: (\d+))?$/.exec(input);
        const find = /^search (\S+) (.+)$/.exec(input);
        if (read || find) {
          const a = get((read ?? find)![1]); const text = await track(store.text(a, bound)); check(bound);
          const result = read ? page(text, Number(read[2] ?? 1)) : search(text, find![2]);
          reply(`${result.text}\n${result.nextOffset ? `Continue with artifact tool: offset=${result.nextOffset}${"nextColumn" in result ? ` column=${result.nextColumn}` : ""}` : "[End]"}`); return;
        }
        if (input && input !== "list") { reply("Usage: /artifacts [list | register PATH | read ID [OFFSET] | search ID LITERAL | help]"); return; }
        if (ctx.mode !== "tui" || input === "list" || !artifacts.size) { reply(`${list()}\n${retention}`); return; }
        if (browsing) { reply("Artifacts viewer already open."); return; }
        browsing = true;
        try {
          while (!bound.aborted) {
            const rows = [...artifacts.values()];
            const labels = rows.map(a => `${safeDisplay(a.title)} — ${a.id}`);
            const choice = await ctx.ui.select("Session artifacts (Esc closes)", labels, { signal: bound });
            if (!choice || bound.aborted) break;
            const a = rows[labels.indexOf(choice)]; if (!a) break;
            const text = await track(store.text(get(a.id), bound)); check(bound);
            let offset = 1, column = 0;
            const history: Array<[number, number]> = [];
            while (!bound.aborted) {
              const p = page(text, offset, LIMITS.pageLines, column);
              const action = await showPage(ctx, `${safeDisplay(a.title)} · ${a.id}`, safeDisplay(p.text), !!p.nextOffset, history.length > 0, bound);
              if (action === "next" && p.nextOffset) { history.push([offset, column]); offset = p.nextOffset; column = p.nextColumn; }
              else if (action === "previous" && history.length) { [offset, column] = history.pop()!; }
              else break;
            }
          }
        } finally { browsing = false; }
      } catch (error) { if (!bound.aborted) reply(`Artifacts: ${String(error).slice(0, 400)}`); }
    },
  });
}
