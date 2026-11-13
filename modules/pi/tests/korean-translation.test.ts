import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import { initTheme, SessionManager } from "@earendil-works/pi-coding-agent";
import { stripTerminalSequences, visibleWidth, type Component } from "@earendil-works/pi-tui";
import koreanTranslation from "../files/extensions/korean-translation";
import { failureMessage, MAX_TEXT_BYTES, needsTranslation, protectText, translate } from "../files/extensions/korean-translation/translate";

const usage = { input: 5, output: 6, cacheRead: 0, cacheWrite: 0, totalTokens: 11, cost: { input: 0.1, output: 0.2, cacheRead: 0, cacheWrite: 0, total: 0.3 } };
const theme = { fg: (_: string, text: string) => text };
const model = { provider: "openai", id: "gpt-6-luna" };
const answer = (text = "The check passed.", stopReason = "stop") => ({
  role: "assistant", api: "openai-responses", provider: "openai", model: "working-model",
  content: [{ type: "text", text }], timestamp: 1, stopReason, usage,
});
const cleanups: (() => void)[] = [];
beforeEach(() => initTheme("light", false));
afterEach(() => { while (cleanups.length) cleanups.pop()!(); });

function harness(sm = SessionManager.inMemory("/tmp/korean-translation-test"), mode = "tui") {
  const handlers = new Map<string, Function>();
  let command: any;
  let renderer: any;
  let editor = "";
  let loader: Component | undefined;
  let respond = async (_model: unknown, context: any, _options: any) => answer(
    context.systemPrompt.includes("into English")
      ? context.messages[0].content.replaceAll("설명해 주세요", "Please explain")
      : context.messages[0].content.replaceAll("The check passed.", "검사가 통과했습니다."),
  );
  const streamSimple = mock((model: unknown, context: unknown, options: unknown) => ({ result: () => respond(model, context, options) }));
  const ui = {
    notify: mock(() => {}), setStatus: mock(() => {}),
    getEditorText: () => editor, setEditorText: (text: string) => { editor = text; },
    custom: (factory: Function) => new Promise(resolve => {
      loader = factory({ requestRender() {} }, theme, {}, (result: unknown) => { loader?.dispose?.(); loader = undefined; resolve(result); });
    }),
  };
  const ctx = {
    mode, ui, hasUI: mode === "tui", sessionManager: sm, signal: undefined as AbortSignal | undefined,
    isIdle: () => true,
    modelRegistry: { find: mock(() => model), hasConfiguredAuth: mock(() => true), streamSimple },
  };
  const append = mock((type: string, data: unknown) => sm.appendCustomEntry(type, data));
  koreanTranslation({
    on: (name: string, handler: Function) => handlers.set(name, handler),
    appendEntry: append,
    getSettings: () => ({ outputPad: 1 }),
    registerCommand: (_name: string, value: unknown) => { command = value; },
    registerEntryRenderer: (_name: string, value: unknown) => { renderer = value; },
  } as never);
  const emit = (name: string, event: unknown = {}) => handlers.get(name)?.(event, ctx);
  emit("session_start");
  cleanups.push(() => { emit("session_shutdown"); loader?.dispose?.(); });
  return {
    ctx, ui, sm, streamSimple, append, emit, renderer, handlers,
    command: (args: string) => command.handler(args, ctx),
    input: (text = "설명해 주세요", extra = {}) => emit("input", { source: "interactive", text, ...extra }),
    output: (message = answer(), extra = {}) => emit("turn_end", { message, outcome: "completed", messageEntryId: "assistant-id", entries: [], ...extra }),
    respond: (fn: typeof respond) => { respond = fn; },
    cancelInput: () => loader?.handleInput?.("\x1b"),
    loader: () => loader,
  };
}

function pending(_model: unknown, _context: unknown, { signal }: { signal: AbortSignal }): Promise<never> {
  return new Promise((_resolve, reject) => {
    if (signal.aborted) reject(signal.reason);
    else signal.addEventListener("abort", () => reject(signal.reason), { once: true });
  });
}

describe("literal preservation", () => {
  test.each([
    "설명해 주세요 `rm -rf /tmp/테스트` 및 ``a ` b``",
    "설명해 주세요\n```ts\nconst text = '한국어';\n```\n다음 문장",
    "설명해 주세요\n~~~~sh\n# 한국어\n~~~\n~~~~\n",
    "설명해 주세요\n```sh\nprintf '한글'",
    "/skill:review 설명해 주세요 https://example.com/a?q=1#x src/index.ts ../test.ts /tmp/test.txt ~/test.txt",
  ])("restores protected spans byte-for-byte: %s", text => {
    const protectedText = protectText(text);
    expect(protectedText.restore(protectedText.text)).toBe(text);
    expect(protectedText.restore(protectedText.text.replace("설명해 주세요", "Please explain")))
      .toBe(text.replace("설명해 주세요", "Please explain"));
  });

  test("rejects removed, repeated, reordered, and invented placeholders", () => {
    const protectedText = protectText("`one` then `two`");
    const tokens = protectedText.text.match(/\{\{PI_KEEP_[^}]+\}\}/g)!;
    for (const changed of [tokens[0], `${tokens[0]} ${tokens[0]} ${tokens[1]}`, `${tokens[1]} ${tokens[0]}`, protectedText.text.replace("_0}}", "_42}}")]) {
      expect(() => protectedText.restore(changed)).toThrow("protected");
    }
  });

  test("detects prose rather than code, paths, command prefixes, or links", () => {
    expect(needsTranslation("```py\nprint('한국어')\n```", "English")).toBe(false);
    expect(needsTranslation("`한국어` https://example.com/한국어", "English")).toBe(false);
    expect(needsTranslation("/skill:review `echo hello` src/index.ts", "Korean")).toBe(false);
    expect(needsTranslation("설명해 주세요 `code`", "English")).toBe(true);
    expect(needsTranslation("The check passed. `code`", "Korean")).toBe(true);
  });
});

describe("input, provider isolation, and output", () => {
  test.each([undefined, "steer", "followUp"])("translates interactive input without changing delivery mode %s or images", async streamingBehavior => {
    const h = harness();
    const images = [{ type: "image", mimeType: "image/png", data: "private-image-not-for-translator" }];
    const result = await h.input("설명해 주세요 `foo()`", { images, streamingBehavior });
    expect(result).toEqual({ action: "transform", text: "Please explain `foo()`", images });
    const [calledModel, context, options] = h.streamSimple.mock.calls[0] as any;
    expect(calledModel).toEqual(model);
    expect(h.ctx.modelRegistry.find).toHaveBeenCalledWith("openai", "gpt-6-luna");
    expect(context.messages).toHaveLength(1);
    expect(context.tools).toBeUndefined();
    expect(JSON.stringify(context)).not.toContain(images[0].data);
    expect(options).toMatchObject({ reasoning: "low", cacheRetention: "none", maxTokens: 16_384 });
    expect(options.signal).toBeInstanceOf(AbortSignal);
    const entry = h.sm.getBranch()[0] as any;
    expect(entry).toMatchObject({ type: "custom", data: { direction: "input", text: "설명해 주세요 `foo()`", usage } });
    expect(h.sm.buildSessionContext().messages).toEqual([]);
    h.sm.appendMessage({ role: "user", content: result.text, timestamp: 1 });
    expect(h.sm.buildSessionContext().messages[0].content).toBe("Please explain `foo()`");
    expect(h.loader()).toBeUndefined();
  });

  test("preserves skill invocation while translating its arguments", async () => {
    const h = harness();
    expect((await h.input("/skill:review 설명해 주세요")).text).toBe("/skill:review Please explain");
  });

  test("adds Korean below the English turn without changing messages, tools, or continuation", async () => {
    const h = harness();
    const message = answer("The check passed.\n```sh\necho hello\n```");
    (message.content as any[]).push({ type: "toolCall", id: "call", name: "bash", arguments: { command: "echo hello" } });
    const original = structuredClone(message);
    const previousDraft = { type: "custom", customType: "another-extension", data: 1 };
    h.sm.appendMessage(message as never);
    const result = await h.output(message, { entries: [previousDraft], continue: true });
    expect(result.entries[0]).toBe(previousDraft);
    expect(result).not.toHaveProperty("continue");
    expect(message).toEqual(original);
    expect(h.streamSimple.mock.calls[0][1].messages[0].content).not.toContain('"command"');
    const draft = result.entries[1];
    expect(draft).toMatchObject({ type: "custom", customType: "korean-translation", data: { direction: "output", sourceId: "assistant-id", text: "검사가 통과했습니다.\n```sh\necho hello\n```" } });
    h.sm.appendCustomEntry(draft.customType, draft.data);
    expect(h.sm.buildSessionContext().messages).toEqual([original]);
    expect(h.ui.setStatus.mock.calls.at(-1)).toEqual(["korean-translation", undefined]);
  });

  test("requests English prose using a structured section without replacing the system prompt", () => {
    const h = harness();
    const event = { systemPrompt: "Keep me", systemPromptOptions: { sections: { existing: "Keep this too" } } };
    expect(h.emit("before_agent_start", event)).toBeUndefined();
    expect(event.systemPrompt).toBe("Keep me");
    expect(event.systemPromptOptions.sections).toMatchObject({ existing: "Keep this too", korean_translation: expect.stringContaining("English") });
  });

  test("passes through English, extension messages, shell input, code-only text and incomplete turns", async () => {
    const h = harness();
    for (const text of ["Please explain", "!echo 한국어", "`한국어`", "```py\nprint('한국어')\n```", ""]) expect(await h.input(text)).toBeUndefined();
    expect(await h.input("설명해 주세요", { source: "extension" })).toBeUndefined();
    expect(await h.output(answer("`foo()`"))).toBeUndefined();
    expect(await h.output(answer(), { outcome: "aborted" })).toBeUndefined();
    expect(await h.output(answer(), { outcome: "error" })).toBeUndefined();
    expect(h.streamSimple).not.toHaveBeenCalled();
  });

  test.each(["rpc", "json", "print"])("makes no translation calls in %s mode", async mode => {
    const h = harness(undefined, mode);
    expect(await h.input()).toBeUndefined();
    expect(await h.output()).toBeUndefined();
    expect(h.streamSimple).not.toHaveBeenCalled();
  });

  test("does not register in subagent processes", () => {
    const previous = process.env.PI_SUBAGENT_CHILD;
    process.env.PI_SUBAGENT_CHILD = "1";
    try { expect(harness().handlers.size).toBe(0); }
    finally {
      if (previous === undefined) delete process.env.PI_SUBAGENT_CHILD;
      else process.env.PI_SUBAGENT_CHILD = previous;
    }
  });
});

describe("failures and lifecycle", () => {
  test.each(["error", "length", "aborted", "toolUse"])("rejects translator stop reason %s without leaking provider error details", async stopReason => {
    const h = harness();
    h.respond(async () => ({ ...answer("partial", stopReason), errorMessage: "secret-provider-detail" }));
    expect(await h.input()).toEqual({ action: "handled" });
    expect(h.ctx.ui.getEditorText()).toBe("설명해 주세요");
    expect(h.sm.getBranch()).toEqual([]);
    expect(JSON.stringify(h.ui.notify.mock.calls)).not.toContain("secret-provider-detail");
    expect(JSON.stringify(h.ui.notify.mock.calls)).toContain(stopReason);
  });

  test("missing model/authentication, empty text, damaged literals, and oversized input fail closed", async () => {
    const h = harness();
    h.ctx.modelRegistry.find.mockReturnValueOnce(undefined as never);
    expect(await h.input()).toEqual({ action: "handled" });
    h.ctx.modelRegistry.hasConfiguredAuth.mockReturnValueOnce(false);
    expect(await h.input()).toEqual({ action: "handled" });
    expect(h.streamSimple).not.toHaveBeenCalled();
    h.respond(async () => answer(""));
    expect(await h.input()).toEqual({ action: "handled" });
    h.respond(async () => answer("Please explain"));
    expect(await h.input("설명해 주세요 `foo()`")).toEqual({ action: "handled" });
    const calls = h.streamSimple.mock.calls.length;
    expect(await h.input("한".repeat(MAX_TEXT_BYTES))).toEqual({ action: "handled" });
    expect(h.streamSimple.mock.calls.length).toBe(calls);
    expect(h.sm.getBranch()).toEqual([]);
  });

  test("Escape cancels the real loader, restores input, and leaves a newer draft untouched", async () => {
    const h = harness();
    h.respond(pending);
    const result = h.input();
    h.cancelInput();
    expect(await result).toEqual({ action: "handled" });
    expect(h.ctx.ui.getEditorText()).toBe("설명해 주세요");
    const next = h.input();
    h.ctx.ui.setEditorText("새 초안");
    h.cancelInput();
    expect(await next).toEqual({ action: "handled" });
    expect(h.ctx.ui.getEditorText()).toBe("새 초안");
    expect(h.sm.getBranch()).toEqual([]);
  });

  test("session shutdown cancels pending input without touching the replacement session", async () => {
    const h = harness();
    h.respond(pending);
    const result = h.input();
    h.emit("session_shutdown");
    expect(await result).toEqual({ action: "handled" });
    expect(h.ui.notify).not.toHaveBeenCalled();
    expect(h.ctx.ui.getEditorText()).toBe("");
    expect(h.sm.getBranch()).toEqual([]);
  });

  test("output cancellation preserves the response and clears progress", async () => {
    const h = harness();
    h.respond(pending);
    const controller = new AbortController();
    h.ctx.signal = controller.signal;
    const result = h.output();
    controller.abort();
    expect(await result).toBeUndefined();
    expect(h.ui.notify).toHaveBeenCalled();
    expect(h.ui.setStatus.mock.calls.at(-1)).toEqual(["korean-translation", undefined]);
    expect(h.sm.getBranch()).toEqual([]);
  });

  test("timeout signals reach the provider and raw exceptions remain redacted", async () => {
    const h = harness();
    h.respond(pending);
    await expect(translate("hello", "Korean", h.ctx as never, AbortSignal.timeout(5))).rejects.toThrow();
    h.respond(async () => { throw new Error("API_KEY=secret"); });
    expect(await h.output()).toBeUndefined();
    expect(JSON.stringify(h.ui.notify.mock.calls)).not.toContain("secret");
    expect(failureMessage(new Error("private"))).toContain("authentication");
  });

  test("UI failures block input rather than falling through with untranslated text", async () => {
    const h = harness();
    h.ui.custom = async () => { throw new Error("UI unavailable"); };
    expect(await h.input()).toEqual({ action: "handled" });
    expect(h.ctx.ui.getEditorText()).toBe("설명해 주세요");
    expect(h.streamSimple).not.toHaveBeenCalled();
  });

  test("persistence failure blocks input rather than losing its Korean original", async () => {
    const h = harness();
    h.append.mockImplementationOnce(() => { throw new Error("disk full"); });
    expect(await h.input()).toEqual({ action: "handled" });
    expect(h.ctx.ui.getEditorText()).toBe("설명해 주세요");
  });

  test("toggles restore from the selected branch on reload, resume, fork, and tree navigation", async () => {
    const h = harness();
    h.sm.appendCustomEntry("root", {});
    const root = h.sm.getLeafId()!;
    await h.command("off");
    expect(await h.input()).toBeUndefined();
    const resumed = harness(h.sm);
    expect(await resumed.input()).toBeUndefined();
    const fork = SessionManager.inMemory("/tmp/translation-fork");
    for (const entry of h.sm.getBranch()) if (entry.type === "custom") fork.appendCustomEntry(entry.customType, entry.data);
    expect(await harness(fork).input()).toBeUndefined();
    h.sm.branch(root);
    h.emit("session_tree");
    expect((await h.input()).action).toBe("transform");
    await h.command("off");
    await h.command("on");
    expect((await h.input()).action).toBe("transform");
    const count = h.sm.getBranch().length;
    await h.command("status");
    await h.command("invalid");
    h.ctx.isIdle = () => false;
    await h.command("off");
    expect(h.sm.getBranch()).toHaveLength(count);
  });

  test("saved Korean entries use native Markdown at narrow widths without retranslation", async () => {
    const h = harness();
    const data = { direction: "output", text: "검사가 **통과했습니다**.\n\n- `src/example.ts`\n- [문서](https://example.com)" };
    const component = h.renderer({ data }, { expanded: false }, theme);
    for (const width of [12, 30, 80]) {
      const lines = component.render(width);
      const plain = lines.map(stripTerminalSequences);
      expect(plain[0]).toBe("─".repeat(width));
      expect(plain[1].trimStart()).toStartWith("검사가");
      expect(plain.join("")).not.toContain("한국어 번역");
      expect(plain.join("").replaceAll(/\s/g, "")).toContain("검사가통과했습니다.");
      for (const line of lines) expect(visibleWidth(line)).toBeLessThanOrEqual(width);
      component.invalidate();
    }
    expect(h.streamSimple).not.toHaveBeenCalled();
  });

  test("keeps the original input label", () => {
    const h = harness();
    const component = h.renderer({ data: { direction: "input", text: "설명해 주세요" } }, { expanded: false }, theme);
    const lines = component.render(30).map(stripTerminalSequences);
    expect(lines[0].trim()).toBe("입력 원문");
    expect(lines[1].trim()).toBe("설명해 주세요");
    expect(h.streamSimple).not.toHaveBeenCalled();
  });
});
