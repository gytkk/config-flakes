import { lstat, readdir } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

function context(error: unknown, action: string): Error {
  const detail = error instanceof Error ? error.message : String(error);
  return new Error(`${action}: ${detail}`, { cause: error });
}

function isCode(error: unknown, code: string): boolean {
  return typeof error === "object" && error !== null && "code" in error
    && (error as NodeJS.ErrnoException).code === code;
}

async function entries(path: string) {
  try {
    const stat = await lstat(path);
    if (!stat.isDirectory() || stat.isSymbolicLink()) return [];
    return await readdir(path, { withFileTypes: true });
  } catch (error) {
    if (isCode(error, "ENOENT")) return [];
    throw context(error, `cannot scan rollout directory ${path}`);
  }
}

function within(root: string, candidate: string): boolean {
  const path = relative(root, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function safeDirectory(root: string, candidate: string): Promise<boolean> {
  if (!within(root, candidate)) return false;
  let current = root;
  const paths = [root];
  for (const part of relative(root, candidate).split(sep).filter(Boolean)) {
    current = join(current, part);
    paths.push(current);
  }
  for (const path of paths) {
    try {
      const stat = await lstat(path);
      if (!stat.isDirectory() || stat.isSymbolicLink()) return false;
    } catch (error) {
      if (isCode(error, "ENOENT")) return false;
      throw context(error, `cannot inspect rollout directory ${path}`);
    }
  }
  return true;
}

async function match(dir: string, suffix: string): Promise<string | undefined> {
  const names = (await entries(dir))
    .filter((entry) => entry.isFile() && entry.name.endsWith(suffix))
    .map((entry) => entry.name)
    .sort()
    .reverse();
  return names[0] ? resolve(dir, names[0]) : undefined;
}

export async function findRollout(
  sessionsDir: string,
  agentId: string,
  preferredDir?: string,
): Promise<string | undefined> {
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]*$/.test(agentId)) {
    throw new Error(`invalid rollout agent id: ${agentId}`);
  }
  const root = resolve(sessionsDir);
  const suffix = `-${agentId}.jsonl`;
  if (preferredDir) {
    const preferred = resolve(preferredDir);
    if (await safeDirectory(root, preferred)) {
      const found = await match(preferred, suffix);
      if (found) return found;
    }
  }

  const years = (await entries(root)).filter((entry) => entry.isDirectory() && /^\d{4}$/.test(entry.name));
  for (const year of years.sort((a, b) => b.name.localeCompare(a.name))) {
    const yearDir = resolve(root, year.name);
    const months = (await entries(yearDir)).filter((entry) => entry.isDirectory() && /^(0[1-9]|1[0-2])$/.test(entry.name));
    for (const month of months.sort((a, b) => b.name.localeCompare(a.name))) {
      const monthDir = resolve(yearDir, month.name);
      const days = (await entries(monthDir)).filter((entry) => entry.isDirectory() && /^(0[1-9]|[12]\d|3[01])$/.test(entry.name));
      for (const day of days.sort((a, b) => b.name.localeCompare(a.name))) {
        const found = await match(resolve(monthDir, day.name), suffix);
        if (found) return found;
      }
    }
  }
  return undefined;
}
