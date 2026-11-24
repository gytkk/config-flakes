import { readFileSync } from "node:fs";

const publicOrigin = "https://hindsight.pylv.dev";

function readSecret(secretPath) {
  const path = secretPath.replace(/^\$\{XDG_RUNTIME_DIR\}(?=\/)/, () => {
    if (!process.env.XDG_RUNTIME_DIR) throw new Error("Hindsight: XDG_RUNTIME_DIR is unset; cannot locate the agenix token");
    return process.env.XDG_RUNTIME_DIR;
  });
  return readFileSync(path, "utf8").trimEnd();
}

export function withSepiaToken(result, secretPath, loopback = false) {
  const cfg = result.cfg;
  if (cfg.disabled || cfg.serverMode !== "self-hosted" || ![publicOrigin, "http://127.0.0.1:8888"].includes(cfg.apiUrl.replace(/\/$/, ""))) return result;
  const token = readSecret(secretPath);
  if (!/^[0-9a-f]{64}$/.test(token)) throw new Error("Hindsight: invalid agenix client token");
  const apiUrl = loopback ? "http://127.0.0.1:8888" : cfg.apiUrl;
  return { ...result, cfg: { ...cfg, apiUrl, apiToken: token } };
}

export function sepiaRequest(base, url, init, accessFile, lanDispatcher) {
  if (base !== publicOrigin) return init;
  if (new URL(url).origin !== publicOrigin) throw new Error("Hindsight: refusing cross-origin request");
  const options = { ...init, redirect: "error" };
  if (lanDispatcher) options.dispatcher = lanDispatcher;
  if (accessFile) {
    const { client_id, client_secret } = JSON.parse(readSecret(accessFile));
    if (!client_id || !client_secret) throw new Error("Hindsight: incomplete Cloudflare Access credential");
    options.headers = { ...init.headers, "CF-Access-Client-Id": client_id, "CF-Access-Client-Secret": client_secret };
  }
  return options;
}
