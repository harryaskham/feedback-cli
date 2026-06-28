# Session summary — CI: route jobs to azure-ephemeral runners (bd-07aba5)

## Goal
Per Harry's cross-project directive, move feedback-cli CI onto the new
azure-ephemeral self-hosted GitHub runners (x86-capable) to unstick the
starved/flaky existing fleet.

## Bead
- bd-07aba5 (CI: route x86/Linux jobs to [self-hosted, azure-ephemeral]).

## Before
- ci.yml build-test + release.yml resolve/test/publish all on
  [self-hosted, linux] / [self-hosted, linux, x86_64]. v0.1.0 release run sat
  queued ~21h on the starved fleet.

## After
- All 4 jobs runs-on: [self-hosted, azure-ephemeral]. feedback-cli has no
  macOS/darwin jobs, so nothing left on the old runners.

## Diff
- .github/workflows/ci.yml (1 runs-on), .github/workflows/release.yml (3 runs-on).
- yq-validated; no logic/step changes, label routing only.

## Operator-takeaway
Runners need Nix (confirmed on base image) + the SSH key for the private mcp-cli
flake input, or test/build jobs fail at the private-crate fetch. Sequencing:
CI triggers on azure-ephemeral now; cancel any run that queues before runners
are live.
