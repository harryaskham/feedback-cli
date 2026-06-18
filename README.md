# feedback-cli

Configurable error / feedback / perf reporting for Rust CLIs built on the
[harryaskham](https://github.com/harryaskham) [`mcp-cli`](https://github.com/harryaskham/mcp-cli)
stack.

A CLI **builds this crate in** and exposes a [`FeedbackConfig`] that its project
configuration populates. The config selects a **reporting strategy** — *how*
structured feedback is fed back to the owning project. Because the strategy is
plain serde data, a project can switch backends purely from config, with no CLI
code change.

## Reporting strategies

| `strategy.type` | Behaviour |
|---|---|
| `webhook`   | POST each event as JSON to a URL with an optional bearer token. The CLI is a **webhook consumer**: it calls the endpoint and the receiver (e.g. a caco webhook) decides what to do — create a bead, log a `caco` error/perf/exception, page someone, … |
| `caco_cli`  | Shell out to the local `caco` CLI (`caco log error` / `caco log perf`), optionally also filing a bead. |
| `stderr`    | Write one JSON line per event to stderr (the default). |
| `disabled`  | Drop every event. |

## Configure from project config

```jsonc
{
  "component": "my-cli",
  "project": "acme",
  "strategy": {
    "type": "webhook",
    "url": "https://example.invalid/feedback",
    "token_env": "ACME_FEEDBACK_TOKEN"
  }
}
```

`token_env` reads the bearer token from an environment variable at runtime, so
secrets never land in committed config. (`token` accepts an inline value when you
must.)

## Report events

```rust
use feedback_cli::{FeedbackConfig, FeedbackEvent, Metric, Reporter};

// A real CLI deserializes this from its project config; the default reports to stderr.
let reporter = Reporter::from_config(&FeedbackConfig::default());

reporter.report(&FeedbackEvent::error("startup", "failed to read config").with_detail("ENOENT"))?;
reporter.report(&FeedbackEvent::perf("build", "slow link", Metric::new("link_ms", 4200.0)))?;
# Ok::<(), feedback_cli::FeedbackError>(())
```

Any [`mcp_cli::StructuredError`] can be reported in one call — handy for feeding
an MCP tool failure straight back:

```rust
use feedback_cli::{FeedbackConfig, Reporter};
use mcp_cli::{ErrorCategory, StructuredError};

# struct MyError;
# impl StructuredError for MyError {
#     fn category(&self) -> ErrorCategory { ErrorCategory::ExecutionFailure }
#     fn code(&self) -> String { "boom".into() }
#     fn message(&self) -> String { "it broke".into() }
# }
let reporter = Reporter::from_config(&FeedbackConfig::default());
reporter.report_error("my-tool", &MyError)?;
# Ok::<(), feedback_cli::FeedbackError>(())
```

## Webhook payload contract

The `webhook` strategy sends **one HTTP request per event**:

- method `POST` (override with `method`), `Content-Type: application/json`;
- `Authorization: Bearer <token>` when a token is configured (`token` or `token_env`);
- extra `headers` if configured;
- body = the JSON serialization of a `FeedbackEvent`.

A non-2xx response (or transport error) surfaces as `FeedbackError::Http`; a 2xx
is success. It is up to the receiver (e.g. a caco webhook) to decide the action.

Example error-event body:

```json
{
  "kind": "error",
  "component": "build",
  "summary": "linker failed",
  "detail": "ld: symbol not found",
  "severity": "error",
  "labels": ["category:execution_failure"],
  "fields": { "crate": "acme-cli" },
  "fingerprint": "ld_symbol_not_found",
  "project": "acme",
  "timestamp_unix_ms": 1750000000000
}
```

| field | type | notes |
|---|---|---|
| `kind` | `"error"` \| `"exception"` \| `"perf"` \| `"info"` | required; maps to the caco surface |
| `component` | string | required; source/subsystem |
| `summary` | string | required |
| `severity` | `"info"`\|`"warning"`\|`"error"`\|`"critical"` | optional |
| `detail` | string | optional body/stack/context |
| `labels` | string[] | optional; omitted when empty |
| `fields` | object<string,string> | optional; omitted when empty |
| `fingerprint` | string | optional dedupe key |
| `project` | string | optional |
| `metric` | `{ name, value, unit?, threshold?, baseline? }` | present on `perf` events |
| `timestamp_unix_ms` | number (u64) | always present; ms since the Unix epoch |

Optional fields are omitted when `None`/empty. Suggested receiver mapping:
`error`/`exception` → create a bead and/or `caco log error`; `perf` →
`caco log perf`; `info` → log or drop. See `cargo run --example report` for a
live dump of these payloads.

## Expose over MCP

Mirroring `updatable-cli`, [`register_feedback_tools`] mounts `feedback_report`
and `feedback_status` tools onto any `mcp-cli` [`ToolRouter`], resolving the
config from the host context per call:

```rust
use feedback_cli::{register_feedback_tools, FeedbackConfig};
use mcp_cli::ToolRouter;

struct Ctx;
let mut router: ToolRouter<Ctx> = ToolRouter::new();
register_feedback_tools(&mut router, |_ctx: &Ctx| FeedbackConfig::from_env());
assert!(router.tool_metadata().iter().any(|t| t.name == "feedback_report"));
```

## Development

This crate is part of the harryaskham ecosystem and uses the shared nix-flake +
`[patch]` vendor machinery: `mcp-cli` is pulled from `github:harryaskham/*` (not
vendored) and patched into the cargo build inside the nix sandbox.

```sh
nix build .#feedback-cli      # build the library
nix flake check               # build + unit + doc tests
nix run .#doctor              # verify project conventions
nix run .#release -- patch    # bump version, tag, and trigger release.yml
```

Plain cargo also works on a host that can fetch the private `mcp-cli` git
dependency (configure `url."ssh://git@github.com/".insteadOf "https://github.com/"`):

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

MIT.

[`FeedbackConfig`]: https://docs.rs/feedback-cli
[`register_feedback_tools`]: https://docs.rs/feedback-cli
[`ToolRouter`]: https://docs.rs/mcp-cli
[`mcp_cli::StructuredError`]: https://docs.rs/mcp-cli
