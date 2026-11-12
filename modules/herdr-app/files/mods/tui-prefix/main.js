import { View, div } from "gpui-kit";
import { v_flex, h_flex, InputState, Input } from "gpui-base";
import { agents, spaces, panes, focused, theme, run, exec, props, closeOverlay, openOverlay, nextCommand } from "herdr";

const ACTIONS = {
  c: ["tab.new"], X: ["tab.close"], p: ["tab.prev"], n: ["tab.next"],
  T: ["tab.rename"], W: ["space.rename"], D: ["space.close"],
  "<": ["agent.prev"], ">": ["agent.next"],
  h: ["pane.go", { direction: "left" }], j: ["pane.go", { direction: "down" }],
  k: ["pane.go", { direction: "up" }], l: ["pane.go", { direction: "right" }],
  H: ["pane.swap", { direction: "left" }], J: ["pane.swap", { direction: "down" }],
  K: ["pane.swap", { direction: "up" }], L: ["pane.swap", { direction: "right" }],
  v: ["pane.split", { direction: "right" }], "-": ["pane.split", { direction: "down" }],
  x: ["pane.close"], z: ["pane.zoom"], b: ["layout.toggle", { id: "sidebar" }],
  s: ["app.settings"],
};
const HELP = [
  ["c / X / T", "New / close / rename tab"],
  ["p / n / 1–9", "Previous / next / numbered tab"],
  ["S / N", "Select space / new space from zoxide"],
  ["W / D / G", "Rename / close space / new worktree"],
  ["A / < / >", "Select / previous / next agent"],
  ["Shift+1–9", "Agent number in the agent picker"],
  ["h j k l / Shift", "Focus pane / swap pane"],
  ["Tab / Shift+Tab", "Next / previous pane"],
  ["v / - / x / z", "Split right / split down / close / zoom"],
  ["b / s", "Sidebar / settings"],
];
const SHIFTED = { ",": "<", ".": ">", "/": "?", "1": "!", "2": "@", "3": "#", "4": "$", "5": "%", "6": "^", "7": "&", "8": "*", "9": "(" };

function mixColors(a, b, amount) {
  return "#" + [1, 3, 5].map(i => {
    const from = parseInt(a.slice(i, i + 2), 16);
    const to = parseInt(b.slice(i, i + 2), 16);
    return Math.round(from + (to - from) * amount).toString(16).padStart(2, "0");
  }).join("");
}

function badgeText(background) {
  const linear = [1, 3, 5].map(i => parseInt(background.slice(i, i + 2), 16) / 255)
    .map(c => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const luminance = linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
  return luminance > 0.179 ? "#000000" : "#ffffff";
}

function keyOf(event) {
  if (event.modifiers.platform || event.modifiers.alt || event.modifiers.control) return event.keystroke;
  const key = event.key;
  if (!event.modifiers.shift) return key;
  return SHIFTED[key] ?? (key.length === 1 ? key.toUpperCase() : `shift-${key}`);
}

export default class Prefix extends View {
  init(_props, cx) {
    this.overlay = props()?.overlay;
    this.mode = "idle";
    this.query = "";
    this.at = 0;
    this.error = "";
    this.busy = false;
    this.handle = cx.focus_handle();
    this.input = InputState.new({ placeholder: "Type to filter…" });
    this.input.on("change", (_e, cx) => {
      this.query = this.input.value();
      this.at = 0;
      cx.notify();
    });
    this.input.on("submit", () => this.choose(this.rows()[this.at]));
    if (this.overlay) {
      this.capture();
      if (props().mode === "help") {
        this.mode = "help";
        this.handle.focus();
      } else this.pick(props().mode, cx);
    } else {
      cx.spawn(async cx => {
        for (;;) {
          const command = await nextCommand();
          if (command === "activate") {
            if (this.mode === "prefix") {
              this.dismiss();
              cx.notify();
              continue;
            }
            this.capture();
            this.mode = "prefix";
            this.handle.focus();
            this.focusWatch?.cancel();
            this.focusWatch = cx.timer.every(100, cx => {
              if (!this.handle.is_focused()) {
                this.mode = "idle";
                this.focusWatch.cancel();
                cx.notify();
              }
            });
            cx.notify();
          }
        }
      });
    }
  }

  capture() {
    this.origin = focused();
    this.machine = spaces().find(s => s.id === this.origin?.space)?.machine ?? "";
    this.agentRows = agents().map(a => ({ label: `${a.program?.name ?? "Agent"} · ${a.title || a.address}`, command: "agent.focus", args: { terminal: a.address } }));
  }

  dismiss() {
    this.focusWatch?.cancel();
    if (this.overlay) closeOverlay(this.overlay);
    else {
      this.mode = "idle";
      if (this.origin?.pane) run("pane.focus", { pane: this.origin.pane });
      else if (this.origin?.terminal) run("pane.focus", { terminal: this.origin.terminal });
    }
  }

  execute(command, args = {}) {
    this.dismiss();
    run(command, args);
  }

  rows() {
    const words = this.query.toLowerCase().split(/\s+/).filter(Boolean);
    return (this.items ?? []).filter(row => words.every(word => row.label.toLowerCase().includes(word)));
  }

  choose(row) {
    if (row && !this.busy) this.execute(row.command, row.args);
  }

  pick(mode, cx) {
    if (!this.overlay) {
      this.dismiss();
      openOverlay({ mod: "tui-prefix", kind: "dialog", width: 640, props: { mode } });
      return;
    }
    this.mode = mode;
    this.at = 0;
    this.query = "";
    this.error = "";
    this.items = mode === "agents" ? this.agentRows : mode === "spaces"
      ? spaces().map(s => ({ label: s.name, command: "space.select", args: { id: s.id } })) : [];
    this.input.focus();
    cx.notify();
    if (mode !== "directories") return;
    this.busy = true;
    cx.spawn(async cx => {
      try {
        const result = await exec("zoxide", ["query", "-l"], { on: this.machine || null });
        if (result.code !== 0) throw new Error(result.stderr.trim() || "zoxide query failed");
        this.items = result.stdout.split(/\r?\n/).filter(Boolean).map(folder => ({
          label: folder,
          command: "space.new",
          args: {
            name: folder.replace(/\/$/, "").split("/").pop() || folder,
            machine: this.machine,
            folder,
          },
        }));
      } catch (error) {
        this.error = `Cannot load zoxide directories: ${error.message}`;
      }
      this.busy = false;
      cx.notify();
    });
  }

  key(event, cx) {
    if (this.mode === "idle") return;
    // Consume the prefix's second key even when unknown; never leak it into a shell.
    if (this.mode !== "prefix") {
      if (event.key !== "up" && event.key !== "down") return;
      cx.stop_propagation();
      const count = this.rows().length;
      if (count) this.at = (this.at + (event.key === "down" ? 1 : count - 1)) % count;
      cx.notify();
      return;
    }
    cx.stop_propagation();
    if (event.is_held) return;
    const key = keyOf(event);
    if (key === "escape" || key === "ctrl-space") return this.dismiss();
    if (ACTIONS[key]) return this.execute(...ACTIONS[key]);
    if (/^[1-9]$/.test(key)) return this.execute("tab.select", { index: Number(key) });
    const agentIndex = "!@#$%^&*(".indexOf(key);
    if (key.length === 1 && agentIndex >= 0) return this.choose(this.agentRows[agentIndex]);
    if (key === "S") return this.pick("spaces", cx);
    if (key === "A") return this.pick("agents", cx);
    if (key === "N") return this.pick("directories", cx);
    if (key === "G") {
      this.dismiss();
      openOverlay({ mod: "worktree", kind: "dialog", width: 640, props: { space: this.origin?.space, action: "new" } });
      return;
    }
    if (key === "tab" || key === "shift-tab") {
      const list = panes().filter(p => p.tab === this.origin?.tab);
      const at = list.findIndex(p => p.pane === this.origin?.pane);
      const next = list[(at + (key === "tab" ? 1 : list.length - 1)) % list.length];
      if (next) this.execute("pane.focus", { pane: next.pane });
      return;
    }
    if (key === "?") return this.pick("help", cx);
    this.dismiss();
  }

  render() {
    const t = theme();
    if (!this.overlay) {
      const active = this.mode === "prefix";
      const prefixAccent = mixColors(t.ansi3, "#ffb300", 0.55);
      const space = spaces().find(s => s.active);
      const bar = h_flex().id("tui-prefix-bar").track_focus(this.handle).size_full()
        .px(14).gap(16).items_center().overflow_hidden().border_t(1).border_color(active ? prefixAccent : t.border)
        .bg(active ? mixColors(t.chrome, prefixAccent, 0.15) : t.chrome).font_family("JetBrains Mono").text_color(t.foreground).text_size(12)
        .on_key_down((event, cx) => { this.key(event, cx); cx.notify(); });
      if (active) {
        const shortcut = (key, label) => h_flex().flex_none().gap(6).items_center()
          .child(div().h(18).px(5).flex().items_center().rounded(3).bg(t.surface)
            .border_1().border_color(t.border).font_semibold().text_size(11).child(key))
          .child(div().text_size(11.5).child(label));
        bar.child(div().h(18).px(7).flex_none().flex().items_center().rounded(3)
          .bg(prefixAccent).text_color(badgeText(prefixAccent)).font_semibold().text_size(10.5).child("PREFIX"));
        bar.child(h_flex().flex_1().min_w(0).gap(18).items_center().overflow_hidden()
          .child(shortcut("c", "New tab"))
          .child(shortcut("p / n", "Tabs"))
          .child(shortcut("S", "Spaces"))
          .child(shortcut("N", "New space"))
          .child(shortcut("A", "Agents"))
          .child(shortcut("v / -", "Split")));
        bar.child(shortcut("?", "Help"))
          .child(div().id("prefix-cancel").cursor_pointer().rounded(3)
            .hover(style => style.bg(t.hover)).on_click((_e, cx) => { this.dismiss(); cx.notify(); })
            .child(shortcut("Esc", "Cancel")));
      } else {
        bar.child(div().truncate().child([space?.machine || "This computer", space?.branch, `${agents().length} agents`].filter(Boolean).join("  ·  ")))
          .child(div().flex_1())
          .child(div().child("Ctrl+Space  prefix"));
      }
      return bar;
    }
    const help = this.mode === "help";
    const title = help ? "Prefix shortcuts" : { spaces: "Select space", agents: "Select agent", directories: "New space — zoxide" }[this.mode];
    const root = v_flex().id("tui-prefix-picker").track_focus(this.handle).w_full().p(18).gap(10)
      .bg(t.popover).text_color(t.foreground).text_size(13)
      .on_key_down((event, cx) => { this.key(event, cx); cx.notify(); })
      .child(h_flex().items_center().gap(12)
        .child(div().flex_1().text_size(16).font_semibold().child(title))
        .child(div().id("prefix-close").cursor_pointer().px(6).py(4)
          .on_click(() => this.dismiss()).child("Esc")));
    if (help) {
      root.children(HELP.map(([key, label]) => h_flex().gap(12)
        .child(div().w(145).flex_none().font_semibold().child(key))
        .child(div().flex_1().child(label))));
      root.child(div().child("No timeout · Ctrl+Space again cancels prefix mode"));
    } else {
      root.child(Input.new(this.input).w_full().h(36));
      const rows = this.rows();
      this.at = Math.max(0, Math.min(this.at, rows.length - 1));
      const start = Math.max(0, this.at - 9);
      root.children(rows.slice(start, start + 10).map((row, offset) => div().id(`prefix-row-${start + offset}`)
        .px(8).py(6).rounded(4).truncate().cursor_pointer()
        .when(start + offset === this.at, el => el.bg(t.selected))
        .hover(s => s.bg(t.hover)).on_click(() => this.choose(row))
        .child(this.mode === "agents" ? `${start + offset + 1}  ${row.label}` : row.label)));
      if (!rows.length) root.child(div().child(this.busy ? "Loading directories…" : "No matches"));
      root.child(div().child("↑ / ↓ select · Enter open · Esc cancel"));
    }
    if (this.error) root.child(div().child(this.error));
    return root;
  }
}
