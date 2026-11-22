import { expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, writeFileSync, chmodSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import register from "../files/extensions/session-record";

test("shutdown passes only a persisted default-store session reference", async () => {
  const home = mkdtempSync(join(tmpdir(), "pi-record-test-"));
  const old = process.env.HOME;
  process.env.HOME = home;
  try {
    const bin = join(home, ".local/bin");
    const dir = join(home, ".pi/agent/sessions/--project--");
    mkdirSync(bin, {recursive:true}); mkdirSync(dir, {recursive:true});
    const hook = join(bin, "agent-session-record");
    writeFileSync(hook, '#!/bin/sh\n[ "$*" = "hook pi session-shutdown" ] || exit 2\nwhile read -r line; do echo "$line" >> "$HOME/received"; done\n'); chmodSync(hook, 0o700);
    const path = join(dir, "timestamp_example.jsonl");
    writeFileSync(path, "private transcript contents");
    let end: Function;
    register({on(name, handler) { if (name === "session_shutdown") end = handler; }} as any);
    for (const [file, isPrivate] of [[undefined, false], [join(home, "private.jsonl"), false], [path, true], [path, false]]) {
      await end!({}, {sessionManager:{ getSessionFile:()=>file, getHeader:()=>({private:isPrivate}), getSessionId:()=>"example" }});
    }
    expect(readFileSync(join(home,"received"), "utf8").trim().split("\n").map(s => JSON.parse(s))).toEqual([{session_id:"example", transcript_path:path}]);
  } finally {
    if (old === undefined) delete process.env.HOME; else process.env.HOME = old;
    rmSync(home, {recursive: true, force: true});
  }
});
