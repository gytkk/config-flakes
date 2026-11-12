#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
APP_DIR="$TEST_ROOT/apps/pi"
export FIXTURE_SOURCE="$TEST_ROOT/source"
export FIXTURE_TAG="v99.1.2"
export FIXTURE_DOWNLOAD_FAILURE="0"
mkdir -p "$APP_DIR" "$TEST_ROOT/bin" "$FIXTURE_SOURCE/nix" "$FIXTURE_SOURCE/packages/coding-agent/install-lock"
cp "$SOURCE_ROOT/apps/pi/"{update.sh,patches.nix,herdr-file-links.patch,message-renderer-wrappers.patch} "$APP_DIR/"
mkdir -p "$TEST_ROOT/scripts" "$FIXTURE_SOURCE/packages/coding-agent/src/utils"
cp "$SOURCE_ROOT/scripts/check-nix-patches.sh" "$TEST_ROOT/scripts/"
cat > "$FIXTURE_SOURCE/packages/coding-agent/src/utils/open-browser.ts" <<'EOF'
import { spawn } from "node:child_process";

/**
 * Open a URL or file in the platform browser/default handler.
 *
 * This intentionally never invokes a shell. On Windows, do not use
 * `cmd /c start`: cmd.exe re-parses metacharacters (&, |, ^, ...) before
 * `start` runs, which would make attacker-controlled URLs injectable.
 */
export function openBrowser(target: string): void {
	const [cmd, args]: [string, string[]] =
		process.platform === "darwin"
			? ["open", [target]]
			: ["xdg-open", [target]];
	spawn(cmd, args).unref();
}
EOF
mkdir -p "$FIXTURE_SOURCE/packages/tui/src"
cat > "$FIXTURE_SOURCE/packages/tui/src/tui-alt-screen.ts" <<'EOF'
				this.selectionAnchor = undefined;
				this.selectionFocus = undefined;
				try {
					this.openUrl(clickedUrl);
				} catch {
					// URL activation is best-effort.
				}
EOF
# Populate the wrapper patch's old-side contexts; rendering behavior has its own runtime suite.
uv run --no-project python - "$APP_DIR/message-renderer-wrappers.patch" "$FIXTURE_SOURCE" <<'PY'
import pathlib
import re
import sys

files = {}
path = None
index = None
for line in pathlib.Path(sys.argv[1]).read_text().splitlines():
    if line.startswith("--- a/"):
        path = line.removeprefix("--- a/")
        files.setdefault(path, [])
        index = None
    elif match := re.match(r"@@ -(\d+)", line):
        index = int(match[1]) - 1
    elif index is not None and (line.startswith((" ", "-")) or not line):
        text = line[1:] if line else ""
        lines = files[path]
        while len(lines) < index:
            lines.append("")
        if index < len(lines):
            assert lines[index] == text, (path, index)
        else:
            lines.append(text)
        index += 1
for path, lines in files.items():
    target = pathlib.Path(sys.argv[2]) / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(lines) + "\n")
PY
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
for file in nix/package.nix nix/model-catalog.json packages/coding-agent/install-lock/package-lock.json package-lock.json; do
  touch "$FIXTURE_SOURCE/$file"
done

printf 'pname = "pi-workspace-packages";\n' > "$FIXTURE_SOURCE/nix/package.nix"

cat > "$TEST_ROOT/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "${!#}" = 'https://api.github.com/repos/earendil-works/pi/releases/latest' ]
jq -n --arg tag "$FIXTURE_TAG" '{tag_name: $tag}'
EOF
cat > "$TEST_ROOT/bin/nix" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "$*" = 'store prefetch-file --unpack --json https://github.com/earendil-works/pi/archive/refs/tags/v99.1.2.tar.gz' ]
[ "$FIXTURE_DOWNLOAD_FAILURE" = 0 ]
jq -n --arg path "$FIXTURE_SOURCE" '{hash: "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", storePath: $path}'
EOF
chmod +x "$TEST_ROOT/bin/curl" "$TEST_ROOT/bin/nix"
export PATH="$TEST_ROOT/bin:$PATH"

reset_package() {
  cp "$SOURCE_ROOT/apps/pi/package.nix" "$APP_DIR/package.nix"
  cp "$APP_DIR/package.nix" "$TEST_ROOT/original.nix"
}

assert_failure() {
  if bash "$APP_DIR/update.sh" > "$TEST_ROOT/output" 2>&1; then
    echo 'FAIL: updater accepted invalid release metadata' >&2
    exit 1
  fi
  cmp "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"
  if compgen -G "$APP_DIR/.package.nix.*" >/dev/null; then
    echo 'FAIL: updater left a temporary candidate' >&2
    exit 1
  fi
}

reset_package
bash "$APP_DIR/update.sh"
cp "$TEST_ROOT/original.nix" "$TEST_ROOT/expected.nix"
perl -0pi -e 's/version = "[^"]+";/version = "99.1.2";/; s/sha256 = "[^"]+";/sha256 = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";/' "$TEST_ROOT/expected.nix"
cmp "$TEST_ROOT/expected.nix" "$APP_DIR/package.nix"
echo 'PASS: update changes only the upstream version and source hash'

reset_package
current=$(rg -m1 -o 'version = "([^"]+)";' --replace '$1' "$APP_DIR/package.nix")
FIXTURE_TAG="v$current" bash "$APP_DIR/update.sh"
cmp "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"
echo 'PASS: current version is unchanged without prefetching'

for tag in invalid v99.1.2-beta.1 ''; do
  reset_package
  FIXTURE_TAG="$tag" assert_failure
done
echo 'PASS: invalid and prerelease tags preserve the package'

reset_package
FIXTURE_DOWNLOAD_FAILURE=1 assert_failure
echo 'PASS: download failure preserves the package'

reset_package
printf '{ "version": "99.1.1" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
assert_failure
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
echo 'PASS: mismatched release version preserves the package'

for file in nix/package.nix nix/model-catalog.json packages/coding-agent/install-lock/package-lock.json package-lock.json; do
  reset_package
  rm "$FIXTURE_SOURCE/$file"
  assert_failure
  touch "$FIXTURE_SOURCE/$file"
  if [ "$file" = nix/package.nix ]; then
    printf 'pname = "pi-workspace-packages";\n' > "$FIXTURE_SOURCE/$file"
  fi
done
echo 'PASS: missing upstream packaging files preserve the package'

reset_package
printf 'pname = "renamed-workspace";\n' > "$FIXTURE_SOURCE/nix/package.nix"
assert_failure
printf 'pname = "pi-workspace-packages";\n' > "$FIXTURE_SOURCE/nix/package.nix"
echo 'PASS: changed workspace derivation preserves the package'

reset_package
perl -0pi -e 's/export function openBrowser/export function renamedOpener/' "$FIXTURE_SOURCE/packages/coding-agent/src/utils/open-browser.ts"
assert_failure
echo 'PASS: incompatible file opener patch preserves the package'

perl -0pi -e 's/export function renamedOpener/export function openBrowser/' "$FIXTURE_SOURCE/packages/coding-agent/src/utils/open-browser.ts"
reset_package
perl -0pi -e 's/this.openUrl\(clickedUrl\)/this.renamedOpener(clickedUrl)/' "$FIXTURE_SOURCE/packages/tui/src/tui-alt-screen.ts"
assert_failure
echo 'PASS: incompatible mouse activation patch preserves the package'

perl -0pi -e 's/this.renamedOpener\(clickedUrl\)/this.openUrl(clickedUrl)/' "$FIXTURE_SOURCE/packages/tui/src/tui-alt-screen.ts"
reset_package
perl -0pi -e 's/import type \{ MarkdownTransformer, MessageRenderer \}/import type { RenamedTransformer, MessageRenderer }/' "$FIXTURE_SOURCE/packages/coding-agent/src/modes/interactive/components/custom-message.ts"
assert_failure
echo 'PASS: incompatible custom-message wrapper patch preserves the package'
