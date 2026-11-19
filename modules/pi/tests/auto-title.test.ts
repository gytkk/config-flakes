import { expect, mock, test } from "bun:test";
import autoTitle from "../files/extensions/auto-title";

function fixture() {
  const handlers = new Map<string, Function>();
  let name: string | undefined;
  const response = Promise.withResolvers<unknown>();
  const model = { provider: "openai", id: "gpt-6-luna" };
  const find = mock(() => model);
  const streamSimple = mock(() => ({ result: () => response.promise }));
  const ctx = { mode: "tui", ui: { notify: mock() }, modelRegistry: { find, streamSimple, hasConfiguredAuth: () => true } };
  const setSessionName = mock((value: string) => { name = value; });
  autoTitle({ on: (key: string, cb: Function) => handlers.set(key, cb), getSessionName: () => name, setSessionName } as never);
  return { ctx, find, streamSimple, setSessionName, response,
    input: (text: string, source = "interactive") => handlers.get("input")!({ text, source }, ctx),
    shutdown: () => handlers.get("session_shutdown")!(),
    async finish(text: string) {
      response.resolve({ stopReason: "stop", content: [{ type: "text", text }] });
      await Bun.sleep(0);
    },
  };
}

test("names an unnamed session without blocking input or requesting another title", async () => {
  const h = fixture();
  expect(h.input("a".repeat(3000))).toBeUndefined();
  h.input("another task");
  expect(h.find).toHaveBeenCalledWith("openai", "gpt-6-luna");
  expect(h.streamSimple).toHaveBeenCalledTimes(1);
  expect(h.streamSimple).toHaveBeenCalledWith(expect.anything(), expect.objectContaining({
    messages: [{ role: "user", content: "a".repeat(2000), timestamp: expect.any(Number) }],
  }), expect.anything());
  await h.finish("Fix\x07\nsidebar titles");
  expect(h.setSessionName).toHaveBeenCalledWith("Fix sidebar titles");
});

test("skips headless, command, noninteractive and already named sessions", () => {
  const h = fixture();
  h.ctx.mode = "rpc"; h.input("rename me"); h.ctx.mode = "tui";
  h.input("/reload"); h.input("!pwd"); h.input(" "); h.input("extension message", "extension");
  h.setSessionName("Manual"); h.input("another task");
  expect(h.streamSimple).not.toHaveBeenCalled();
});

for (const action of ["rename", "shutdown"]) {
  test(`late response cannot overwrite the title after ${action}`, async () => {
    const h = fixture(); h.input("Fix sidebar titles");
    if (action === "rename") h.setSessionName("Manual"); else h.shutdown();
    await h.finish("Late title");
    expect(h.setSessionName).not.toHaveBeenCalledWith("Late title");
    expect(h.ctx.ui.notify).not.toHaveBeenCalled();
  });
}

test("provider failure leaves the session unnamed and reports a bounded warning", async () => {
  const h = fixture(); h.input("Fix sidebar titles");
  h.response.reject(new Error("private provider detail")); await Bun.sleep(0);
  expect(h.ctx.ui.notify).toHaveBeenCalledWith(expect.not.stringContaining("private provider detail"), "warning");
  expect(h.setSessionName).not.toHaveBeenCalled();
});
