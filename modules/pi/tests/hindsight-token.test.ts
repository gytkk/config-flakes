import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { sepiaRequest, withSepiaToken } from "../files/hindsight-token.mjs";

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
