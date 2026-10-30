import { afterEach, beforeEach, describe, expect, mock, spyOn, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import * as os from "node:os";
import { join } from "node:path";
import { context, generate, main } from "../plugins/git-commit-suggestions/git-commit-message";

const emptyContext = { rules: {}, recent_subjects: "", staged_diff: "a diff" };
const completion = (content: unknown = "feat: add commit suggestions", finish = "stop") =>
  Response.json({ choices: [{ message: { content }, finish_reason: finish }] });

describe("commit context", () => {
  let root: string;
  let fakeHome: string;
  let previous: string;
  const git = (...args: string[]) => execFileSync("git", args, { encoding: "utf8" });

  beforeEach(() => {
    previous = process.cwd();
    root = realpathSync(mkdtempSync(join(os.tmpdir(), "commit-message-")));
    process.chdir(root);
    git("init", "-q");
    fakeHome = join(root, "home");
    mkdirSync(fakeHome);
    spyOn(os, "homedir").mockReturnValue(fakeHome);
    spyOn(globalThis, "fetch").mockRejectedValue(new Error("unexpected API call"));
  });

  afterEach(() => {
    mock.restore();
    process.chdir(previous);
    rmSync(root, { recursive: true, force: true });
  });

  test("reads staged content and root rules from a subdirectory on an unborn branch", () => {
    mkdirSync(join(fakeHome, ".codex"));
    writeFileSync(join(fakeHome, ".codex/AGENTS.md"), "Use concise subjects.");
    writeFileSync("AGENTS.md", "Write subjects in Korean.");
    writeFileSync("CLAUDE.md", "Use Conventional Commits.");
    writeFileSync("code", "staged content\n");
    git("add", "code");
    writeFileSync("code", "unstaged content\n");
    writeFileSync("untracked", "private untracked content\n");
    const before = git("diff", "--cached");
    process.chdir(fakeHome);
    const data = context()!;
    expect(data.staged_diff).toContain("staged content");
    expect(data.staged_diff).not.toContain("unstaged content");
    expect(JSON.stringify(data)).not.toContain("private untracked content");
    expect(data.recent_subjects).toBe("");
    expect(data.rules).toEqual({
      "global AGENTS.md": "Use concise subjects.",
      "repository AGENTS.md": "Write subjects in Korean.",
      "repository CLAUDE.md": "Use Conventional Commits.",
    });
    expect(git("diff", "--cached")).toBe(before);
  });

  test("reads only the five most recent subjects and tolerates missing optional rules", () => {
    for (let i = 0; i < 6; i++) {
      git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
        "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null",
        "commit", "--allow-empty", "-qm", `fix: previous style ${i}`);
    }
    writeFileSync("code", "new content\n");
    git("add", "code");
    expect(context()).toMatchObject({
      rules: {},
      recent_subjects: [5, 4, 3, 2, 1].map((i) => `fix: previous style ${i}\n`).join(""),
    });
  });

  test("an empty index does not call the API", async () => {
    expect(context()).toBeNull();
    expect(await main()).toBe(0);
    expect(fetch).not.toHaveBeenCalled();
  });

  test("rejects a large diff before calling the API", () => {
    writeFileSync("large", "x".repeat(64001));
    git("add", "large");
    expect(context).toThrow("staged diff exceeds");
    expect(fetch).not.toHaveBeenCalled();
  });

  test("main prints a subject and reports errors with a nonzero status", async () => {
    writeFileSync("code", "new content\n");
    git("add", "code");
    const previousKey = process.env.OPENAI_API_KEY;
    process.env.OPENAI_API_KEY = "test-key";
    const output = spyOn(console, "log").mockImplementation(() => {});
    const errors = spyOn(console, "error").mockImplementation(() => {});
    try {
      spyOn(globalThis, "fetch").mockResolvedValue(completion());
      expect(await main()).toBe(0);
      expect(output).toHaveBeenCalledWith("feat: add commit suggestions");
      delete process.env.OPENAI_API_KEY;
      expect(await main()).toBe(1);
      expect(errors).toHaveBeenCalledWith(expect.stringContaining("git-commit-message: OPENAI_API_KEY is missing"));
    } finally {
      if (previousKey === undefined) delete process.env.OPENAI_API_KEY;
      else process.env.OPENAI_API_KEY = previousKey;
    }
  });

  test("limits rule files together and counts Unicode codepoints", () => {
    writeFileSync("code", "new content\n");
    git("add", "code");
    writeFileSync("AGENTS.md", "😀".repeat(32000));
    writeFileSync("CLAUDE.md", "x".repeat(32000));
    expect(context()!.rules["repository AGENTS.md"]).toHaveLength(64000);
    writeFileSync("CLAUDE.md", "x".repeat(32001));
    expect(context).toThrow("commit rule files exceed");
    expect(fetch).not.toHaveBeenCalled();
  });

  test("counts staged diff size in Unicode codepoints", () => {
    writeFileSync("code", "😀".repeat(32000));
    git("add", "code");
    expect(context()!.staged_diff).toContain("😀");
  });
});

describe("commit completion", () => {
  let previousKey: string | undefined;

  beforeEach(() => {
    previousKey = process.env.OPENAI_API_KEY;
    process.env.OPENAI_API_KEY = "test-key";
    spyOn(globalThis, "fetch").mockResolvedValue(completion());
  });

  afterEach(() => {
    mock.restore();
    if (previousKey === undefined) delete process.env.OPENAI_API_KEY;
    else process.env.OPENAI_API_KEY = previousKey;
  });

  test("preserves literal subjects and sends the model, rules, history, and timeout contract", async () => {
    const subject = "fix: preserve '$HOME', `code`, and $(commands)!";
    const send = spyOn(globalThis, "fetch").mockResolvedValue(completion(subject));
    const timeout = spyOn(AbortSignal, "timeout");
    const data = {
      rules: { "repository AGENTS.md": "Write subjects in Korean." },
      recent_subjects: "fix: previous style\n",
      staged_diff: "a diff",
    };
    expect(await generate(data)).toBe(subject);
    const [url, options] = send.mock.calls[0];
    expect(url).toBe("https://api.openai.com/v1/chat/completions");
    expect(options!.method).toBe("POST");
    const headers = new Headers(options!.headers);
    expect(headers.get("Authorization")).toBe("Bearer test-key");
    expect(headers.get("Content-Type")).toBe("application/json");
    const payload = JSON.parse(options!.body as string);
    expect(payload).toMatchObject({
      model: "gpt-6.1-terra", reasoning_effort: "none", max_completion_tokens: 256, store: false,
    });
    expect(payload.messages[0].role).toBe("developer");
    expect(payload.messages[0].content).toContain("Repository rules override global rules");
    expect(payload.messages[0].content).toContain("AGENTS.md wins");
    expect(payload.messages[1].role).toBe("user");
    expect(JSON.parse(payload.messages[1].content)).toEqual(data);
    expect(timeout).toHaveBeenCalledWith(8000);
    expect(options!.signal).toBeInstanceOf(AbortSignal);
  });

  test("rejects empty, multiline, nonprintable, long, and incomplete subjects", async () => {
    for (const [content, finish] of [
      ["", "stop"], ["   ", "stop"], [null, "stop"], ["a\nb", "stop"],
      ["a\x1bb", "stop"], ["a\tb", "stop"], ["a\u00a0b", "stop"],
      ["a\u200bb", "stop"], ["a\u2028b", "stop"], ["a\ud800b", "stop"],
      ["x".repeat(301), "stop"], ["partial", "length"],
    ] as const) {
      spyOn(globalThis, "fetch").mockResolvedValue(completion(content, finish));
      await expect(generate(emptyContext)).rejects.toThrow();
    }
  });

  test("trims subjects and permits 300 Unicode codepoints", async () => {
    const subject = "😀".repeat(300);
    spyOn(globalThis, "fetch").mockResolvedValue(completion(` ${subject} `));
    expect(await generate(emptyContext)).toBe(subject);
  });

  test("rejects missing API keys before a request", async () => {
    delete process.env.OPENAI_API_KEY;
    await expect(generate(emptyContext)).rejects.toThrow("OPENAI_API_KEY is missing");
    expect(fetch).not.toHaveBeenCalled();
  });

  test("HTTP errors expose only the status and do not read the response body", async () => {
    const response = new Response("private response body", { status: 401 });
    const readBody = spyOn(response, "text");
    const readJson = spyOn(response, "json");
    spyOn(globalThis, "fetch").mockResolvedValue(response);
    await expect(generate(emptyContext)).rejects.toThrow(/^OpenAI returned HTTP 401$/);
    expect(readBody).not.toHaveBeenCalled();
    expect(readJson).not.toHaveBeenCalled();
  });

  test("rejects malformed JSON and completion shapes", async () => {
    for (const response of [new Response("not JSON"), Response.json({}),
      Response.json({ choices: [] }), Response.json({ choices: [null] })]) {
      spyOn(globalThis, "fetch").mockResolvedValue(response);
      await expect(generate(emptyContext)).rejects.toThrow();
    }
  });

  test("propagates request timeout failures", async () => {
    spyOn(globalThis, "fetch").mockRejectedValue(new DOMException("request timed out", "TimeoutError"));
    await expect(generate(emptyContext)).rejects.toThrow("request timed out");
  });
});
