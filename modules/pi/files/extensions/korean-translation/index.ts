import {
  BorderedLoader, DynamicBorder, getMarkdownTheme,
  type ExtensionAPI, type ExtensionContext,
} from "@earendil-works/pi-coding-agent";
import { Container, Markdown, Text } from "@earendil-works/pi-tui";
import { failureMessage, needsTranslation, translate, TRANSLATION_MODEL, type Translation } from "./translate";

import { outputTranslation } from "./output";

const ENTRY_TYPE = "korean-translation";
const STATE_TYPE = "korean-translation-enabled";
const INPUT_TIMEOUT_MS = 60_000;
const OUTPUT_TIMEOUT_MS = 180_000;
type DisplayEntry = { direction: "input" | "output"; text: string; sourceId?: string; usage?: Translation["usage"] };
type InputResult = { translation: Translation } | { error: unknown };

export default function koreanTranslation(pi: ExtensionAPI) {
  if (process.env.PI_SUBAGENT_CHILD === "1") return;
  let enabled = true;
  let lifetime = new AbortController();

  let output: ReturnType<typeof outputTranslation> | undefined;
  function clearOutput(ctx: ExtensionContext) {
    if (!output) return;
    output.cancel();
    output = undefined;
    ctx.ui.setWidget(ENTRY_TYPE, undefined);
  }
  function startOutput(ctx: ExtensionContext) {
    return outputTranslation(ctx, lifetime.signal, OUTPUT_TIMEOUT_MS, text => {
      ctx.ui.setWidget(ENTRY_TYPE, () => new Markdown(text, 1, 0, getMarkdownTheme()));
    });
  }

  function restore(ctx: ExtensionContext) {
    clearOutput(ctx);
    enabled = true;
    for (const entry of ctx.sessionManager.getBranch()) {
      if (entry.type === "custom" && entry.customType === STATE_TYPE && typeof entry.data === "boolean") {
        enabled = entry.data;
      }
    }
  }

  pi.on("session_start", (_event, ctx) => {
    lifetime = new AbortController();
    restore(ctx);
  });
  pi.on("session_tree", (_event, ctx) => restore(ctx));
  pi.on("session_shutdown", (_event, ctx) => { lifetime.abort(); clearOutput(ctx); });
  pi.on("turn_start", (_event, ctx) => clearOutput(ctx));

  pi.registerCommand("translate", {
    description: "Korean/English translation: /translate [on|off|status]",
    handler: async (args, ctx) => {
      if (ctx.mode !== "tui") {
        ctx.ui.notify("번역은 Pi 대화 화면에서만 사용할 수 있습니다.", "warning");
        return;
      }
      const action = args.trim() || "status";
      if (action !== "status" && action !== "on" && action !== "off") {
        ctx.ui.notify("사용법: /translate on|off|status", "warning");
        return;
      }
      if (action !== "status") {
        if (!ctx.isIdle()) {
          ctx.ui.notify("현재 응답이 끝난 뒤 번역 설정을 바꿔 주세요.", "warning");
          return;
        }
        pi.appendEntry(STATE_TYPE, action === "on");
        enabled = action === "on";
      }
      ctx.ui.notify(`한국어/영어 번역 ${enabled ? "켜짐" : "꺼짐"} (openai/${TRANSLATION_MODEL})`, "info");
    },
  });

  pi.registerEntryRenderer<DisplayEntry>(ENTRY_TYPE, (entry, _options, theme) => {
    const outputPad = pi.getSettings().outputPad ?? 1;
    const box = new Container();
    const data = entry.data;
    if (!data || typeof data.text !== "string") return box;
    box.addChild(data.direction === "input"
      ? new Text(theme.fg("accent", "입력 원문"), outputPad, 0)
      : new DynamicBorder((text: string) => theme.fg("border", text)));
    box.addChild(new Markdown(data.text, outputPad, 0, getMarkdownTheme()));
    return box;
  });

  pi.on("before_agent_start", (event, ctx) => {
    if (!enabled || ctx.mode !== "tui") return;
    event.systemPromptOptions.sections = {
      ...event.systemPromptOptions.sections,
      korean_translation: "Write conversational responses in English. A separate display layer translates them into Korean. Preserve the language required for files, exact quotations, and requested artifacts.",
    };
  });

  pi.on("input", async (event, ctx) => {
    if (!enabled || ctx.mode !== "tui" || event.source !== "interactive" || event.text.startsWith("!")) return;
    if (!needsTranslation(event.text, "English")) return;
    const sessionSignal = lifetime.signal;
    const result = await ctx.ui.custom<InputResult>((tui, theme, _keys, done) => {
      const loader = new BorderedLoader(tui, theme, "영어로 번역 중...");
      const signal = AbortSignal.any([sessionSignal, loader.signal, AbortSignal.timeout(INPUT_TIMEOUT_MS)]);
      // Finish through the request's settlement path, including Escape cancellation.
      translate(event.text, "English", ctx, signal).then(
        translation => done({ translation }),
        error => done({ error }),
      );
      return loader;
    }).catch(error => ({ error }));
    if (sessionSignal.aborted) return { action: "handled" };
    if (!result || "error" in result) {
      ctx.ui.notify(`입력 번역을 완료하지 못해 요청을 보내지 않았습니다. ${failureMessage(result && "error" in result ? result.error : undefined)}`, "warning");
      if (!ctx.ui.getEditorText()) ctx.ui.setEditorText(event.text);
      return { action: "handled" };
    }
    // Custom entries preserve the Korean original without adding it to model context.
    try {
      pi.appendEntry<DisplayEntry>(ENTRY_TYPE, { direction: "input", text: event.text, usage: result.translation.usage });
    } catch {
      ctx.ui.notify("입력 원문을 세션에 저장하지 못해 요청을 보내지 않았습니다.", "error");
      if (!ctx.ui.getEditorText()) ctx.ui.setEditorText(event.text);
      return { action: "handled" };
    }
    return { action: "transform", text: result.translation.text, images: event.images };
  });

  pi.on("message_update", (event, ctx) => {
    if (!enabled || ctx.mode !== "tui" || event.message.role !== "assistant") return;
    const update = event.assistantMessageEvent;
    if (update.type !== "text_delta" || !update.delta.includes("\n")) return;
    const text = event.message.content.filter(block => block.type === "text").map(block => block.text).join("\n\n");
    if (!/\n[ \t\r]*\n/.test(text)) return;
    (output ??= startOutput(ctx)).update(text);
  });

  pi.on("turn_end", async (event, ctx) => {
    if (!enabled || ctx.mode !== "tui" || event.outcome !== "completed" || event.message.role !== "assistant") { clearOutput(ctx); return; }
    const text = event.message.content.filter(block => block.type === "text").map(block => block.text).join("\n\n");
    if (!needsTranslation(text, "Korean")) { clearOutput(ctx); return; }
    if (output && !output.matches(text)) clearOutput(ctx);
    const sessionSignal = lifetime.signal;
    const current = output ??= startOutput(ctx);
    ctx.ui.setStatus(ENTRY_TYPE, "한국어로 번역 중...");
    try {
      const translation = await current.finish(text);
      return {
        entries: [...event.entries, {
          type: "custom" as const,
          customType: ENTRY_TYPE,
          data: { direction: "output", text: translation.text, sourceId: event.messageEntryId, usage: translation.usage } satisfies DisplayEntry,
        }],
      };
    } catch (error) {
      if (!sessionSignal.aborted) {
        ctx.ui.notify(`응답 번역을 완료하지 못했습니다. 영어 원문은 그대로 유지됩니다. ${failureMessage(error)}`, "warning");
      }
    } finally {
      if (output === current) {
        clearOutput(ctx);
        ctx.ui.setStatus(ENTRY_TYPE, undefined);
      }
    }
  });
}
