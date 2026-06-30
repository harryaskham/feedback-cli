# Session summary — web client caco_bead field-ordering parity fix

## Goal

On a fully drained feedback-cli board (zero open/in-progress/blocked/draft
beads), run a scoped self-audit of the project's headline contract — that the
web/ios/android drop-in clients "mirror `FeedbackEvent::to_caco_bead` exactly"
(README) — and fix any real cross-language drift found.

## Bead(s)

- `bd-fb6e3a` — web client: sort caco_bead footer fields by key to match
  Rust/iOS/Android contract (bug, P3)

## Before state

- Failing tests: none (project is green; v0.1.0 released).
- Drift: the Rust contract stores `FeedbackEvent.fields` as `BTreeMap` (sorted
  key order) and the iOS (`.sorted`) + Android (`.toSortedMap()`) clients sort
  fields; the web client (`web/feedback.ts` `toCacoBead`) iterated
  `Object.entries(e.fields)` in INSERTION order. A multi-field event therefore
  rendered an inconsistent description footer on web vs every other client.
- No automated guard exists for cross-client caco_bead parity (doctor.sh only
  checks Rust/CI conventions).

## After state

- Failing tests: none.
- `web/feedback.ts` `toCacoBead` now sorts fields by key before building the
  footer, matching the Rust/iOS/Android ordering.
- Verified with node 26 type-stripping: `node --check web/feedback.ts` parses,
  and a functional check confirms fields `{zeta,alpha,mike}` now render as
  `alpha`/`mike`/`zeta` in the footer.

## Diff summary

- Code commit: pending final squash SHA from the reintegration receipt.
- Files touched: `web/feedback.ts` (1 logic line + explanatory comment).
- Tests: +0 / -0 (no TS toolchain in-repo; validated functionally via node).
- Behavioural delta: web caco_bead description footer field order is now
  deterministic and identical to the Rust contract and the iOS/Android clients.
  `toEvent` left unchanged — it emits a JSON object where key order is not
  semantically significant.

## Operator-takeaway

The 4 feedback clients (Rust + web/ios/android) encode the same caco_bead
mapping by hand, so they can silently drift. This session fixed one real
divergence (web field ordering); the durable risk is that nothing guards the
parity automatically. Filed a draft bead proposing a lightweight cross-client
parity check (the iOS/Android footer ordering was already correct, so the gap
is a guard, not the mappings).
