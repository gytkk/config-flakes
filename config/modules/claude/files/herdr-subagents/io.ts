import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { object, validId } from "./projection";

export async function teammateRemoved(transcript: string, teamsRoot: string, sessionId: string): Promise<boolean> {
  try {
    const metadata = object(JSON.parse(await readFile(transcript.replace(/\.jsonl$/, ".meta.json"), "utf8")));
    if (!validId(metadata?.teamName) || !validId(metadata?.name)) return false;
    const config = object(JSON.parse(await readFile(join(teamsRoot, metadata.teamName, "config.json"), "utf8")));
    if (config?.name !== metadata.teamName || config.leadSessionId !== sessionId) return false;
    if (!Array.isArray(config.members) || !config.members.every((member: unknown) => typeof object(member)?.agentId === "string")) {
      throw new Error("invalid team member list");
    }
    return !config.members.some((member: { agentId: string }) => member.agentId === `${metadata.name}@${metadata.teamName}`);
  } catch (error) {
    if (isCode(error, "ENOENT")) return false;
    throw context(error instanceof SyntaxError ? new Error("invalid JSON") : error, "cannot read teammate membership");
  }
}

export async function childTranscripts(root: string): Promise<Map<string, string>> {
  const found = new Map<string, string>();
  const pending = [root];
  while (pending.length) {
    const directory = pending.pop()!;
    let entries;
    try { entries = await readdir(directory, { withFileTypes: true }); }
    catch (error) { if (isCode(error, "ENOENT")) continue; throw context(error, "cannot scan child transcripts"); }
    for (const entry of entries) {
      if (entry.isDirectory()) pending.push(join(directory, entry.name));
      const id = entry.name.match(/^agent-([a-zA-Z0-9_-]{1,128})\.jsonl$/)?.[1];
      if (entry.isFile() && id) found.set(id, join(directory, entry.name));
    }
  }
  return found;
}

function context(error: unknown, action: string): Error {
  const detail = error instanceof Error ? error.message : String(error);
  return new Error(`${action}: ${detail}`, { cause: error });
}

function isCode(error: unknown, code: string): boolean {
  return typeof error === "object" && error !== null && "code" in error
    && (error as NodeJS.ErrnoException).code === code;
}
