# Session summary — feature-gate the webhook (ureq/TLS) dependency

## Goal

Make feedback-cli leaner for the growing CLI suite embedding it: the heavy
ureq/rustls TLS stack should be opt-out, since CLIs that only report via stderr
/ caco_cli / disabled don't need it. Keep it fully non-breaking (default-on).

## Bead(s)

- `bd-2131bb` — Feature-gate the webhook (ureq/TLS) dependency behind a
  default-on `webhook` feature (filed + claimed + implemented this session).

## Before state

- ureq (rustls/ring) was an unconditional dependency; every consumer paid the
  compile-time + binary cost even without webhooks.
- Failing tests: none (14 unit + 3 integration + 2 doc).

## After state

- `ureq` is optional; `[features] default = ["webhook"]`, `webhook = ["dep:ureq"]`.
- `WebhookSink::record` delegates to a cfg-gated `send()`: with the feature it
  POSTs via ureq; without it, `send()` returns `FeedbackError::Config` (builds +
  serializes still work; delivery is a clear no-TLS error). `WebhookSink` /
  `WebhookConfig` / `WebhookPayload` / `ReportStrategy::Webhook` stay
  always-present so the config schema is unchanged.
- Webhook-exercising tests gated behind the feature; README "Cargo features".
- Failing tests: none. Default config: 14 unit + 3 integration + 2 doc pass,
  clippy (pedantic, -D warnings) + fmt clean, ureq in dep tree. `--no-default-features`:
  builds + clippy -D warnings clean, 15 non-webhook tests pass, ureq ABSENT from
  the dep tree.

## Diff summary

- Code commit: `9b9c945` on top of `f780137` (final landed squash SHA from the
  reintegration receipt).
- Files: `Cargo.toml` (optional dep + features), `src/lib.rs` (cfg-gated send +
  dead_code allow + test gate), `tests/caco_webhook_e2e.rs` (feature gate),
  `README.md` (Cargo features).
- Tests: +0 (existing webhook tests gated); behaviour preserved under default.
- Behavioural delta: none for default consumers; webhook delivery now opt-out.

## Operator-takeaway

feedback-cli is now embeddable without the TLS stack: CLIs that only want local
or caco-cli reporting can depend on it with `default-features = false` and skip
ureq/rustls entirely. Default behaviour (and templates/hello-world-cli) is
unchanged. feedback-cli queue otherwise drained.
