set -euo pipefail

codex=$1
jq=$2

plugins="$("$codex" plugin list --json)"
installed="$(printf '%s\n' "$plugins" | "$jq" -r 'any(.installed[]; .pluginId == "ponytail@ponytail" and .installed)')"
if [ "$installed" = "true" ]; then
  echo "Keeping installed ponytail@ponytail; update explicitly with codex plugin add ponytail@ponytail."
  exit 0
fi

"$codex" plugin marketplace add DietrichGebert/ponytail
"$codex" plugin add ponytail@ponytail
