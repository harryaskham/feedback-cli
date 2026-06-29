# feedback-cli — web (TypeScript) drop-in

Same idea as the Rust `feedback-cli` crate, for a webapp: report structured
feedback to your project as a caco bead, over the **same webhook + payload
contract**. Zero dependencies (uses `fetch`).

## Drop in

Copy `feedback.ts` into your app, then:

```ts
import { reportFeedback } from "./feedback";

await reportFeedback(
  { url: "https://<host>/hooks/global/feedback", token: import.meta.env.VITE_FEEDBACK_TOKEN },
  { kind: "error", component: "checkout", summary: "payment failed",
    detail: String(err), labels: ["web"], severity: "error" });
```

- `payload: "caco_bead"` (default) → `{title,description,type,priority,labels}` (bead hook).
- `payload: "event"` → native `FeedbackEvent` JSON (use a `bead.title_from: summary` hook).
- `type` = `bug` for error/exception else `task`; `priority` critical1/warning3/info4/error|none2;
  labels always include `feedback` + `kind:<kind>`. Identical mapping to the Rust crate.

Token is a per-hook/section bearer; keep it out of source (env/secret). For a public
SPA, prefer a thin same-origin proxy so the token isn't shipped to the browser.
