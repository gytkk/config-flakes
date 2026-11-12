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
sudo nixos-rebuild switch --flake .#pylv-sepia
```

Run from the configured checkout (`~/development/config-flakes` by default).
Home Manager installs out-of-store links to that checkout, so pulling the
reviewed revision there is part of applying a configuration update.

### Build memory

This host limits Nix to one package build at a time because concurrent package builds can exhaust its memory. Individual build tasks can still use multiple CPU cores.

When applying this limit for the first time from a local checkout, pass it explicitly because the new Nix settings take effect only after activation:

```bash
sudo nixos-rebuild switch --flake .#pylv-sepia --max-jobs 1
```

## Container services

Podman runs the Ghost container declared in
[the Ghost module](ghost.nix). The trading dashboard's container services come
from the private [trading-agent repository](https://github.com/gytkk/trading-agent).
Neither integration requires Docker Engine.

[The trading integration](trading-agent.nix) enables the product's NixOS module
for `trading.pylv.dev`, using UID 1000 and GID 100. It supplies the profile and
Access configuration under `~/.config/trading-agent`, imported databases under
`~/.local/share/trading-agent/imports/pylv-onyx-20261024/databases`, and the
account configuration and credentials under `~/.config/trading-agent/live-accounts`.

[The source pin](trading-agent-source.nix) selects the product revision; the
evaluating user's Git credentials must have read access. Review that revision
before activation. Container definitions, resource limits, health checks,
broker behavior, and product-specific deployment checks belong to that
revision's operating documentation, not a second copy in this repository.

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
[the Menu module](menu.nix). The server is intentionally not a recipient of either private key.
To recover the manual key from the repository root:

```bash
umask 077
agx -d menu-deploy-key.age > ~/.ssh/menu-deploy
ssh-keygen -y -f ~/.ssh/menu-deploy > ~/.ssh/menu-deploy.pub
public_key="$(cut -d ' ' -f 2 ~/.ssh/menu-deploy.pub)"
rg --fixed-strings "$public_key" systems/hosts/pylv-sepia/menu.nix
```

The Sveltia CMS GitHub PAT has a recovery-only encrypted backup at
`secrets/menu-github-pat.age`. Its recipients are the administrator and both
Devsisters workstations; neither NixOS server can decrypt it. Create or rotate
the backup from the repository root with:

```bash
agx -e menu-github-pat.age
```

Use a fine-grained PAT restricted to `gytkk/menu` with `Contents: Read and
write`, and give it a short expiration. To recover it for Sveltia, decrypt it
locally with `agx -d menu-github-pat.age`, paste it only into
the CMS token prompt on a trusted browser, and clear the terminal and clipboard
afterward. The `agx` wrapper selects the repository's secrets directory and
workstation identity. The secret is intentionally not declared under `age.secrets`,
deployed to a host, or included in the CMS configuration.

From the **Menu repository checkout** on an authorized deployment machine,
stream its verified `dist` archive with the exact commit and archive SHA-256:

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
[legacy Astro module](astro-blog-legacy.nix) keeps the old forced-command account, key and
`/srv/astro-blog` data available without serving the old nginx origin. Keep it
and the encrypted backup until two Menu releases and a rollback have been
verified; remove them only as a separate cleanup step.

## GitHub Actions runner

`gytkk/config-flakes` package CI and updates run on GitHub-hosted `ubuntu-latest`
runners, not this production host. The former self-hosted runner's 8 GiB limit
did not cover builds in `nix-daemon.service`, so CI could exhaust host memory.

After pulling the migration, stop the old runner before applying the NixOS
configuration that removes its service and decrypted token:

```bash
sudo systemctl stop github-runner-ws.service
sudo nixos-rebuild switch --flake .#pylv-sepia
```

Remove the obsolete `ws` runner registration in the repository's GitHub Actions
settings and revoke its dedicated PAT. Existing runner cache/state is not deleted
by this migration.

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

Publication settings, staff accounts, routes, and themes remain managed in
Ghost Admin. This flake does not implement Admin API settings reconciliation
or install a custom Ghost theme.

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

User packages are managed via Home Manager modules in `systems/base/pylv/`.

Edit `systems/base/pylv/home.nix` or `systems/base/pylv/sepia.nix`:

```nix
home.packages = with pkgs; [
  # add packages here
];
```

## File Structure

| File | Description |
| --- | --- |
| [Host configuration](configuration.nix) | NixOS system configuration and host module imports |
| [Ghost](ghost.nix) | Ghost container, loopback nginx origin, and local backups |
| [Menu](menu.nix) | Menu static site origin and deployment integration |
| [Legacy Astro deployment](astro-blog-legacy.nix) | Previous deployment account and data retained for rollback |
| [Trading integration](trading-agent.nix) / [source pin](trading-agent-source.nix) | Product module and host runtime paths |
| [Disk layout](disk-config.nix) | Disk partitioning configuration (disko) |
| [Hardware configuration](hardware-configuration.nix) | Auto-generated hardware configuration |

## Shared Modules

This configuration uses shared modules from the main flake:

- `modules/git` - Git configuration
- `modules/zsh` - Zsh with Starship and the generated canonical theme
- `modules/vim` - Neovim configuration
- `modules/claude` - Claude Code configuration
