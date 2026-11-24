#!/usr/bin/env bash
# Pass the built Hindsight client directory. Stop session startup before daemon access.
set -euo pipefail
client=${1:?expected a Hindsight client directory}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/dist" "$work/home"
ln -s "$(realpath "$client/skill")" "$work/skill"
cp "$client/dist/pi.js" "$work/dist/probe.mjs"
chmod u+w "$work/dist/probe.mjs"
printf '%s\n' '
const stop = new Error("reached daemon setup");
try {
  await RuntimeCore.prototype.seedIfCold.call({ harness: "pi", get cfg() { throw stop; } }, ".");
  throw new Error("session startup skipped");
} catch (error) { if (error !== stop) throw error; }
' >> "$work/dist/probe.mjs"
env -u HINDSIGHT_DISABLE_HOOKS HOME="$work/home" node "$work/dist/probe.mjs"
if [[ -e $work/home/.pi/agent/skills ]]; then
  echo 'Hindsight session startup created the legacy Pi skills directory' >&2
  exit 1
fi
