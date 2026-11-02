import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
type Run = (binary: string, args: string[]) => Promise<string>;

const runCommand: Run = async (binary, args) => {
  const { stdout } = await execFileAsync(binary, args, { timeout: 10_000, maxBuffer: 1024 * 1024 });
  return stdout;
};

export async function openHerdrFile(
  target: string,
  { env = process.env, cwd = process.cwd(), run = runCommand }:
    { env?: NodeJS.ProcessEnv; cwd?: string; run?: Run } = {},
): Promise<boolean> {
  if (env.HERDR_ENV !== "1" || !env.HERDR_PANE_ID || process.platform === "win32" || !/^file:/i.test(target)) {
    return false;
  }

  const url = new URL(target);
  if (url.hostname && url.hostname !== "localhost") throw new Error("Only local file links are supported");
  if (url.search || url.hash) throw new Error("File links with queries or fragments are not supported");
  decodeURIComponent(url.pathname);
  const path = fileURLToPath(url);
  if (/[\x00-\x1f\x7f]/.test(path)) throw new Error("File links must not contain control characters");

  const binary = env.HERDR_BIN_PATH || "herdr";
  // Check the editor before creating a pane that would immediately fail to start.
  await run("nvim", ["--version"]);
  const response = JSON.parse(await run(binary, [
    "pane", "split", "--current", "--direction", "right", "--cwd", cwd, "--focus",
  ]));
  const pane = response?.result?.pane?.pane_id;
  if (typeof pane !== "string" || !pane) throw new Error("Herdr split returned no pane ID");

  // pane run accepts shell text, not argv. Quote the decoded path as one literal argument.
  const quotedPath = `'${path.replaceAll("'", "'\\''")}'`;
  try {
    await run(binary, ["pane", "run", pane, `exec nvim -- ${quotedPath}`]);
  } catch (error) {
    throw new Error(`Could not start Neovim in new Herdr pane ${pane}; the pane was left open`, { cause: error });
  }
  return true;
}
