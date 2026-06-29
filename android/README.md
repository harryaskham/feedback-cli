# feedback-cli — Android (Kotlin) drop-in

Same idea as the Rust `feedback-cli` crate, for an Android app: report
structured feedback to your project as a caco bead, over the **same webhook +
payload contract**. Only `java.net.HttpURLConnection` + `org.json` (no extra deps).

## Drop in

Add `Feedback.kt` to your module, call off the main thread (e.g. `Dispatchers.IO`):

```kotlin
withContext(Dispatchers.IO) {
  reportFeedback(
    WebhookConfig("https://<host>/hooks/global/feedback", BuildConfig.FEEDBACK_TOKEN),
    FeedbackEvent(Kind.ERROR, "sync", "upload failed", detail = e.message, labels = listOf("android")))
}
```

Default sends the `caco_bead` shape (`{title,description,type,priority,labels}`),
identical mapping to the Rust crate: `type` bug for error/exception else task;
`priority` critical1/warning3/info4/error|none2; labels include `feedback`+`kind:<k>`.
Needs `<uses-permission android:name="android.permission.INTERNET"/>`; keep the
bearer token out of source.
