//! `feedback-cli` — configurable error / feedback / perf reporting for CLIs built
//! on the [`mcp-cli`](https://github.com/harryaskham/mcp-cli) stack.
//!
//! A CLI *builds this crate in* and exposes a [`FeedbackConfig`] that its project
//! configuration populates. The config selects a **reporting strategy** — *how*
//! structured feedback is fed back to the owning project:
//!
//! - [`ReportStrategy::Webhook`] — POST each event as JSON to a configured URL
//!   with an optional bearer token. The CLI is a *webhook consumer*: it calls the
//!   endpoint and it is up to the receiver (e.g. a caco webhook) to decide what
//!   to do — create a bead, log a `caco` error/perf/exception, page someone, etc.
//! - [`ReportStrategy::CacoCli`] — shell out to the local `caco` CLI
//!   (`caco log error` / `caco log perf`), optionally also filing a bead.
//! - [`ReportStrategy::Stderr`] — write one JSON line per event to stderr.
//! - [`ReportStrategy::Disabled`] — drop everything (a safe default for tools
//!   that have not opted in).
//!
//! Because the strategy is plain serde data, a project can switch backends purely
//! from config without touching the CLI's code:
//!
//! ```
//! use feedback_cli::{FeedbackConfig, ReportStrategy};
//!
//! let config: FeedbackConfig = serde_json::from_str(r#"{
//!     "component": "my-cli",
//!     "project": "acme",
//!     "strategy": { "type": "webhook", "url": "https://example.invalid/hook", "token_env": "ACME_HOOK_TOKEN" }
//! }"#).unwrap();
//! assert!(matches!(config.strategy, ReportStrategy::Webhook(_)));
//! ```
//!
//! At runtime the CLI builds a [`Reporter`] from the config and reports events:
//!
//! ```
//! use feedback_cli::{FeedbackConfig, FeedbackEvent, Reporter};
//!
//! // Default config reports to stderr; a real CLI would load this from project config.
//! let reporter = Reporter::from_config(&FeedbackConfig::default());
//! reporter
//!     .report(&FeedbackEvent::error("startup", "failed to read config").with_detail("ENOENT"))
//!     .unwrap();
//! ```
//!
//! Any [`mcp_cli::StructuredError`] can be turned into an event with
//! [`FeedbackEvent::from_structured_error`], so an MCP tool failure can be fed
//! back with one call. The [`register_feedback_tools`] registrar also exposes the
//! reporter over MCP, mirroring the `updatable-cli` pattern.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use mcp_cli::{ErrorCategory, StructuredError, ToolRouter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod mcp;

/// The `caco` logging surface an event maps onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    /// A failure worth recording (`caco log error`).
    Error,
    /// A caught/uncaught exception (`caco log error` with an `exception` label).
    Exception,
    /// A performance observation or regression (`caco log perf`).
    Perf,
    /// Informational progress / status.
    Info,
}

impl FeedbackKind {
    /// Lowercase string form used in serialized payloads.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            FeedbackKind::Error => "error",
            FeedbackKind::Exception => "exception",
            FeedbackKind::Perf => "perf",
            FeedbackKind::Info => "info",
        }
    }
}

/// Severity of a feedback event, ordered least → most severe.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational.
    Info,
    /// Something unexpected that did not stop the operation.
    Warning,
    /// A failure the owning project should know about.
    Error,
    /// A failure that needs urgent attention.
    Critical,
}

impl Severity {
    /// Lowercase string form used in serialized payloads and `caco` flags.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Critical => "critical",
        }
    }
}

/// A numeric performance metric attached to a [`FeedbackKind::Perf`] event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Metric {
    /// Metric name, e.g. `build_time` or `p99_latency`.
    pub name: String,
    /// Observed value.
    pub value: f64,
    /// Optional unit, e.g. `ms`, `bytes`, `ops/s`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Optional regression threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Optional baseline / expected value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<f64>,
}

impl Metric {
    /// Create a metric from a name and value.
    #[must_use]
    pub fn new(name: impl Into<String>, value: f64) -> Self {
        Self {
            name: name.into(),
            value,
            unit: None,
            threshold: None,
            baseline: None,
        }
    }
}

/// A single structured feedback event — the payload that is reported back to the
/// project (sent to a webhook, passed to `caco`, or printed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackEvent {
    /// Which `caco` surface this maps onto.
    pub kind: FeedbackKind,
    /// Component / subsystem that produced the event (required by `caco`).
    pub component: String,
    /// Short human-readable summary.
    pub summary: String,
    /// Optional detailed body, stack trace, or context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Optional severity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    /// Optional labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Ordered key/value context.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    /// Optional dedupe fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Optional project name override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Optional metric for perf events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<Metric>,
    /// Milliseconds since the Unix epoch when the event was created.
    pub timestamp_unix_ms: u64,
}

impl FeedbackEvent {
    /// Create an event of `kind` with the current timestamp.
    #[must_use]
    pub fn new(
        kind: FeedbackKind,
        component: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            component: component.into(),
            summary: summary.into(),
            detail: None,
            severity: None,
            labels: Vec::new(),
            fields: BTreeMap::new(),
            fingerprint: None,
            project: None,
            metric: None,
            timestamp_unix_ms: now_unix_ms(),
        }
    }

    /// Shortcut for a [`FeedbackKind::Error`] event.
    #[must_use]
    pub fn error(component: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(FeedbackKind::Error, component, summary).with_severity(Severity::Error)
    }

    /// Shortcut for a [`FeedbackKind::Exception`] event.
    #[must_use]
    pub fn exception(component: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(FeedbackKind::Exception, component, summary)
            .with_severity(Severity::Error)
            .with_label("exception")
    }

    /// Shortcut for a [`FeedbackKind::Info`] event.
    #[must_use]
    pub fn info(component: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(FeedbackKind::Info, component, summary).with_severity(Severity::Info)
    }

    /// Shortcut for a [`FeedbackKind::Perf`] event carrying a [`Metric`].
    #[must_use]
    pub fn perf(component: impl Into<String>, summary: impl Into<String>, metric: Metric) -> Self {
        Self::new(FeedbackKind::Perf, component, summary).with_metric(metric)
    }

    /// Project an [`mcp_cli::StructuredError`] into a feedback event.
    ///
    /// The error's category maps to a [`Severity`], its code becomes the
    /// fingerprint and a label, its message becomes the summary, and any
    /// structured `details` object is flattened into string fields.
    #[must_use]
    pub fn from_structured_error<E>(component: impl Into<String>, error: &E) -> Self
    where
        E: StructuredError + ?Sized,
    {
        let code = error.code();
        let mut event = Self::new(FeedbackKind::Error, component, error.message())
            .with_severity(severity_for_category(error.category()))
            .with_label(format!("category:{}", category_str(error.category())))
            .with_fingerprint(code.clone());
        if !code.is_empty() {
            event = event.with_label(format!("code:{code}"));
        }
        if let Some(details) = error.details() {
            flatten_json_into_fields("details", &details, &mut event.fields);
        }
        event
    }

    /// Set the detailed body.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set the severity.
    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Add a label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Add a key/value context field.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Set the dedupe fingerprint.
    #[must_use]
    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.fingerprint = Some(fingerprint.into());
        self
    }

    /// Set the project override.
    #[must_use]
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Attach a metric (perf events).
    #[must_use]
    pub fn with_metric(mut self, metric: Metric) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Serialize the event to a compact JSON string (the webhook payload).
    ///
    /// # Errors
    /// Returns [`FeedbackError::Serialization`] if the event cannot be encoded.
    pub fn to_json(&self) -> Result<String, FeedbackError> {
        serde_json::to_string(self).map_err(|err| FeedbackError::Serialization(err.to_string()))
    }

    /// Render this event as a caco bead-create body matching the caco webhook
    /// `bead` handler's native fields (`title`, `description`, `type`,
    /// `priority`, `labels`).
    ///
    /// `summary` becomes the bead title; `detail` plus a structured context
    /// footer becomes the description; `kind`/`severity` choose the bead type
    /// and priority. Used by the [`WebhookPayload::CacoBead`] payload mode.
    #[must_use]
    pub fn to_caco_bead(&self) -> serde_json::Value {
        let bead_type = match self.kind {
            FeedbackKind::Error | FeedbackKind::Exception => "bug",
            FeedbackKind::Perf | FeedbackKind::Info => "task",
        };
        let priority = match self.severity {
            Some(Severity::Critical) => 1,
            Some(Severity::Warning) => 3,
            Some(Severity::Info) => 4,
            Some(Severity::Error) | None => 2,
        };
        let mut labels = vec![
            "feedback".to_owned(),
            format!("kind:{}", self.kind.as_str()),
        ];
        for label in &self.labels {
            if !labels.contains(label) {
                labels.push(label.clone());
            }
        }
        let mut context = vec![format!("component: {}", self.component)];
        if let Some(severity) = self.severity {
            context.push(format!("severity: {}", severity.as_str()));
        }
        if let Some(fingerprint) = &self.fingerprint {
            context.push(format!("fingerprint: {fingerprint}"));
        }
        if let Some(project) = &self.project {
            context.push(format!("project: {project}"));
        }
        for (key, value) in &self.fields {
            context.push(format!("{key}: {value}"));
        }
        if let Some(metric) = &self.metric {
            let unit = metric.unit.as_deref().unwrap_or("");
            context.push(format!("metric {}: {}{}", metric.name, metric.value, unit));
        }
        context.push(format!("timestamp_unix_ms: {}", self.timestamp_unix_ms));
        let footer = context.join("\n");
        let description = match &self.detail {
            Some(detail) if !detail.is_empty() => format!("{detail}\n\n{footer}"),
            _ => footer,
        };
        serde_json::json!({
            "title": self.summary,
            "description": description,
            "type": bead_type,
            "priority": priority,
            "labels": labels,
        })
    }

    /// Render [`FeedbackEvent::to_caco_bead`] as a compact JSON string.
    ///
    /// # Errors
    /// Returns [`FeedbackError::Serialization`] if the value cannot be encoded.
    pub fn to_caco_bead_json(&self) -> Result<String, FeedbackError> {
        serde_json::to_string(&self.to_caco_bead())
            .map_err(|err| FeedbackError::Serialization(err.to_string()))
    }
}

/// A destination for [`FeedbackEvent`]s. Implement this to forward events to a
/// backend other than the built-ins.
pub trait FeedbackSink: Send + Sync {
    /// Record a single event.
    ///
    /// # Errors
    /// Returns a [`FeedbackError`] if the event could not be delivered.
    fn record(&self, event: &FeedbackEvent) -> Result<(), FeedbackError>;

    /// A short, secret-free label describing where this sink delivers, used in
    /// receipts and status output.
    fn describe(&self) -> String;
}

/// A sink that discards every event.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl FeedbackSink for NullSink {
    fn record(&self, _event: &FeedbackEvent) -> Result<(), FeedbackError> {
        Ok(())
    }

    fn describe(&self) -> String {
        "disabled".to_owned()
    }
}

/// A sink that writes one JSON line per event to stderr.
#[derive(Debug, Default, Clone, Copy)]
pub struct StderrSink;

impl FeedbackSink for StderrSink {
    fn record(&self, event: &FeedbackEvent) -> Result<(), FeedbackError> {
        eprintln!("{}", event.to_json()?);
        Ok(())
    }

    fn describe(&self) -> String {
        "stderr".to_owned()
    }
}

/// Selects the JSON body shape the [`WebhookSink`] POSTs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WebhookPayload {
    /// POST the native [`FeedbackEvent`] JSON (default). Lossless; the receiver
    /// maps fields (a caco webhook can set `bead.title_from = "summary"`).
    #[default]
    Event,
    /// POST a caco bead-create body (`title`/`description`/`type`/`priority`/
    /// `labels`) matching the caco webhook `bead` handler's native fields, so no
    /// receiver-side field mapping is needed. See [`FeedbackEvent::to_caco_bead`].
    CacoBead,
}

/// A sink that POSTs each event as JSON to a configured webhook URL.
///
/// Sending requires the default-on `webhook` cargo feature (ureq/TLS). Without
/// it the sink still builds and serializes, but [`WebhookSink::record`] returns
/// a config error rather than pulling in the TLS stack.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "webhook"), allow(dead_code))]
pub struct WebhookSink {
    url: String,
    token: Option<String>,
    method: String,
    timeout_secs: u64,
    headers: BTreeMap<String, String>,
    payload: WebhookPayload,
}

impl WebhookSink {
    /// Build a webhook sink from a resolved [`WebhookConfig`].
    ///
    /// # Errors
    /// Returns [`FeedbackError::Config`] if the URL is empty or the token cannot
    /// be resolved from its environment variable.
    pub fn from_config(config: &WebhookConfig) -> Result<Self, FeedbackError> {
        Self::from_config_for(config, None)
    }

    /// Like [`WebhookSink::from_config`] but resolves the conventional
    /// `CACOPHONY_<PROJECT>_WEBHOOK_TOKEN` env var for `project` when no inline
    /// `token`/`token_env` is set.
    ///
    /// # Errors
    /// Returns [`FeedbackError::Config`] if the URL is empty or an explicit
    /// `token_env` is set but unreadable.
    pub fn from_config_for(
        config: &WebhookConfig,
        project: Option<&str>,
    ) -> Result<Self, FeedbackError> {
        if config.url.trim().is_empty() {
            return Err(FeedbackError::Config(
                "webhook url must not be empty".to_owned(),
            ));
        }
        let token = config.resolve_token_for(project)?;
        Ok(Self {
            url: config.url.clone(),
            token,
            method: config
                .method
                .clone()
                .unwrap_or_else(|| "POST".to_owned())
                .to_uppercase(),
            timeout_secs: config.timeout_secs.unwrap_or(30),
            headers: config.headers.clone(),
            payload: config.payload,
        })
    }
}

impl FeedbackSink for WebhookSink {
    fn record(&self, event: &FeedbackEvent) -> Result<(), FeedbackError> {
        let payload = match self.payload {
            WebhookPayload::Event => event.to_json()?,
            WebhookPayload::CacoBead => event.to_caco_bead_json()?,
        };
        self.send(&payload)
    }

    fn describe(&self) -> String {
        // Never include the token.
        format!("webhook {} {}", self.method, self.url)
    }
}

impl WebhookSink {
    /// POST the serialized payload over HTTPS via ureq.
    #[cfg(feature = "webhook")]
    fn send(&self, payload: &str) -> Result<(), FeedbackError> {
        let timeout = std::time::Duration::from_secs(self.timeout_secs);
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .timeout_write(timeout)
            .build();
        let mut request = agent
            .request(&self.method, &self.url)
            .set("Content-Type", "application/json");
        if let Some(token) = &self.token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        for (name, value) in &self.headers {
            request = request.set(name, value);
        }
        match request.send_string(payload) {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(code, _)) => Err(FeedbackError::Http(format!(
                "webhook {} returned status {code}",
                self.url
            ))),
            Err(err) => Err(FeedbackError::Http(format!(
                "webhook {} failed: {err}",
                self.url
            ))),
        }
    }

    /// Stub used when the `webhook` feature is disabled: building and serializing
    /// still work, but delivery returns a clear config error instead of pulling
    /// in a TLS stack.
    #[cfg(not(feature = "webhook"))]
    #[allow(clippy::unused_self)]
    fn send(&self, _payload: &str) -> Result<(), FeedbackError> {
        Err(FeedbackError::Config(
            "webhook reporting requires the `webhook` cargo feature".to_owned(),
        ))
    }
}

/// A sink that shells out to the local `caco` CLI.
#[derive(Debug, Clone)]
pub struct CacoCliSink {
    binary: String,
    project: Option<String>,
    create_beads: bool,
}

impl CacoCliSink {
    /// Build a caco-cli sink from a resolved [`CacoCliConfig`].
    #[must_use]
    pub fn from_config(config: &CacoCliConfig) -> Self {
        Self {
            binary: config.binary.clone().unwrap_or_else(|| "caco".to_owned()),
            project: config.project.clone(),
            create_beads: config.create_beads,
        }
    }

    /// Build the argv vector(s) this sink would run for `event`, without
    /// executing them. Exposed for testing and dry-runs.
    #[must_use]
    pub fn commands(&self, event: &FeedbackEvent) -> Vec<Vec<String>> {
        let project = event.project.clone().or_else(|| self.project.clone());
        let severity = event.severity.unwrap_or(Severity::Error);
        let mut commands = Vec::new();

        let mut primary = vec![self.binary.clone(), "log".to_owned()];
        let subcommand = if event.kind == FeedbackKind::Perf {
            "perf"
        } else {
            "error"
        };
        primary.push(subcommand.to_owned());
        if let Some(project) = &project {
            primary.push("--project".to_owned());
            primary.push(project.clone());
        }
        primary.push("--component".to_owned());
        primary.push(event.component.clone());
        primary.push("--summary".to_owned());
        primary.push(event.summary.clone());

        if event.kind == FeedbackKind::Perf {
            if let Some(metric) = &event.metric {
                primary.push("--metric".to_owned());
                primary.push(metric.name.clone());
                primary.push("--value".to_owned());
                primary.push(metric.value.to_string());
                if let Some(unit) = &metric.unit {
                    primary.push("--unit".to_owned());
                    primary.push(unit.clone());
                }
                if let Some(threshold) = metric.threshold {
                    primary.push("--threshold".to_owned());
                    primary.push(format!("{threshold}"));
                }
                if let Some(baseline) = metric.baseline {
                    primary.push("--baseline".to_owned());
                    primary.push(format!("{baseline}"));
                }
            }
        } else {
            primary.push("--severity".to_owned());
            primary.push(severity.as_str().to_owned());
            if let Some(detail) = &event.detail {
                primary.push("--detail".to_owned());
                primary.push(detail.clone());
            }
            if let Some(fingerprint) = &event.fingerprint {
                primary.push("--fingerprint".to_owned());
                primary.push(fingerprint.clone());
            }
        }
        if !event.labels.is_empty() {
            primary.push("--labels".to_owned());
            primary.push(event.labels.join(","));
        }
        commands.push(primary);

        if self.create_beads && matches!(event.kind, FeedbackKind::Error | FeedbackKind::Exception)
        {
            let mut bead = vec![self.binary.clone(), "bd".to_owned(), "create".to_owned()];
            if let Some(project) = &project {
                bead.push("--project".to_owned());
                bead.push(project.clone());
            }
            bead.push("--type".to_owned());
            bead.push("bug".to_owned());
            bead.push("--title".to_owned());
            bead.push(format!("[{}] {}", event.component, event.summary));
            if let Some(detail) = &event.detail {
                bead.push("--description".to_owned());
                bead.push(detail.clone());
            }
            commands.push(bead);
        }

        commands
    }
}

impl FeedbackSink for CacoCliSink {
    fn record(&self, event: &FeedbackEvent) -> Result<(), FeedbackError> {
        for argv in self.commands(event) {
            let (program, args) = argv
                .split_first()
                .ok_or_else(|| FeedbackError::Command("empty caco command".to_owned()))?;
            let status = std::process::Command::new(program)
                .args(args)
                .status()
                .map_err(|err| FeedbackError::Command(format!("failed to run {program}: {err}")))?;
            if !status.success() {
                return Err(FeedbackError::Command(format!(
                    "{} exited with {}",
                    argv.join(" "),
                    status
                        .code()
                        .map_or_else(|| "signal".to_owned(), |c| c.to_string())
                )));
            }
        }
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "caco-cli ({}{})",
            self.binary,
            if self.create_beads { ", +beads" } else { "" }
        )
    }
}

/// Webhook reporting strategy configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WebhookConfig {
    /// Endpoint to POST events to.
    pub url: String,
    /// Inline bearer token. Prefer [`WebhookConfig::token_env`] so secrets stay
    /// out of committed config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Name of an environment variable to read the bearer token from at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
    /// HTTP method (defaults to `POST`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Request timeout in seconds (defaults to 30).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Extra headers sent with every request.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// The JSON body shape to POST (default: the native event JSON).
    #[serde(default)]
    pub payload: WebhookPayload,
}

impl WebhookConfig {
    /// Resolve the bearer token using only the inline value or an explicit
    /// `token_env` (no conventional fallback). Equivalent to
    /// [`WebhookConfig::resolve_token_for`] with no project.
    ///
    /// # Errors
    /// Returns [`FeedbackError::Config`] if `token_env` is set but unreadable.
    pub fn resolve_token(&self) -> Result<Option<String>, FeedbackError> {
        self.resolve_token_for(None)
    }

    /// Resolve the bearer token. Precedence:
    /// 1. inline `token`,
    /// 2. explicit `token_env` (errors if the named var is unset),
    /// 3. the conventional env vars `CACOPHONY_<PROJECT>_WEBHOOK_TOKEN` (when a
    ///    `project` is known) then `CACOPHONY_WEBHOOK_TOKEN`.
    ///
    /// Returns `None` when no token is configured or discoverable (an
    /// unauthenticated webhook).
    ///
    /// # Errors
    /// Returns [`FeedbackError::Config`] if an explicit `token_env` is set but
    /// the named variable is not present.
    pub fn resolve_token_for(
        &self,
        project: Option<&str>,
    ) -> Result<Option<String>, FeedbackError> {
        if let Some(token) = &self.token {
            return Ok(Some(token.clone()));
        }
        if let Some(var) = &self.token_env {
            return match std::env::var(var) {
                Ok(value) => Ok(Some(value)),
                Err(_) => Err(FeedbackError::Config(format!(
                    "webhook token_env `{var}` is not set"
                ))),
            };
        }
        for var in conventional_token_env_vars(project) {
            if let Ok(value) = std::env::var(&var) {
                if !value.is_empty() {
                    return Ok(Some(value));
                }
            }
        }
        Ok(None)
    }
}

/// The conventional webhook bearer-token env var names, most specific first:
/// `CACOPHONY_<PROJECT>_WEBHOOK_TOKEN` (when `project` is set) then the generic
/// `CACOPHONY_WEBHOOK_TOKEN`. The project is upper-cased with non-alphanumeric
/// characters replaced by `_`.
#[must_use]
pub fn conventional_token_env_vars(project: Option<&str>) -> Vec<String> {
    let mut vars = Vec::new();
    if let Some(project) = project {
        let slug: String = project
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        if slug.chars().any(|c| c != '_') {
            vars.push(format!("CACOPHONY_{slug}_WEBHOOK_TOKEN"));
        }
    }
    vars.push("CACOPHONY_WEBHOOK_TOKEN".to_owned());
    vars
}

/// caco-cli reporting strategy configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CacoCliConfig {
    /// caco binary to invoke (defaults to `caco`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    /// Project to pass with `--project` (falls back to the event's project).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Also file a bead for error/exception events.
    #[serde(default)]
    pub create_beads: bool,
}

/// File reporting strategy configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FileConfig {
    /// Path to append JSON-lines events to (one JSON object per line).
    pub path: String,
}

/// A sink that appends one JSON line per event to a file (created if missing,
/// parent directories created as needed).
#[derive(Debug, Clone)]
pub struct FileSink {
    path: std::path::PathBuf,
}

impl FileSink {
    /// Build a file sink from a resolved [`FileConfig`].
    ///
    /// # Errors
    /// Returns [`FeedbackError::Config`] if the path is empty.
    pub fn from_config(config: &FileConfig) -> Result<Self, FeedbackError> {
        if config.path.trim().is_empty() {
            return Err(FeedbackError::Config(
                "file path must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            path: std::path::PathBuf::from(&config.path),
        })
    }
}

impl FeedbackSink for FileSink {
    fn record(&self, event: &FeedbackEvent) -> Result<(), FeedbackError> {
        use std::io::Write as _;
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    FeedbackError::Io(format!("create dir {}: {err}", parent.display()))
                })?;
            }
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|err| FeedbackError::Io(format!("open {}: {err}", self.path.display())))?;
        writeln!(file, "{}", event.to_json()?)
            .map_err(|err| FeedbackError::Io(format!("write {}: {err}", self.path.display())))
    }

    fn describe(&self) -> String {
        format!("file {}", self.path.display())
    }
}

/// How a [`Reporter`] delivers events. This is the unit that project config sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReportStrategy {
    /// Drop all events.
    Disabled,
    /// Write one JSON line per event to stderr.
    #[default]
    Stderr,
    /// POST events to a webhook.
    Webhook(WebhookConfig),
    /// Shell out to the local `caco` CLI.
    CacoCli(CacoCliConfig),
    /// Append one JSON line per event to a file.
    File(FileConfig),
}

impl ReportStrategy {
    /// Build the [`FeedbackSink`] this strategy describes.
    ///
    /// # Errors
    /// Returns a [`FeedbackError`] if the strategy is misconfigured (e.g. a
    /// webhook with an empty URL or an unresolvable token env var).
    pub fn build_sink(
        &self,
        default_project: Option<&str>,
    ) -> Result<Box<dyn FeedbackSink>, FeedbackError> {
        Ok(match self {
            ReportStrategy::Disabled => Box::new(NullSink),
            ReportStrategy::Stderr => Box::new(StderrSink),
            ReportStrategy::Webhook(config) => {
                Box::new(WebhookSink::from_config_for(config, default_project)?)
            }
            ReportStrategy::CacoCli(config) => Box::new(CacoCliSink::from_config(config)),
            ReportStrategy::File(config) => Box::new(FileSink::from_config(config)?),
        })
    }
}

/// Top-level feedback configuration a CLI exposes to its project config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackConfig {
    /// Master on/off switch; when false the reporter is a no-op regardless of
    /// `strategy`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default component/source applied to events that don't set their own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Default project applied to events that don't set their own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The reporting strategy.
    #[serde(default)]
    pub strategy: ReportStrategy,
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            component: None,
            project: None,
            strategy: ReportStrategy::default(),
        }
    }
}

impl FeedbackConfig {
    /// Construct a webhook config from environment variables, intended as a
    /// convenience for CLIs that prefer env over a config file.
    ///
    /// Reads (when present):
    /// - `FEEDBACK_WEBHOOK_URL` → enables the webhook strategy,
    /// - `FEEDBACK_WEBHOOK_TOKEN_ENV` → the env var holding the bearer token,
    /// - `FEEDBACK_COMPONENT`, `FEEDBACK_PROJECT` → defaults.
    ///
    /// When `FEEDBACK_WEBHOOK_URL` is unset the strategy defaults to stderr.
    #[must_use]
    pub fn from_env() -> Self {
        let component = std::env::var("FEEDBACK_COMPONENT").ok();
        let project = std::env::var("FEEDBACK_PROJECT").ok();
        let strategy =
            std::env::var("FEEDBACK_WEBHOOK_URL")
                .ok()
                .map_or(ReportStrategy::Stderr, |url| {
                    ReportStrategy::Webhook(WebhookConfig {
                        url,
                        token_env: std::env::var("FEEDBACK_WEBHOOK_TOKEN_ENV").ok(),
                        ..WebhookConfig::default()
                    })
                });
        Self {
            enabled: true,
            component,
            project,
            strategy,
        }
    }
}

/// Builds events from a configured strategy and delivers them.
pub struct Reporter {
    sink: Box<dyn FeedbackSink>,
    enabled: bool,
    default_component: Option<String>,
    default_project: Option<String>,
}

impl std::fmt::Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporter")
            .field("enabled", &self.enabled)
            .field("sink", &self.sink.describe())
            .field("default_component", &self.default_component)
            .field("default_project", &self.default_project)
            .finish()
    }
}

impl Reporter {
    /// Build a reporter from config. Falls back to a [`NullSink`] (and prints a
    /// warning to stderr) if the strategy is misconfigured, so a bad config never
    /// crashes the host CLI.
    #[must_use]
    pub fn from_config(config: &FeedbackConfig) -> Self {
        let sink = if config.enabled {
            config
                .strategy
                .build_sink(config.project.as_deref())
                .unwrap_or_else(|err| {
                    eprintln!("feedback-cli: disabling reporting: {err}");
                    Box::new(NullSink)
                })
        } else {
            Box::new(NullSink)
        };
        Self {
            sink,
            enabled: config.enabled,
            default_component: config.component.clone(),
            default_project: config.project.clone(),
        }
    }

    /// Build a reporter directly around a custom sink.
    #[must_use]
    pub fn with_sink(sink: Box<dyn FeedbackSink>) -> Self {
        Self {
            sink,
            enabled: true,
            default_component: None,
            default_project: None,
        }
    }

    /// Whether reporting is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// A secret-free description of where events are delivered.
    #[must_use]
    pub fn destination(&self) -> String {
        self.sink.describe()
    }

    /// Report a fully-built event, applying default component/project.
    ///
    /// # Errors
    /// Returns a [`FeedbackError`] if delivery fails.
    pub fn report(&self, event: &FeedbackEvent) -> Result<(), FeedbackError> {
        if !self.enabled {
            return Ok(());
        }
        let mut event = event.clone();
        if event.component.is_empty() {
            if let Some(component) = &self.default_component {
                event.component.clone_from(component);
            }
        }
        if event.project.is_none() {
            event.project.clone_from(&self.default_project);
        }
        self.sink.record(&event)
    }

    /// Report an [`mcp_cli::StructuredError`] as an error event.
    ///
    /// # Errors
    /// Returns a [`FeedbackError`] if delivery fails.
    pub fn report_error<E>(
        &self,
        component: impl Into<String>,
        error: &E,
    ) -> Result<(), FeedbackError>
    where
        E: StructuredError + ?Sized,
    {
        self.report(&FeedbackEvent::from_structured_error(component, error))
    }
}

/// Errors surfaced while reporting feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedbackError {
    /// A webhook HTTP request failed.
    Http(String),
    /// An event could not be serialized.
    Serialization(String),
    /// A `caco` CLI invocation failed.
    Command(String),
    /// The reporting strategy was misconfigured.
    Config(String),
    /// A local I/O operation (e.g. writing the file sink) failed.
    Io(String),
}

impl FeedbackError {
    fn code(&self) -> &'static str {
        match self {
            FeedbackError::Http(_) => "feedback_webhook_failed",
            FeedbackError::Serialization(_) => "feedback_serialization_failed",
            FeedbackError::Command(_) => "feedback_command_failed",
            FeedbackError::Config(_) => "feedback_config_invalid",
            FeedbackError::Io(_) => "feedback_io_failed",
        }
    }

    fn detail(&self) -> &str {
        match self {
            FeedbackError::Http(m)
            | FeedbackError::Serialization(m)
            | FeedbackError::Command(m)
            | FeedbackError::Config(m)
            | FeedbackError::Io(m) => m,
        }
    }
}

impl std::fmt::Display for FeedbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.detail())
    }
}

impl std::error::Error for FeedbackError {}

impl StructuredError for FeedbackError {
    fn category(&self) -> ErrorCategory {
        match self {
            FeedbackError::Http(_) | FeedbackError::Command(_) | FeedbackError::Io(_) => {
                ErrorCategory::ExecutionFailure
            }
            FeedbackError::Serialization(_) => ErrorCategory::SerializationError,
            FeedbackError::Config(_) => ErrorCategory::ConfigError,
        }
    }

    fn code(&self) -> String {
        FeedbackError::code(self).to_owned()
    }

    fn message(&self) -> String {
        self.detail().to_owned()
    }
}

/// Install a global panic hook that reports each panic as a
/// [`FeedbackKind::Exception`] event through `config`, then chains to the
/// previously-installed hook (so the default panic message / abort behaviour is
/// preserved).
///
/// Opt-in: call this once near the start of `main`. Reporting failures are
/// swallowed, so installing the hook never changes the panic path itself.
pub fn install_panic_hook(config: &FeedbackConfig) {
    let reporter = Reporter::from_config(config);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = panic_payload_message(info.payload());
        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));
        let _ = reporter.report(&panic_feedback_event(&message, location.as_deref()));
        previous(info);
    }));
}

/// Extract a human-readable message from a panic payload (`&str` / `String`).
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_owned()
    }
}

/// Build the [`FeedbackEvent`] for a panic with an optional source location.
fn panic_feedback_event(message: &str, location: Option<&str>) -> FeedbackEvent {
    let mut event = FeedbackEvent::exception("panic", message);
    if let Some(location) = location {
        event = event.with_field("location", location.to_owned());
    }
    if let Some(name) = std::thread::current().name() {
        event = event.with_field("thread", name.to_owned());
    }
    event
}

fn default_true() -> bool {
    true
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

const fn severity_for_category(category: ErrorCategory) -> Severity {
    match category {
        ErrorCategory::Validation
        | ErrorCategory::UnsupportedCapability
        | ErrorCategory::TargetNotFound => Severity::Warning,
        ErrorCategory::MissingPermission
        | ErrorCategory::ConfigError
        | ErrorCategory::SerializationError => Severity::Error,
        ErrorCategory::PlatformAdapterFailure
        | ErrorCategory::ExecutionFailure
        | ErrorCategory::Timeout => Severity::Critical,
    }
}

const fn category_str(category: ErrorCategory) -> &'static str {
    match category {
        ErrorCategory::Validation => "validation",
        ErrorCategory::UnsupportedCapability => "unsupported_capability",
        ErrorCategory::MissingPermission => "missing_permission",
        ErrorCategory::TargetNotFound => "target_not_found",
        ErrorCategory::PlatformAdapterFailure => "platform_adapter_failure",
        ErrorCategory::ExecutionFailure => "execution_failure",
        ErrorCategory::ConfigError => "config_error",
        ErrorCategory::SerializationError => "serialization_error",
        ErrorCategory::Timeout => "timeout",
    }
}

/// Flatten a JSON value into dotted string fields under `prefix`.
fn flatten_json_into_fields(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                flatten_json_into_fields(&format!("{prefix}.{key}"), child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                flatten_json_into_fields(&format!("{prefix}.{index}"), child, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_owned(), s.clone());
        }
        serde_json::Value::Null => {
            out.insert(prefix.to_owned(), "null".to_owned());
        }
        other => {
            out.insert(prefix.to_owned(), other.to_string());
        }
    }
}

/// MCP tool input for reporting an event.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReportArgs {
    /// Which surface the event maps onto.
    pub kind: FeedbackKind,
    /// Component / subsystem (falls back to the config default).
    #[serde(default)]
    pub component: Option<String>,
    /// Short summary.
    pub summary: String,
    /// Optional detail body.
    #[serde(default)]
    pub detail: Option<String>,
    /// Optional severity.
    #[serde(default)]
    pub severity: Option<Severity>,
    /// Optional labels.
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    /// Optional context fields.
    #[serde(default)]
    pub fields: Option<BTreeMap<String, String>>,
    /// Optional project override.
    #[serde(default)]
    pub project: Option<String>,
    /// Optional dedupe fingerprint.
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// Optional perf metric.
    #[serde(default)]
    pub metric: Option<Metric>,
}

impl ReportArgs {
    /// Build a [`FeedbackEvent`] from these args.
    #[must_use]
    pub fn into_event(self) -> FeedbackEvent {
        let mut event =
            FeedbackEvent::new(self.kind, self.component.unwrap_or_default(), self.summary);
        event.detail = self.detail;
        event.severity = self.severity;
        event.labels = self.labels.unwrap_or_default();
        event.fields = self.fields.unwrap_or_default();
        event.project = self.project;
        event.fingerprint = self.fingerprint;
        event.metric = self.metric;
        event
    }
}

/// Receipt returned by the `feedback_report` MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReportReceipt {
    /// Whether the event was delivered (false when reporting is disabled).
    pub reported: bool,
    /// Secret-free description of the delivery destination.
    pub destination: String,
}

/// Summary returned by the `feedback_status` MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackStatus {
    /// Whether reporting is enabled.
    pub enabled: bool,
    /// Strategy discriminant (`disabled`, `stderr`, `webhook`, `caco_cli`).
    pub strategy: String,
    /// Secret-free description of the delivery destination.
    pub destination: String,
    /// Default component, if configured.
    pub component: Option<String>,
    /// Default project, if configured.
    pub project: Option<String>,
}

fn strategy_name(strategy: &ReportStrategy) -> &'static str {
    match strategy {
        ReportStrategy::Disabled => "disabled",
        ReportStrategy::Stderr => "stderr",
        ReportStrategy::Webhook(_) => "webhook",
        ReportStrategy::CacoCli(_) => "caco_cli",
        ReportStrategy::File(_) => "file",
    }
}

/// Register `feedback_report` and `feedback_status` tools onto an MCP router,
/// mirroring `updatable-cli`'s `register_update_tool`.
///
/// The `config_builder` resolves a [`FeedbackConfig`] from the host context per
/// call, so the strategy can follow live project config.
pub fn register_feedback_tools<Ctx: Send + Sync + 'static>(
    router: &mut ToolRouter<Ctx>,
    config_builder: impl Fn(&Ctx) -> FeedbackConfig + Send + Sync + 'static,
) {
    let config_builder = std::sync::Arc::new(config_builder);

    let report_builder = config_builder.clone();
    router.add_typed_tool(
        "feedback_report",
        "Report a structured feedback/error/perf event through the configured strategy.",
        move |ctx: &Ctx, args: ReportArgs| {
            let config = report_builder(ctx);
            let reporter = Reporter::from_config(&config);
            let destination = reporter.destination();
            reporter.report(&args.into_event())?;
            Ok::<_, FeedbackError>(ReportReceipt {
                reported: reporter.is_enabled(),
                destination,
            })
        },
    );

    let status_builder = config_builder;
    router.add_typed_tool(
        "feedback_status",
        "Report the resolved feedback reporting configuration (secret-free).",
        move |ctx: &Ctx, _input: EmptyArgs| {
            let config = status_builder(ctx);
            let reporter = Reporter::from_config(&config);
            Ok::<_, FeedbackError>(FeedbackStatus {
                enabled: config.enabled,
                strategy: strategy_name(&config.strategy).to_owned(),
                destination: reporter.destination(),
                component: config.component.clone(),
                project: config.project.clone(),
            })
        },
    );
}

/// Empty argument type for parameterless MCP tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EmptyArgs {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A sink that records events in memory for assertions.
    #[derive(Default)]
    struct CapturingSink {
        events: Mutex<Vec<FeedbackEvent>>,
    }

    impl FeedbackSink for CapturingSink {
        fn record(&self, event: &FeedbackEvent) -> Result<(), FeedbackError> {
            self.events.lock().unwrap().push(event.clone());
            Ok(())
        }
        fn describe(&self) -> String {
            "capturing".to_owned()
        }
    }

    #[test]
    fn event_builders_set_kind_and_severity() {
        let err = FeedbackEvent::error("svc", "boom");
        assert_eq!(err.kind, FeedbackKind::Error);
        assert_eq!(err.severity, Some(Severity::Error));

        let exc = FeedbackEvent::exception("svc", "panic");
        assert_eq!(exc.kind, FeedbackKind::Exception);
        assert!(exc.labels.contains(&"exception".to_owned()));

        let perf = FeedbackEvent::perf("svc", "slow", Metric::new("latency", 12.0));
        assert_eq!(perf.kind, FeedbackKind::Perf);
        assert!((perf.metric.as_ref().unwrap().value - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn event_json_round_trips() {
        let event = FeedbackEvent::error("svc", "boom")
            .with_detail("stack")
            .with_field("k", "v")
            .with_project("proj");
        let json = event.to_json().unwrap();
        let parsed: FeedbackEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
        assert!(json.contains("\"kind\":\"error\""));
    }

    #[test]
    fn structured_error_maps_to_event() {
        let err = FeedbackError::Config("bad url".to_owned());
        let event = FeedbackEvent::from_structured_error("svc", &err);
        assert_eq!(event.severity, Some(Severity::Error));
        assert_eq!(
            event.fingerprint.as_deref(),
            Some("feedback_config_invalid")
        );
        assert!(event.labels.iter().any(|l| l == "category:config_error"));
        assert!(
            event
                .labels
                .iter()
                .any(|l| l == "code:feedback_config_invalid")
        );
    }

    #[test]
    fn config_deserializes_each_strategy() {
        let webhook: FeedbackConfig = serde_json::from_str(
            r#"{"strategy":{"type":"webhook","url":"https://h/x","token_env":"T"}}"#,
        )
        .unwrap();
        assert!(matches!(webhook.strategy, ReportStrategy::Webhook(_)));
        assert!(webhook.enabled);

        let caco: FeedbackConfig =
            serde_json::from_str(r#"{"strategy":{"type":"caco_cli","create_beads":true}}"#)
                .unwrap();
        match caco.strategy {
            ReportStrategy::CacoCli(c) => assert!(c.create_beads),
            other => panic!("unexpected: {other:?}"),
        }

        let disabled: FeedbackConfig =
            serde_json::from_str(r#"{"strategy":{"type":"disabled"}}"#).unwrap();
        assert!(matches!(disabled.strategy, ReportStrategy::Disabled));

        // Default strategy is stderr.
        let bare: FeedbackConfig = serde_json::from_str("{}").unwrap();
        assert!(matches!(bare.strategy, ReportStrategy::Stderr));
    }

    #[test]
    fn webhook_config_requires_url_and_resolves_token() {
        let err = WebhookSink::from_config(&WebhookConfig::default()).unwrap_err();
        assert!(matches!(err, FeedbackError::Config(_)));

        let cfg = WebhookConfig {
            url: "https://h/x".to_owned(),
            token: Some("secret".to_owned()),
            ..WebhookConfig::default()
        };
        let sink = WebhookSink::from_config(&cfg).unwrap();
        // Description must never leak the token.
        assert!(!sink.describe().contains("secret"));
    }

    #[test]
    fn webhook_payload_defaults_to_event() {
        let cfg: WebhookConfig = serde_json::from_str(r#"{"url":"https://h/x"}"#).unwrap();
        assert_eq!(cfg.payload, WebhookPayload::Event);
        let caco: WebhookConfig =
            serde_json::from_str(r#"{"url":"https://h/x","payload":"caco_bead"}"#).unwrap();
        assert_eq!(caco.payload, WebhookPayload::CacoBead);
    }

    #[test]
    fn to_caco_bead_maps_event_to_bead_fields() {
        let event = FeedbackEvent::error("build", "linker failed")
            .with_detail("ld: symbol not found")
            .with_field("crate", "acme")
            .with_label("ci");
        let bead = event.to_caco_bead();
        assert_eq!(bead["title"], "linker failed");
        assert_eq!(bead["type"], "bug"); // error -> bug
        assert_eq!(bead["priority"], 2); // error severity -> 2
        let labels: Vec<String> = bead["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        assert!(labels.contains(&"feedback".to_owned()));
        assert!(labels.contains(&"kind:error".to_owned()));
        assert!(labels.contains(&"ci".to_owned()));
        let description = bead["description"].as_str().unwrap();
        assert!(description.contains("ld: symbol not found"));
        assert!(description.contains("component: build"));
        assert!(description.contains("crate: acme"));

        // perf -> task; info -> task; exception -> bug.
        assert_eq!(
            FeedbackEvent::perf("p", "slow", Metric::new("m", 1.0)).to_caco_bead()["type"],
            "task"
        );
        assert_eq!(
            FeedbackEvent::exception("x", "boom").to_caco_bead()["type"],
            "bug"
        );
    }

    #[test]
    fn conventional_token_env_var_names() {
        assert_eq!(
            conventional_token_env_vars(Some("feedback-cli")),
            vec![
                "CACOPHONY_FEEDBACK_CLI_WEBHOOK_TOKEN".to_owned(),
                "CACOPHONY_WEBHOOK_TOKEN".to_owned(),
            ]
        );
        assert_eq!(
            conventional_token_env_vars(None),
            vec!["CACOPHONY_WEBHOOK_TOKEN".to_owned()]
        );
    }

    #[test]
    fn resolve_token_precedence_inline_and_explicit_env() {
        // Inline token wins regardless of project.
        let cfg = WebhookConfig {
            url: "https://h/x".to_owned(),
            token: Some("inline".to_owned()),
            ..WebhookConfig::default()
        };
        assert_eq!(
            cfg.resolve_token_for(Some("p")).unwrap().as_deref(),
            Some("inline")
        );
        // An explicit but unset token_env is a hard error, not a silent fallback.
        let cfg = WebhookConfig {
            url: "https://h/x".to_owned(),
            token_env: Some("DEFINITELY_UNSET_FBCLI_VAR_ZZ".to_owned()),
            ..WebhookConfig::default()
        };
        assert!(matches!(
            cfg.resolve_token_for(Some("p")),
            Err(FeedbackError::Config(_))
        ));
    }

    #[test]
    fn file_sink_appends_json_lines() {
        let path = std::env::temp_dir().join(format!(
            "feedback-cli-test-{}-{}.jsonl",
            std::process::id(),
            now_unix_ms()
        ));
        let _ = std::fs::remove_file(&path);
        let sink = FileSink::from_config(&FileConfig {
            path: path.display().to_string(),
        })
        .unwrap();
        sink.record(&FeedbackEvent::error("svc", "one")).unwrap();
        sink.record(&FeedbackEvent::info("svc", "two")).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"summary\":\"one\""));
        assert!(lines[1].contains("\"summary\":\"two\""));
        assert!(sink.describe().starts_with("file "));

        // empty path is a config error
        assert!(matches!(
            FileSink::from_config(&FileConfig {
                path: String::new()
            }),
            Err(FeedbackError::Config(_))
        ));

        // the strategy round-trips from config and names correctly
        let cfg: FeedbackConfig =
            serde_json::from_str(r#"{"strategy":{"type":"file","path":"/tmp/x.jsonl"}}"#).unwrap();
        assert!(matches!(cfg.strategy, ReportStrategy::File(_)));
        assert_eq!(strategy_name(&cfg.strategy), "file");
    }

    #[test]
    fn panic_payload_message_extracts_str_and_string() {
        let s: &(dyn std::any::Any + Send) = &"boom";
        assert_eq!(panic_payload_message(s), "boom");
        let owned: Box<dyn std::any::Any + Send> = Box::new(String::from("kaboom"));
        assert_eq!(panic_payload_message(owned.as_ref()), "kaboom");
        let other: &(dyn std::any::Any + Send) = &42i32;
        assert_eq!(panic_payload_message(other), "panic");
    }

    #[test]
    fn panic_feedback_event_is_an_exception_with_location() {
        let event = panic_feedback_event("boom", Some("src/x.rs:1:2"));
        assert_eq!(event.kind, FeedbackKind::Exception);
        assert_eq!(event.summary, "boom");
        assert_eq!(
            event.fields.get("location").map(String::as_str),
            Some("src/x.rs:1:2")
        );
        assert!(event.labels.iter().any(|l| l == "exception"));
    }

    #[test]
    fn disabled_reporter_is_noop() {
        let config = FeedbackConfig {
            enabled: false,
            ..FeedbackConfig::default()
        };
        let reporter = Reporter::from_config(&config);
        assert!(!reporter.is_enabled());
        reporter.report(&FeedbackEvent::error("svc", "x")).unwrap();
    }

    #[test]
    fn reporter_applies_defaults_and_delivers() {
        let captured = std::sync::Arc::new(CapturingSink::default());
        let reporter = Reporter {
            sink: Box::new(ArcSink(captured.clone())),
            enabled: true,
            default_component: Some("default-cmp".to_owned()),
            default_project: Some("default-proj".to_owned()),
        };

        reporter
            .report(&FeedbackEvent::new(FeedbackKind::Info, "", "hi"))
            .unwrap();
        let events = captured.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].component, "default-cmp");
        assert_eq!(events[0].project.as_deref(), Some("default-proj"));
    }

    /// Adapter so a shared `Arc<CapturingSink>` can be used as the reporter's
    /// boxed sink while the test still holds a handle for assertions.
    struct ArcSink(std::sync::Arc<CapturingSink>);
    impl FeedbackSink for ArcSink {
        fn record(&self, event: &FeedbackEvent) -> Result<(), FeedbackError> {
            self.0.record(event)
        }
        fn describe(&self) -> String {
            self.0.describe()
        }
    }

    #[test]
    fn caco_cli_builds_expected_commands() {
        let sink = CacoCliSink::from_config(&CacoCliConfig {
            binary: None,
            project: Some("proj".to_owned()),
            create_beads: true,
        });
        let event = FeedbackEvent::error("svc", "boom").with_detail("trace");
        let commands = sink.commands(&event);
        assert_eq!(commands.len(), 2, "error + bead");
        assert_eq!(commands[0][0], "caco");
        assert!(commands[0].contains(&"error".to_owned()));
        assert!(commands[0].contains(&"--component".to_owned()));
        assert!(commands[0].contains(&"proj".to_owned()));
        assert_eq!(commands[1][1], "bd");
        assert_eq!(commands[1][2], "create");

        let perf = FeedbackEvent::perf("svc", "slow", Metric::new("latency", 9.5));
        let perf_cmds = CacoCliSink::from_config(&CacoCliConfig::default()).commands(&perf);
        assert_eq!(perf_cmds.len(), 1);
        assert!(perf_cmds[0].contains(&"perf".to_owned()));
        assert!(perf_cmds[0].contains(&"--metric".to_owned()));
        assert!(perf_cmds[0].contains(&"latency".to_owned()));
    }

    #[test]
    #[cfg(feature = "webhook")]
    fn webhook_sink_posts_event_with_auth_header() {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .ok();
            // Read until the full request (headers + Content-Length body) is in
            // hand; a single read() can return just the headers before the body
            // arrives in a later TCP segment.
            let mut data = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        data.extend_from_slice(&tmp[..n]);
                        let text = String::from_utf8_lossy(&data);
                        if let Some(hdr_end) = text.find("\r\n\r\n") {
                            let body_len = text[..hdr_end]
                                .lines()
                                .find_map(|line| {
                                    let (name, value) = line.split_once(':')?;
                                    if name.trim().eq_ignore_ascii_case("content-length") {
                                        value.trim().parse::<usize>().ok()
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            if data.len() >= hdr_end + 4 + body_len {
                                break;
                            }
                        }
                    }
                }
            }
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            String::from_utf8_lossy(&data).to_string()
        });

        let sink = WebhookSink::from_config(&WebhookConfig {
            url: format!("http://{addr}/hook"),
            token: Some("tok123".to_owned()),
            ..WebhookConfig::default()
        })
        .unwrap();
        sink.record(&FeedbackEvent::error("svc", "boom")).unwrap();

        let request = handle.join().unwrap();
        assert!(request.starts_with("POST /hook"), "request was: {request}");
        assert!(request.contains("Authorization: Bearer tok123"));
        assert!(request.contains("\"summary\":\"boom\""));
    }

    #[test]
    fn registrar_mounts_report_and_status_tools() {
        use mcp_cli::JsonEnvelope;
        use serde_json::json;

        struct Ctx;
        let mut router: ToolRouter<Ctx> = ToolRouter::new();
        register_feedback_tools(&mut router, |_ctx: &Ctx| FeedbackConfig {
            enabled: true,
            component: Some("cmp".to_owned()),
            project: None,
            strategy: ReportStrategy::Disabled,
        });

        let names: Vec<String> = router.tool_metadata().into_iter().map(|m| m.name).collect();
        assert!(names.iter().any(|n| n == "feedback_report"));
        assert!(names.iter().any(|n| n == "feedback_status"));

        // status tool returns the resolved strategy.
        let env = router.call_tool(&Ctx, "feedback_status", json!({}));
        match env {
            JsonEnvelope::Success { data, .. } => {
                assert_eq!(data["strategy"], "disabled");
                assert_eq!(data["enabled"], true);
            }
            JsonEnvelope::Error { error, .. } => panic!("unexpected error: {error:?}"),
        }

        // report tool succeeds against the disabled strategy.
        let env = router.call_tool(
            &Ctx,
            "feedback_report",
            json!({ "kind": "error", "summary": "x" }),
        );
        assert!(matches!(env, JsonEnvelope::Success { .. }));
    }
}
