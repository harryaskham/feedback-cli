# Session summary — feedback-cli end-to-end webhook->caco-bead test

## Goal

Spike the last open feedback-cli bead: an end-to-end test proving feedback-cli's
webhook path drives the intended caco bead creation. Since a live caco hook is
operator-gated, prove the contract against a faithful mock of caco-daemon's
`bead` webhook handler so the feedback-cli side is fully validated now.

## Bead(s)

- `bd-08b0cf` — End-to-end webhook -> caco bead/perf integration test
  (claimed + implemented this session). Closed earlier this session:
  `bd-4fe7e8` (caco webhook integration) and `bd-bf2453` (templates wiring,
  landed by templates-dev).

## Before state

- feedback-cli had unit coverage of the webhook POST + auth header, but no
  end-to-end test against caco's actual bead-create field mapping.
- Failing tests: none (14 unit + 2 doc).

## After state

- `tests/caco_webhook_e2e.rs`: a std-only mock caco `bead` webhook (mirrors
  caco-daemon `dispatch_bead`: title/description/type/priority/labels) asserts:
  payload=caco_bead -> error becomes a `bug` bead (title=summary, priority=2,
  labels feedback+kind:error+ci, description carries detail + component/field
  context) and perf becomes a `task` bead; auth + Content-Type headers present;
  payload=event posts native FeedbackEvent JSON; non-2xx -> FeedbackError::Http.
- Failing tests: none. 14 unit + 3 integration + 2 doc tests pass; clippy
  (pedantic, -D warnings) and fmt clean.

## Diff summary

- Code commit: `d7d7af2` on top of `01686cc` (final landed squash SHA from the
  reintegration receipt).
- Files: `tests/caco_webhook_e2e.rs` (new).
- Tests: +3 integration, -0, flipped 0.
- Behavioural delta: test-only; no public API change.

## Operator-takeaway

The feedback-cli -> caco webhook path is now proven end-to-end against caco's
exact bead-handler parsing, so wiring it live is config-only. The remaining
live smoke against a real configured caco webhook hook is operator-gated and is
filed as a separate operator-action follow-up; the feedback-cli queue is
otherwise drained.
