import type {
  ExtensionAPI,
  ExtensionContext,
} from "@earendil-works/pi-coding-agent";

const STATUS_KEY = "codex-usage";
const REFRESH_MS = 60_000;
const MAX_AGE_MS = 10 * 60_000;
const COMMAND_ARGS = [
  "usage", "--provider", "codex", "--source", "oauth", "--json", "--no-credits",
];

type WeeklyUsage = {
  remainingPercent: number;
  resetsAt: number | undefined;
  updatedAt: number;
};

type Session = {
  ctx: ExtensionContext;
  controller: AbortController;
  timer?: ReturnType<typeof setInterval>;
  pending?: Promise<void>;
  usage?: WeeklyUsage;
  error?: string;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function parseWeeklyUsage(stdout: string): WeeklyUsage {
  let payload: unknown;
  try {
    payload = JSON.parse(stdout);
  } catch {
    throw new Error("CodexBar returned invalid JSON.");
  }
  const entries = Array.isArray(payload)
    ? payload.filter((item) => isRecord(item) && item.provider === "codex")
    : [];
  if (entries.length !== 1) {
    throw new Error("Expected one Codex account in CodexBar output.");
  }
  const entry = entries[0];
  if (entry.error) {
    throw new Error("CodexBar could not fetch Codex usage. Check Codex CLI login and connectivity.");
  }
  const usage = entry.usage;
  const weekly = isRecord(usage) ? usage.secondary : undefined;
  if (!isRecord(weekly)) {
    throw new Error("CodexBar did not report a weekly limit.");
  }
  if (weekly.windowMinutes != null && weekly.windowMinutes !== 10_080) {
    throw new Error("CodexBar returned a non-weekly secondary limit.");
  }
  const used = weekly.usedPercent;
  if (typeof used !== "number" || !Number.isFinite(used) || used < 0 || used > 100) {
    throw new Error("CodexBar returned an invalid weekly percentage.");
  }
  const updatedAt = typeof usage.updatedAt === "string" ? Date.parse(usage.updatedAt) : NaN;
  if (!Number.isFinite(updatedAt)) {
    throw new Error("CodexBar did not report a valid usage timestamp.");
  }
  let resetsAt: number | undefined;
  if (weekly.resetsAt != null) {
    resetsAt = typeof weekly.resetsAt === "string" ? Date.parse(weekly.resetsAt) : NaN;
    if (!Number.isFinite(resetsAt)) {
      throw new Error("CodexBar returned an invalid weekly reset time.");
    }
  }
  return { remainingPercent: 100 - used, resetsAt, updatedAt };
}

function formatResetTime(timestamp: number): string {
  const resetFormatter = new Intl.DateTimeFormat("en-CA", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  });
  const parts = Object.fromEntries(
    resetFormatter.formatToParts(timestamp).map(({ type, value }) => [type, value]),
  );
  return `${parts.year}-${parts.month}-${parts.day} ${parts.hour}:${parts.minute}`;
}

export function formatWeeklyUsage(usage: WeeklyUsage, now = Date.now()): string {
  if (now - usage.updatedAt > MAX_AGE_MS || usage.updatedAt > now + 60_000) {
    return "weekly unavailable (stale data)";
  }
  if (usage.resetsAt !== undefined && usage.resetsAt <= now) {
    return "weekly reset due; awaiting fresh usage";
  }
  const filled = Math.floor(usage.remainingPercent / 10);
  const bar = "#".repeat(filled) + "-".repeat(10 - filled);
  const remaining = `${bar} ${Number(usage.remainingPercent.toFixed(1))}%`;
  const hours = usage.resetsAt === undefined ? undefined : Math.floor((usage.resetsAt - now) / 3_600_000);
  const reset = hours === undefined ? "unknown" : `${Math.floor(hours / 24)}d ${hours % 24}h`;
  return `${remaining} (⏳ ${reset})`;
}

export default function (pi: ExtensionAPI) {
  if (process.env.PI_SUBAGENT_CHILD === "1") return;
  let session: Session | undefined;

  const publish = (state: Session): void => {
    if (state.controller.signal.aborted || state.ctx.mode !== "tui") return;
    state.ctx.ui.setStatus(STATUS_KEY, state.error
      ? "weekly unavailable (/codex-usage)"
      : state.usage ? formatWeeklyUsage(state.usage) : "weekly loading...");
  };

  const refresh = (state: Session): Promise<void> => {
    if (state.pending) return state.pending;
    state.pending = (async () => {
      try {
        let result;
        try {
          result = await pi.exec("codexbar", COMMAND_ARGS, {
            timeout: 30_000,
            signal: state.controller.signal,
          });
        } catch {
          throw new Error("Could not run CodexBar. Check that codexbar is on PATH.");
        }
        if (state.controller.signal.aborted) return;
        if (result.killed) throw new Error("CodexBar usage request timed out after 30 seconds.");
        if (result.code !== 0) {
          throw new Error(`CodexBar exited with code ${result.code}. Check Codex CLI login and connectivity.`);
        }
        state.usage = parseWeeklyUsage(result.stdout);
        state.error = undefined;
      } catch (error) {
        // Do not expose raw CLI output: it can contain account or authentication data.
        state.usage = undefined;
        state.error = error instanceof Error ? error.message : "CodexBar usage request failed.";
      } finally {
        state.pending = undefined;
        publish(state);
      }
    })();
    return state.pending;
  };

  const stop = (): void => {
    if (!session) return;
    clearInterval(session.timer);
    session.controller.abort();
    if (session.ctx.mode === "tui") session.ctx.ui.setStatus(STATUS_KEY, undefined);
    session = undefined;
  };

  pi.on("session_start", (_event, ctx) => {
    stop();
    const state: Session = { ctx, controller: new AbortController() };
    session = state;
    if (ctx.mode !== "tui") return;
    publish(state);
    void refresh(state);
    state.timer = setInterval(() => {
      publish(state);
      void refresh(state);
    }, REFRESH_MS);
    state.timer.unref();
  });

  pi.on("session_shutdown", stop);

  pi.registerCommand("codex-usage", {
    description: "Refresh CodexBar weekly quota and local reset time",
    handler: async (args, ctx) => {
      if (args.trim()) {
        ctx.ui.notify("Usage: /codex-usage", "warning");
        return;
      }
      if (!session) {
        session = { ctx, controller: new AbortController() };
      }
      const state = session;
      await refresh(state);
      if (state.controller.signal.aborted) return;
      if (state.error || !state.usage) {
        ctx.ui.notify(state.error ?? "CodexBar usage unavailable.", "error");
        return;
      }
      const usage = state.usage;
      const reset = usage.resetsAt === undefined
        ? "unknown" : formatResetTime(usage.resetsAt);
      ctx.ui.notify([
        formatWeeklyUsage(usage),
        `Reset: ${reset}`,
        `Updated: ${new Date(usage.updatedAt).toLocaleString()}`,
        "Source: CodexBar OAuth (Codex CLI account, not Pi login)",
      ].join("\n"), "info");
    },
  });
}
