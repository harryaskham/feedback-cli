# Session summary — live-webhook smoke driver (examples/smoke.rs)

## Goal

Prepare the feedback-cli side of the operator-gated live webhook smoke so it's a
one-command run the moment a caco bead hook + per-hook token exist.

## Bead(s)

- Enables `bd-cbd710` ([operator-action] configure a caco webhook bead hook + run
  feedback-cli live smoke). Not closed — the actual live run still needs the
  operator-configured hook + token.

## Before state

- examples/report.rs only printed payloads (no real POST); no runnable live-smoke
  harness.

## After state

- examples/smoke.rs: reads FEEDBACK_WEBHOOK_URL (+ FEEDBACK_PROJECT, token via the
  CACOPHONY_<PROJECT>_WEBHOOK_TOKEN / CACOPHONY_WEBHOOK_TOKEN convention or
  FEEDBACK_WEBHOOK_TOKEN_ENV), builds a webhook config with payload=caco_bead, and
  POSTs an error + perf FeedbackEvent so a bug bead is created end-to-end.
  Example-only (compiled, not run in CI).

## Diff summary

- Code commit: see reintegration receipt (on top of 6f11249).
- Files: examples/smoke.rs (new).
- Tests: +0 (example-only). cargo fmt/clippy (default + --no-default-features,
  -D warnings)/test green (18 unit + 3 integration + 2 doc).
- Behavioural delta: none to the library; adds a runnable smoke example.

## Operator-takeaway

Once you configure the caco webhook bead hook with a specific per-hook token and
point FEEDBACK_WEBHOOK_URL + the token env at it, `cargo run --example smoke`
runs the live bd-cbd710 smoke and you check `caco bd list` for the created bead.
