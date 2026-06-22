# Session summary — feedback-cli ↔ caco webhook integration

## Goal

Wire feedback-cli's webhook reporting to caco. On re-validating the blocker I
found the caco webhook receiver already exists (`POST /hooks/<scope>/<hook-id>`
with a `bead` handler, bd-bbe6a1), so this delivers the feedback-cli side of the
integration: a turnkey bead payload mode and the bearer-token convention, with
a documented recipe, so a host CLI can create caco beads from feedback events
with config only.

## Bead(s)

- `bd-4fe7e8` — Integrate feedback-cli webhook strategy with caco webhooks
  (claimed + implemented this session; the caco-side hook config + live e2e
  remain, tracked by `bd-08b0cf`).

## Before state

- feedback-cli `webhook` strategy POSTed only the native FeedbackEvent JSON; no
  caco-shaped body, no token convention. bd-4fe7e8 was marked blocked-on-feature.
- Failing tests: none (12 unit + 2 doc at session start).

## After state

- `WebhookPayload` mode on `WebhookConfig`: `event` (default, lossless) or
  `caco_bead` (body matching the caco `bead` handler: summary→title,
  detail+context→description, error/exception→bug, perf/info→task,
  severity→priority, labels). `FeedbackEvent::to_caco_bead[_json]` renders it.
- Bearer-token convention: `resolve_token_for` / `build_sink` / `Reporter`
  thread the default project so, with no inline `token`/`token_env`, feedback-cli
  auto-resolves `CACOPHONY_<PROJECT>_WEBHOOK_TOKEN` then `CACOPHONY_WEBHOOK_TOKEN`
  (`conventional_token_env_vars` helper).
- README "Wiring to a caco webhook" recipe (both payload paths + token
  convention + illustrative caco webhooks config); `examples/report.rs` dumps the
  caco_bead body.
- Failing tests: none. 14 unit + 2 doc tests pass; clippy (pedantic, -D warnings)
  and fmt clean; example runs.

## Diff summary

- Code commit: `50bb3b7` on top of `7c62d5c` (final landed squash SHA from the
  reintegration receipt).
- Files: `src/lib.rs` (payload mode, to_caco_bead, token convention, tests),
  `examples/report.rs`, `README.md`, `.gitignore` (ignore `.direnv/`).
- Tests: +4 (caco_bead mapping, payload default, token var names, token
  precedence), -0, flipped 0.
- Behavioural delta: webhook can now emit a caco bead-create body and
  auto-resolve the conventional bearer token; default behaviour unchanged.

## Operator-takeaway

feedback-cli → caco beads now works config-only: point the `webhook` strategy at
a caco `/hooks/<project>/<id>` bead hook, use `payload: "caco_bead"`, and set
`CACOPHONY_<PROJECT>_WEBHOOK_TOKEN`. The only remaining step is operator-side:
configure the actual caco webhook hook + run the live e2e (`bd-08b0cf`), which is
now unblocked.
