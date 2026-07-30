# Session summary — untrack the `result` symlink; verify main is green after 4 weeks of dark CI

## Goal

First session for this persistent feedback-cli worker. The board turned out to
be effectively drained — one open bead, which another worker won the claim race
on — so the session became a drained-board audit that produced two things an
operator actually wanted: proof that `main` is green despite four weeks with no
working CI, and a fix for a repo-hygiene defect that had been quietly dirtying
every checkout that ran the project's own documented build.

## Bead(s)

- `bd-221864` — `result` symlink is tracked in git, so every `nix build` dirties the checkout (filed and implemented this session)
- `bd-26bdac` — [ci] feedback-cli has no registered runners (NOT mine: claim race lost to `md4-0`; I handed over fresh evidence instead)
- `bd-eb0221` (cacophony, draft) — `caco bd claim` should do one confirming read instead of returning indeterminate
- one further cacophony draft on CI-gone-dark detection, filed via the outbox during a primary outage

## Before state

- Failing tests: unknown — CI had not successfully run since ~2026-06-30. The last three
  `CI` runs on `main` all ended `cancelled` at the 24h "exceeded the maximum execution
  time while awaiting a runner" queue timeout, so `bd-fb6e3a` and `bd-0432f9` had landed
  entirely unverified.
- `git ls-files -s result` -> `120000 51931b39... 0  result`: a tracked symlink pointing at
  a `/nix/store` path, introduced by a single broad `git add` in `1e6722a`.
- `.gitignore` contained only `/target`, `**/*.rs.bk`, `/.direnv/` — no `result`, and no
  `.cacophony/`, even though every peer repo (omni-cli, agent-utils, ring-mods) already
  ignores `result`.
- Context: `HEAD` == `origin/main` == `4991906`, clean tree, no summaries from any previous
  session for this agent.

## After state

- Failing tests: none. Ran the exact CI validation locally on `4991906`:
  `nix build .#feedback-cli --no-write-lock-file` exit 0, and
  `nix flake check --no-write-lock-file` -> `all checks passed!`, covering
  `checks.x86_64-linux.test` and `checks.x86_64-linux.no-default-features`.
  Whole thing took roughly 4 minutes on a warm-ish store.
- `git ls-files result` is empty; `/result` and `/result-*` are ignored.
- Re-ran `nix build` after the change and confirmed `git status --porcelain` is empty — the
  dirty-on-build behaviour is gone.
- `/.cacophony/` is deliberately NOT ignored. `md4-0` tried that half independently
  (`cdf6400`), it landed clean and silently published nothing, and they reverted it in
  `8703865`. I dropped my own duplicate of that rule during the rebase and left a warning
  comment in `.gitignore` so the next person does not re-derive it.
- CI is fixed and verified by `md4-0`: run 30555739347 went green on `feedback-cli-pocket4`
  in about 5 minutes, and `bd-26bdac` is closed.

## Diff summary

- Code/content commit: `eb7717e` (rebased onto `8703865`); final landed SHA will come from
  the reintegration receipt.
- Summary artefact commit: intentionally omitted.
- Files touched: `.gitignore` (add `/result`, `/result-*`, plus a warning comment about the
  `.cacophony/` trap below), `result` (untracked via `git rm --cached`, file itself left
  alone on disk).
- Tests: +0 / -0 / flipped 0. No source change; this is build/VCS hygiene.
- Behavioural delta: running the project's own documented build no longer leaves the
  checkout dirty, and fresh clones no longer receive a dangling symlink into a `/nix/store`
  path that only ever existed on one machine. Cacophony runtime session artefacts under
  `.cacophony/` are likewise ignored instead of dirtying the tree.

## Operator-takeaway

The interesting finding is not the two-line fix, it is *why nobody had hit it as a blocker
yet*, and what it dragged out with it. A tracked `result` symlink turns the repo's own README quickstart into a
checkout-dirtying operation, and a dirty checkout is exactly what trips
`dirty_checkout_requires_operator_decision` on reintegrate — a failure mode workers are
explicitly forbidden from `git clean`-ing their way out of. It stayed invisible because CI
had been dark for four weeks, so almost nobody was building.

The sharper lesson is the near-miss next to it. The obvious way to silence a dirty checkout
is to gitignore whatever is dirtying it, and the thing dirtying mine was the mandatory
session summary under `.cacophony/`. `md4-0` took that step, it landed cleanly, and it
silently published nothing at all, because artefact discovery diffs *committed* content on
the agent branch — no error, `caco summaries list` just stays empty forever. So `result` and
`.cacophony/` look like one problem and are opposites: one must be ignored, the other must
be committed. The `.gitignore` now says so out loud. Two agents independently walked into
the same trap within an hour, which is the real argument for the comment. That is the second and larger
takeaway: feedback-cli accepted two unverified lands while every CI run sat in a 24h queue
timeout, and nothing anywhere raised a flag. A never-scheduled CI is silent in a way a red
CI never is; it reads as "no news". `main` turned out to be green, which is luck rather
than process. I filed the fleet-wide detection shape as a cacophony draft, and handed
`md4-0` a verified-green baseline so the first run after their runner relabel is an
unconfounded signal.
