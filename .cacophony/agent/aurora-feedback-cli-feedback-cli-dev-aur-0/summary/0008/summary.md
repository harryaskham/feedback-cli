# Session summary — optional non-blocking webhook delivery

## Goal

Stop webhook reporting from stalling a CLI: WebhookSink POSTed synchronously, so
a slow/unreachable webhook blocked the caller up to the timeout. Add an opt-in
background delivery mode (default stays synchronous).

## Bead(s)

- `bd-b5c557` — Optional non-blocking webhook delivery (promoted from draft +
  claimed + implemented this session, per the operator's "don't idle, work
  beads" directive).

## Before state

- WebhookSink::record always delivered inline via ureq (blocking).
- Failing tests: none (17 unit + 3 integration + 2 doc).

## After state

- WebhookConfig.blocking (default true) + queue_capacity (default 256).
- Delivery enum: Sync (inline) | Async { bounded SyncSender + worker thread }.
  blocking=false enqueues without blocking (drops on full); Drop closes the
  channel and joins the worker, flushing queued events on reporter drop.
  Refactored sync/async to share WebhookTarget + deliver_webhook.
- describe() reports (sync)/(async). Manual Default keeps blocking=true so
  existing callers/tests stay synchronous. Async requires the webhook feature.
- README note on the strategies table.
- Failing tests: none. Default: 18 unit + 3 integration + 2 doc (incl. async
  flush-on-drop), clippy(-D warnings) + fmt green. --no-default-features: 16
  unit + 2 doc, clippy green.

## Diff summary

- Code commit: see reintegration receipt (on top of 273213d).
- Files: src/lib.rs (WebhookTarget/deliver_webhook/Delivery + WebhookConfig
  fields + Default + async test), README.md.
- Tests: +1 (webhook_async_delivery_flushes_on_drop).
- Behavioural delta: opt-in async webhook delivery; default unchanged (sync).

## Operator-takeaway

A CLI on a slow/unreachable webhook can now set blocking=false so feedback never
stalls it (best-effort, flushed on exit). Default behaviour is unchanged. This
was the last genuinely useful in-scope feature; feedback-cli's draft queue now
holds only the v0.1.0 release task (operator-gated). I remain scope-locked.
