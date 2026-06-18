# Session summary — feedback-cli webhook payload contract + example

## Goal

After landing the feedback-cli scaffold, drive the webhook reporting path
forward by pinning down the concrete artifact the cross-team integration needs:
a documented, stable webhook payload contract plus a runnable example, so the
caco webhook receiver (bd-4fe7e8) and the templates wiring (bd-bf2453) have an
exact, reviewable spec to integrate against.

## Bead(s)

- `bd-4fe7e8` — Integrate feedback-cli webhook strategy with caco webhooks
  (this increment documents the payload contract; full integration stays open,
  blocked on the external caco webhook ingestion feature).

## Before state

- feedback-cli v0.1.0 landed on main; webhook payload shape was implicit in the
  `FeedbackEvent` serde form, undocumented for receivers.
- `webhook_sink_posts_event_with_auth_header` was flaky: a single `read()` could
  capture only the HTTP headers before the body arrived in a later TCP segment.

## After state

- README has a "Webhook payload contract" section: method/auth/headers, an
  example JSON body, a per-field type table, and the suggested caco receiver
  mapping (error/exception -> bead/`caco log error`; perf -> `caco log perf`).
- `examples/report.rs` (`cargo run --example report`) dumps the error/perf
  webhook payloads, shows strategy selection from config, and previews the
  `caco_cli` argv — no network/caco needed.
- Webhook test reads the full request (headers + Content-Length body); stable
  across repeated runs.
- `cargo fmt`/`clippy --all-targets -D warnings` (pedantic)/`test` all green.

## Diff summary

- Code/content commit: `17669df` on top of `6d32819` (final landed squash SHA
  from the reintegration receipt).
- Files: `examples/report.rs` (new), `README.md` (+contract section),
  `src/lib.rs` (test read-loop hardening).
- Tests: +0 (existing webhook test de-flaked); example compiled under
  `--all-targets`.
- Behavioural delta: none to the public API; documentation + test robustness.

## Operator-takeaway

The webhook JSON contract is now explicit and reviewable (README + `examples/report.rs`).
That's the hand-off artifact for whoever builds the caco webhook receiver and
for templates-dev wiring feedback-cli in. Remaining work is still gated: the
caco webhook ingestion feature (bd-4fe7e8) and the templates wiring (bd-bf2453,
needs templates-dev + your call on resuming that agent).
