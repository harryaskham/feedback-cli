# Session summary — multi-platform drop-in feedback clients (bd-43cef5)

## Goal
Like feedback-cli for Rust CLIs, give cacophony apps drop-in feedback->bead from
web/iPhone/macOS/Android over the same webhook + payload contract.

## Bead
- bd-43cef5 (epic) — web/ ios/ android/ subprojects.

## After
- web/feedback.ts (TS, fetch), ios/Feedback.swift (Swift iOS+macOS), android/Feedback.kt
  (Kotlin) + per-dir READMEs; default caco_bead payload, optional event. Bearer-token.
- to_caco_bead mapping (type/priority/labels) identical to Rust; node-verified.
- README "Platform clients" section. Non-Rust → cargo/nix CI unaffected.

## Operator-takeaway
FFI over the Rust core is a later option; v1 is tiny native HTTP clients. Point any
app at /hooks/<scope>/<id> + bearer; same beads as the CLI.
