import { exec, run, terminals } from "herdr";

export async function handleLink({ link, source }) {
  const terminal = terminals().find(t => t.address === source.terminal);
  const snapshot = terminal?.labels?.["pi.subagents"];
  if (!snapshot || !source.cwd || link.kind !== "file" || !source.pane) return "pass";
  const metadata = JSON.parse(snapshot);
  const session = metadata.editorSession ?? metadata.session;
  const id = source.terminal.split("/").pop();
  if (!/^t[0-9a-z]+$/.test(id) || !/^[a-f0-9-]{36}$/.test(session)) return "pass";
  const result = await exec("pi-session-editor", [id, session, link.raw], { cwd: source.cwd, on: source.machine });
  if (result.code === 2) return "pass";
  if (result.code !== 0) throw new Error(result.stderr.trim() || "Could not open the Pi session editor");
  const address = result.stdout.trim();
  if (!/^t[0-9a-z]+$/.test(address)) throw new Error("Invalid editor terminal address");
  run("terminal.peek", { terminal: source.machine ? `${source.machine}/${address}` : address });
  return "handled";
}
