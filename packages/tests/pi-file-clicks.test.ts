import { describe, expect, test } from "bun:test";
import { pathToFileURL } from "node:url";

// Point to packages/tui/src in a source tree with the local patches applied.
const source = process.env.PI_TUI_SOURCE;

describe.skipIf(!source)("patched fullscreen file-link mouse routing", async () => {
  if (!source) return;
  const { TuiAltScreen } = await import(pathToFileURL(`${source}/tui-alt-screen.ts`).href);

  function screen(url: string, tool = "") {
    const prefix = tool ? `${tool} ` : "";
    const opened: string[] = [];
    const clicks: string[] = [];
    const tui = new TuiAltScreen({ columns: 80, rows: 3 }, undefined, undefined, {
      openUrl: (target: string) => opened.push(target),
      copyOnSelect: false,
    });
    // Keep the actual mouse and selection handlers, without a terminal render loop.
    tui.previousScreen = [`${prefix}\x1b]8;;${url}\x1b\\file.ts\x1b]8;;\x1b\\`];
    tui.requestRender = () => {};
    tui.dispatchMouseToLayout = (event: { type: string }) => {
      if (event.type === "click") clicks.push(event.type);
    };
    const mouse = (button: number, release = false, x = prefix.length + 1) =>
      tui.handleMouseEvent({ button, release, x, y: 0 });
    return { opened, clicks, mouse };
  }

  for (const url of ["file:///tmp/file.ts", "FILE:///tmp/file.ts"]) {
    for (const modifier of [0, 4, 8, 16, 20, 24]) {
      for (const release of [0, 3]) {
        test(`${url}: modifier ${modifier}, release ${release}`, () => {
          const { opened, clicks, mouse } = screen(url);
          mouse(modifier);
          mouse(modifier | release, true);
          expect(opened).toEqual(modifier & 16 ? [url] : []);
          // Block the default opener and enclosing click handlers on plain clicks.
          expect(clicks).toEqual([]);
        });
      }
    }
  }

  for (const tool of ["read", "edit"]) {
    for (const modifier of [0, 16]) {
      test(`${tool} heading requires Ctrl to open its file path (modifier ${modifier})`, () => {
        const url = "file:///tmp/file.ts";
        const { opened, clicks, mouse } = screen(url, tool);
        mouse(modifier);
        mouse(modifier, true);
        expect(opened).toEqual(modifier === 16 ? [url] : []);
        expect(clicks).toEqual([]);
      });
    }
  }

  test("Ctrl-drag does not open a file", () => {
    const { opened, mouse } = screen("file:///tmp/file.ts");
    mouse(16);
    mouse(48, false, 4);
    mouse(16, true, 4);
    expect(opened).toEqual([]);
  });

  test("plain web-link clicks still open", () => {
    const url = "https://example.com";
    const { opened, mouse } = screen(url);
    mouse(0);
    mouse(0, true);
    expect(opened).toEqual([url]);
  });
});
