# Agent skills activation

The common Home Manager profile imports [default.nix](default.nix) to install or update skills for enabled Codex, Claude, and Pi agents during each switch. Any enabled Codex or Pi module enables the shared target once; Claude enables its global copy target.

Home Manager links each selected local Codex/Pi skill directory from the Nix store into `~/.agents/skills/`. The parent directory remains user-managed. Edit local skills in the repository and switch to apply them. Claude's selected skills and external Codex/Pi skills still use the [installation adapter](../../scripts/install-agent-skills.py) after `linkGeneration`. It resolves each external source once across targets; activation needs network access for external sources.

The shared target's manifest runtimes describe consumers, not Skills CLI agent IDs. The adapter keeps `--agent codex pi` to install into `~/.agents/skills`.

After resolving external revisions, the adapter skips staging, license downloads, CLI installation, and backups when every target matches its last successful receipt. It compares the complete installation plan, including the CLI version, external revisions, local file contents and modes, ownership records, installed files, and selected CLI lock entries. Unchanged switches still check external source revisions. The manifest is authoritative: when an installed skill is missing, modified, recorded under another source, or not owned, the adapter runs the full installation, backs up the existing directory, and replaces it. Previously managed skills that the manifest no longer selects are backed up and removed. An older installation without this metadata runs the full installation once. Failed operations do not provide a successful cache record.

Local source fingerprinting accepts regular files shared through Nix store hardlinks and records their content and modes. Installed skills and backups must contain independent regular files; symlinks and special files are rejected in both source and installation trees.

If installation damages an ownership record or CLI lock JSON, the failed receipt retains the original error and snapshots the damaged bytes. Restoration requires those bytes and the installed files to remain unchanged. A damaged lock is restored in full from its pre-installation bytes, including unrelated entries; valid locks still restore only selected entries. If metadata cannot be read for a byte snapshot, the receipt records the failure and restoration requires manual preservation rather than overwriting an unknown state.

CLI and Git commands run in separate process groups with a 300-second timeout. Before collecting installation state or releasing the installer lock, the adapter kills remaining processes in the command's group and waits up to five seconds to reap its launcher. This also applies to interruptions and commands whose launcher exits while a child keeps output pipes open. Descendants that deliberately create another session or process group are outside this cleanup boundary.

Before switching an existing shared pool from CLI copies to links, run the following from the **updated checkout**, using the same `XDG_STATE_HOME` as activation (normally `~/.local/state`). This one-time install backs up previously owned copies, including local edits and the retired `pi-agent` skill, and releases those paths from CLI ownership. Inspect the reported backup before switching. If an unowned directory still conflicts, preserve it separately; do not force Home Manager to overwrite it.

```bash
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
uv run --project agent-core scripts/install-agent-skills.py install --target shared --apply
```

Fresh hosts do not need this step. On rollback, deactivate the Home Manager links before restoring CLI copies. See [installation and recovery](../../agent-core/README.md#skill-설치와-복구) for legacy discovery paths and receipt restoration.
