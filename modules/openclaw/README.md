# OpenClaw

OpenClaw is installed with the official rootless installer under `~/.openclaw`.
The application owns its executable, plugins, mutable configuration, state,
updates, and systemd user service.

This directory contains only declarative host integration:

- `default.nix` provides NixOS dependencies, runtime
  environment variables, and proxy authentication.
- `nginx-proxy.nix` provides the LAN and public-origin reverse proxies and the
  LAN firewall rule.
- `home.nix` adds the user-owned executable to `PATH` and supplies the NixOS runtime paths required by the OpenClaw-managed user service. The systemd environment drop-in is materialized as a regular file because OpenClaw refuses update ownership checks when an effective service definition is a symlink.

The modules do not install an OpenClaw package, generate `openclaw.json`, set
`OPENCLAW_NIX_MODE`, or own `openclaw-gateway.service`.

Automation credentials belong to OpenClaw's local Secret Store. Discord uses a native `store` SecretRef named `DISCORD_BOT_TOKEN`; Nix does not provision its runtime token. The encrypted agenix source remains available for recovery. The Notion to Google Calendar automation uses protected `NOTION_CALENDAR_*` entries through OpenClaw's enabled secret egress proxy, restricted to the corresponding API hosts. Its model-free script job calls Gateway `exec` to receive the proxy context. Plain command jobs do not receive that context. The workspace automation README owns the runner configuration and key names.

The Secret Store protects disclosure through OpenClaw, but its local SQLite database is not encrypted at rest. Protect and back up OpenClaw's private state directory. Existing Brave and Gateway credentials remain native SecretRefs; GitHub CLI continues to own its OAuth login and Git credential helper.

The secret egress proxy requires OpenSSL in a trusted system directory. `default.nix` supplies it through `environment.systemPackages`, because OpenClaw's certificate generator does not search the user profile or `PATH`. After applying this module with `nixos-rebuild switch`, confirm `/run/current-system/sw/bin/openssl` exists and remove any temporary `/usr/bin/openssl` migration symlink.

## Public Control UI

The public URL is `https://claw.pylv.dev`. The Onyx Cloudflare Tunnel forwards this hostname directly to the loopback Gateway at `http://127.0.0.1:18789`. Cloudflare Access protects the hostname with the existing owner-email policy; OpenClaw separately requires its Gateway password and browser device pairing. The public route does not use the nginx bearer-token injection configured for the legacy proxy listeners.

The user-owned `openclaw-cloudflared.service` runs the connector independently of Nix and the Gateway service. Its private runtime directory is `~/.local/share/openclaw-cloudflared/`; `tunnel-token` is mode `0600`, and `package` is an indirect Nix GC root for its cloudflared runtime. User lingering keeps the service running after logout. Check it with `systemctl --user status openclaw-cloudflared.service` on Onyx. Update the runtime root explicitly when maintaining cloudflared; it is not upgraded by a Home Manager switch.

Mutable Gateway configuration keeps `gateway.bind` set to `loopback`, sets `gateway.publicOrigin` to `https://claw.pylv.dev`, and includes that exact HTTPS origin in `gateway.controlUi.allowedOrigins`. Preserve other allowed origins when editing the list. `gateway.trustedProxies` includes the loopback connector addresses so the Gateway can attribute Cloudflare's forwarded client IP. Keep password authentication and device checks enabled.

Use the authenticated `cf` CLI on `pylv-onyx` to inspect the remotely managed DNS and tunnel configuration:

```bash
cf dns records list --zone pylv.dev --name claw.pylv.dev --per-page 5
cf tunnels list --is-deleted false
cf tunnels config get d5120ed4-e5e0-4d21-9255-973d2b37c57c
cf zero-trust access applications list --domain '*.pylv.dev' --exact
```

Tunnel configuration updates replace the complete configuration. Preserve the existing Hermes ingress, origin options, and final catchall when changing the Claw route. Keep tunnel credentials outside the repository and the Nix store. An unauthenticated request to the public URL must reach Cloudflare Access; verify the UI and WebSocket connection after signing in as well.

## ChatGPT MCP relay

The [bundled MCP Relay plugin](https://docs.openclaw.ai/plugins/mcp-relay) connects outbound to OpenClaw's relay. It requires a release that includes `mcp-relay`; the published `2026.10.10-beta.1` package includes it, while `2026.10.5` stable does not. Application updates and relay configuration remain user-owned. Use a verified backup and an explicitly selected release when updating for this feature.

If `plugins.allow` is configured, append `mcp-relay` without removing existing entries before running `openclaw plugins enable mcp-relay`. Restart the Gateway and inspect the connection with `openclaw mcp-relay status --json`. Enabling the plugin does not authorize a ChatGPT account by itself.

Connect ChatGPT to `https://mcp.openclaw.ai/mcp`, then use the Gateway sign-in flow with `https://claw.pylv.dev` and approve the requested access in OpenClaw. Do not paste the Gateway password or token into ChatGPT. The relay connection uses TLS, but the relay service can read the relayed traffic. The public Control UI route and the outbound relay connection are separate.

## Independent agent configuration

OpenClaw owns its instructions, skills, plugins, and session state independently of Codex, Claude Code, and Pi. The shared `agent-core`, `modules/agent-skills`, and `agent-session-record` integrations target the coding agents only. Manage OpenClaw skills through its own workspace and configuration.

## Task routing across repositories

Configuration work uses the `gytkk/config-flakes` checkout at `~/ws`. Read its root `AGENTS.md` first:

- Home Manager and NixOS edits belong in `~/ws/`. Read the root `AGENTS.md` and run flake commands from `~/ws`. OpenClaw host integration belongs in `modules/openclaw/`.
- Coding-agent shared rules, adapters, and skills belong in `~/ws/agent-core/`. Read `agent-core/README.md`; these sources target coding agents, not OpenClaw.
- Applications use the private `gytkk/menu` and `gytkk/trading-agent` repositories, normally checked out at `~/menu` and `~/trading-agent`.
- Hardware work uses the private `gytkk/keyboards` and `gytkk/keycult-trackball` repositories, normally checked out at `~/keyboards` and `~/keycult-trackball`. Read each project's own instructions and run commands from its root. The former application and hardware subtrees are no longer part of the configuration checkout.

The private OpenClaw workspace checkout at `~/claw-space` (`gytkk/claw-space`) is independent. Its `AGENTS.md`, workspace skills, automation runbooks, and workspace skill validator are **not imported into this tree**. Project work targets the repositories above; `agent-core/` is not authoritative for OpenClaw instructions. Follow the runtime-provided workspace location until a separately authorized migration is verified. Keep personal memory, credentials, session data, and automation state outside tracked project files.

The standalone `~/claw-space` checkout replaces the former `~/development/ws` location without a compatibility symlink. Preparing a checkout is not a runtime cutover: preserve local-only context, ignore rules, permissions, and delivery/deduplication state, then update workspace discovery, skill/resource paths, and installed automation commands under explicit authorization. The workspace validator belongs at `~/claw-space/scripts/validate-workspace-skills.py`, not in this configuration repository. Audit active runbooks and installed jobs for old absolute paths, validate the configuration and skill discovery, and verify each affected workflow before retiring its old location. Preserve historical notes and vendor sources rather than rewriting them to look current. Moving files alone does not update running jobs or the current session context. No NixOS activation or live OpenClaw change is implied by project task routing.

## Removing the shared agent integration

Existing hosts require a one-time configuration change before applying the new Home Manager generation. The module does not edit mutable `openclaw.json` or delete previously installed files.

1. Back up the mutable OpenClaw configuration and the external `~/.local/share/openclaw/agent-core` and `~/.local/share/openclaw/extensions` trees. Preserve skill CLI locks, ownership records, and session recorder archives and queues separately; an OpenClaw state archive does not include these external resources.
2. In `openclaw.json`, remove only `~/.local/share/openclaw/agent-core/skills` from `skills.load.extraDirs` (usually stored as an absolute path). Remove the `agent-core-context` and `agent-session-record` directories from `plugins.load.paths`, their IDs from `plugins.allow` if present, and their entries from `plugins.entries`. Preserve unrelated paths, plugins, and workspace instructions.
3. Run `openclaw config validate`, restart the Gateway, and check `openclaw plugins doctor` before switching. This stops shared prompt injection and session capture while the old instruction file is still available.
4. Apply the NixOS configuration with `sudo nixos-rebuild switch --flake .#pylv-onyx`. Home Manager removes its generated instruction link and rewrites the service environment drop-in without `AGENT_CORE_OPENCLAW_INSTRUCTIONS`. Reload the user service manager and restart the Gateway so the running process uses the new environment.
5. Verify `openclaw config validate`, `openclaw plugins doctor`, and `openclaw skills list`. Confirm that the shared skill directory and removed plugins are no longer loaded. Preserve the old copied skill and extension directories as backups until this verification passes; switch does not delete them.

The current session recorder supports Claude and Codex only. Existing OpenClaw archives remain intact, but OpenClaw replay and queued uploads require the previous recorder revision. A queue retry with the new recorder quarantines unsupported OpenClaw items, including retries triggered by coding-agent hooks. Back up pending transcript and manifest pairs before switching and use the previous recorder revision to recover them. Skill installer backup receipts containing an OpenClaw target likewise require the previous installer revision for restoration.

Some OpenClaw versions also discover `~/.agents/skills` automatically; see the [OpenClaw skill loading documentation](https://docs.openclaw.ai/tools/skills). If that shared coding-agent directory is visible in `openclaw skills list`, configure OpenClaw's own skill selection or use an isolated runtime home to keep its skill catalog independent. Removing this repository's explicit `extraDirs` entry alone does not control upstream automatic discovery.

After configuration validation and a Gateway restart, create and verify a fresh state archive outside the state and workspace trees:

```bash
openclaw backup create --output ~/backups/openclaw --verify
```

A legacy link such as `~/.openclaw/codex-openclaw-home/auth.json` belongs to mutable host state. Inspect and resolve it separately before treating a state archive as complete; this module does not modify credentials.
