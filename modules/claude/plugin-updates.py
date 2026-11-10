"""Skip Claude updates only when sources and the last successful install match."""

import argparse
import fcntl
import hashlib
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path


def read_json(path, default=None):
    if not path.exists():
        return default
    return json.loads(path.read_text())


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def tree(path, *, installed=False):
    if not path.is_dir() or path.is_symlink():
        return None
    files = {}
    for item in sorted(path.rglob("*")):
        relative = item.relative_to(path)
        if ".git" in relative.parts:
            continue
        if item.is_symlink():
            # Following arbitrary links could miss changes outside the plugin.
            return None
        if item.is_file():
            mode = item.stat().st_mode & 0o777
            # Activation repairs shell executable bits after this helper returns.
            if installed and item.suffix == ".sh":
                mode |= 0o111
            files[str(relative)] = [mode, hashlib.sha256(item.read_bytes()).hexdigest()]
        elif not item.is_dir():
            return None
    return digest(files)


class Updater:
    def __init__(self, home, cache, claude):
        self.root = home / ".claude/plugins"
        self.cache_path = cache
        self.claude = claude
        self.remote_revisions = {}
        self.marketplace_revisions = {}
        try:
            self.cache = read_json(cache, {})
            if not isinstance(self.cache, dict):
                raise TypeError("expected an object")
        except (OSError, ValueError, TypeError) as error:
            print(f"Ignoring invalid plugin update cache: {error}", flush=True)
            self.cache = {}

    def command(self, args, timeout=30):
        result = subprocess.run(
            args,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=True,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
        )
        return result.stdout.strip()

    def remote(self, url, ref=None):
        key = (url, ref)
        if key not in self.remote_revisions:
            refs = (
                [ref, f"refs/heads/{ref}", f"refs/tags/{ref}", f"refs/tags/{ref}^{{}}"]
                if ref
                else ["HEAD"]
            )
            rows = self.command(["git", "ls-remote", url, *refs]).splitlines()
            matches = dict(row.split() for row in rows)
            # Annotated tags identify the commit through their peeled ref.
            peeled = [sha for sha, name in matches.items() if name.endswith("^{}")]
            values = peeled or list(matches)
            if len(set(values)) != 1 or not re.fullmatch(r"[0-9a-f]{40}", values[0]):
                raise ValueError("remote ref does not identify one commit")
            self.remote_revisions[key] = values[0]
        return self.remote_revisions[key]

    def cli(self, *args):
        print("Running Claude: " + " ".join(args), flush=True)
        try:
            output = self.command([self.claude, "plugin", *args], timeout=60)
            if output:
                print(output, flush=True)
            return True
        except (OSError, subprocess.SubprocessError) as error:
            print(f"Claude {' '.join(args)} failed: {error}", flush=True)
            if isinstance(error, subprocess.CalledProcessError):
                print((error.stdout or "") + (error.stderr or ""), flush=True)
            return False

    def marketplace(self, name, entry):
        source = entry.get("source", {})
        if source.get("source") == "directory":
            return True
        path = Path(entry.get("installLocation", ""))
        key = "marketplace:" + name
        revision = None
        try:
            if source.get("source") not in ("github", "git") or not path.is_dir():
                raise ValueError("marketplace source cannot be checked")
            checkout = (path / ".git").exists()
            url = (
                self.command(["git", "-C", str(path), "remote", "get-url", "origin"])
                if checkout
                else "https://github.com/" + source["repo"] + ".git"
                if source["source"] == "github"
                else source["url"]
            )
            revision = self.remote(url, source.get("ref"))
            if checkout:
                current = self.command(["git", "-C", str(path), "rev-parse", "HEAD"])
                # The existing activation chmods shell scripts in marketplace clones.
                dirty = self.command(
                    [
                        "git",
                        "-C",
                        str(path),
                        "-c",
                        "core.fileMode=false",
                        "status",
                        "--porcelain",
                        "--untracked-files=no",
                    ]
                )
                unchanged = current == revision and not dirty
            else:
                content = tree(path, installed=True)
                unchanged = content is not None and self.cache.get(key) == {
                    "source": source,
                    "revision": revision,
                    "content": content,
                }
            if unchanged and (path / ".claude-plugin/marketplace.json").is_file():
                self.marketplace_revisions[name] = revision
                print(f"Marketplace unchanged: {name}", flush=True)
                return True
        except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
            print(
                f"Cannot confirm marketplace revision for {name}: {error}", flush=True
            )
        self.cache.pop(key, None)
        self.save()
        success = self.cli("marketplace", "update", name)
        if success and revision is not None:
            self.marketplace_revisions[name] = revision
            content = tree(path, installed=True)
            if (
                content is not None
                and (path / ".claude-plugin/marketplace.json").is_file()
            ):
                self.cache[key] = {
                    "source": source,
                    "revision": revision,
                    "content": content,
                }
                self.save()
        return success

    def plugin_source(self, plugin, marketplace):
        name = plugin.rsplit("@", 1)[0]
        root = Path(marketplace["installLocation"])
        catalog = read_json(root / ".claude-plugin/marketplace.json")
        entry = next(item for item in catalog["plugins"] if item["name"] == name)
        source = entry.get("source")
        if isinstance(source, str) and source.startswith("./"):
            content = tree(root / source)
            if content is None:
                raise ValueError("plugin source is missing or contains symlinks")
            revision = content
            # Versionless Git plugins use the repository revision as their version.
            manifest = read_json(root / source / ".claude-plugin/plugin.json", {})
            if (
                not (manifest.get("version") or entry.get("version"))
                and marketplace.get("source", {}).get("source") != "directory"
            ):
                revision += self.marketplace_revisions.get(
                    plugin.rsplit("@", 1)[1]
                ) or self.command(["git", "-C", str(root), "rev-parse", "HEAD"])
        elif isinstance(source, dict) and source.get("source") in (
            "github",
            "url",
            "git-subdir",
        ):
            url = (
                "https://github.com/" + source["repo"] + ".git"
                if source["source"] == "github"
                else source["url"]
            )
            revision = source.get("sha") or self.remote(url, source.get("ref"))
        else:
            raise ValueError("plugin source cannot be checked without invoking Claude")
        return digest([1, self.claude, marketplace.get("source"), entry, revision])

    def installed(self, plugin):
        entries = (
            read_json(self.root / "installed_plugins.json", {})
            .get("plugins", {})
            .get(plugin, [])
        )
        users = [entry for entry in entries if entry.get("scope") == "user"]
        if len(users) != 1:
            return None
        entry = users[0]
        path = entry.get("installPath")
        content = tree(Path(path), installed=True) if path else None
        return {"entry": entry, "content": content} if content is not None else None

    def save(self):
        self.cache_path.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(
            mode="w", dir=self.cache_path.parent, delete=False
        ) as file:
            json.dump(self.cache, file, sort_keys=True)
        Path(file.name).replace(self.cache_path)

    def update(self, plugins):
        known = read_json(self.root / "known_marketplaces.json", {})
        ready = {}
        for name in dict.fromkeys(plugin.rsplit("@", 1)[1] for plugin in plugins):
            ready[name] = self.marketplace(name, known.get(name, {}))
        known = read_json(self.root / "known_marketplaces.json", {})
        for plugin in plugins:
            marketplace = plugin.rsplit("@", 1)[1]
            source = None
            try:
                if ready[marketplace]:
                    source = self.plugin_source(plugin, known[marketplace])
            except (
                OSError,
                ValueError,
                KeyError,
                TypeError,
                StopIteration,
                subprocess.SubprocessError,
            ) as error:
                print(f"Cannot confirm plugin source for {plugin}: {error}", flush=True)
            try:
                installed = self.installed(plugin)
            except (OSError, ValueError, KeyError, TypeError) as error:
                print(f"Cannot confirm installed plugin {plugin}: {error}", flush=True)
                installed = None
            record = {"source": source, "installed": installed}
            if (
                source is not None
                and installed is not None
                and self.cache.get(plugin) == record
            ):
                print(f"Plugin unchanged: {plugin}", flush=True)
                continue
            # Invalidate before mutation so failure cannot leave a successful record.
            self.cache.pop(plugin, None)
            self.save()
            if (
                self.cli("update" if installed else "install", "-s", "user", plugin)
                and source is not None
            ):
                installed = self.installed(plugin)
                if installed is not None:
                    self.cache[plugin] = {"source": source, "installed": installed}
                    self.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", required=True)
    parser.add_argument("--plugins", type=Path, required=True)
    args = parser.parse_args()
    home = Path.home()
    cache = (
        Path(os.environ.get("XDG_CACHE_HOME", home / ".cache"))
        / "claude-nix/plugin-updates.json"
    )
    cache.parent.mkdir(parents=True, exist_ok=True)
    with cache.with_suffix(".lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        Updater(home, cache, args.claude).update(read_json(args.plugins))


if __name__ == "__main__":
    main()
