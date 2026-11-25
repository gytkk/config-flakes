import { exec, run, terminals } from "herdr";

export async function handleLink({ link, source }) {
  if (link.kind !== "file" || !source.pane || !source.cwd) return "pass";
  const terminal = terminals().find(t => t.address === source.terminal);
  if (terminal?.report?.app !== "pi" && terminal?.program?.key !== "pi") return "pass";
  const result = await exec("pi-session-editor", [link.raw], { cwd: source.cwd, on: source.machine });
  if (result.code === 2) return "pass";
  if (result.code !== 0) throw new Error(result.stderr.trim() || "Could not resolve the file on the Pi host");
  const { path, line } = JSON.parse(result.stdout);
  if (typeof path !== "string" || !path.startsWith("/") || !Number.isSafeInteger(line) || line < 1) {
    throw new Error("Invalid file location from the Pi host");
  }
  run("pane.split", { pane: source.pane, direction: "right", argv: ["nvim", `+${line}`, "--", path] });
  return "handled";
}
