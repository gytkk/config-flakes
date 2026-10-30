#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 4 ]; then
  echo "Usage: $0 OUTPUT ARCHIVE VERSION TARGET" >&2
  exit 2
fi

output=$1
archive=$2
version=$3
target=$4

fail() {
  echo "ERROR: Codex package $output: $*" >&2
  exit 1
}

jq -e --arg version "$version" --arg target "$target" '
  .layoutVersion == 1 and .version == $version and .target == $target
  and .variant == "codex" and .entrypoint == "bin/codex"
  and .resourcesDir == "codex-resources" and .pathDir == "codex-path"
' "$output/codex-package.json" >/dev/null || fail "missing or incompatible codex-package.json"

# Compare paths, not bytes: Linux fixups rewrite ELF interpreters and RPATHs.
members=$(tar -tzf "$archive") || fail "cannot list source archive $archive"
while IFS= read -r member; do
  [ -e "$output/$member" ] || fail "missing bundle member: $member"
done <<< "$members"

executables=(bin/codex bin/codex-code-mode-host codex-path/rg)
# Codex 0.163.0 removed bundled zsh; older bundles still need its executable check.
if [ -e "$output/codex-resources/zsh/bin/zsh" ]; then
  executables+=(codex-resources/zsh/bin/zsh)
fi
if [[ "$target" == *-linux-* ]]; then
  executables+=(codex-resources/bwrap)
fi
for executable in "${executables[@]}"; do
  [ -x "$output/$executable" ] || fail "not executable: $executable"
done

echo "OK: complete Codex $version bundle for $target"
