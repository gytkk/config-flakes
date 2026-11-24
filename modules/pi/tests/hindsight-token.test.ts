import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { withSepiaToken } from "../files/hindsight-token.mjs";

const endpoint = "http://127.0.0.1:8888";
const cfg = { apiUrl: endpoint, serverMode: "self-hosted", disabled: false };

test("never reads the sepia credential for disabled banks, other URLs or cloud mode", () => {
  for (const override of [{ disabled: true }, { serverMode: "cloud" }, { apiUrl: endpoint + ".invalid" }, { apiUrl: "https://example.org" }]) {
    const bank = { cfg: { ...cfg, ...override } };
    expect(withSepiaToken(bank, "/missing")).toBe(bank);
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
