import { beforeEach, expect, test } from "bun:test";
import { CustomMessageComponent, initTheme, ToolExecutionComponent, type MessageRenderer } from "@earendil-works/pi-coding-agent";
import { stripTerminalSequences, visibleWidth } from "@earendil-works/pi-tui";
import roundedTools, { roundToolRenderers } from "../files/extensions/rounded-tools";

beforeEach(() => initTheme("light", false));
const textResult = (text: string) => ({ content: [{ type: "text" as const, text }], details: {} });
const plain = (lines: string[]) => lines.map(stripTerminalSequences);
const diagnostics = [
  "LSP diagnostics:", "", "⚠️ typescript:",
  ...Array.from({ length: 20 }, (_, i) => `src/example.ts:${i + 1}:2 - error: Cannot find module 'module-${i}'. [2307]`),
  "… 12 more diagnostics",
].join("\n");

function tool(text: string) {
  const component = new ToolExecutionComponent("lsp_diagnostics", "test-lsp", { path: "src/example.ts" },
    { outputPad: 1 }, roundToolRenderers("lsp_diagnostics"), { requestRender() {} } as never, process.cwd());
  component.updateResult(textResult(text));
  return component;
}

function notice(summary?: string, content: string | ReturnType<typeof textResult>["content"] = "") {
  let renderer!: MessageRenderer;
  roundedTools({
    on() {},
    registerToolRenderer() {},
    registerMessageRenderer(name: string, value: MessageRenderer) {
      expect(name).toBe("pi-lsp-diagnostics");
      renderer = value;
    },
  } as never);
  const message = { role: "custom" as const, customType: "pi-lsp-diagnostics", content,
    details: { path: "src/example.ts", summary }, display: true, timestamp: Date.now() };
  return { component: new CustomMessageComponent(message, renderer), message };
}

function expectCompact(component: ToolExecutionComponent | CustomMessageComponent, summary: string) {
  const lines = plain(component.render(100));
  expect(lines.filter(Boolean)).toHaveLength(4);
  expect(lines[1]).toStartWith("╭");
  expect(lines.at(-1)).toStartWith("╰");
  expect(lines.join("\n")).toContain(summary);
  expect(lines.join("\n")).toContain("to expand");
  expect(lines.join("\n")).not.toContain("Cannot find module");
  for (let width = 0; width < 110; width++) {
    const narrow = component.render(width);
    expect(narrow.length).toBeLessThanOrEqual(5);
    for (const line of narrow) expect(visibleWidth(line)).toBeLessThanOrEqual(width);
  }
}

test("diagnostics calls collapse to two content rows, retaining received details on expansion", () => {
  const component = tool(diagnostics);
  expectCompact(component, "20 errors; +12 more");
  component.setExpanded(true);
  const expanded = plain(component.render(100)).join("\n");
  expect(expanded).toContain("module-19");
  expect(expanded).toContain("… 12 more diagnostics");
  component.setExpanded(false);
  expectCompact(component, "20 errors; +12 more");
});

test("automatic notices use the same frame and summary without changing message data", () => {
  const { component, message } = notice(diagnostics);
  const original = JSON.stringify(message);
  expectCompact(component, "20 errors; +12 more");
  component.setExpanded(true);
  expect(plain(component.render(100)).join("\n")).toContain("module-19");
  initTheme("dark", false);
  component.invalidate();
  component.setExpanded(false);
  expectCompact(component, "20 errors; +12 more");
  expect(JSON.stringify(message)).toBe(original);
});

test("counts severity labels, not message prose or related locations", () => {
  const text = ["LSP diagnostics:", "⚠️ test:",
    "a.ts:1:1 - error: mentions warning: but remains an error",
    "  related: a.ts:2:1 error: context",
    "a.ts:3:1 - warning: unused", "a.ts:4:1 - info: note", "a.ts:5:1 - hint: suggestion",
    "a.ts:6:1 - diagnostic: unclassified"].join("\n");
  const rendered = plain(tool(text).render(120)).join("\n");
  expect(rendered).toContain("1 error; 1 warning; 1 info; 1 hint; 1 diagnostic");
});

test.each([
  ["LSP diagnostics:\n\n✅ gopls: no diagnostics", "gopls: no diagnostics"],
  ["LSP diagnostics:\n\n✅ no diagnostics recorded for src/a.ts", "no diagnostics recorded"],
  ["LSP diagnostics:\n\n⚠️ typescript: root markers not found (tsconfig.json)", "root markers not found"],
  ["LSP diagnostics:\n\n⚠️ gopls: executable not found", "executable not found"],
  ["Cancelled", "Cancelled"],
  ["", "No diagnostic output"],
])("keeps diagnostic availability and empty states honest: %s", (text, expected) => {
  expect(plain(tool(text).render(120)).join("\n")).toContain(expected);
});

test("notices without summary details fall back to text content", () => {
  for (const content of [diagnostics, textResult(diagnostics).content]) {
    expectCompact(notice(undefined, content).component, "20 errors; +12 more");
  }
});

test("keeps mixed server availability warnings alongside diagnostic counts", () => {
  const text = `${diagnostics}\n\n⚠️ gopls: executable not found`;
  expect(plain(tool(text).render(120)).join("\n")).toContain("20 errors; +12 more; gopls: executable not found");
});
