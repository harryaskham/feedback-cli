# Session summary — feedback-cli CI taken off the dead azure-ephemeral label

## Goal

feedback-cli's CI had been silently non-functional for a month: every job
targeted a self-hosted runner label that no runner carried, so runs sat
unassigned for 24 hours and were auto-cancelled. Several commits landed on main
with zero verification. The tracking bead was filed as infra-lane and explicitly
marked not-worker-implementable, so it sat open and unclaimed on an otherwise
drained board. This session re-tested that assumption, found the infra half had
since been fixed, landed the remaining in-repo half, and verified CI green.

## Bead(s)

- `bd-26bdac` — [ci] feedback-cli has no registered runners — CI jobs
  queue-timeout (24h) with no azure-ephemeral runner (closed)
- Filed in `cacophony`: `bd-27e88c` (P1) — malformed assignee triple from
  `CACOPHONY_PROJECT` blocks reintegration; plus three drafts on worker-scope
  dedup, stale post-claim reads, and fleet-wide azure-ephemeral exposure.

## Before state

- `gh api repos/harryaskham/feedback-cli/actions/runners` at bead-filing time:
  `total_count: 0`.
- Last 7 CI runs on main (2026-06-28 → 2026-07-02): all `cancelled`, each after
  sitting queued with `runner=""` until the 24h ceiling. No completed run since
  2026-06-28.
- `ci.yml` (1 job) and `release.yml` (3 jobs) pinned to
  `runs-on: [self-hosted, azure-ephemeral]`.
- Failing tests: none known — but none *runnable* in CI, which was the point.

## After state

- `gh api .../actions/runners` now returns `total_count: 14`: 7 persistent
  x86_64 NixOS runners (`feedback-cli-ms-dev{,-2..-6}`, `-pocket4`), one
  aarch64-darwin (`-ms-mac`, offline), 6 Windows. None carry `azure-ephemeral`.
- All 4 job definitions target `[self-hosted, nix, x86_64-linux]`.
- Run `30555739347` on `cf8cea4`: picked up by `feedback-cli-pocket4` within
  ~15s, **completed success** in ~5 min (checkout, `nix --version`,
  `nix build .#feedback-cli`, `nix flake check` all green). First completed CI
  run on this repo since 2026-06-28.
- `bd-26bdac` closed. Project board: 0 open, 0 in-progress.

## Diff summary

- Code commits: `90f9963` (workflow retarget), `cdf6400` + its revert
  (gitignore, see below); landed on main at `cf8cea4`, final squash SHA for the
  follow-up from the reintegration receipt.
- Files touched: `.github/workflows/ci.yml`, `.github/workflows/release.yml`,
  `flake.nix`, `.gitignore`.
- Tests: +0 / -0. The change makes the existing suite reachable by CI at all.
- Behavioural delta: CI and release jobs are dispatchable again. The two heavy
  job budgets go 30m → 60m so a cold-cache first run cannot fail on the clock.
  Stale "builds on the azure-ephemeral pool" claims removed from both workflow
  headers and one `flake.nix` comment.

## Operator-takeaway

Two things worth carrying forward.

First: a bead marked "infra-lane, not worker-implementable" is a claim about the
world at filing time, not a permanent property. The infra half here was quietly
fixed weeks ago, nobody re-tested the assumption, and a P2 broken-on-main bug sat
open on a drained board while the actual fix was a four-line label swap. Re-probe
the stated blocker on anything parked for weeks.

Second, and more expensive: I broke summary publication mid-session and had to
catch it myself. The composed instructions contradict each other — the
session-recording mixin says author the summary and then *stop*, never
`git add`/`git commit` it, while the reintegration dirty-guard (`bd-ab3050`)
refuses to proceed with any untracked file, exempting only four managed hook
paths. Trying to satisfy both, I gitignored `.cacophony/`. That "worked" — it
landed cleanly and produced nothing. Artefact discovery is
`git diff <target> <agent-branch>` over **committed** content
(`cacophony_state::list_artefact_paths`), so an ignored summary is never found,
never enqueued, and never published; `caco summaries list` stays empty forever
and no error is raised anywhere. The real design is: commit the summary on the
agent branch, and the daemon splits it onto the `cacophony-state` branch and
strips it from the code commit — main carries zero `.cacophony/` paths and that
branch already holds 16 prior summaries from this project. The failure is
silent and permanent per repo, so anyone who "fixes" the guard conflict the way
I first did quietly deletes their project's memory. That contradiction needs
resolving in the mixin text, not in each agent's judgment.
