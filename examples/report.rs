//! Demonstrates the `feedback-cli` API with no network or `caco` dependency:
//! it prints the exact webhook JSON payload for sample events, shows selecting a
//! strategy from config, and previews the argv a `caco_cli` sink would run.
//!
//! Run with: `cargo run --example report`

use feedback_cli::{
    CacoCliConfig, CacoCliSink, FeedbackConfig, FeedbackEvent, Metric, ReportStrategy,
};

fn main() {
    // 1) The webhook payload contract: this is exactly the JSON body a `webhook`
    //    strategy POSTs (Content-Type: application/json) to the configured URL.
    let error = FeedbackEvent::error("build", "linker failed")
        .with_detail("ld: symbol not found")
        .with_field("crate", "acme-cli")
        .with_fingerprint("ld_symbol_not_found")
        .with_project("acme");
    println!("// error event webhook payload:");
    println!("{}", error.to_json().expect("serialize error event"));

    let perf = FeedbackEvent::perf(
        "build",
        "slow link",
        Metric {
            name: "link_ms".to_owned(),
            value: 4200.0,
            unit: Some("ms".to_owned()),
            threshold: Some(2000.0),
            baseline: Some(1500.0),
        },
    )
    .with_project("acme");
    println!("// perf event webhook payload:");
    println!("{}", perf.to_json().expect("serialize perf event"));

    // 1b) The SAME error event rendered as a caco bead-create body, i.e. what the
    //     `caco_bead` payload mode POSTs (matches the caco webhook `bead` handler
    //     fields directly, so no receiver-side title_from mapping is needed).
    println!("// error event as a caco bead-create body (payload = caco_bead):");
    println!(
        "{}",
        error.to_caco_bead_json().expect("serialize caco bead")
    );

    // 2) Selecting a strategy purely from project config (serde). Here: a caco
    //    webhook with the turnkey caco_bead payload and the conventional token
    //    env var. With no `token`/`token_env`, feedback-cli also auto-resolves
    //    CACOPHONY_<PROJECT>_WEBHOOK_TOKEN then CACOPHONY_WEBHOOK_TOKEN.
    let config: FeedbackConfig = serde_json::from_str(
        r#"{"component":"acme-cli","project":"acme","strategy":{"type":"webhook","url":"https://host.example/hooks/acme/feedback","token_env":"CACOPHONY_ACME_WEBHOOK_TOKEN","payload":"caco_bead"}}"#,
    )
    .expect("parse config");
    assert!(matches!(config.strategy, ReportStrategy::Webhook(_)));
    println!("// resolved strategy: {:?}", config.strategy);

    // 3) What the `caco_cli` strategy would run (argv preview, nothing executed).
    let sink = CacoCliSink::from_config(&CacoCliConfig {
        binary: None,
        project: Some("acme".to_owned()),
        create_beads: true,
    });
    println!("// caco_cli argv for the error event:");
    for argv in sink.commands(&error) {
        println!("//   {}", argv.join(" "));
    }
}
