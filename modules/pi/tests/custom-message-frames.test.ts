import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import { mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  CustomMessageComponent, discoverAndLoadExtensions, ExtensionRunner, initTheme, SessionManager,
  type MessageRenderer, type MessageRendererWrapper,
} from "@earendil-works/pi-coding-agent";
import { Container, hyperlink, getOsc8LinkAtColumn, stripTerminalSequences, visibleWidth, type TuiMouseEvent } from "@earendil-works/pi-tui";
import roundedTools from "../files/extensions/rounded-tools";

const roots: string[] = [];
beforeEach(() => initTheme("light", false));
afterEach(async () => { await Promise.all(roots.splice(0).map((path) => rm(path, { recursive: true, force: true }))); });
const patched = typeof ExtensionRunner.prototype.getMessageRendererWrappers === "function";
const plain = (lines: string[]) => lines.map(stripTerminalSequences);
const message = (customType = "future-extension", content = "**Preserved** message") => ({
  role: "custom" as const, customType, content, display: true, timestamp: 1,
});

async function load(extra?: string) {
  const root = await mkdtemp(join(tmpdir(), "pi-message-frame-test-"));
  roots.push(root);
  const agent = join(root, "agent");
  const cwd = join(root, "project");
  await mkdir(join(agent, "extensions"), { recursive: true });
  await mkdir(cwd);
  await symlink(fileURLToPath(new URL("../files/extensions/rounded-tools.ts", import.meta.url)), join(agent, "extensions/rounded-tools.ts"));
  if (extra) await writeFile(join(agent, "extensions/00-custom.ts"), extra);
  const loaded = await discoverAndLoadExtensions([], cwd, agent);
  expect(loaded.errors).toEqual([]);
  return new ExtensionRunner(loaded.extensions, loaded.runtime, cwd, SessionManager.inMemory(cwd), {} as never);
}

function component(wrappers: MessageRendererWrapper[], customType?: string, renderer?: MessageRenderer) {
  return new CustomMessageComponent(message(customType), renderer, undefined, 1, [], undefined, undefined, wrappers);
}
function expectFrame(row: CustomMessageComponent, width = 70) {
  const lines = plain(row.render(width));
  expect(lines.filter((line) => line.startsWith("╭"))).toHaveLength(1);
  expect(lines.filter((line) => line.startsWith("╰"))).toHaveLength(1);
  for (const line of lines) expect(visibleWidth(line)).toBeLessThanOrEqual(width);
}

test("older Pi keeps tool rendering available and reports the required upgrade", () => {
  let start!: (event: unknown, ctx: unknown) => void;
  const renderer = mock(() => {});
  roundedTools({ registerToolRenderer() {}, registerMessageRenderer: renderer,
    on(name: string, callback: typeof start) { expect(name).toBe("session_start"); start = callback; },
  } as never);
  const notify = mock(() => {});
  start({}, { mode: "json", ui: { notify } });
  expect(notify).not.toHaveBeenCalled();
  start({}, { mode: "tui", ui: { notify } });
  expect(notify).toHaveBeenCalledWith(expect.stringContaining("restart Pi"), "warning");
  expect(renderer).not.toHaveBeenCalled();
});

describe.skipIf(!patched)("patched custom-message wrapper hook", () => {
  test("frames default Markdown for every message type, including future extensions", async () => {
    const runner = await load();
    const wrappers = runner.getMessageRendererWrappers();
    expect(wrappers).toHaveLength(1);
    for (const type of ["pi-bg-bash", "pi-bg-bash-output", "web-search-results", "web-search-error", "subagent-notify", "future-extension"]) {
      const row = component(wrappers, type);
      expectFrame(row);
      expect(plain(row.render(70)).join("\n")).toContain("Preserved message");
      row.setExpanded(true);
      row.setOutputPad(0);
      row.invalidate();
      expectFrame(row);
      for (const width of [0, 1, 4, 5, 20, 70, 120]) {
        for (const line of row.render(width)) expect(visibleWidth(line)).toBeLessThanOrEqual(width);
      }
    }
  });

  test("preserves custom renderers, their expansion options, links and mouse coordinates", async () => {
    const runner = await load();
    const events: TuiMouseEvent[] = [];
    const row = component(runner.getMessageRendererWrappers(), "custom", (_message, options) => ({
      render: () => [hyperlink(options.expanded ? "expanded" : "collapsed", "file:///tmp/example.ts")],
      invalidate() {},
      handleMouse(event) { events.push(event); return { handled: true, capture: true }; },
    }));
    expectFrame(row);
    expect(getOsc8LinkAtColumn(row.render(50)[2], 1)).toBe("file:///tmp/example.ts");
    expect(row.handleMouse({ type: "click", button: "left", x: 3, y: 2, screenX: 3, screenY: 2,
      width: 50, height: 4, shift: false, alt: false, ctrl: false })?.capture).toBe(true);
    expect(events[0]).toMatchObject({ x: 2, y: 0, width: 48, height: 1 });
    row.setExpanded(true);
    expect(plain(row.render(50)).join("\n")).toContain("expanded");
  });

  test("applies wrappers to renderer fallback and preserves Markdown transformers", async () => {
    const wrappers = (await load()).getMessageRendererWrappers();
    for (const renderer of [() => undefined, () => { throw new Error("renderer failed"); }]) {
      const row = new CustomMessageComponent(message(), renderer, undefined, 1,
        [(text) => text.replace("Preserved", "Transformed")], undefined, undefined, wrappers);
      expectFrame(row);
      expect(plain(row.render(70)).join("\n")).toContain("Transformed message");
    }
  });

  test("empty custom renderers remain empty and inline graphics remain unframed", async () => {
    const wrappers = (await load()).getMessageRendererWrappers();
    expect(component(wrappers, "hidden", () => new Container()).render(60)).toEqual([""]);
    const graphic = "\x1b_Ga=T,f=100;AAAA\x1b\\";
    expect(component(wrappers, "image", () => ({ render: () => [graphic], invalidate() {} })).render(60)).toEqual(["", graphic]);
  });

  test("registered wrappers compose in load order and failures retain the message", async () => {
    const runner = await load(`import { Container, Text } from "@earendil-works/pi-tui";
      export default function(pi) {
        pi.registerMessageRendererWrapper((body) => { const c = new Container(); c.addChild(new Text("first", 0, 0)); c.addChild(body); return c; });
        pi.registerMessageRendererWrapper(() => { throw new Error("wrapper failure"); });
      }`);
    expect(runner.getMessageRendererWrappers()).toHaveLength(3);
    const row = component(runner.getMessageRendererWrappers());
    const text = plain(row.render(90)).join("\n");
    expectFrame(row, 90);
    expect(text).toContain("Custom message wrapper failed: wrapper failure");
    expect(text).toContain("first");
    expect(text).toContain("Preserved message");
  });
});
