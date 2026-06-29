// feedback-cli Android client (Kotlin) — a tiny, dependency-free drop-in so an
// Android app can report structured feedback back to its project as a caco
// bead, using the SAME webhook + payload contract as the Rust `feedback-cli`
// crate. Single file (java.net.HttpURLConnection only). Call off the main
// thread (e.g. a coroutine on Dispatchers.IO).
//
//   reportFeedback(WebhookConfig("https://host/hooks/global/feedback", token),
//     FeedbackEvent(Kind.ERROR, "sync", "upload failed", detail = e.message, labels = listOf("android")))

package dev.cacophony.feedback

import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

enum class Kind(val v: String) { ERROR("error"), EXCEPTION("exception"), PERF("perf"), INFO("info") }
enum class Severity(val v: String) { INFO("info"), WARNING("warning"), ERROR("error"), CRITICAL("critical") }

data class Metric(val name: String, val value: Double, val unit: String? = null,
                  val threshold: Double? = null, val baseline: Double? = null)

data class FeedbackEvent(
    val kind: Kind,
    val component: String,
    val summary: String,
    val detail: String? = null,
    val severity: Severity? = null,
    val labels: List<String> = emptyList(),
    val fields: Map<String, String> = emptyMap(),
    val fingerprint: String? = null,
    val project: String? = null,
    val metric: Metric? = null,
    val timestampUnixMs: Long = System.currentTimeMillis(),
)

data class WebhookConfig(val url: String, val token: String? = null, val payload: String = "caco_bead")

// caco_bead payload (mirrors FeedbackEvent::to_caco_bead in src/lib.rs).
fun toCacoBead(e: FeedbackEvent): JSONObject {
    val type = if (e.kind == Kind.ERROR || e.kind == Kind.EXCEPTION) "bug" else "task"
    val priority = when (e.severity) {
        Severity.CRITICAL -> 1; Severity.WARNING -> 3; Severity.INFO -> 4; else -> 2
    }
    val labels = mutableListOf("feedback", "kind:${e.kind.v}")
    e.labels.forEach { if (!labels.contains(it)) labels.add(it) }
    val ctx = mutableListOf("component: ${e.component}")
    e.severity?.let { ctx.add("severity: ${it.v}") }
    e.fingerprint?.let { ctx.add("fingerprint: $it") }
    e.project?.let { ctx.add("project: $it") }
    e.fields.toSortedMap().forEach { (k, v) -> ctx.add("$k: $v") }
    e.metric?.let { ctx.add("metric ${it.name}: ${it.value}${it.unit ?: ""}") }
    ctx.add("timestamp_unix_ms: ${e.timestampUnixMs}")
    val footer = ctx.joinToString("\n")
    val description = if (!e.detail.isNullOrEmpty()) "${e.detail}\n\n$footer" else footer
    return JSONObject(mapOf(
        "title" to e.summary, "description" to description,
        "type" to type, "priority" to priority, "labels" to labels,
    ))
}

/** POST the feedback event to the configured caco webhook. Returns true on 2xx. Call off-main-thread. */
fun reportFeedback(cfg: WebhookConfig, e: FeedbackEvent): Boolean {
    val conn = URL(cfg.url).openConnection() as HttpURLConnection
    return try {
        conn.requestMethod = "POST"
        conn.doOutput = true
        conn.setRequestProperty("Content-Type", "application/json")
        cfg.token?.let { conn.setRequestProperty("Authorization", "Bearer $it") }
        conn.outputStream.use { it.write(toCacoBead(e).toString().toByteArray()) }
        conn.responseCode in 200..299
    } finally {
        conn.disconnect()
    }
}
