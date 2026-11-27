import type { ExtensionContext } from "@earendil-works/pi-coding-agent";
import { addUsage, MAX_TEXT_BYTES, needsTranslation, translate, TranslationError, type Translation } from "./translate";

// Only cut at a finished blank line outside literal spans. An unfinished line may still become a fence.
function completedPrefix(text: string) {
  let end = 0;
  let fence = "";
  let inline = "";
  for (const match of text.matchAll(/[^\n]*\n/g)) {
    const line = match[0];
    if (fence) {
      const close = line.trim();
      if (close.startsWith(fence) && [...close].every(char => char === fence[0])) fence = "";
      continue;
    }
    const opening = /^ {0,3}(`{3,}|~{3,})/.exec(line);
    if (opening) fence = opening[1];
    else if (!inline && !line.trim()) end = match.index + line.length;
    else for (const [ticks] of line.matchAll(/`+/g)) {
      if (!inline || ticks === inline) inline = inline ? "" : ticks;
    }
  }
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
      if (show && part.text.trim()) preview(part.text);
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
