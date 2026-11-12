import { afterEach, expect, mock, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { formatGitStatus, watchGitStatus } from "../files/extensions/prompt-ui/git-status";
import promptUI from "../files/extensions/prompt-ui";

const cleanups: (() => void)[] = [];
afterEach(() => { while (cleanups.length) cleanups.pop()!(); });
const ok = (stdout = "") => ({ stdout, stderr: "", code: 0, killed: false, truncated: false });
async function until(predicate: () => boolean) {
  const deadline = Date.now() + 2000;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error("Timed out waiting for Git status update");
    await Bun.sleep(5);
  }
}

test.each([
  ["", ""],
  [" M file\0", "*1"],
  ["M  file\0", "+1"],
  ["MM file\0?? new\0", "*1 +1 ?1"],
  [" D gone\0A  added\0!! ignored\0", "*1 +1"],
  ["RM new name\0?? old\nname\0?? odd\n M name\0", "*1 +1 ?1"],
  ["C  copy\0original\0", "+1"],
  ["UU a\0AA b\0DD c\0AU d\0UA e\0DU f\0UD g\0", "*7"],
])("formats porcelain records without parsing filenames: %j", (input, expected) => {
  expect(formatGitStatus(input)).toBe(expected);
});

test("counts individual files from a real repository, including partial staging and renames", () => {
  const cwd = mkdtempSync(join(tmpdir(), "pi-git-status-"));
  cleanups.push(() => rmSync(cwd, { recursive: true, force: true }));
  const git = (...args: string[]) => execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8" });
  git("init", "-q");
  writeFileSync(join(cwd, "tracked"), "initial\n");
  writeFileSync(join(cwd, "old"), "rename me\n");
  git("add", ".");
  git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgSign=false", "commit", "-qm", "initial");
  writeFileSync(join(cwd, "tracked"), "staged\n");
  git("add", "tracked");
  writeFileSync(join(cwd, "tracked"), "unstaged\n");
  git("mv", "old", "renamed\nfile");
  mkdirSync(join(cwd, "untracked"));
  writeFileSync(join(cwd, "untracked", "one"), "");
  writeFileSync(join(cwd, "untracked", "two\n?? odd"), "");
  expect(formatGitStatus(git("--no-optional-locks", "status", "--porcelain=v1", "-z", "--untracked-files=all"))).toBe("*1 +2 ?2");
});

test("polls without overlapping queries and cancels late results on disposal", async () => {
  let resolve!: (result: ReturnType<typeof ok>) => void;
  const exec = mock(() => new Promise<ReturnType<typeof ok>>(done => { resolve = done; }));
  const update = mock(() => {});
  const warn = mock(() => {});
  const stop = watchGitStatus(exec as never, "/workspace", update, warn, 5);
  cleanups.push(stop);
  expect(exec).toHaveBeenCalledTimes(1);
  const [, args, options] = exec.mock.calls[0] as unknown as [string, string[], { signal: AbortSignal; timeout: number; maxBuffer: number }];
  expect(args).toContain("--no-optional-locks");
  expect(args).toContain("--untracked-files=all");
  expect(args).toContain("/workspace");
  expect(options.timeout).toBe(5000);
  expect(options.maxBuffer).toBe(4 * 1024 * 1024);
  await Bun.sleep(20);
  expect(exec).toHaveBeenCalledTimes(1);
  resolve(ok(" M tracked\0"));
  await until(() => exec.mock.calls.length === 2);
  expect(update).toHaveBeenLastCalledWith("*1");
  stop();
  expect(options.signal.aborted).toBe(true);
  resolve(ok("?? stale\0"));
  await Bun.sleep(20);
  expect(update).toHaveBeenCalledTimes(1);
  expect(exec).toHaveBeenCalledTimes(2);
  expect(warn).not.toHaveBeenCalled();
});

test("reports failures once, rejects partial output, and recovers on later polls", async () => {
  let result = { ...ok(" M incomplete\0"), truncated: true };
  const update = mock(() => {});
  const warn = mock(() => {});
  const exec = mock(async () => result);
  cleanups.push(watchGitStatus(exec as never, "/workspace", update, warn, 5));
  await until(() => exec.mock.calls.length >= 3);
  expect(update).toHaveBeenLastCalledWith("git!");
  expect(warn).toHaveBeenCalledTimes(1);
  result = ok("?? new\0");
  await until(() => update.mock.calls.some(args => args[0] === "?1"));
  result = { ...ok(), code: 128, stderr: "fatal: not a git repository (or any of the parent directories): .git" };
  await until(() => update.mock.calls.some(args => args[0] === ""));
  expect(warn).toHaveBeenCalledTimes(1);
});

test("handles rejected Git execution without an unhandled rejection", async () => {
  const update = mock(() => {});
  const warn = mock(() => {});
  cleanups.push(watchGitStatus(async () => { throw new Error("spawn git ENOENT"); }, "/workspace", update, warn));
  await until(() => warn.mock.calls.length === 1);
  expect(update).toHaveBeenCalledWith("git!");
  expect(warn).toHaveBeenCalledWith("Git status (/workspace): spawn git ENOENT");
});

test("starts only in TUI and cancels queries on shutdown and footer disposal", async () => {
  const handlers = new Map<string, Function>();
  const exec = mock(() => new Promise(() => {}));
  promptUI({ on: (name: string, handler: Function) => handlers.set(name, handler), exec } as never);
  const ctx = {
    mode: "print", sessionManager: { getCwd: () => "/workspace" },
    ui: { setStatus() {}, setEditorComponent() {}, notify() {}, setFooter: mock(() => {}) },
  };
  handlers.get("session_start")!({}, ctx);
  expect(exec).not.toHaveBeenCalled();
  let footer: { dispose(): void };
  const unsubscribe = mock(() => {});
  ctx.ui.setFooter = mock((factory: Function) => {
    footer = factory({ requestRender() {} }, {}, { onBranchChange: () => unsubscribe });
  }) as never;
  ctx.mode = "tui";
  handlers.get("session_start")!({}, ctx);
  expect(exec).toHaveBeenCalledTimes(1);
  handlers.get("session_shutdown")!();
  footer!.dispose();
  expect(unsubscribe).toHaveBeenCalledTimes(1);
  expect((exec.mock.calls[0] as any)[2].signal.aborted).toBe(true);
  handlers.get("session_start")!({}, ctx);
  cleanups.push(() => handlers.get("session_shutdown")!());
  expect(exec).toHaveBeenCalledTimes(2);
  expect((exec.mock.calls[1] as any)[2].signal.aborted).toBe(false);
});
