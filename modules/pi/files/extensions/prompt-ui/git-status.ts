import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export function formatGitStatus(output: string): string {
  let unstaged = 0, staged = 0, untracked = 0;
  const records = output.split("\0");
  for (let i = 0; i < records.length; i++) {
    const xy = records[i].slice(0, 2);
    if (xy.length !== 2 || xy === "!!") continue;
    if (xy === "??") {
      untracked++;
      continue;
    }
    // Unmerged paths are unresolved work, not successfully staged changes.
    if (xy.includes("U") || xy === "AA" || xy === "DD") unstaged++;
    else {
      if (xy[0] !== " ") staged++;
      if (xy[1] !== " ") unstaged++;
    }
    // Porcelain -z emits the original name as a second record for renames/copies.
    if (/[RC]/.test(xy)) i++;
  }
  return [unstaged ? `*${unstaged}` : "", staged ? `+${staged}` : "", untracked ? `?${untracked}` : ""]
    .filter(Boolean).join(" ");
}

export function watchGitStatus(
  exec: ExtensionAPI["exec"],
  cwd: string,
  update: (status: string) => void,
  warn: (message: string) => void,
  intervalMs = 2000,
): () => void {
  const controller = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let lastError = "";
  const refresh = async () => {
    try {
      const result = await exec("git", ["--no-optional-locks", "-C", cwd, "status", "--porcelain=v1", "-z", "--untracked-files=all"], {
        signal: controller.signal, timeout: 5000, maxBuffer: 4 * 1024 * 1024,
      });
      if (controller.signal.aborted) return;
      if (result.code !== 0 || result.killed || result.truncated) {
        if (!result.killed && !result.truncated && /not a git repository/i.test(result.stderr)) {
          lastError = "";
          update("");
          return;
        }
        throw new Error(result.killed || result.truncated ? "Git status exceeded its time or output limit" : result.stderr.trim() || `Git status exited with code ${result.code}`);
      }
      lastError = "";
      update(formatGitStatus(result.stdout));
    } catch (error) {
      if (controller.signal.aborted) return;
      update("git!");
      const message = `Git status (${cwd}): ${error instanceof Error ? error.message : String(error)}`
        .replace(/[\x00-\x1f\x7f-\x9f]/g, " ").slice(0, 500);
      if (message !== lastError) warn(message);
      lastError = message;
    } finally {
      if (!controller.signal.aborted) {
        timer = setTimeout(() => { void refresh(); }, intervalMs);
        timer.unref();
      }
    }
  };
  void refresh();
  return () => {
    controller.abort();
    clearTimeout(timer);
  };
}
