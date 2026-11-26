import { expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { sepiaRequest, teamAccessToken, withSepiaToken } from "../files/hindsight-token.mjs";

const tailnet = "https://pylv-sepia.tailbbb9bf.ts.net";
const lan = "https://hindsight.pylv.dev";
const cfg = { apiUrl: lan, serverMode: "self-hosted", disabled: false };

test("does not read the managed credential for disabled banks or unrelated origins", () => {
  for (const override of [{ disabled: true }, { serverMode: "cloud" }, { apiUrl: tailnet + ".invalid" }, { apiUrl: "https://example.org" }]) {
    const bank = { cfg: { ...cfg, ...override } };
    expect(withSepiaToken(bank, "/missing", tailnet)).toBe(bank);
  }
});

test("uses the host route without forwarding bearer tokens across origins", () => {
  const dir = mkdtempSync(join(tmpdir(), "pi-token-"));
  const path = join(dir, "key");
  try {
    for (const [endpoint, digit] of [[tailnet, "a"], [lan, "b"], ["http://127.0.0.1:8888", "c"]]) {
      const token = digit.repeat(64);
      writeFileSync(path, token + "\n");
      expect(withSepiaToken({ cfg }, path, endpoint).cfg).toEqual({ ...cfg, apiUrl: endpoint, apiToken: token });
      const dispatcher = endpoint === lan ? "LAN" : null;
      expect(sepiaRequest(endpoint, endpoint + "/v1/default/banks", {}, endpoint, dispatcher)).toEqual({
        redirect: "error", ...(dispatcher ? { dispatcher } : {})
      });
      expect(() => sepiaRequest(endpoint, "https://example.org", {}, endpoint, dispatcher)).toThrow("cross-origin");
    }
    writeFileSync(path, "invalid");
    expect(() => withSepiaToken({ cfg }, path, tailnet)).toThrow("invalid agenix client token");
  } finally {
    rmSync(dir, { recursive: true });
  }
});

const team = "https://hindsight.dev.teamdev.ai";

function teamFixture() {
  const dir = mkdtempSync(join(tmpdir(), "pi-team-"));
  const piAuthPath = join(dir, "mcp-auth.json");
  const cachePath = join(dir, "cache.json");
  writeFileSync(piAuthPath, JSON.stringify({
    "mcp__other|https://example.org/mcp": { serverUrl: "https://example.org/mcp", tokens: { refresh_token: "other" } },
    "mcp__hindsight_work_log|https://hindsight.dev.teamdev.ai/mcp/ai-tech-work-log/": {
      serverUrl: "https://hindsight.dev.teamdev.ai/mcp/ai-tech-work-log/",
      tokens: { refresh_token: "pi-refresh" },
    },
  }));
  const refreshed: string[] = [];
  let clock = 1000;
  const opts = {
    piAuthPath,
    cachePath,
    now: () => clock,
    refresh: (rt: string) => (refreshed.push(rt), { access_token: `access-${refreshed.length}`, expires_in: 300 }),
  };
  return { dir, piAuthPath, cachePath, refreshed, opts, advance: (s: number) => (clock += s) };
}

test("refreshes the team server token from Pi's login and reuses it until it nears expiry", () => {
  const f = teamFixture();
  try {
    const teamCfg = { apiUrl: team + "/", serverMode: "self-hosted", disabled: false };
    expect(withSepiaToken({ cfg: teamCfg }, "/missing", tailnet, f.opts).cfg).toEqual({ ...teamCfg, apiToken: "access-1" });
    expect(f.refreshed).toEqual(["pi-refresh"]);
    expect(statSync(f.cachePath).mode & 0o777).toBe(0o600);
    f.advance(239);
    expect(teamAccessToken(f.opts)).toBe("access-1");
    f.advance(1);
    expect(teamAccessToken(f.opts)).toBe("access-2");
    expect(JSON.parse(readFileSync(f.cachePath, "utf8"))).toEqual({ access_token: "access-2", exp: 1540 });
    expect(withSepiaToken({ cfg: { ...teamCfg, disabled: true } }, "/missing", tailnet, f.opts).cfg.apiToken).toBeUndefined();
  } finally {
    rmSync(f.dir, { recursive: true });
  }
});

test("asks for a Pi login when no team server refresh token is stored", () => {
  const f = teamFixture();
  try {
    writeFileSync(f.piAuthPath, "{}");
    expect(() => teamAccessToken(f.opts)).toThrow("/mcp login hindsight-work-log");
    rmSync(f.piAuthPath);
    expect(() => teamAccessToken(f.opts)).toThrow("/mcp login hindsight-work-log");
    expect(f.refreshed).toEqual([]);
  } finally {
    rmSync(f.dir, { recursive: true });
  }
});
