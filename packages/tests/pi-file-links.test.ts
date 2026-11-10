import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { openHerdrFile } from "../apps/pi/herdr-file-opener";

const env = { HERDR_ENV: "1", HERDR_PANE_ID: "w1:p2", HERDR_BIN_PATH: "/opt/herdr" };
const split = JSON.stringify({ result: { pane: { pane_id: "w1:p3" } } });

function recorder() {
  const calls: { binary: string; args: string[] }[] = [];
  const run = async (binary: string, args: string[]) => {
    calls.push({ binary, args });
    return args[1] === "split" ? split : "";
  };
  return { calls, run };
}

test("opens a decoded file in a right-hand pane, preserving cwd and focusing the editor", async () => {
  const { calls, run } = recorder();
  const path = "/tmp/한글 file's $(touch unwanted); & #%.ts";
  expect(await openHerdrFile(pathToFileURL(path).href, { env, cwd: "/project root", run })).toBe(true);
  expect(calls.slice(0, 2)).toEqual([
    { binary: "nvim", args: ["--version"] },
    { binary: "/opt/herdr", args: ["pane", "split", "--current", "--direction", "right", "--cwd", "/project root", "--focus"] },
  ]);
  expect(calls[2].binary).toBe("/opt/herdr");
  expect(calls[2].args.slice(0, 3)).toEqual(["pane", "run", "w1:p3"]);
  const command = calls[2].args[3];
  expect(command.startsWith("exec nvim -- ")).toBe(true);
  // Interpret the actual quoting with a shell, replacing only the editor invocation.
  const quoted = command.slice("exec nvim -- ".length);
  const result = spawnSync("sh", ["-c", `printf '%s' ${quoted}`], { encoding: "utf8" });
  expect(result.status).toBe(0);
  expect(result.stdout).toBe(path);
});

test("leaves web links and links outside Herdr to the platform opener", async () => {
  const { calls, run } = recorder();
  for (const target of ["https://example.com", "http://localhost:3000", "/tmp/file", "mailto:user@example.com"]) {
    expect(await openHerdrFile(target, { env, run })).toBe(false);
  }
  for (const outside of [{}, { HERDR_ENV: "1" }, { HERDR_PANE_ID: "w1:p2" }]) {
    expect(await openHerdrFile("file:///tmp/test", { env: outside, run })).toBe(false);
  }
  expect(calls).toEqual([]);
});

test("rejects remote, malformed and control-bearing links before splitting", async () => {
  const { calls, run } = recorder();
  for (const target of ["file://remote/tmp/file", "file:///tmp/%", "file:///tmp/%00", "file:///tmp/%0A", "file:///tmp/test?q=1", "file:///tmp/test#L1"]) {
    await expect(openHerdrFile(target, { env, run })).rejects.toThrow();
  }
  expect(calls).toEqual([]);
});

test("uses PATH when HERDR_BIN_PATH is absent", async () => {
  const { calls, run } = recorder();
  await openHerdrFile("file://localhost/tmp/test", { env: { ...env, HERDR_BIN_PATH: "" }, run });
  expect(calls[1].binary).toBe("herdr");
});

test("missing editor and split failures do not send input to a pane", async () => {
  for (const failure of ["editor", "split", "response"]) {
    const calls: string[][] = [];
    const run = async (binary: string, args: string[]) => {
      calls.push(args);
      if (binary === "nvim" && failure !== "editor") return "";
      if (failure === "response") return "{}";
      throw new Error("unavailable");
    };
    await expect(openHerdrFile("file:///tmp/test", { env, run })).rejects.toThrow();
    expect(calls.some((args) => args[1] === "run")).toBe(false);
  }
});

test("a command failure reports the new pane and never closes an existing pane", async () => {
  const run = async (_binary: string, args: string[]) => {
    if (args[1] === "split") return split;
    if (args[0] === "--version") return "";
    expect(args.slice(0, 3)).toEqual(["pane", "run", "w1:p3"]);
    throw new Error("admission failed");
  };
  await expect(openHerdrFile("file:///tmp/test", { env, run })).rejects.toThrow("w1:p3; the pane was left open");
});
