// feedback-cli Apple client (iOS + macOS) — a tiny, dependency-free drop-in so
// a Swift app can report structured feedback back to its project as a caco
// bead, using the SAME webhook + payload contract as the Rust `feedback-cli`
// crate. Single file: drop into an app/SwiftPM target.
//
//   try await reportFeedback(
//     WebhookConfig(url: "https://host/hooks/global/feedback", token: "..."),
//     FeedbackEvent(kind: .error, component: "sync", summary: "upload failed",
//                   detail: err.localizedDescription, labels: ["ios"]))

import Foundation

public enum FeedbackKind: String { case error, exception, perf, info }
public enum Severity: String { case info, warning, error, critical }

public struct Metric {
    public var name: String; public var value: Double
    public var unit: String?; public var threshold: Double?; public var baseline: Double?
    public init(name: String, value: Double, unit: String? = nil, threshold: Double? = nil, baseline: Double? = nil) {
        self.name = name; self.value = value; self.unit = unit; self.threshold = threshold; self.baseline = baseline
    }
}

public struct FeedbackEvent {
    public var kind: FeedbackKind
    public var component: String
    public var summary: String
    public var detail: String?
    public var severity: Severity?
    public var labels: [String]
    public var fields: [String: String]
    public var fingerprint: String?
    public var project: String?
    public var metric: Metric?
    public var timestampUnixMs: UInt64
    public init(kind: FeedbackKind, component: String, summary: String, detail: String? = nil,
                severity: Severity? = nil, labels: [String] = [], fields: [String: String] = [:],
                fingerprint: String? = nil, project: String? = nil, metric: Metric? = nil,
                timestampUnixMs: UInt64 = UInt64(Date().timeIntervalSince1970 * 1000)) {
        self.kind = kind; self.component = component; self.summary = summary; self.detail = detail
        self.severity = severity; self.labels = labels; self.fields = fields; self.fingerprint = fingerprint
        self.project = project; self.metric = metric; self.timestampUnixMs = timestampUnixMs
    }
}

public struct WebhookConfig {
    public var url: String; public var token: String?; public var payload: String
    public init(url: String, token: String? = nil, payload: String = "caco_bead") {
        self.url = url; self.token = token; self.payload = payload
    }
}

// caco_bead payload (mirrors FeedbackEvent::to_caco_bead in src/lib.rs).
public func toCacoBead(_ e: FeedbackEvent) -> [String: Any] {
    let type = (e.kind == .error || e.kind == .exception) ? "bug" : "task"
    let priority: Int = e.severity == .critical ? 1 : e.severity == .warning ? 3 : e.severity == .info ? 4 : 2
    var labels = ["feedback", "kind:\(e.kind.rawValue)"]
    for l in e.labels where !labels.contains(l) { labels.append(l) }
    var ctx = ["component: \(e.component)"]
    if let s = e.severity { ctx.append("severity: \(s.rawValue)") }
    if let f = e.fingerprint { ctx.append("fingerprint: \(f)") }
    if let p = e.project { ctx.append("project: \(p)") }
    for (k, v) in e.fields.sorted(by: { $0.key < $1.key }) { ctx.append("\(k): \(v)") }
    if let m = e.metric { ctx.append("metric \(m.name): \(m.value)\(m.unit ?? "")") }
    ctx.append("timestamp_unix_ms: \(e.timestampUnixMs)")
    let footer = ctx.joined(separator: "\n")
    let description = (e.detail?.isEmpty == false) ? "\(e.detail!)\n\n\(footer)" : footer
    return ["title": e.summary, "description": description, "type": type, "priority": priority, "labels": labels]
}

// POST the feedback event to the configured caco webhook. Returns true on 2xx.
@discardableResult
public func reportFeedback(_ cfg: WebhookConfig, _ e: FeedbackEvent) async throws -> Bool {
    var req = URLRequest(url: URL(string: cfg.url)!)
    req.httpMethod = "POST"
    req.setValue("application/json", forHTTPHeaderField: "Content-Type")
    if let t = cfg.token { req.setValue("Bearer \(t)", forHTTPHeaderField: "Authorization") }
    req.httpBody = try JSONSerialization.data(withJSONObject: toCacoBead(e))
    let (_, resp) = try await URLSession.shared.data(for: req)
    return (200..<300).contains((resp as? HTTPURLResponse)?.statusCode ?? 0)
}
