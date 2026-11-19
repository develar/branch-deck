# Proposal

## Why

On 2026-11-19 the fetch of the baseline of the IntelliJ monorepo took a forced update of `origin/master` onto
`41b940fd`, a 76-commit JitPack history the server served for about 17 minutes under that name. Each sync after it
read `41b940fd..HEAD`, the whole history of the monorepo: 1 623 531 commits in the grouper, 214 689 branches and every
commit as unassigned on the page, so the app grew by gigabytes. The update check beside the sync already knew ("master
and origin/master have no common history"); the sync never asked. Nothing bounded a read of the commits ahead by the
fact that they must build on the baseline.

## What Changes

- Every read of the commits ahead of the baseline (the sync, the grouping, the rewrites, the checks, a pick) refuses
  at the first root commit it streams, off the local baseline: `UnrelatedHistory`, with the root. No new process: the
  one `log` of the walk is stopped there, and the app holds none of the commits.
- A sync it refuses is deferred, not failed (`SyncDeferred::UnrelatedHistory`, code `unrelated_history`, exit 4 in
  `brd`), before any event and before the update check, whose `merge-base` would walk the whole history again: the
  table of the sync before stays.
- The page says it in a `warning` alert above the table too, as no work of the user ends it, naming the root the
  baseline lacks; the user's Sync says it in a `warning` toast. `brd` names the root in `error.commits` and asks the
  user (`ask_user`).
- A fetch of the baseline that takes a forced update logs it.

Capabilities: `repository/onboarding`, `repository/update-project`, `cli/agent-cli`.
