# pylv-sepia

NixOS configuration for pylv-sepia server, managed from the main flake.

## Initial Installation

```bash
nix run github:nix-community/nixos-anywhere -- \
  --flake .#pylv-sepia \
  root@<hostname>
```

## Updating Configuration

### From Local Machine

```bash
# From repository root
nix run nixpkgs#nixos-rebuild -- switch \
  --flake .#pylv-sepia \
  --target-host root@<hostname>
```

### On the Server

```bash
sudo nixos-rebuild switch --flake github:gytkk/nix-flakes#pylv-sepia
```

### Build memory

This host limits Nix to one package build at a time because concurrent package builds can exhaust its memory. Individual build tasks can still use multiple CPU cores.

When applying this limit for the first time from a local checkout, pass it explicitly because the new Nix settings take effect only after activation:

```bash
sudo nixos-rebuild switch --flake .#pylv-sepia --max-jobs 1
```

## Container services

Podman runs the application containers declared with NixOS `virtualisation.oci-containers`. Each application has a separate module, fixed image version, persistent data outside the image, and systemd services with health checks and journal logs. Ghost uses `ghost.nix`; the authenticated read-only trading dashboard uses `trading-agent.nix` and the module exported by the trading-agent source. Neither application requires Docker Engine.

| Container | Service | Memory limit | CPU limit | Persistent state |
| --- | --- | --- | --- | --- |
| `ghost` | `podman-ghost` | 2 GiB | 2 CPUs | `/var/lib/ghost` |
| `trading-agent-admin` | `podman-trading-agent-admin` | 512 MiB | 1 CPU | Access configuration and read-only IPC mount |
| `trading-agent-reader` | `podman-trading-agent-reader` | 512 MiB | 2 CPUs | Read-only imported profile databases |
| `trading-agent-accounts` | `podman-trading-agent-accounts` | 512 MiB | 1 CPU | Read-only broker queries, private credentials and persistent account snapshots |

Trading-agent owns its image build, Cargo and Bun dependency hashes, and container service definitions. This host pins its source in `trading-agent-source.nix` and supplies only runtime paths, hostname, and process UID/GID. The initial pin fetches an exact committed revision from the local repository at `/home/gytkk/workspace/trading-agent`; that revision has not been published. Rebuilding on another machine requires making that commit available at the same local repository path or replacing the source URL with an accessible repository containing the verified commit. Running containers use immutable Nix store outputs and do not execute files from the checkout.

The admin binds to `127.0.0.1:8765` using host networking, preserving the existing Cloudflare Tunnel route and Access verification. Its container mounts only `~/.config/trading-agent/access.toml` and the read-only IPC directory. The reader mounts `~/.config/trading-agent/profiles.toml`, the imported database directory at its existing absolute path, the current-account snapshots directory read-only, and the writable private socket directory `/run/trading-agent-ipc`. This directory is separate from the per-container RuntimeDirectory managed by oci-containers. The processes use UID 1000 and GID 100 to preserve file ownership and IPC peer checks. The admin and reader cannot read broker credentials or tokens. The earlier `trade-admin` and `trade-admin-reader` user units are disabled after cutover.

The account monitor mounts its private registry, EngineConfig files and credential directory from `~/.config/trading-agent/live-accounts/`. It attempts to query KIS domestic stocks, KIS US stocks and Toss holdings on a 60-second schedule with `read_only=true` and live writes disabled; slow broker responses can delay refreshes. It stores broker tokens under `/var/lib/trading-agent-account-state/broker/` and append-only query results under `snapshots/accounts.sqlite3`. Only the snapshots subdirectory is shared with the reader. The dashboard defaults to current account valuations; historical execution data remains under the profile list. No trading supervisor, advisor or order execution process is started.

```bash
sudo systemctl status podman-ghost podman-trading-agent-admin podman-trading-agent-reader podman-trading-agent-accounts
sudo podman ps
sudo journalctl -u podman-trading-agent-admin -u podman-trading-agent-reader -u podman-trading-agent-accounts
```

Ghost's health check requests its local HTTP endpoint. Trading's reader health check verifies socket creation, its web health check verifies HTTP 401 for an unauthenticated request, and the account monitor health check verifies snapshot database creation. A healthy container does not establish broker connectivity or current account balances. After deployment, separately verify profile queries, account query states and Cloudflare Access login. The account monitor logs only source aliases and outcome codes. Database backups and restore procedures remain separate from image rollback; see the Ghost backup section below and the trading-agent repository's operating documentation.

After deployment or a reader restart, run the IPC check from the local trading-agent checkout. It verifies private directory and socket ownership, registered profile database availability, and rejection of operating controls. This catches ownership changes that a socket-existence health check cannot detect.

```bash
bun trade-engine/scripts/check-admin-ipc.mjs --socket /run/trading-agent-ipc/read.sock --uid 1000 --expected-profiles 5
```

## Menu by pylv

The static Astro origin listens only on `127.0.0.1:12369` for
`menu.pylv.dev`; it is intentionally absent from the firewall. Nginx serves
`/srv/menu/current`, an atomic symlink to a validated release.

The dedicated `menu-deploy` account accepts only its forced SSH command. The
manual client private key lives at `~/.ssh/menu-deploy`; an encrypted recovery
copy for the administrator and deployment Mac is stored in
`secrets/menu-deploy-key.age`. GitHub Actions uses a separate CI key whose
private half exists only as the `MENU_DEPLOY_SSH_KEY` repository secret. Both
public keys have the same forced command and forwarding restrictions in
`menu.nix`. The server is intentionally not a recipient of either private key.
To recover the manual key from the repository root:

```bash
umask 077
(
  cd secrets
  RULES=./secrets.nix agenix -d menu-deploy-key.age
) > ~/.ssh/menu-deploy
ssh-keygen -y -f ~/.ssh/menu-deploy > ~/.ssh/menu-deploy.pub
public_key="$(cut -d ' ' -f 2 ~/.ssh/menu-deploy.pub)"
rg --fixed-strings "$public_key" hosts/pylv-sepia/menu.nix
```

The Sveltia CMS GitHub PAT has a recovery-only encrypted backup at
`secrets/menu-github-pat.age`. Its recipients are the administrator and both
Devsisters workstations; neither NixOS server can decrypt it. Create or rotate
the backup from the repository root with:

```bash
(
  cd secrets
  agx -e menu-github-pat.age
)
```

Use a fine-grained PAT restricted to `gytkk/menu` with `Contents: Read and
write`, and give it a short expiration. To recover it for Sveltia, decrypt it
locally with `agx -d menu-github-pat.age` from `secrets/`, paste it only into
the CMS token prompt on a trusted browser, and clear the terminal and clipboard
afterward. The secret is intentionally not declared under `age.secrets`,
deployed to a host, or included in the CMS configuration.

From an authorized deployment machine, stream a verified tar archive with the
exact commit and archive SHA-256:

```bash
archive="$(mktemp)"
trap 'rm -f "$archive"' EXIT
tar --create --file "$archive" --directory dist .
revision="$(git rev-parse HEAD)"
archive_sha256="$(sha256sum "$archive" | cut --delimiter=' ' --fields=1)"
ssh -i ~/.ssh/menu-deploy menu-deploy@pylv-sepia \
  "deploy $revision $archive_sha256" < "$archive"
```

The archive must include `index.html`, `404.html`, `rss.xml`,
`sitemap-index.xml`, `admin/index.html`, and `admin/config.yml`. The server
validates and extracts it into `/srv/menu/releases/<sha>/`, then switches
`current` without restarting nginx. To roll back a retained validated release:

```bash
ssh -i ~/.ssh/menu-deploy menu-deploy@pylv-sepia \
  "rollback <40-or-64-lowercase-hex-sha>"
```

Run the following checks on `pylv-sepia` after deployment:

```bash
curl --fail --header 'Host: menu.pylv.dev' http://127.0.0.1:12369/
readlink -f /srv/menu/current
```

GitHub-hosted deployment runners join the tailnet with Tailscale workload
identity federation and the `tag:github-actions-menu` tag. Tailnet policy must
permit that tag to reach only `100.65.157.38` on `tcp:22`. The federated
identity must require GitHub's OIDC issuer, the `gytkk/menu` repository,
`refs/heads/main`, and the deployment workflow, and must have writable
`auth_keys` scope for the same tag. The workflow pins the non-secret federated
identity client ID and audience alongside the server's Ed25519 host key, and
serializes production deployments. Keep only the CI SSH private key in a
GitHub repository secret.

Cloudflare Tunnel routing and the public hostname are managed separately in
Cloudflare and are not configured by this flake. The transitional
`astro-blog-legacy.nix` module keeps the old forced-command account, key and
`/srv/astro-blog` data available without serving the old nginx origin. Keep it
and the encrypted backup until two Menu releases and a rollback have been
verified; remove them only as a separate cleanup step.

## Ghost

Ghost runs as one declarative Podman container using the
`ghost:6.53.0-bookworm` release pinned by OCI digest. The container runs Ghost
in production mode with SQLite and persists its complete content directory at
`/var/lib/ghost` on the host. No database password or other Ghost secret is
stored in Nix.

The container shares the host network, but Ghost binds only to
`127.0.0.1:2368`. This avoids a Podman DNAT dependency while keeping the
application private. Nginx provides the Cloudflare Tunnel origin only at
`http://127.0.0.1:12368`, forwards requests to Ghost, and fixes the public
scheme and host as `https://ghost.pylv.dev`. Neither port is opened in the
NixOS firewall.

The Ghost 6 runtime in the image accepts the SQLite configuration override, but
the Docker Official Image documentation only supports the SQLite filename in
development mode and requires MySQL for its supported production deployment.
Treat this single-instance production SQLite deployment as an intentional,
locally operated exception and test Ghost upgrades against a restored backup
before changing the pinned image.

### Declarative Settings and Theme Plan

Status: proposed. Implement this only after approving the remaining product and
security decisions below.

#### Desired State

- Anyone can read public posts without creating an account.
- Member signup, newsletters, and comments are disabled initially.
- Staff continue to authenticate through Ghost Admin at `/ghost`.
- Supported publication settings are reconciled through the Ghost Admin API;
  the SQLite database is never edited directly.
- A repository-managed custom theme controls layout and styling.
- Secrets remain encrypted with agenix and never enter the Nix store, generated
  units, logs, or Git history.

The initial managed Ghost settings are:

| Ghost setting | Desired value | Purpose |
| --- | --- | --- |
| `is_private` | `false` | Allow unauthenticated site visits |
| `members_signup_access` | `none` | Disable member signup and member features |
| `default_content_visibility` | `public` | Make new posts public by default |
| `comments_enabled` | `off` | Disable member-only comments |

#### Configuration Boundaries

| Concern | Source of truth |
| --- | --- |
| Container, database, URL, mail, storage, and proxy | `ghost.nix` |
| Selected publication settings and navigation | Admin API desired-state file |
| Owner account and initial custom integration | One-time Ghost Admin bootstrap |
| Admin API credential | agenix secret |
| Layout, templates, CSS, JavaScript, and theme defaults | Repository-managed Ghost theme |
| Posts, pages, images, and other editorial content | Ghost content database and uploads |
| Public hostname and optional admin access policy | Cloudflare Zero Trust dashboard |

Editorial content should not be continuously reconciled by Nix. This keeps
normal authoring in Ghost Admin independent from infrastructure deployments.

#### Proposed Repository Layout

```text
hosts/pylv-sepia/
├── ghost.nix
└── ghost/
    ├── settings.json
    ├── sync-settings.js
    ├── routes.yaml
    └── theme/
        ├── package.json
        ├── default.hbs
        ├── index.hbs
        ├── post.hbs
        ├── page.hbs
        ├── partials/
        └── assets/
            ├── css/
            ├── js/
            └── images/
```

Keep the sync script narrow: it should update only keys explicitly present in
`settings.json`. It must not overwrite unrelated settings changed through Ghost
Admin.

#### Implementation Phases

1. **Finish the current runtime rollout**
   - Activate the host-network and backup ownership fix.
   - Run a manual backup and verify the SQLite integrity check and archive.
   - Confirm the origin returns HTTP 200 after a Ghost restart.

2. **Bootstrap Admin API access**
   - Create the Owner account manually if it does not exist.
   - Create one custom integration dedicated to configuration reconciliation.
   - Encrypt its Admin API key as a new agenix secret.
   - Confirm the key can read and update settings without printing it in logs.

3. **Add idempotent settings reconciliation**
   - Add the desired-state JSON and a small Admin API client.
   - Generate short-lived Admin API JWTs only in memory.
   - Add `ghost-settings-sync.service` after `podman-ghost.service`.
   - Wait for the Ghost health check before calling the API, use bounded retries,
     and fail with actionable errors.
   - Snapshot the currently managed values before the first write so they can be
     restored without editing SQLite.

4. **Add the repository-managed theme**
   - Start from the current Source theme behavior rather than rebuilding every
     feature at once.
   - Package the theme from the repository and mount it read-only into
     `/var/lib/ghost/content/themes`.
   - Validate it with GScan during checks.
   - Activate it through the Admin API only after validation and Ghost startup.
   - Keep Source available as the immediate fallback theme.
   - Put adjustable colors, typography, and layout choices in Ghost custom theme
     settings; keep structural layout in Handlebars and CSS.

5. **Add routing only when required**
   - Keep Ghost's default routes for the first theme release.
   - Add and mount `routes.yaml` only for a concrete collection, channel, or URL
     requirement.
   - Validate feeds, pagination, tags, authors, and canonical URLs after routing
     changes.

6. **Make the publication public safely**
   - Remove the current host-wide Cloudflare Access requirement before launch.
   - Prefer Ghost's built-in Staff authentication initially.
   - If Cloudflare Access is retained for admin defense in depth, do not protect
     `/ghost*` blindly: explicitly verify that the public Content API,
     ActivityPub endpoints, assets, feeds, and any member endpoints remain
     reachable without Cloudflare authentication.

#### Verification and Rollout

Before committing each logical change:

```bash
nixfmt hosts/pylv-sepia/ghost.nix
nix flake check --no-build
nix build .#nixosConfigurations.pylv-sepia.config.system.build.toplevel
```

Also verify:

- GScan accepts the packaged theme.
- The generated Podman unit mounts the intended theme and no secret value.
- The sync unit is idempotent and a second run makes no unexpected changes.
- An unauthenticated request can read the home page and a public post.
- Member signup, newsletter subscription, and comments are unavailable.
- Ghost Admin still accepts the Owner account.
- RSS, sitemap, public assets, Content API, and ActivityPub endpoints do not
  redirect to Cloudflare Access.
- A manual `ghost-backup.service` run produces an integrity-checked archive.

Activate only after the checks pass:

```bash
sudo nixos-rebuild switch --flake .#pylv-sepia
```

#### Rollback

- Retain a pre-change backup and the API snapshot of managed settings.
- Revert the relevant Git commit and switch the previous NixOS configuration.
- Reactivate Source through Ghost Admin or the Admin API if the custom theme
  fails.
- Restore managed settings through the API snapshot; do not patch SQLite.

#### Decisions Required Before Implementation

- Select the first theme direction: Source-derived minimal customization or a
  new visual design.
- Decide whether Ghost Staff authentication alone is sufficient or whether a
  carefully scoped Cloudflare Access policy is required for admin routes.
- Decide whether navigation should be API-managed or remain editorial.
- Select an SMTP provider before enabling password recovery, staff invitations,
  or members. Select Mailgun separately before enabling newsletters.
- Decide whether GScan is run through a pinned package derivation or a separate
  repository check.

### Deploy

Review and activate the configuration from the repository root:

```bash
nix flake check --no-build
sudo nixos-rebuild switch --flake .#pylv-sepia
```

The first activation pulls the pinned image and starts `podman-ghost.service`.
This repository does not configure the Cloudflare public hostname.

In Cloudflare Zero Trust, open the existing sepia tunnel and add this public
hostname:

- Public hostname: `ghost.pylv.dev`
- Service type: `HTTP`
- Origin URL: `http://127.0.0.1:12368`

No new tunnel token, secret, DNS listener, or firewall rule is required.

### Status and Restart

```bash
sudo systemctl status podman-ghost.service nginx.service
sudo podman healthcheck run ghost
curl --fail --head \
  --header 'Host: ghost.pylv.dev' \
  http://127.0.0.1:12368/
sudo journalctl --unit podman-ghost.service --unit nginx.service
```

Restart Ghost without restarting the Cloudflare Tunnel:

```bash
sudo systemctl restart podman-ghost.service
```

### Local Backups

`ghost-backup.timer` runs daily with up to one hour of randomized delay. It
stops `podman-ghost.service`, uses SQLite's online backup command, adds the rest
of `/var/lib/ghost` (including uploads, themes, and settings), writes archives
under `/var/lib/ghost-backups`, and removes archives older than 30 days. Because
Ghost remains stopped while both the database snapshot and content archive are
created, each completed archive is a coherent restore point. The backup starts
Ghost again after either success or failure.

Each daily or manually started backup briefly interrupts the blog while the
archive is created.

```bash
sudo systemctl status ghost-backup.timer
sudo systemctl start ghost-backup.service
sudo journalctl --unit ghost-backup.service
sudo ls -lh /var/lib/ghost-backups/ghost-*.tar.gz
```

These are local backups on the same server. They do not protect against host or
disk loss. Copy and verify them in an off-host backup system.

### Restore

Choose an archive, extract it to a staging directory, and verify the database
before stopping Ghost:

```bash
backup=/var/lib/ghost-backups/ghost-YYYYMMDDTHHMMSSZ.tar.gz
restore=/var/lib/ghost-restore
sudo test ! -e "$restore"
sudo install -d -m 0700 "$restore"
sudo tar -xzf "$backup" -C "$restore"
sudo "$(command -v sqlite3)" -readonly \
  "$restore/data/ghost.db" \
  'PRAGMA integrity_check;'
```

The integrity check must print `ok`. Then replace the content directory while
Ghost is stopped, retaining the previous directory as a local rollback:

```bash
restore=/var/lib/ghost-restore
rollback="/var/lib/ghost.before-restore-$(date --utc +%Y%m%dT%H%M%SZ)"
sudo systemctl stop podman-ghost.service
sudo mv /var/lib/ghost "$rollback"
sudo mv /var/lib/ghost-restore /var/lib/ghost
sudo chown -R 1000:1000 /var/lib/ghost
sudo chmod 0750 /var/lib/ghost
sudo systemctl start podman-ghost.service
sudo podman healthcheck run ghost
```

After verifying the public site and Ghost Admin, retain or remove the rollback
directory according to the server's storage policy.

## Adding Packages

User packages are managed via Home Manager modules in `base/pylv/`.

Edit `base/pylv/home.nix` or `base/pylv/sepia.nix`:

```nix
home.packages = with pkgs; [
  # add packages here
];
```

## File Structure

| File | Description |
| --- | --- |
| `configuration.nix` | NixOS system configuration and host module imports |
| `ghost.nix` | Ghost container, loopback nginx origin, and local backups |
| `menu.nix` | Menu static site origin and deployment integration |
| `disk-config.nix` | Disk partitioning configuration (disko) |
| `hardware-configuration.nix` | Auto-generated hardware configuration |

## Shared Modules

This configuration uses shared modules from the main flake:

- `modules/git` - Git configuration
- `modules/zsh` - Zsh with Starship and the generated canonical theme
- `modules/vim` - Neovim configuration
- `modules/claude` - Claude Code configuration
