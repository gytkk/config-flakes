import type {
  ExtensionAPI,
  ExtensionContext,
} from "@earendil-works/pi-coding-agent";

const STATUS_KEY = "subscription-usage";
const TICK_MS = 60_000;
const MAX_AGE_MS = 10 * 60_000;

type ProviderId = "codex" | "claude";

type ProviderSource = {
  label: string;
  args: string[];
  refreshMs: number;
  timeoutMs: number;
  account: string;
  hint: string;
};

const SOURCES: Record<ProviderId, ProviderSource> = {
  codex: {
    label: "Codex",
    args: ["usage", "--provider", "codex", "--source", "oauth", "--json", "--no-credits"],
    refreshMs: 60_000,
    timeoutMs: 30_000,
    account: "CodexBar OAuth (Codex CLI account, not Pi login)",
    hint: "Check Codex CLI login and connectivity.",
  },
  // CodexBar's Claude CLI source starts a Claude Code session and takes about 30 seconds.
  claude: {
    label: "Claude",
    args: ["usage", "--provider", "claude", "--source", "cli", "--json", "--no-credits"],
    refreshMs: 5 * 60_000,
    timeoutMs: 60_000,
    account: "CodexBar Claude CLI (Claude Code account)",
    hint: "Check Claude Code login and connectivity.",
  },
};

const MODEL_PROVIDERS: Record<string, ProviderId> = {
  openai: "codex",
  "openai-codex": "codex",
  "claude-sdk": "claude",
  anthropic: "claude",
};

export function usageProviderFor(modelProvider: string | undefined): ProviderId | undefined {
  return modelProvider === undefined ? undefined : MODEL_PROVIDERS[modelProvider];
}

type WeeklyUsage = {
  remainingPercent: number;
  resetsAt: number | undefined;
  updatedAt: number;
};

type ProviderState = {
  pending?: Promise<void>;
  attemptedAt?: number;
  usage?: WeeklyUsage;
  error?: string;
};

type Session = {
  ctx: ExtensionContext;
  controller: AbortController;
  provider?: ProviderId;
  providers: Partial<Record<ProviderId, ProviderState>>;
  timer?: ReturnType<typeof setInterval>;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function parseWeeklyUsage(stdout: string, provider: ProviderId = "codex"): WeeklyUsage {
  const { label, hint } = SOURCES[provider];
  let payload: unknown;
  try {
    payload = JSON.parse(stdout);
  } catch {
    throw new Error("CodexBar returned invalid JSON.");
  }
  const entries = Array.isArray(payload)
    ? payload.filter((item) => isRecord(item) && item.provider === provider)
    : [];
  if (entries.length !== 1) {
    throw new Error(`Expected one ${label} account in CodexBar output.`);
  }
  const entry = entries[0];
  if (entry.error) {
    throw new Error(`CodexBar could not fetch ${label} usage. ${hint}`);
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

  const providerState = (state: Session, provider: ProviderId): ProviderState =>
    state.providers[provider] ??= {};

  const publish = (state: Session): void => {
    if (state.controller.signal.aborted || state.ctx.mode !== "tui") return;
    if (!state.provider) {
      state.ctx.ui.setStatus(STATUS_KEY, undefined);
      return;
    }
    const current = providerState(state, state.provider);
    state.ctx.ui.setStatus(STATUS_KEY, current.error
      ? "weekly unavailable (/usage)"
      : current.usage ? formatWeeklyUsage(current.usage) : "weekly loading...");
  };

  const refresh = (state: Session, provider: ProviderId): Promise<void> => {
    const current = providerState(state, provider);
    if (current.pending) return current.pending;
    const source = SOURCES[provider];
    current.attemptedAt = Date.now();
    current.pending = (async () => {
      try {
        let result;
        try {
          result = await pi.exec("codexbar", source.args, {
            timeout: source.timeoutMs,
            signal: state.controller.signal,
          });
        } catch {
          throw new Error("Could not run CodexBar. Check that codexbar is on PATH.");
        }
        if (state.controller.signal.aborted) return;
        if (result.killed) {
          throw new Error(`CodexBar ${source.label} usage request timed out after ${source.timeoutMs / 1000} seconds.`);
        }
        if (result.code !== 0) {
          throw new Error(`CodexBar exited with code ${result.code}. ${source.hint}`);
        }
        current.usage = parseWeeklyUsage(result.stdout, provider);
        current.error = undefined;
      } catch (error) {
        // Do not expose raw CLI output: it can contain account or authentication data.
        current.usage = undefined;
        current.error = error instanceof Error ? error.message : "CodexBar usage request failed.";
      } finally {
        current.pending = undefined;
        publish(state);
      }
    })();
    return current.pending;
  };

  const refreshIfDue = (state: Session): void => {
    if (!state.provider) return;
    const attemptedAt = providerState(state, state.provider).attemptedAt;
    if (attemptedAt === undefined || Date.now() - attemptedAt >= SOURCES[state.provider].refreshMs) {
      void refresh(state, state.provider);
    }
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
    const state: Session = {
      ctx,
      controller: new AbortController(),
      provider: usageProviderFor(ctx.model?.provider),
      providers: {},
    };
    session = state;
    if (ctx.mode !== "tui") return;
    publish(state);
    refreshIfDue(state);
    state.timer = setInterval(() => {
      publish(state);
      refreshIfDue(state);
    }, TICK_MS);
    state.timer.unref();
  });

  pi.on("model_select", (event) => {
    const state = session;
    if (!state || state.ctx.mode !== "tui") return;
    const provider = usageProviderFor(event.model.provider);
    if (provider === state.provider) return;
    state.provider = provider;
    publish(state);
    refreshIfDue(state);
  });

  pi.on("session_shutdown", stop);

  pi.registerCommand("usage", {
    description: "Refresh the selected model's weekly subscription quota and local reset time",
    handler: async (args, ctx) => {
      if (args.trim()) {
        ctx.ui.notify("Usage: /usage", "warning");
        return;
      }
      const provider = usageProviderFor(ctx.model?.provider);
      if (!provider) {
        ctx.ui.notify(
          `No subscription usage source for provider ${ctx.model?.provider ?? "(no model)"}. Select an OpenAI or Claude model.`,
          "warning",
        );
        return;
      }
      if (!session) {
        session = { ctx, controller: new AbortController(), provider, providers: {} };
      }
      const state = session;
      await refresh(state, provider);
      if (state.controller.signal.aborted) return;
      const current = providerState(state, provider);
      if (current.error || !current.usage) {
        ctx.ui.notify(current.error ?? "CodexBar usage unavailable.", "error");
        return;
      }
      const usage = current.usage;
      const reset = usage.resetsAt === undefined
        ? "unknown" : formatResetTime(usage.resetsAt);
      ctx.ui.notify([
        `${SOURCES[provider].label} ${formatWeeklyUsage(usage)}`,
        `Reset: ${reset}`,
        `Updated: ${new Date(usage.updatedAt).toLocaleString()}`,
        `Source: ${SOURCES[provider].account}`,
      ].join("\n"), "info");
    },
  });
}
