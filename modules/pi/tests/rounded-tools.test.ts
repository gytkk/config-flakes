import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createBashToolDefinition, createCodemodeExtension, createEditToolDefinition, createReadToolDefinition, initTheme,
  ToolExecutionComponent, type ToolRenderers, type ToolRendererResolver, type Theme, type ThemeColor,
} from "@earendil-works/pi-coding-agent";
import {
  Container, Text, getCapabilities, getOsc8LinkAtColumn, hyperlink, setCapabilities,
  stripTerminalSequences, visibleWidth, type TuiMouseEvent,
} from "@earendil-works/pi-tui";
import roundedTools, { roundToolRenderers } from "../files/extensions/rounded-tools";

let theme: Theme;
const roots: string[] = [];
const capabilities = getCapabilities();
beforeEach(() => initTheme("dark", false));
afterEach(async () => {
  setCapabilities(capabilities);
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});
const textResult = (text: string) => ({ content: [{ type: "text" as const, text }], details: undefined });
const plain = (lines: string[]) => lines.map(stripTerminalSequences);

function cells(line: string) {
  let bg = "";
  let fg = "";
  const result: { char: string; bg: string; fg: string }[] = [];
  for (const part of line.split(/(\x1b\[[\d;]*m)/)) {
    if (!part.startsWith("\x1b[")) {
      for (const char of part) result.push({ char, bg, fg });
      continue;
    }
    const codes = part.slice(2, -1).split(";").map(Number);
    for (let i = 0; i < codes.length; i++) {
      const code = codes[i];
      if (code === 0) { bg = ""; fg = ""; }
      else if (code === 49) bg = "";
      else if (code === 39) fg = "";
      else if (code === 38 || code === 48) {
        const count = codes[i + 1] === 2 ? 5 : 3;
        const color = `\x1b[${codes.slice(i, i + count).join(";")}m`;
        if (code === 48) bg = color;
        else fg = color;
        i += count - 1;
      }
    }
  }
  return result;
}

function expectOutline(lines: string[], tone: ThemeColor) {
  const rows = lines.filter(Boolean).map(cells);
  const border = theme.getFgAnsi(tone);
  for (const row of rows) {
    expect(row.every((cell) => cell.bg === "")).toBe(true);
    expect(row[0].fg).toBe(border);
    expect(row.at(-1)!.fg).toBe(border);
  }
  expect(rows[0].every((cell) => cell.fg === border)).toBe(true);
  expect(rows.at(-1)!.every((cell) => cell.fg === border)).toBe(true);
}

function tool(name: string, args: unknown = {}, base?: ToolRenderers, outputPad = 1) {
  const renderers = roundToolRenderers(name, base);
  return new ToolExecutionComponent(name, `test-${name}`, args, { outputPad }, {
    ...renderers,
    renderCall(args, currentTheme, ctx) {
      theme = currentTheme;
      return renderers.renderCall!(args, currentTheme, ctx);
    },
  }, { requestRender: mock(() => {}) } as never, process.cwd());
}

function click(width: number, height: number, x: number, y: number): TuiMouseEvent {
  return { type: "click", button: "left", x, y, screenX: x, screenY: y, width, height,
    shift: false, alt: false, ctrl: false };
}

function expectOneFrame(lines: string[], width: number) {
  const text = plain(lines);
  expect(text.filter((line) => /^╭─── .+ ─+╮$/.test(line))).toHaveLength(1);
  expect(text.filter((line) => line === `╰${"─".repeat(width - 2)}╯`)).toHaveLength(1);
  for (const line of text.filter(Boolean)) {
    expect(visibleWidth(line)).toBe(width);
    expect(line).toMatch(/^(?:╭─── .+ ─+╮|│.*│|╰─+╯)$/);
  }
}

test("registers a renderer resolver without overriding tools or execution", () => {
  let resolver!: ToolRendererResolver;
  const registerMessageRenderer = mock(() => {});
  roundedTools({ on() {}, registerToolRenderer: (value: ToolRendererResolver) => { resolver = value; }, registerMessageRenderer } as never);
  expect(registerMessageRenderer).not.toHaveBeenCalled();
  const next = mock(() => ({ renderCall: () => new Text("custom", 0, 0) }));
  expect(resolver("mcp__late_tool", next)?.renderShell).toBe("self");
  expect(next).toHaveBeenCalledTimes(1);
});

describe("rounded tool composition", () => {
  test("leaves the thin outline unfilled across states and themes", () => {
    for (const pad of [0, 1]) {
      const row = tool("custom", {}, undefined, pad);
      for (const name of ["light", "dark"]) {
        initTheme(name, false);
        row.invalidate();
        row.updateResult(textResult("running"), true);
        expectOutline(row.render(60), "warning");
        row.updateResult(textResult("done"));
        expectOutline(row.render(60), "success");
        row.updateResult({ ...textResult("failed"), isError: true });
        expectOutline(row.render(60), "error");
      }
    }
  });

  test("leaves self-shell gaps unpainted and preserves explicit backgrounds", () => {
    const row = tool("custom", {}, {
      renderShell: "self",
      renderCall: (_args, theme) => new Text(
        `before\x1b[0mafter\x1b[m reset\x1b[49m background\n${theme.bg("toolErrorBg", "inner")} tail`, 0, 0),
    });
    row.updateResult(textResult("done"));
    const lines = row.render(60);
    expectOutline([lines[1], lines[2], lines.at(-1)!], "success");
    const nested = cells(lines[3]);
    expect(nested.slice(1, 6).every((cell) => cell.bg === theme.getBgAnsi("toolErrorBg"))).toBe(true);
    expect(nested.slice(6, -1).every((cell) => cell.bg === "")).toBe(true);
    expect(nested[0].bg).toBe("");
    expect(nested.at(-1)!.bg).toBe("");
    expectOneFrame(lines, 60);
  });

  test("keeps a single closed frame before, during, and after results", () => {
    const row = tool("custom", { query: "한글 😀" });
    expectOneFrame(row.render(60), 60);
    row.updateResult(textResult("partial"), true);
    expectOneFrame(row.render(60), 60);
    expect(plain(row.render(60)).join("\n")).toContain("partial");
    row.updateResult(textResult("finished"), false);
    expectOneFrame(row.render(60), 60);
    expect(plain(row.render(60)).join("\n")).not.toContain("partial");
  });

  test("reuses an unchanged frame across renders and refreshes on content, state, theme, and width changes", () => {
    const row = tool("custom");
    row.render(60);
    const renderers = roundToolRenderers("custom");
    const ctx = { state: {}, isPartial: false, isError: false, outputPad: 1, expanded: false, keyHint: () => "" };
    const frame = renderers.renderCall!({}, theme, ctx as never);
    renderers.renderResult!(textResult("done"), {} as never, theme, ctx as never);
    expect(frame.render(60)).toBe(frame.render(60));
    row.updateResult(textResult("changed"));
    expect(plain(row.render(60)).join("\n")).toContain("changed");
    row.updateResult({ ...textResult("failed"), isError: true });
    expectOutline(row.render(60), "error");
    initTheme("light", false);
    row.invalidate();
    expectOutline(row.render(60), "error");
    expectOneFrame(row.render(40), 40);
  });

  test("keeps delegate state and lastComponent separate for both slots and tools", () => {
    const calls: unknown[] = [];
    const results: unknown[] = [];
    const states = new Set<object>();
    const base: ToolRenderers = {
      renderCall(_args, _theme, ctx) {
        states.add(ctx.state);
        calls.push(ctx.lastComponent);
        expect(ctx.lastComponent === undefined || ctx.lastComponent instanceof Text).toBe(true);
        const text = ctx.lastComponent as Text | undefined ?? new Text("call", 0, 0);
        ctx.state.call = text;
        return text;
      },
      renderResult(_result, _options, _theme, ctx) {
        results.push(ctx.lastComponent);
        expect(ctx.state.call).toBeInstanceOf(Text);
        expect(ctx.lastComponent === undefined || ctx.lastComponent instanceof Container).toBe(true);
        return ctx.lastComponent ?? new Container();
      },
    };
    const wrapped = roundToolRenderers("custom", base);
    const create = () => new ToolExecutionComponent("custom", "id", {}, {}, wrapped,
      { requestRender() {} } as never, process.cwd());
    const a = create();
    const b = create();
    a.updateResult(textResult("first"));
    a.setExpanded(true);
    b.updateResult(textResult("second"));
    expect(states.size).toBe(2);
    expect(calls.filter(Boolean).length).toBeGreaterThan(0);
    expect(results.filter(Boolean).length).toBeGreaterThan(0);
    expectOneFrame(a.render(60), 60);
    expectOneFrame(b.render(60), 60);
  });

  test("keeps intentionally empty renderers invisible", () => {
    const row = tool("hidden", {}, {
      renderCall: () => new Container(), renderResult: () => new Container(),
    });
    expect(row.render(50)).toEqual([]);
    row.updateResult(textResult("not displayed"));
    expect(row.render(50)).toEqual([]);
  });

  test("shows only the first bash command line until the block expands", () => {
    const args = { command: "set -eu\r\nprintf second\nprintf third", timeout: 15 };
    const row = tool("bash", args, createBashToolDefinition(process.cwd()));
    const render = () => plain(row.render(60)).join("\n");
    expect(render()).toContain("$ set -eu … (timeout 15s)");
    expect(render()).not.toContain("printf");
    row.updateResult(textResult("command output"));
    expect(render()).toContain("command output");
    const lines = row.render(60);
    expect(row.handleMouse(click(60, lines.length, 5, 2))?.handled).toBe(true);
    expect(render()).toContain("printf second");
    expect(render()).toContain("printf third");
    row.setExpanded(false);
    expect(render()).not.toContain("printf");
    row.setExpanded(true);
    expect(render()).toContain("printf third");
  });

  test("preserves expansion through clicks and explicit keyboard-driven updates", () => {
    const row = tool("custom");
    row.updateResult(textResult(Array.from({ length: 20 }, (_, i) => `line-${i}`).join("\n")));
    let lines = row.render(60);
    expect(plain(lines).join("\n")).not.toContain("line-19");
    expect(row.handleMouse(click(60, lines.length, 5, 3))?.handled).toBe(true);
    lines = row.render(60);
    expect(plain(lines).join("\n")).toContain("line-19");
    expectOneFrame(lines, 60);
    row.setExpanded(false);
    expect(plain(row.render(60)).join("\n")).not.toContain("line-19");
  });

  test("forwards child mouse coordinates and retains OSC 8 file links", () => {
    const events: TuiMouseEvent[] = [];
    const row = tool("custom", {}, {
      renderShell: "self",
      renderCall: () => ({
        render: () => [hyperlink("example.ts", "file:///tmp/example.ts")],
        invalidate() {},
        handleMouse(event) { events.push(event); return { handled: true, capture: true }; },
      }),
    });
    const lines = row.render(40);
    expect(getOsc8LinkAtColumn(lines[2], 1)).toBe("file:///tmp/example.ts");
    const result = row.handleMouse(click(40, lines.length, 3, 2));
    expect(result?.capture).toBe(true);
    expect(events[0]).toMatchObject({ x: 2, y: 0, width: 38, height: 1 });
  });

  test("leaves custom inline graphics unframed to preserve terminal image placement", () => {
    const graphics = "\x1b_Ga=T,f=100;AAAA\x1b\\";
    const row = tool("graphics", {}, {
      renderShell: "self",
      renderCall: () => ({ render: () => [graphics], invalidate() {} }),
    });
    expect(row.render(50)).toEqual(["", graphics]);
  });

  test("handles narrow panes, ANSI, wide text, theme changes, and both padding settings", () => {
    for (const pad of [0, 1]) {
      const row = tool("custom", { path: "한글/😀.ts" }, undefined, pad);
      row.updateResult(textResult("\x1b[31m한글 😀\x1b[0m ".repeat(25)));
      for (let width = 0; width <= 120; width++) {
        for (const line of row.render(width)) expect(visibleWidth(line)).toBeLessThanOrEqual(width);
      }
      const dark = row.render(60);
      initTheme("light", false);
      row.invalidate();
      expectOneFrame(row.render(60), 60);
      expect(row.render(60)).not.toEqual(dark);
      initTheme("dark", false);
    }
  });

  test("reports renderer failures, shows plain output, and can recover", () => {
    let fail = true;
    const row = tool("custom", {}, {
      renderCall: () => { if (fail) throw new Error("call failure"); return new Text("recovered", 0, 0); },
      renderResult: () => { if (fail) throw new Error("result failure"); return new Container(); },
    });
    row.updateResult(textResult("fallback result"));
    const failed = plain(row.render(80)).join("\n");
    expect(failed).toContain("call renderer failed");
    expect(failed).toContain("result renderer failed");
    expect(failed).toContain("fallback result");
    fail = false;
    row.invalidate();
    expect(plain(row.render(80)).join("\n")).toContain("recovered");
    expect(plain(row.render(80)).join("\n")).not.toContain("renderer failed");
  });
});

describe("installed Pi renderers", () => {
  test("tints only the completed and file-error markers with their outline color", () => {
    const todo = tool("todo", {}, {
      renderCall: (_args, t) => new Text(t.fg("toolTitle", t.bold("todo ")) + "Update task", 0, 0),
      renderResult: (_result, _options, t) => new Text(t.fg("success", "● completed"), 0, 0),
    });
    todo.updateResult(textResult("done"));
    const read = tool("read", { path: "missing.ts" }, createReadToolDefinition(process.cwd()));
    read.updateResult({ ...textResult("File not found"), isError: true });
    for (const [row, mark, label, name] of [[todo, "●", "completed", "todo"], [read, "×", "File not found", "read"]] as const) {
      const lines = row.render(60);
      const header = lines.find(line => stripTerminalSequences(line).startsWith(`╭─── ${name} `))!;
      const status = cells(lines.find(line => stripTerminalSequences(line).includes(`${mark} ${label}`))!);
      const i = status.findIndex(cell => cell.char === mark);
      expect(status[i].fg).toBe(cells(header)[0].fg);
      expect(status[i + 2].fg).not.toBe(status[i].fg);
      expect(plain(lines).filter(line => line.includes(name))).toHaveLength(1);
    }
  });

  test("preserves read highlighting, file links, and collapsed output", () => {
    const row = tool("read", { path: "/tmp/sample.ts" }, createReadToolDefinition(process.cwd()));
    row.updateResult(textResult("const answer = 42;"));
    expect(plain(row.render(70)).join("\n")).not.toContain("const answer");
    row.setExpanded(true);
    expectOneFrame(row.render(70), 70);
    expect(plain(row.render(70)).join("\n")).toContain("const answer = 42;");
    expect(row.render(70).join("\n")).toContain("\x1b]");
  });

  test("preserves edit's shared call/result diff state without executing an edit", () => {
    const row = tool("edit", { path: "/tmp/sample.ts", edits: [{ oldText: "old", newText: "new" }] },
      createEditToolDefinition(process.cwd()));
    row.updateResult({ ...textResult("updated"), details: { diff: "-1 old\n+1 new", firstChangedLine: 1 } });
    expectOneFrame(row.render(70), 70);
    expect(plain(row.render(70)).join("\n")).toContain("old");
    expect(plain(row.render(70)).join("\n")).toContain("new");
    row.invalidate();
    expectOneFrame(row.render(70), 70);
  });

  test("preserves codemode scripts, nested calls, and final output", () => {
    let base!: ToolRenderers;
    createCodemodeExtension()({ registerTool: (definition: ToolRenderers) => { base = definition; } } as never);
    const row = tool("codemode", { code: 'text(await tools.read({path: "sample.ts"}));' }, base);
    row.updateResult({ ...textResult(""), details: {
      calls: [{ name: "read", args: "sample.ts", status: "running" }],
    } }, true);
    expectOneFrame(row.render(70), 70);
    expect(plain(row.render(70)).filter(line => line.includes("codemode"))).toEqual([`╭─── codemode ${"─".repeat(55)}╮`]);
    expect(plain(row.render(70)).join("\n")).toContain("read sample.ts");
    expectOutline(row.render(70), "warning");
    row.updateResult({ ...textResult("file contents"), details: {
      calls: [{ name: "read", args: "sample.ts", status: "ok", durationMs: 12 }],
    } });
    expectOneFrame(row.render(70), 70);
    expect(plain(row.render(70)).join("\n")).toContain("file contents");
    expect(plain(row.render(70)).join("\n")).toContain("12ms");
    expectOutline(row.render(70), "success");
  });

  test("leaves native images outside the text frame without duplicating them", () => {
    setCapabilities({ ...capabilities, images: "kitty" });
    const row = tool("read", { path: "image.png" }, createReadToolDefinition(process.cwd()));
    row.updateResult({ content: [{ type: "image", mimeType: "image/png",
      data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j7uoAAAAASUVORK5CYII=",
    }], details: undefined });
    const lines = row.render(70);
    const bottom = plain(lines).findIndex((line) => line.startsWith("╰"));
    const image = lines.findIndex((line) => line.includes("\x1b_G"));
    expect(image).toBeGreaterThan(bottom);
    expect(lines.filter((line) => line.includes("\x1b_G"))).toHaveLength(1);
  });
});

test("loads once through Pi's Node/jiti extension discovery", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-rounded-tools-"));
  roots.push(root);
  const agent = join(root, "agent");
  await mkdir(join(agent, "extensions"), { recursive: true });
  await symlink(fileURLToPath(new URL("../files/extensions/rounded-tools.ts", import.meta.url)),
    join(agent, "extensions/rounded-tools.ts"));
  const entry = fileURLToPath(import.meta.resolve("@earendil-works/pi-coding-agent"));
  const result = spawnSync("node", ["--input-type=module", "-e", `
    import { pathToFileURL } from "node:url";
    const { discoverAndLoadExtensions } = await import(pathToFileURL(process.argv[1]).href);
    const loaded = await discoverAndLoadExtensions([], process.argv[2], process.env.PI_CODING_AGENT_DIR);
    console.log(JSON.stringify({ errors: loaded.errors, count: loaded.extensions.length,
      renderers: loaded.extensions.flatMap(e => e.toolRenderers ?? []).length }));
  `, entry, root], { encoding: "utf8", timeout: 30_000,
    env: { ...process.env, PI_CODING_AGENT_DIR: agent, JITI_FS_CACHE: "0" },
  });
  if (result.error) throw result.error;
  expect(result.status).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual({ errors: [], count: 1, renderers: 1 });
}, 60_000);

test("an unpatched Pi gets the Home Manager notice only where installing the patched Pi could add message frames", () => {
  const saved = process.env.PI_UNPATCHED;
  const sessionHandlers = (unpatched: string | undefined) => {
    if (unpatched === undefined) delete process.env.PI_UNPATCHED;
    else process.env.PI_UNPATCHED = unpatched;
    const events: string[] = [];
    roundedTools({ registerToolRenderer: () => {}, on: (event: string) => events.push(event) } as never);
    return events;
  };
  try {
    expect(sessionHandlers(undefined)).toEqual(["session_start"]);
    expect(sessionHandlers("1")).toEqual([]);
  } finally {
    if (saved === undefined) delete process.env.PI_UNPATCHED;
    else process.env.PI_UNPATCHED = saved;
  }
});
