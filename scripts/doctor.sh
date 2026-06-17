#!/usr/bin/env bash
# Project "doctor" / bootstrap check for the feedback-cli library crate.
#
# Verifies that the common conventions a library project should have are
# actually ON, so you catch drift early. Run it once after instantiating, and
# in CI.
#
#   nix run .#doctor    # offline checks (workflows, .envrc, versions, lib shape)

set -euo pipefail

for arg in "$@"; do
  case "$arg" in
    -h | --help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown arg: $arg" >&2
      exit 2
      ;;
  esac
done

root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$root"

problems=0
ok() { echo "  ok: $*"; }
warn() {
  echo "  WARN: $*"
  problems=$((problems + 1))
}
fail() {
  echo "  FAIL: $*"
  problems=$((problems + 1))
}

echo "==> feedback-cli project doctor ($root)"

# 1) Expected workflows exist and target the right runners.
echo "[workflows]"
for wf in ci release; do
  f=".github/workflows/$wf.yml"
  if [ -f "$f" ]; then ok "$f present"; else fail "$f missing"; fi
done
if grep -q "self-hosted" .github/workflows/ci.yml 2>/dev/null; then
  ok "ci.yml uses the self-hosted lane"
else
  warn "ci.yml should run on [self-hosted, linux]"
fi
if grep -q "self-hosted" .github/workflows/release.yml 2>/dev/null; then
  ok "release.yml uses the self-hosted lane"
else
  warn "release.yml should build on [self-hosted, linux]"
fi
# release.yml must be tag-triggered so a pushed vX.Y.Z publishes the library.
if grep -Eq '^\s*tags:' .github/workflows/release.yml 2>/dev/null; then
  ok "release.yml is tag-triggered (on: push: tags)"
else
  warn "release.yml should trigger on pushed version tags (on: push: tags: ['v*.*.*'])"
fi

# 2) .envrc sets CACOPHONY_PROJECT.
echo "[direnv]"
if grep -q "CACOPHONY_PROJECT" .envrc 2>/dev/null; then
  ok ".envrc sets CACOPHONY_PROJECT"
else
  warn ".envrc should export CACOPHONY_PROJECT"
fi

# 3) Version strings agree across Cargo.toml and the flake.
echo "[versions]"
cargo_ver="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
flake_ver="$(grep -m1 'version = "' flake.nix | sed -E 's/.*"([^"]+)".*/\1/' || true)"
ok "Cargo.toml version: ${cargo_ver:-<none>}"
if [ -n "$flake_ver" ] && [ "$flake_ver" != "$cargo_ver" ]; then
  fail "flake.nix version ($flake_ver) != Cargo.toml version ($cargo_ver)"
elif [ -n "$flake_ver" ]; then
  ok "flake.nix version matches ($flake_ver)"
fi

# 4) This is a *library* crate: Cargo.toml must declare a [lib] target (and no
#    [[bin]]).
echo "[lib shape]"
if grep -Eq '^\[lib\]' Cargo.toml; then
  ok "Cargo.toml declares a [lib] target"
else
  fail "Cargo.toml should declare a [lib] target (this crate is a library)"
fi
if grep -Eq '^\[\[bin\]\]' Cargo.toml; then
  warn "Cargo.toml declares a [[bin]] target — feedback-cli is binary-free"
else
  ok "no [[bin]] target (library-only, as intended)"
fi

# 5) The crate depends on the mcp-cli ecosystem crate.
echo "[ecosystem]"
if grep -q "mcp-cli" Cargo.toml; then
  ok "Cargo.toml depends on mcp-cli"
else
  warn "feedback-cli is expected to build on mcp-cli"
fi

echo
if [ "$problems" -eq 0 ]; then
  echo "==> doctor: all checks passed"
else
  echo "==> doctor: $problems issue(s) — see WARN/FAIL above"
  exit 1
fi
