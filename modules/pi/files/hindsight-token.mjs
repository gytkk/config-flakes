import { execFileSync } from "node:child_process";
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const managedOrigin = "https://hindsight.pylv.dev";
const teamOrigin = "https://hindsight.dev.teamdev.ai";
const teamTokenEndpoint = "https://auth.devsisters.cloud/auth/realms/devsisters/protocol/openid-connect/token";
// Access tokens live for five minutes; refresh a little early so a request does not leave with a dying one.
const teamTokenMarginSeconds = 60;

const teamDefaults = {
  piAuthPath: join(homedir(), ".pi", "agent", "mcp-auth.json"),
  cachePath: join(homedir(), ".hindsight", "teamdev-access-token.json"),
  refresh: refreshTeamToken,
  now: () => Date.now() / 1000,
};

// The plugin resolves its credential synchronously, so the refresh runs in a child process.
// The refresh token travels on stdin to keep it out of the process list.
const refreshScript = `
(async () => {
  const body = new URLSearchParams({ grant_type: "refresh_token", client_id: "hindsight", refresh_token: require("fs").readFileSync(0, "utf8") });
  const r = await fetch(${JSON.stringify(teamTokenEndpoint)}, { method: "POST", body, redirect: "error" });
  const j = await r.json().catch(() => ({}));
  if (!r.ok) { process.stderr.write(r.status + " " + (j.error || "") + " " + (j.error_description || "")); process.exit(1); }
  process.stdout.write(JSON.stringify({ access_token: j.access_token, expires_in: j.expires_in }));
})().catch((e) => { process.stderr.write(String(e && e.message || e)); process.exit(1); });
`;

function refreshTeamToken(refreshToken) {
  try {
    return JSON.parse(execFileSync(process.execPath, ["-e", refreshScript], { input: refreshToken, timeout: 15000, stdio: ["pipe", "pipe", "pipe"] }));
  } catch (error) {
    const detail = String(error.stderr || error.message).trim();
    throw new Error(`Hindsight: refreshing the ${teamOrigin} token failed (${detail}); run /mcp login hindsight-work-log in Pi`);
  }
}

function piRefreshToken(piAuthPath) {
  let store;
  try {
    store = JSON.parse(readFileSync(piAuthPath, "utf8"));
  } catch (error) {
    throw new Error(`Hindsight: cannot read Pi's MCP credentials at ${piAuthPath} (${error.code || error.message}); run /mcp login hindsight-work-log in Pi`);
  }
  const entry = Object.values(store).find((e) => {
    try {
      return new URL(e.serverUrl).origin === teamOrigin && e.tokens?.refresh_token;
    } catch {
      return false;
    }
  });
  if (!entry) throw new Error(`Hindsight: Pi has no ${teamOrigin} login; run /mcp login hindsight-work-log in Pi`);
  return entry.tokens.refresh_token;
}

export function teamAccessToken(opts = teamDefaults) {
  const { piAuthPath, cachePath, refresh, now } = { ...teamDefaults, ...opts };
  try {
    const cached = JSON.parse(readFileSync(cachePath, "utf8"));
    if (typeof cached.access_token === "string" && cached.exp - teamTokenMarginSeconds > now()) return cached.access_token;
  } catch {
    // A missing or unreadable cache only means a refresh is due.
  }
  const issued = now();
  const fresh = refresh(piRefreshToken(piAuthPath));
  if (typeof fresh.access_token !== "string" || typeof fresh.expires_in !== "number") throw new Error("Hindsight: Keycloak returned no access token");
  const tmp = `${cachePath}.${process.pid}.tmp`;
  writeFileSync(tmp, JSON.stringify({ access_token: fresh.access_token, exp: issued + fresh.expires_in }), { mode: 0o600 });
  renameSync(tmp, cachePath);
  return fresh.access_token;
}

function readSecret(secretPath) {
  const path = secretPath.replace(/^\$\{XDG_RUNTIME_DIR\}(?=\/)/, () => {
    if (!process.env.XDG_RUNTIME_DIR) throw new Error("Hindsight: XDG_RUNTIME_DIR is unset; cannot locate the agenix token");
    return process.env.XDG_RUNTIME_DIR;
  });
  return readFileSync(path, "utf8").trimEnd();
}

export function withSepiaToken(result, secretPath, endpoint, team = teamDefaults) {
  const cfg = result.cfg;
  if (!cfg.disabled && cfg.serverMode === "self-hosted" && cfg.apiUrl.replace(/\/$/, "") === teamOrigin) {
    return { ...result, cfg: { ...cfg, apiToken: teamAccessToken(team) } };
  }
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
