---
name: Pull Requests
description: Requirements for the read-only GitHub pull request status of the virtual branches, read through the gh CLI and shown as a chip in the branch row, the branch preview and the outgoing overview.
style: plain-1
targets:
  - ../../crates/sync-types/src/pull_request.rs
  - ../../crates/sync-core/src/pull_requests.rs
  - ../../src-tauri/src/commands/pull_requests.rs
  - ../../src/stores/pull-requests.ts
  - ../../src/lib/pull-request.ts
  - ../../src/components/branch/PullRequestChip.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/repository/OutgoingIndicator.tsx
  - ../../src/components/repository/OutgoingPopover.tsx
  - ../../src/components/repository/PullRequestStatusLine.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/usePullRequestOverview.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/lib/refresh-gate.ts
  - ../../src/components/branch/StatusChip.tsx
  - ../../src/lib/git/push.ts
---

# Pull Requests

Status: Implemented
Date: 2026-10-14

## Summary

Reviews happen on GitHub pull requests of the pushed virtual branches. Branch Deck shows, per branch, the pull request
the user opened for it: its number, the review decision, the checks and whether it shows the local branch yet. The
status is read through the `gh` CLI, never written; opening a pull request opens the browser.

## Goals

- See which branches are approved, which wait for a review and which wait for the author (changes requested, failing
  checks) without leaving the app.
- Notice that a pull request still shows an older commit than the local branch, so a push is due.
- Open the pull request of a branch in one click.

## Non-goals

- Creating, updating, merging or commenting on pull requests. Approvals are final and a bot merges; the app only reads.
- Pull requests of others, of forks, and of branches that are not virtual branches of the user.
- GitHub Enterprise and other hosts: only github.com.
- Polling: GitHub is asked after a sync, a push, a focus and a manual refresh only.

## Requirements

### Reading pull requests

- The repository must be the one `origin` pushes to (`git remote get-url --push origin`), as `push_branch` pushes to
  `origin`. SSH (`git@github.com:o/r.git`, `ssh://git@github.com/o/r.git`, `ssh.github.com:443`), HTTPS (with or without
  `.git` and credentials) and `git://` URLs name a github.com repository.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`parses_github_remote_urls`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
- A remote on another host, a local path or no `origin` must report "not on GitHub" without running `gh`.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`rejects_remote_urls_outside_github`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`a_remote_outside_github_does_not_run_gh`)
- One `gh pr list --repo github.com/<owner>/<name> --author @me --state all --limit 50 --json …` per refresh, run in the
  repository directory. `--repo` is required: with several remotes and no `gh repo set-default`, `gh` would not know
  which repository to ask.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
- `gh` must be found as git is: `/opt/homebrew/bin/gh`, `/usr/local/bin/gh` on macOS (an app started from the Finder
  has no Homebrew in `PATH`), then `PATH`.
- `gh` must run without a terminal and must never wait for input: stdin is null; `GH_PROMPT_DISABLED`,
  `GH_NO_UPDATE_NOTIFIER`, `GH_NO_EXTENSION_UPDATE_NOTIFIER` and `NO_COLOR` are set, `GH_FORCE_TTY` and
  `CLICOLOR_FORCE` removed. Both pipes are read while it runs, so a large answer can't block it.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`reads_large_output_while_waiting`)
- `gh` must be killed after 30 s, and the timeout reported as an error.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`kills_gh_after_the_timeout`)
- Failures must be classified, never thrown:
  - no `gh` binary: "gh missing";
  - exit code 4 (`gh help exit-codes`), or an `HTTP 401` of an expired token: "not authenticated";
  - any other failure: an error with the first line of stderr, or the exit code.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`reports_a_missing_gh`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`classifies_gh_failures`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`rejects_output_that_is_not_a_pull_request_list`)

### Matching pull requests to branches

- A pull request belongs to the virtual branch whose pushed name, `{prefix}/virtual/{name}`, is its head branch, and
  only when the head repository owner is the owner of the push remote (case-insensitive). A pull request from a fork
  (`isCrossRepository`) is never matched.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`parses_and_matches_pull_requests_by_virtual_branch`)
- Of several pull requests of one branch, an open one wins over a merged one over a closed one, then the most recently
  updated.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`closed_pull_requests_prefer_the_newest_and_empty_decisions_are_none`)
- The checks of the head commit are summarized from `statusCheckRollup`:
  - a check run that is not completed, or a status context `PENDING`/`EXPECTED`, is running;
  - a completed check run concluding `FAILURE`, `TIMED_OUT`, `CANCELLED`, `ACTION_REQUIRED` or `STARTUP_FAILURE`, or a
    status context `FAILURE`/`ERROR`, failed; every other completed check passed (`SKIPPED` and `NEUTRAL` too);
  - the summary is failing with any failure, else pending with any running check, else passing; none without checks.
  The names of the first three failed checks are kept.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`pending_checks_without_failures_are_pending`)
- The reviewers are the latest review of each reviewer (approved, changes requested, commented; pending and dismissed
  reviews say nothing), then the requested reviewers and teams. A reviewer asked again after a review is requested.

### Refreshing

- The pull requests are kept in memory per repository and branch prefix; nothing is persisted.
- At most one request per repository is in flight. A push or a manual refresh during a request asks once more after it;
  a sync or a focus during a request adds nothing. The gate is shared with [Patronus](patronus.spec.md)
  (`src/lib/refresh-gate.ts`), each with its own policy.
  [@test] ../../src/stores/pull-requests.test.ts (`keeps one request in flight; a push during it asks again after it`)
  [@test] ../../src/lib/refresh-gate.test.ts (`runs one task per key; a follow-up reason asks once more after it, with its reason`)
- A repository that answered "not on GitHub" is asked again only by the refresh button: a sync or a focus does not run
  `git remote get-url` for it again.
- After every successful sync, GitHub is asked again if the last request started at least 60 s ago.
  [@test] ../../src/stores/pull-requests.test.ts (`asks once a minute at most after syncs`)
- On window focus, GitHub is asked again if the pull requests are older than 2 minutes. Nothing runs while the window is
  not focused.
  [@test] ../../src/stores/pull-requests.test.ts (`asks on focus only when the pull requests are older than two minutes`)
- A push and the Refresh button ask right away, whatever the gap.
  [@test] ../../src/stores/pull-requests.test.ts (`asks right after a push and on a manual refresh, ignoring the gap`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should refresh after a push`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should refresh from the overview`)
- GitHub can show the old head for a moment after a push: when the pull request of the pushed branch does not show the
  pushed commit yet, it is asked once more 5 s later.
  [@test] ../../src/stores/pull-requests.test.ts (`looks again a little later while GitHub still shows the old head`)
  [@test] ../../src/stores/pull-requests.test.ts (`does not look again when the pull request shows the pushed commit`)
- A failed refresh must keep the pull requests shown and only change the availability.
  [@test] ../../src/stores/pull-requests.test.ts (`keeps the shown pull requests when a refresh fails`)

### Chip

- The Status cell of a branch row shows the chip after the status text: the pull request icon and `#760`. A branch
  without a pull request shows nothing.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show each pull request state in the status cell`)
- The hue of an open pull request follows the review decision: approved `success`, changes requested `destructive`,
  review required and no decision muted. A draft is muted with a dashed outline and the draft icon; a merged one `info`
  with the merge icon; a closed one muted with the closed icon. A draft, merged or closed pull request shows its state,
  not the decision.
  [@test] ../../src/lib/pull-request.test.ts (`colours an open pull request by its review decision`)
  [@test] ../../src/lib/pull-request.test.ts (`shows the state of a draft, a merged and a closed pull request instead of the decision`)
- An open pull request with failing checks gets a small `destructive` dot; an open one whose head is not the local tip
  of the virtual branch gets a `warning` dot ("not updated"). A merged or closed pull request gets no dot.
  [@test] ../../src/lib/pull-request.test.ts (`marks failing checks and a head that is not the local tip, for open pull requests only`)
- Its accessible name is "Pull request #760: Approved".
- Hovering the chip opens the details: the number and the state, the title, the checks ("1 failing (tests), 2
  running, 9 passed", "All 4 passed"), the reviewers, the last update, the "not updated" notice with the commit the pull
  request shows, and "Open on GitHub".
  [@test] ../../src/lib/pull-request.test.ts (`lists the failing checks first`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say when the pull request does not show the local branch`)
- A click on the chip opens the pull request in the browser and does not toggle the row.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
- The branch context menu has "Open Pull Request" in its history group when the branch has a pull request, also when the
  branch can't be modified.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)

### Branch preview

- The meta line of the [branch preview](branch-preview.spec.md) ends with the chip; the body starts with a pull request
  section (the details of the chip popover) above the commits; `O` and the footer button "Open Pull Request" open it.
  A branch without a pull request has neither.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request in the branch preview and open it with O`)

### Outgoing overview

- The [outgoing overview](../repository/outgoing.spec.md) shows the chip of each row between the state word and the
  push button. The branches with an open pull request and nothing to push follow the outgoing rows, with "pushed" and
  the chip.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the chips in a column and a dot for a pull request that waits for the author`)
- The indicator gets a `destructive` dot when an open pull request has changes requested or failing checks.
  [@test] ../../src/lib/pull-request.test.ts (`is set by requested changes or failing checks of an open pull request`)
- With nothing to push and an open pull request, the indicator stays: the pull request icon and the count of open pull
  requests, named "Nothing to push, 1 pull request open"; the overview has the same title.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should keep the indicator for an open pull request when nothing is outgoing`)
- The footer of the overview has a line "Pull requests checked just now" with a Refresh button. When `gh` can't answer,
  the line is a quiet hint instead: "Install the GitHub CLI (gh) to see pull requests", "Run gh auth login to see pull
  requests", or "GitHub: <the error>". It is never a toast. A repository that is not on GitHub has no line.
  [@test] ../../src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should hint quietly when gh is not logged in`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say nothing for a repository that is not on GitHub`)

## User Experience

- The chip is text-sized (`text-xs`, `size-3.5` icon, tabular figures), a plain button that underlines on hover; only
  the draft has an outline (`border-dashed`). The dots are `size-1.5 rounded-full`, as the status dot of the row.
- `info` is the merged colour; push states never use it ([Outgoing Commits](../repository/outgoing.spec.md)).
- The details popover is `w-80`, aligned to the start of the chip; in the preview the same details sit in a nested
  `rounded-lg` panel.

## Testing / Local Run

- `cargo nextest run --workspace --exclude model-ai` covers the parsing, matching, URL parsing and the `gh` process
  handling with a fake `gh` script (`crates/sync-core/src/pull_requests_test.rs`).
- `pnpm test:unit` covers the refresh policy and the chip mapping.
- `pnpm e2e` covers the chip, the popover, the preview section and the overview (`tests/e2e/pull-requests`). E2E never
  runs `gh`: `POST /repositories/{id}/pull-requests` sets what `get_pull_requests` answers (an empty `headRefOid` means
  the local tip), and `GET` returns how often the app asked. The opener plugin is mocked and records the opened URLs in
  `window.__OPENED_URLS__`.

## Open Questions / Risks

- `--author @me` uses the GitHub search, which can lag a few seconds behind a newly opened pull request.
- Only the 50 most recent pull requests of the user are read; an old pull request of a long-lived branch may be missed.
- A branch pushed to a fork as `origin`, with the pull request on the upstream repository, is not matched.

## References

- [Outgoing Commits](../repository/outgoing.spec.md)
- [Branch Preview](branch-preview.spec.md)
- [Patronus](patronus.spec.md): the other review status, shown next to the chip
- [Remote Status for Virtual Branches](../../docs/design/remote-status.md)
