# feedback-cli — Apple (Swift, iOS + macOS) drop-in

Same idea as the Rust `feedback-cli` crate, for a Swift app: report structured
feedback to your project as a caco bead, over the **same webhook + payload
contract**. Zero dependencies (Foundation only). Works on iPhone and macOS.

## Drop in

Add `Feedback.swift` to your app/SwiftPM target, then:

```swift
try await reportFeedback(
  WebhookConfig(url: "https://<host>/hooks/global/feedback", token: token),
  FeedbackEvent(kind: .error, component: "sync", summary: "upload failed",
                detail: err.localizedDescription, labels: ["ios"]))
```

Default sends the `caco_bead` shape (`{title,description,type,priority,labels}`),
identical mapping to the Rust crate: `type` bug for error/exception else task;
`priority` critical1/warning3/info4/error|none2; labels include `feedback`+`kind:<k>`.
Keep the bearer token in the keychain/secret, not source.
