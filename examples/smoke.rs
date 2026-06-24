//! Live webhook smoke test for `feedback-cli` -> a caco webhook bead hook.
//!
//! Unlike `examples/report.rs` (which only prints payloads), this actually
//! POSTs real [`FeedbackEvent`]s through the configured webhook so you can
//! confirm a bead is created end-to-end. Intended for the operator-gated live
//! smoke (`bd-cbd710`): point it at a configured caco `bead` hook and check the
//! board afterwards.
//!
//! Usage (requires the default `webhook` feature):
//!
//! ```sh
//! FEEDBACK_WEBHOOK_URL="https://<host-or-funnel>/hooks/<project>/<hook-id>" \
//! FEEDBACK_PROJECT="<project>" \
//! CACOPHONY_<PROJECT>_WEBHOOK_TOKEN="<the per-hook token>" \   # or CACOPHONY_WEBHOOK_TOKEN,
//!                                                              # or FEEDBACK_WEBHOOK_TOKEN_ENV=<var>
//! cargo run --example smoke
//! ```
//!
//! Then check `caco bd list --project <project>` for the created bug bead
//! (from the error event). The token is resolved by feedback-cli's convention
//! (`CACOPHONY_<PROJECT>_WEBHOOK_TOKEN` then `CACOPHONY_WEBHOOK_TOKEN`) unless
//! `FEEDBACK_WEBHOOK_TOKEN_ENV` names a specific variable.

use std::process::ExitCode;

use feedback_cli::{
    FeedbackConfig, FeedbackEvent, Metric, ReportStrategy, Reporter, WebhookConfig, WebhookPayload,
};

fn main() -> ExitCode {
    let Ok(url) = std::env::var("FEEDBACK_WEBHOOK_URL") else {
        eprintln!(
            "set FEEDBACK_WEBHOOK_URL to the caco webhook, e.g. \
             https://<host>/hooks/<project>/<hook-id>"
        );
        return ExitCode::FAILURE;
    };
    let project = std::env::var("FEEDBACK_PROJECT").ok();

    let mut webhook = WebhookConfig {
        url,
        payload: WebhookPayload::CacoBead,
        ..WebhookConfig::default()
    };
    // Optional explicit override; otherwise the token is auto-resolved from the
    // CACOPHONY_<PROJECT>_WEBHOOK_TOKEN / CACOPHONY_WEBHOOK_TOKEN convention.
    if let Ok(var) = std::env::var("FEEDBACK_WEBHOOK_TOKEN_ENV") {
        webhook.token_env = Some(var);
    }

    let config = FeedbackConfig {
        enabled: true,
        component: Some("feedback-cli-smoke".to_owned()),
        project,
        strategy: ReportStrategy::Webhook(webhook),
    };
    let reporter = Reporter::from_config(&config);
    eprintln!("feedback-cli smoke -> {}", reporter.destination());

    let error = FeedbackEvent::error("smoke", "feedback-cli live smoke: error event")
        .with_detail("should create a `bug` bead via the caco webhook bead handler")
        .with_label("smoke");
    let perf = FeedbackEvent::perf(
        "smoke",
        "feedback-cli live smoke: perf event",
        Metric::new("smoke_ms", 1.0),
    );

    let mut failed = false;
    for event in [&error, &perf] {
        match reporter.report(event) {
            Ok(()) => eprintln!("  delivered: {}", event.summary),
            Err(err) => {
                eprintln!("  FAILED: {} -> {err}", event.summary);
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        eprintln!("delivered 2 events; check `caco bd list` for the created bead(s)");
        ExitCode::SUCCESS
    }
}
