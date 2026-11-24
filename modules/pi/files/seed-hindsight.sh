set -euo pipefail
umask 077
config=$HOME/.hindsight/coding-agent.json
if [[ -e $config || -L $config ]]; then exit 0; fi
mkdir -p -- "${config%/*}"
tmp=$(mktemp "$config.XXXXXXXX")
trap 'rm -f -- "$tmp"' EXIT
cat -- "${1:?Expected Hindsight defaults}" > "$tmp"
# Publish the completed copy without replacing a config created during activation.
ln -T -- "$tmp" "$config" || [[ -f $config || -L $config ]]
