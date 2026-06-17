# Session summary — feedback-cli scaffold (webhook/caco reporting library)

## Goal

Stand up `feedback-cli`, a new harryaskham-ecosystem Rust **library** that CLIs
build in to feed structured error / feedback / perf events back to the owning
project. The reporting backend is selected by project config (not code), so a
project can route feedback to a caco webhook, the caco CLI, or a local sink
without changing the CLI. Follow the existing `updatable-cli` / `mcp-cli`
patterns and the templates' nix flake + cargo vendor machinery, ready to be
wired into the `templates` project with `templates-dev-0`.

## Bead(s)

- None. The `feedback-cli` project is not configured at the cluster bead
  authority (`helsinki` returns upstream 404), and this worker's scope is bound
  to `feedback-cli` so it cannot file under `cacophony` either. Built per direct
  operator instruction; blocker recorded in the agent health scratch note.

## Before state

- Repo contained only `README.md` (one line) on the `Initial commit`; no crate.
- Failing tests: none (no code).
- Bead surface for the project: unavailable (authority 404).

## After state

- A complete library crate that builds against `mcp-cli`:
  - `FeedbackEvent` (error/exception/perf/info) + `Metric`, serde payload.
  - `FeedbackSink` trait + built-ins: `NullSink`, `StderrSink`, `WebhookSink`
    (HTTP POST, URL + bearer token, never logs the token), `CacoCliSink`
    (`caco log error|perf`, optional `caco bd create`).
  - `ReportStrategy` (`disabled` | `stderr` | `webhook` | `caco_cli`) +
    `FeedbackConfig`, both serde-deserializable from project config; `Reporter`
    front-end; `from_env()` convenience.
  - `mcp_cli::StructuredError` integration (`from_structured_error`,
    `report_error`) and an MCP registrar `register_feedback_tools`
    (`feedback_report` / `feedback_status`), mirroring `updatable-cli`.
  - Nix flake (`[patch]` mcp-cli into the sandbox) + `Cargo.lock`, `.envrc`,
    `scripts/{doctor,release}.sh`, CI `ci.yml` + `release.yml`.
- Failing tests: none. 10 unit tests + 1 webhook integration test + 2 doctests
  pass. `cargo clippy --all-targets -- -D warnings` (pedantic) and
  `cargo fmt --check` clean. `nix build .#feedback-cli` reproduces the build +
  tests in-sandbox.

## Diff summary

- Code/content commit: `23bd498` on top of `47bedf3` (final landed squash SHA
  will come from the reintegration receipt).
- Files added: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/mcp.rs`,
  `flake.nix`, `flake.lock`, `.envrc`, `.gitignore`,
  `scripts/doctor.sh`, `scripts/release.sh`,
  `.github/workflows/ci.yml`, `.github/workflows/release.yml`; `README.md` rewritten.
- Tests: +13 (10 unit + 1 integration + 2 doctest), -0, flipped 0.
- Behavioural delta: new crate; no prior behaviour to change.

## Operator-takeaway

`feedback-cli` is a config-driven reporting library: a CLI builds it in and the
**project config** chooses the strategy. The headline path is the `webhook`
consumer — set a URL + token and the receiver (e.g. a caco webhook) decides what
to do with each event. Next step is coordinating with `templates-dev-0` to wire
this crate into the `templates` project (template flakes + Cargo deps), and
getting `feedback-cli` registered at the bead authority so future work on it is
trackable.
