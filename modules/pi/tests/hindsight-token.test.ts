import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { sepiaRequest, withSepiaToken } from "../files/hindsight-token.mjs";

const endpoint = "http://127.0.0.1:8888";
const cfg = { apiUrl: endpoint, serverMode: "self-hosted", disabled: false };

test("never reads the sepia credential for disabled banks, other URLs or cloud mode", () => {
  for (const override of [{ disabled: true }, { serverMode: "cloud" }, { apiUrl: endpoint + ".invalid" }, { apiUrl: "https://example.org" }]) {
    const bank = { cfg: { ...cfg, ...override } };
    expect(withSepiaToken(bank, "/missing")).toBe(bank);
  }
});

test("public Access headers stay on the exact HTTPS origin and denim uses only its LAN dispatcher", () => {
  const base = "https://hindsight.pylv.dev";
  const init = { headers: { Authorization: "Bearer fixture" } };
  const dir = mkdtempSync(join(tmpdir(), "pi-access-"));
  try {
    const path = join(dir, "access");
    writeFileSync(path, JSON.stringify({ client_id: "id", client_secret: "secret" }));
    expect(sepiaRequest(base, base + "/v1/default/banks", init, path, null)).toEqual({
      redirect: "error", headers: { ...init.headers, "CF-Access-Client-Id": "id", "CF-Access-Client-Secret": "secret" }
    });
    expect(sepiaRequest(base, base, init, null, "LAN")).toEqual({ ...init, redirect: "error", dispatcher: "LAN" });
    expect(() => sepiaRequest(base, base + ".invalid", init, path, null)).toThrow("cross-origin");
    expect(sepiaRequest(endpoint, endpoint, init, "/missing", null)).toBe(init);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("reads the current token for the resolved bank and rejects malformed credentials", () => {
  const dir = mkdtempSync(join(tmpdir(), "pi-token-"));
  const path = join(dir, "key");
  try {
    for (const character of ["a", "b"]) {
      writeFileSync(path, character.repeat(64) + "\n");
      expect(withSepiaToken({ cfg }, path).cfg.apiToken).toBe(character.repeat(64));
    }
    writeFileSync(path, "invalid");
    expect(() => withSepiaToken({ cfg }, path)).toThrow("invalid agenix client token");
    expect(cfg).not.toHaveProperty("apiToken");
  } finally {
    rmSync(dir, { recursive: true });
  }
});
