import { readFileSync } from "node:fs";

// Use the effective endpoint after all workspace and bank overrides, including on an auth retry.
export function withSepiaToken(result, secretPath) {
  const cfg = result.cfg;
  if (cfg.disabled || cfg.serverMode !== "self-hosted" || cfg.apiUrl.replace(/\/$/, "") !== "http://127.0.0.1:8888") return result;
  const path = secretPath.replace(/^\$\{XDG_RUNTIME_DIR\}(?=\/)/, () => {
    if (!process.env.XDG_RUNTIME_DIR) throw new Error("Hindsight: XDG_RUNTIME_DIR is unset; cannot locate the agenix token");
    return process.env.XDG_RUNTIME_DIR;
  });
  const token = readFileSync(path, "utf8").trimEnd();
  if (!/^[0-9a-f]{64}$/.test(token)) throw new Error("Hindsight: invalid agenix client token");
  return { ...result, cfg: { ...cfg, apiToken: token } };
}
