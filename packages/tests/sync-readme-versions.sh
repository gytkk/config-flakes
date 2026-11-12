#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
PACKAGES_ROOT="$TEST_ROOT/packages"
APPS_DIR="$PACKAGES_ROOT/apps"
SYNC="$PACKAGES_ROOT/scripts/sync-readme-versions.sh"
REAL_AWK=$(command -v awk)
export REAL_AWK
mkdir -p "$PACKAGES_ROOT/scripts" "$APPS_DIR"/{alpha,alpha-helper,beta,pantry} "$TEST_ROOT/bin"
cp "$SOURCE_ROOT/scripts/sync-readme-versions.sh" "$SYNC"

cat > "$TEST_ROOT/original.md" <<'EOF'
# Fixture packages

Keep this introduction.

## App versions

| App | Version |
|-----|---------|
| obsolete | stale |

## Following section

Keep this tail.
EOF
cat > "$TEST_ROOT/expected.md" <<'EOF'
# Fixture packages

Keep this introduction.

## App versions

| App | Version |
|-----|---------|
| alpha | 1.2.3 |
| alpha-helper | 3.0.0 |
| beta | 2.0.0-rc.1+build.2 |
| pantry | main-69de978b5555 |

## Following section

Keep this tail.
EOF
cat > "$TEST_ROOT/valid-alpha.nix" <<'EOF'
let
  # version = "not-the-package-version";
  version = "1.2.3";
  needsSchemaFix = version == "1.0.0";
in
{ inherit version; }
EOF
printf '{\n  version = "3.0.0";\n}\n' > "$APPS_DIR/alpha-helper/package.nix"
printf '{\n\tversion\t=\t"2.0.0-rc.1+build.2"; # release metadata\n}\n' > "$APPS_DIR/beta/package.nix"
printf '{\n  version = "main-69de978b5555";\n}\n' > "$APPS_DIR/pantry/package.nix"
printf 'preexisting temporary file\n' > "$TEST_ROOT/preexisting.tmp"

reset_fixture() {
  cp "$TEST_ROOT/original.md" "$PACKAGES_ROOT/README.md"
  cp "$TEST_ROOT/preexisting.tmp" "$PACKAGES_ROOT/README.md.tmp"
  cp "$TEST_ROOT/valid-alpha.nix" "$APPS_DIR/alpha/package.nix"
}

assert_cleanup() {
  cmp "$TEST_ROOT/preexisting.tmp" "$PACKAGES_ROOT/README.md.tmp"
  local candidate
  for candidate in "$PACKAGES_ROOT"/.README.*; do
    [ -e "$candidate" ] || continue
    printf 'FAIL: synchronization left a staging file: %s\n' "$candidate" >&2
    exit 1
  done
}

assert_failure() {
  if bash "$SYNC" > "$TEST_ROOT/output" 2>&1; then
    echo 'FAIL: synchronization accepted invalid metadata or a failed render' >&2
    exit 1
  fi
  cmp "$TEST_ROOT/original.md" "$PACKAGES_ROOT/README.md"
  assert_cleanup
}

reset_fixture
bash "$SYNC"
cmp "$TEST_ROOT/expected.md" "$PACKAGES_ROOT/README.md"
assert_cleanup
echo 'PASS: literal release and commit versions replace only the version section'

bash "$SYNC"
cmp "$TEST_ROOT/expected.md" "$PACKAGES_ROOT/README.md"
assert_cleanup
echo 'PASS: repeated synchronization is byte-identical'

for declaration in \
  'pname = "missing-version";' \
  'version = missing.variable;' \
  'version = "1.2.3" + "-suffix";' \
  'version = "${release}";' \
  'version = "";' \
  'version = "1.2.3 | injected";' \
  'version = "1.2.3"' \
  'version = "1.2.3"; trailing.expression' \
  $'version =\n  "1.2.3";' \
  $'version = "1.2.3";\nversion = "1.2.3";' \
  $'version = "1.2.3";\nversion = "9.9.9";' \
  $'version = "1.2.3";\nversion = missing.variable;'; do
  reset_fixture
  printf '{\n  %s\n}\n' "$declaration" > "$APPS_DIR/alpha/package.nix"
  assert_failure
done
echo 'PASS: missing, malformed, expression, and ambiguous versions preserve original bytes'

# Fail after the renderer writes bytes, while allowing the metadata parser to run.
cat > "$TEST_ROOT/bin/awk" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [ "${1:-}" = '-v' ]; then
  printf 'partial rendered README\n'
  exit 1
fi
exec "$REAL_AWK" "$@"
EOF
chmod +x "$TEST_ROOT/bin/awk"
reset_fixture
PATH="$TEST_ROOT/bin:$PATH" assert_failure
echo 'PASS: partial render failure preserves README and preexisting README.md.tmp'
