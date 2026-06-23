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
| `file`      | Append one JSON line per event to a configured path (durable local log; parent dirs created). |
| `stderr`    | Write one JSON line per event to stderr (the default). |
| `disabled`  | Drop every event. |

The `webhook` strategy delivers **synchronously** by default; set
`"blocking": false` in its config for best-effort **background** delivery that
never stalls the CLI (bounded queue; queued events are flushed when the reporter
is dropped).

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

## Wiring to a caco webhook

caco ships a webhook ingress (`POST /hooks/<scope>/<hook-id>`) with a **bead**
handler, so feedback-cli can create beads with zero custom server code. Two ways
to shape the body:

**A. Turnkey — `payload: "caco_bead"` (recommended).** feedback-cli POSTs a
bead-create body (`title`/`description`/`type`/`priority`/`labels`) that matches
the caco `bead` handler's native fields directly. `summary` becomes the title,
`detail` + a structured context footer becomes the description, error/exception
events become `bug`s, perf/info become `task`s, and severity maps to priority.

```jsonc
// feedback-cli config (in the host CLI's project config)
{
  "component": "my-cli",
  "project": "my-project",
  "strategy": {
    "type": "webhook",
    "url": "https://<node-or-funnel-host>/hooks/my-project/feedback",
    "payload": "caco_bead"
    // token resolved by convention (see below) or set token_env explicitly
  }
}
```

**B. Lossless — default `payload: "event"`.** feedback-cli POSTs the full
`FeedbackEvent` JSON; the caco hook maps it with `bead.title_from: "summary"`
(the raw event JSON becomes the description, `labels` map through).

```yaml
# caco config (operator-owned), illustrative
webhooks:
  port: 8444
  funnel: true            # opt-in Tailscale Funnel exposure
  token_env: CACOPHONY_WEBHOOK_TOKEN
  hooks:
    feedback:
      bead:
        project: my-project
        labels: [feedback, external]
        title_from: summary   # only needed for payload "event"
```

### Bearer token convention

The caco webhook requires a bearer token. feedback-cli sends
`Authorization: Bearer <token>` resolved in this order:

1. inline `token`,
2. explicit `token_env` (the named var must be set, else it errors),
3. **convention** (when neither is set): `CACOPHONY_<PROJECT>_WEBHOOK_TOKEN`
   (project upper-cased, non-alphanumerics → `_`) then `CACOPHONY_WEBHOOK_TOKEN`.

So if the host sets `CACOPHONY_MY_PROJECT_WEBHOOK_TOKEN` (or the shared
`CACOPHONY_WEBHOOK_TOKEN`) to the same secret the caco webhook accepts, a config
with just `type`/`url`/`payload` authenticates automatically. The matching env
var names are available programmatically via `conventional_token_env_vars`.

## Panic hook

Opt in to turn unhandled panics into exception feedback automatically — the
literal "hook into exceptions" path:

```rust
use feedback_cli::{install_panic_hook, FeedbackConfig};

fn main() {
    install_panic_hook(&FeedbackConfig::from_env());
    // ... the rest of your CLI. Any panic is now reported as a
    // FeedbackKind::Exception event (with source location + thread) through the
    // configured strategy, before the normal panic output / abort runs.
}
```

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

## Cargo features

- `webhook` *(default)* — enables the `webhook` reporting strategy (HTTPS `POST`
  via [`ureq`]). Disable it to drop the `ureq`/`rustls` TLS stack when a CLI only
  uses the `stderr` / `caco_cli` / `disabled` strategies:

  ```toml
  feedback-cli = { git = "https://github.com/harryaskham/feedback-cli", default-features = false }
  ```

  With the feature off, `WebhookConfig` / `WebhookSink` still exist and build, but
  delivery returns a config error instead of sending.

[`ureq`]: https://docs.rs/ureq

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
