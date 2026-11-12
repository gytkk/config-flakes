import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { settings } from "./module-config";

const moduleRoot = new URL("../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, moduleRoot), "utf8");
const rollout = JSON.parse(read("files/extensions/subagent/config.json"));

// These profiles intentionally use only scalar frontmatter and comma-separated lists.
function profile(name: string) {
  const text = read(`agents/${name}.md`);
  const match = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(text);
  if (!match) throw new Error(`Missing frontmatter in ${name}`);
  const fields = Object.fromEntries(match[1].split("\n").map((line) => {
    const separator = line.indexOf(":");
    if (separator < 0) throw new Error(`Invalid profile line: ${line}`);
    return [line.slice(0, separator), line.slice(separator + 1).trim()];
  }));
  return { fields, body: match[2], tools: fields.tools.split(", ") };
}

describe("foreground specialist profiles", () => {
  test.each(["researcher", "reviewer"])("installs %s without conflicting role overrides", (name: string) => {
    const { fields, tools } = profile(name);
    expect(read("default.nix")).toContain(`".pi/agent/agents/${name}.md".source = mkSymlink "agents/${name}.md";`);
    expect(fields.name).toBe(name);
    expect(fields.systemPromptMode).toBe("replace");
    expect(fields.defaultContext).toBe("fresh");
    expect(fields.acceptanceRole).toBe("read-only");
    expect(fields.inheritGlobalContext).toBe("false");
    expect(fields.inheritSkills).toBe("false");
    expect(fields.output).toBeUndefined();
    expect(fields.defaultProgress).toBeUndefined();
    expect(tools).toContain("contact_supervisor");
    expect(tools).not.toContain("intercom");
    expect(tools).not.toContain("edit");
    expect(tools).not.toContain("write");
    expect(tools).not.toContain("subagent");
    expect(settings.subagents.agentOverrides[name].tools).toBeUndefined();
    expect(settings.subagents.agentOverrides[name].systemPrompt).toBeUndefined();
  });

  test("research tools have an explicitly loaded provider and activation tool", () => {
    const { fields, tools } = profile("researcher");
    expect(fields.inheritProjectContext).toBe("false");
    expect(fields.subagentOnlyExtensions).toBe("~/.pi/agent/npm/node_modules/pi-web-access/dist/index.js");
    expect(settings.packages).toContain("npm:pi-web-access@0.46.0");
    expect(tools).toEqual(["read", "web_enable", "web_search", "fetch_content", "get_search_content", "source_check", "contact_supervisor"]);
  });

  test("reviewer can inspect and validate without implementation tools", () => {
    const { fields, tools, body } = profile("reviewer");
    expect(fields.inheritProjectContext).toBe("true");
    expect(tools).toEqual(["read", "grep", "find", "ls", "bash", "contact_supervisor"]);
    expect(fields.subagentOnlyExtensions).toBeUndefined();
    expect(body).toContain("read-only Git commands");
    expect(body).toContain("authorized by the task or repository instructions");
    expect(body).toContain("Do not edit files");
    expect(body).not.toContain("Do not use shell commands");
  });

  test("worker owns writer tools and bounded completion, not model or artifact defaults", () => {
    const { fields, tools, body } = profile("worker");
    expect(read("default.nix")).toContain('".pi/agent/agents/worker.md".source = mkSymlink "agents/worker.md";');
    expect(fields.name).toBe("worker");
    expect(fields.acceptanceRole).toBe("writer");
    expect(fields.systemPromptMode).toBe("replace");
    expect(fields.defaultContext).toBe("fresh");
    expect(fields.inheritProjectContext).toBe("true");
    expect(fields.inheritGlobalContext).toBe("false");
    expect(fields.inheritSkills).toBe("false");
    expect(tools).toEqual(["read", "grep", "find", "ls", "bash", "edit", "write", "contact_supervisor"]);
    for (const key of ["output", "defaultReads", "defaultProgress", "model", "thinking", "acceptance", "extensions", "subagentOnlyExtensions"]) expect(fields[key]).toBeUndefined();
    for (const key of ["tools", "systemPrompt", "acceptanceRole", "outputSchema"]) expect(settings.subagents.agentOverrides.worker[key]).toBeUndefined();
    expect(JSON.parse(fields.outputSchema).properties.status.enum).toEqual(["completed", "blocked", "failed"]);
    expect(body).toContain('reason: "need_decision"');
    expect(body).toContain("wait for the reply");
    expect(body).toContain("acceptanceReport");
  });

  test("model routing and bounded foreground rollout remain unchanged", () => {
    expect(settings.defaultModel).toBe("gpt-6-astra");
    expect(settings.defaultThinkingLevel).toBe("high");
    expect(settings.subagents.defaultModel).toBe("openai/gpt-6.1-sol");
    expect(settings.subagents.agentOverrides.worker.thinking).toBe("medium");
    expect(settings.subagents.agentOverrides.reviewer.thinking).toBe("high");
    expect(settings.subagents.agentOverrides.researcher.model).toBe("openai/gpt-6-luna");
    expect(settings.subagents.agentOverrides.researcher.thinking).toBe("low");
    expect(rollout.asyncByDefault).toBe(false);
    expect(rollout.forceTopLevelAsync).toBe(false);
    expect(rollout.maxSubagentDepth).toBe(1);
  });
});
