// feedback-cli web client — a tiny, dependency-free drop-in so a webapp can
// report structured feedback back to its project as a caco bead, using the
// SAME webhook + payload contract as the Rust `feedback-cli` crate.
//
// Drop this file into a webapp and:
//
//   import { reportFeedback } from "./feedback";
//   await reportFeedback(
//     { url: "https://host/hooks/global/feedback", token: "..." },
//     { kind: "error", component: "checkout", summary: "payment failed",
//       detail: String(err), labels: ["web"] });
//
// Same hooks as feedback-cli: POSTs JSON (default: caco_bead shape) with an
// `Authorization: Bearer <token>` header to a caco `bead` webhook.

export type FeedbackKind = "error" | "exception" | "perf" | "info";
export type Severity = "info" | "warning" | "error" | "critical";

export interface Metric {
  name: string;
  value: number;
  unit?: string;
  threshold?: number;
  baseline?: number;
}

export interface FeedbackEvent {
  kind: FeedbackKind;
  component: string;
  summary: string;
  detail?: string;
  severity?: Severity;
  labels?: string[];
  fields?: Record<string, string>;
  fingerprint?: string;
  project?: string;
  metric?: Metric;
  /** Defaults to Date.now() when omitted. */
  timestampUnixMs?: number;
}

export interface WebhookConfig {
  url: string;
  token?: string;
  /** "caco_bead" (default) maps to bead fields; "event" sends the native event. */
  payload?: "caco_bead" | "event";
}

/** Native event payload (mirrors feedback-cli's serde shape, snake_case). */
export function toEvent(e: FeedbackEvent): Record<string, unknown> {
  const out: Record<string, unknown> = {
    kind: e.kind,
    component: e.component,
    summary: e.summary,
    timestamp_unix_ms: e.timestampUnixMs ?? Date.now(),
  };
  if (e.detail) out.detail = e.detail;
  if (e.severity) out.severity = e.severity;
  if (e.labels?.length) out.labels = e.labels;
  if (e.fields && Object.keys(e.fields).length) out.fields = e.fields;
  if (e.fingerprint) out.fingerprint = e.fingerprint;
  if (e.project) out.project = e.project;
  if (e.metric) out.metric = e.metric;
  return out;
}

/** caco_bead payload (mirrors FeedbackEvent::to_caco_bead in src/lib.rs). */
export function toCacoBead(e: FeedbackEvent): Record<string, unknown> {
  const type = e.kind === "error" || e.kind === "exception" ? "bug" : "task";
  const priority =
    e.severity === "critical" ? 1 : e.severity === "warning" ? 3 : e.severity === "info" ? 4 : 2;
  const labels = ["feedback", `kind:${e.kind}`];
  for (const l of e.labels ?? []) if (!labels.includes(l)) labels.push(l);
  const ctx: string[] = [`component: ${e.component}`];
  if (e.severity) ctx.push(`severity: ${e.severity}`);
  if (e.fingerprint) ctx.push(`fingerprint: ${e.fingerprint}`);
  if (e.project) ctx.push(`project: ${e.project}`);
  // Sort fields by key to match the Rust contract (BTreeMap) and the iOS /
  // Android clients, so a multi-field event renders an identical footer
  // everywhere (the README promises this client mirrors to_caco_bead exactly).
  for (const [k, v] of Object.entries(e.fields ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
    ctx.push(`${k}: ${v}`);
  if (e.metric) ctx.push(`metric ${e.metric.name}: ${e.metric.value}${e.metric.unit ?? ""}`);
  ctx.push(`timestamp_unix_ms: ${e.timestampUnixMs ?? Date.now()}`);
  const footer = ctx.join("\n");
  const description = e.detail ? `${e.detail}\n\n${footer}` : footer;
  return { title: e.summary, description, type, priority, labels };
}

/** POST the feedback event to the configured caco webhook. Returns true on 2xx. */
export async function reportFeedback(cfg: WebhookConfig, e: FeedbackEvent): Promise<boolean> {
  const body = (cfg.payload ?? "caco_bead") === "event" ? toEvent(e) : toCacoBead(e);
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (cfg.token) headers["authorization"] = `Bearer ${cfg.token}`;
  const res = await fetch(cfg.url, { method: "POST", headers, body: JSON.stringify(body) });
  return res.ok;
}
