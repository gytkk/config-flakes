import { readFileSync } from "node:fs";

const managedOrigin = "https://hindsight.pylv.dev";

function readSecret(secretPath) {
  const path = secretPath.replace(/^\$\{XDG_RUNTIME_DIR\}(?=\/)/, () => {
    if (!process.env.XDG_RUNTIME_DIR) throw new Error("Hindsight: XDG_RUNTIME_DIR is unset; cannot locate the agenix token");
    return process.env.XDG_RUNTIME_DIR;
  });
  return readFileSync(path, "utf8").trimEnd();
}

export function withSepiaToken(result, secretPath, endpoint) {
  const cfg = result.cfg;
  if (cfg.disabled || cfg.serverMode !== "self-hosted" || cfg.apiUrl.replace(/\/$/, "") !== managedOrigin) return result;
  const token = readSecret(secretPath);
  if (!/^[0-9a-f]{64}$/.test(token)) throw new Error("Hindsight: invalid agenix client token");
  return { ...result, cfg: { ...cfg, apiUrl: endpoint, apiToken: token } };
}

export function sepiaRequest(base, url, init, endpoint, lanDispatcher) {
  if (base !== endpoint) return init;
  if (new URL(url).origin !== endpoint) throw new Error("Hindsight: refusing cross-origin request");
  const options = { ...init, redirect: "error" };
  if (lanDispatcher) options.dispatcher = lanDispatcher;
  return options;
}
