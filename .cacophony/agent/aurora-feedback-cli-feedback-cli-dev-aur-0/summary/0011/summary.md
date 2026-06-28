# Session summary — azure-ephemeral secretless/nix-toolchain CI hardening (bd-142533)

## Goal
Make feedback-cli's azure-ephemeral CI work on the secretless, off-tailnet,
Nix-only runner pool (Harry's follow-up directives to the runs-on migration).

## Bead
- bd-142533 (follow-up to bd-07aba5).

## Before
- ci.yml build-test + release.yml test assumed a runner-side SSH key for the
  private mcp-cli fetch (fails on secretless runners). release.yml publish used
  bare `gh` (not on the azure image).

## After
- build-test + test inject PRIVATE_DEPS_SSH_KEY deploy key (soft when absent).
- publish runs `gh` via `nix run nixpkgs#gh`.
- Build/test already nix-native; cargo publish already `nix develop --command`.
  No bare-toolchain jobs remain.

## Diff
- .github/workflows/ci.yml, .github/workflows/release.yml. yq-validated.

## Operator-takeaway
Two operator steps remain before feedback-cli CI runs on azure: (a) add
feedback-cli to the pool repos in sandbox/ops/terraform/runners.nix + redeploy;
(b) set the PRIVATE_DEPS_SSH_KEY secret (deploy key, read access to
harryaskham/mcp-cli). Confirm the cross-project secret name for consistency.
