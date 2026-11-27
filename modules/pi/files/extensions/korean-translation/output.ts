import type { ExtensionContext } from "@earendil-works/pi-coding-agent";
import { addUsage, MAX_TEXT_BYTES, needsTranslation, replaceLiterals, translate, TranslationError, type Translation } from "./translate";

// Only cut at a finished blank line outside literal spans. An unfinished line may still become a fence.
function completedPrefix(text: string) {
  const masked = replaceLiterals(text, span => "x".repeat(span.length - Number(span.endsWith("\n"))) + (span.endsWith("\n") ? "\n" : ""), true);
  let end = 0;
  for (const match of masked.matchAll(/^[\t \r]*\n/gm)) end = match.index + match[0].length;
  return end;
}

export function outputTranslation(ctx: ExtensionContext, lifetime: AbortSignal, timeout: number, preview: (text: string) => void) {
  const controller = new AbortController();
  const signal = AbortSignal.any([controller.signal, lifetime, AbortSignal.timeout(timeout), ...(ctx.signal ? [ctx.signal] : [])]);
  const result: Translation = { text: "" };
  let prefix = "";
  let work = Promise.resolve();
  // Observe speculative failures immediately, but report them only for a completed turn.
  function enqueue(chunk: string, show: boolean) {
    work = work.then(async () => {
      signal.throwIfAborted();
      const separator = chunk.match(/(?:\r?\n[\t ]*)*\r?\n$/)?.[0] ?? "";
      const body = chunk.slice(0, chunk.length - separator.length);
      const part = needsTranslation(body, "Korean") ? await translate(body, "Korean", ctx, signal) : { text: body };
      signal.throwIfAborted();
      result.text += part.text + separator;
      if (part.usage) result.usage = addUsage(result.usage, part.usage);
      if (show && part.text.trim()) preview(result.text);
    });
    void work.catch(() => {});
  }
  return {
    cancel: () => controller.abort(),
    matches: (text: string) => text.startsWith(prefix),
    update(text: string) {
      if (!text.startsWith(prefix) || Buffer.byteLength(text) > MAX_TEXT_BYTES) return;
      const tail = text.slice(prefix.length);
      const chunk = tail.slice(0, completedPrefix(tail));
      if (!chunk) return;
      prefix += chunk;
      enqueue(chunk, true);
    },
    async finish(text: string) {
      if (Buffer.byteLength(text) > MAX_TEXT_BYTES) throw new TranslationError("Translation text exceeds 64 KiB; split the message or use /translate off");
      if (text.length > prefix.length) enqueue(text.slice(prefix.length), false);
      await work;
      signal.throwIfAborted();
      return result;
    },
  };
}
