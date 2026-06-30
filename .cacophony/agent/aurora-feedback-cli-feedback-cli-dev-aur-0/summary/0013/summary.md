# Session summary — keyless CI: public mcp-cli over HTTPS (bd-ad343b)

## Goal
Unblock feedback-cli CI on the secretless azure-ephemeral pool by removing the
SSH-key/secret requirement.

## Key discovery
mcp-cli is PUBLIC (gh visibility=public; unauthenticated https ls-remote works).
The flake's "Private repo / SSH" comment was stale.

## Changes
- flake.nix: mcp-cli git+ssh -> git+https; flake.lock re-locked (rev 2e4efc1).
- ci.yml/release.yml: removed SSH-key-injection + https->ssh git-auth steps.
- README: fixed stale ssh/private note.

## Validation
- nix build .#feedback-cli: PASS keyless.
- nix flake check: "all checks passed!" keyless (no ssh agent / no insteadOf).
- yq-validated workflows.

## Operator-takeaway
The PRIVATE_DEPS_SSH_KEY secret is NO LONGER NEEDED for feedback-cli (revert that
ask from bd-142533). Only remaining CI prerequisite: the mono pool-membership
bead (add feedback-cli to sandbox/ops/terraform/runners.nix + redeploy).
