---
name: Pull Requests
description: Requirements for the GitHub pull request status of the virtual branches, read through the gh CLI (also for branches pushed to Space and mirrored to GitHub, as its committed settings say) and shown as a chip in the branch row, the branch preview and the outgoing overview; a pull request Patronus merged reads merged; and "Create Pull Request" in the branch menu, the branch preview and the push toast, which opens the GitHub form of a new pull request into the base of the branch.
style: plain-1
targets:
  - ../../crates/sync-types/src/pull_request.rs
  - ../../crates/sync-core/src/pull_requests.rs
  - ../../src-tauri/src/commands/pull_requests.rs
  - ../../src/stores/pull-requests.ts
  - ../../src/lib/pull-request.ts
  - ../../src/lib/review-commit.ts
  - ../../src/components/branch/PullRequestChip.tsx
  - ../../src/components/branch/ReviewCommitNotice.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/repository/OutgoingIndicator.tsx
  - ../../src/components/repository/OutgoingPopover.tsx
  - ../../src/components/repository/PullRequestStatusLine.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/usePullRequestOverview.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/lib/refresh-gate.ts
  - ../../src/components/branch/StatusChip.tsx
  - ../../src/lib/git/push.ts
  - ../../src/lib/git/new-pull-request.ts
  - ../../src/lib/git/push-toast.tsx
  - ../../crates/bd-cli/src/push_command.rs
---

# Pull Requests

Status: Implemented
Date: 2026-10-14

## Summary

Reviews happen on GitHub pull requests of the pushed virtual branches. Branch Deck shows, per branch, the pull request
the user opened for it: its number, the review decision, the checks and whether it shows the branch as it is. The
status is read through the `gh` CLI; opening a pull request opens the browser. A pushed branch without one gets "Create
Pull Request", which opens GitHub's form of a new pull request into the branch's base. The branches may be pushed to
Space, which mirrors them to GitHub: the committed Space settings name the GitHub repository then. Patronus merges a pull request from its `/patronus merge` comment ([Patronus](patronus.spec.md#pull-requests)),
the one write of the app on GitHub.

## Goals

- See which branches are approved, which wait for a review and which wait for the author (changes requested, failing
  checks) without leaving the app.
- Notice that the branch changed since the commit its pull request shows, so a push is due, without a false alarm after
  a sync that only rebuilt the branch on a newer baseline.
- Open the pull request of a branch in one click.
- Open a review after a plain push in one click: GitHub's form of a new pull request, its base already set.

## Non-goals

- Creating pull requests itself, updating or merging them. Approvals are final and Patronus merges; the app reads,
  leaves the creation to GitHub's form, and its one comment is the Patronus command ([Patronus](patronus.spec.md#pull-requests)).
- Pull requests of others, of forks, and of branches that are not virtual branches of the user.
- GitHub Enterprise and other hosts: only github.com.
- Polling: GitHub is asked after a sync, a push, a focus and a manual refresh only.

## Requirements

### Reading pull requests

- The repository is `git config branchdeck.githubRepository` (`owner/name`), if set; else the one `origin` pushes to
  (`git remote get-url --push origin`), as `push_branch` pushes to `origin`; else the GitHub mirror that the Space
  settings committed in the repository name (`mirror.url` of `.space/repository-settings.json`, read from `HEAD` with
  one `git cat-file`, never from the working tree). SSH (`git@github.com:o/r.git`, `ssh://git@github.com/o/r.git`, `ssh.github.com:443`),
  HTTPS (with or without `.git` and credentials) and `git://` URLs name a github.com repository. No repository host or
  name is built into the app, and nothing searches GitHub for a mirror.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`parses_github_remote_urls`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`the_configured_github_repository_wins_over_origin`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`reads_the_mirror_from_the_committed_space_settings`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`a_space_mirror_must_be_on_github`)
- Otherwise a remote on another host, a local path or no `origin` must report "not on GitHub" without running `gh`.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`rejects_remote_urls_outside_github`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`a_remote_outside_github_does_not_run_gh`)
- One `gh pr list --repo github.com/<owner>/<name> --author @me --state all --limit 50 --json …` per refresh, run in the
  repository directory. `--repo` is required: with several remotes and no `gh repo set-default`, `gh` would not know
  which repository to ask.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
- An answer that lists the pull requests names the repository it asked, its owner and name (`JetBrains/ultimate`), for
  links to GitHub that no pull request carries; the app knows the repository only while `gh` lists its pull requests
  (`getGitHubRepository()`).
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
  [@test] ../../src/stores/pull-requests.test.ts (`names the GitHub repository while gh lists its pull requests`)
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
- A closed pull request with the label `merged` is merged: Patronus rebases, so GitHub can't mark the pull request it
  merged as merged; it closes it and adds the label. The labels are read in the same `gh pr list` (`--json …,labels`).
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`a_pull_request_patronus_closed_with_the_merged_label_is_merged`)
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
- A robot linked to a pull request that merged ([Patronus](patronus.spec.md#pull-requests)) asks again right away: the
  pull request is closed with the label `merged` now.
  [@test] ../../src/stores/patronus.test.ts (`keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged`)
- A failed refresh must keep the pull requests shown and only change the availability.
  [@test] ../../src/stores/pull-requests.test.ts (`keeps the shown pull requests when a refresh fails`)

### Chip

- The Status cell of a branch row shows the chip in its second column, after the status text
  ([Branch Row](branch-row.spec.md)): the pull request icon and `#760`. A branch without a pull request shows nothing.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show each pull request state in the status cell`)
- The hue of an open pull request follows the review decision: approved `success`, changes requested `destructive`,
  review required and no decision muted. A draft is muted with the draft icon; a merged one `info`
  with the merge icon; a closed one muted with the closed icon. A draft, merged or closed pull request shows its state,
  not the decision.
  [@test] ../../src/lib/pull-request.test.ts (`colours an open pull request by its review decision`)
  [@test] ../../src/lib/pull-request.test.ts (`shows the state of a draft, a merged and a closed pull request instead of the decision`)
- An open pull request with failing checks gets a small `destructive` dot. An open one gets a `warning` dot when the
  branch changed since its head: the head is not the commit the remote branch is at now (`remoteTip`), or the branch is
  not up to date with the remote by content, as the [remote status](../../docs/design/remote-status.md) counts it. A head
  at the local tip gets none, nor one at the remote tip of a branch that is up to date: every sync rebuilds the virtual
  branches on the newer baseline, so their hashes change while their commits don't. A merged or closed pull request
  gets no dot, nor any pull request until the remote status of the branch is loaded.
  [@test] ../../src/lib/pull-request.test.ts (`marks failing checks and a head the branch changed since, for open pull requests only`)
  [@test] ../../src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`)
  [@test] ../../src/lib/review-commit.test.ts (`takes the branch as it is for its own tip, and while nothing says otherwise`)
- Its accessible name is "Pull request #760: Approved".
- Hovering the chip opens the details: the number and the state, the title, the checks ("1 failing (tests), 2
  running, 9 passed", "All 4 passed"), the reviewers, the last update, the notice about the head (below), and "Open on
  GitHub".
  [@test] ../../src/lib/pull-request.test.ts (`lists the failing checks first`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
- With the dot, the details say "Shows 3264f30f; the branch has changed since: push to update the pull request.", or,
  when the remote branch has the change already and GitHub doesn't show it yet, "Shows 3264f30f; GitHub doesn't show the
  pushed 88de421d yet." A branch only rebuilt on a newer baseline gets no dot; the details say quietly "Shows an earlier
  build of the same commits."
  [@test] ../../src/lib/pull-request.test.ts (`asks for a push when the branch changed, waits for GitHub after one, and says quietly that it shows an earlier build`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say when the branch changed since the pull request's head`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say only quietly that the pull request shows an earlier build of the same commits`)
- A click on the chip opens the pull request in the browser and does not toggle the row.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
- The branch context menu has "Open Pull Request" in its history group when the branch has a pull request, also when the
  branch can't be modified; it shows `O`, the key of the branch preview.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)

### Creating a pull request

- "Create Pull Request" opens GitHub's form of a new pull request in the browser (the opener plugin, as "Open Pull
  Request"): `https://github.com/<owner>/<name>/compare/<base>...<prefix>/virtual/<branch>?expand=1`. The repository is
  the one `gh` lists the pull requests of (`getGitHubRepository()`), else the one Patronus knows the branches are
  mirrored to (the Space settings, no `gh` needed for a link); the base is the pull request base of the branch
  ([Stacked Branches](stacked-branches.spec.md#pull-requests): its parent's pushed branch, else the baseline as the
  remote names it, `master`, also while the parent isn't on the remote). A branch name keeps its slashes, anything else that a URL path can't hold is encoded.
  GitHub creates nothing until the form is submitted.
  [@test] ../../src/lib/pull-request.test.ts (`opens the pull request form of the pushed branch into its base`)
- The history group of the branch menu (the right-click and "⋯") has "Create Pull Request" when the branch is on the
  remote, has no open or merged pull request and the GitHub repository is known; a branch that isn't pushed or has one
  doesn't. A closed pull request doesn't count: the branch gets a new one, and "Open Pull Request" still opens the
  closed one.
  [@test] ../../src/lib/git/push.test.ts (`offers a new pull request when the branch has only a closed one`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a pushed branch in the push toast, the branch menu and the preview`, `should open the pull request of a pushed branch from the push toast`)
- The toast of a push ("Pushed feature-auth", [Outgoing Commits](../repository/outgoing.spec.md#push)) of any branch,
  stacked or not, with stacked branches on or off, has the action "Create Pull Request" under the same conditions as
  the menu, else "Open Pull Request" when the branch has a pull request, and stays 10 s; off GitHub it has no action.
  The toast reads the branch after the push: the push that created the remote branch offers its pull request. A
  stacked branch's toast also names the base, and a pull request with another base gets "Copy gh pr edit" instead
  ([Stacked Branches](stacked-branches.spec.md#pull-requests)). A push that is a step of another action has no toast
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)).
  [@test] ../../src/lib/git/push.test.ts (`offers to create the pull request of a branch without one, into the baseline`, `opens the pull request the branch has`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a pushed branch in the push toast, the branch menu and the preview`, `should open the pull request of a pushed branch from the push toast`)
- A copied or printed `gh` command (`gh pr edit` of the toast and of the menu's "Copy Retarget Command", the
  `gh pr create` of `bd push`) gets `--repo <owner>/<name>` when the GitHub repository is not the one `origin` pushes
  to (a mirror): `gh` finds no GitHub repository among the remotes of a mirrored checkout.

### Branch preview

- The meta line of the [branch preview](branch-preview.spec.md) ends with the chip; the body starts with a pull request
  section (the details of the chip popover) above the commits; `O` and the footer button "Open Pull Request" open it.
  A branch without a pull request has neither. Where the menu offers "Create Pull Request", the footer has it instead
  of "Open Pull Request"; `O` keeps opening a closed pull request.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request in the branch preview and open it with O`, `should offer to create the pull request of a pushed branch in the push toast, the branch menu and the preview`)

### Outgoing overview

- The [outgoing overview](../repository/outgoing.spec.md) shows the chip of each row between the state word and the
  push button. The branches with an open pull request and nothing to push follow the outgoing rows, with "pushed" and
  the chip.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the chips in a column and a dot for a pull request that waits for the author`)
- The indicator gets a `destructive` dot when an open pull request has changes requested or failing checks, and the
  overview a line that says why: "ABC-123 needs attention: changes requested, failing checks"
  ([Outgoing Commits](../repository/outgoing.spec.md)).
  [@test] ../../src/lib/pull-request.test.ts (`names the requested changes and the failing checks of an open pull request`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the chips in a column and a dot for a pull request that waits for the author`)
- With nothing to push and an open pull request, the indicator stays: the pull request icon and the count of open pull
  requests, named "Nothing to push, 1 pull request open" (", 1 needs attention" after it with the dot); the overview has
  the same title.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should keep the indicator for an open pull request when nothing is outgoing`)
- The footer of the overview has a line "Pull requests checked just now" (it ages while shown, as every relative time:
  [Update Project](../repository/update-project.spec.md)) with a Refresh button, named "Refresh pull requests" (the
  Patronus line has a Refresh button too). When `gh` can't answer, the line is a quiet hint instead: "Install the GitHub
  CLI (gh) to see pull requests", "Run gh auth login to see pull requests", or "GitHub: <the error>". It is never a
  toast. A repository that is not on GitHub has no line.
  [@test] ../../src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should hint quietly when gh is not logged in`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say nothing for a repository that is not on GitHub`)

## User Experience

- The chip is text-sized (`text-xs`, `size-3.5` icon, tabular figures), a plain button that underlines on hover, with
  the `rounded-2xl` focus ring of buttons and badges. No chip has an outline: a draft shows by its icon and its muted
  hue. The dots are `size-1.5 rounded-full shrink-0`, as the status dot of the row.
- The notice of a changed branch is `text-xs text-warning` after the warning triangle; the line of a rebuilt one is
  `text-xs text-muted-foreground`, without an icon (`ReviewCommitNotice`, shared with the Patronus details).
- `info` is the merged colour; push states never use it ([Outgoing Commits](../repository/outgoing.spec.md)).
- The details popover is `w-80`, aligned to the start of the chip; in the preview the same details sit in a nested
  `rounded-lg` panel.

## Testing / Local Run

- `cargo nextest run --workspace` covers the parsing, matching, URL parsing and the `gh` process
  handling with a fake `gh` script (`crates/sync-core/src/pull_requests_test.rs`).
- `pnpm test:unit` covers the refresh policy and the chip mapping.
- `pnpm e2e` covers the chip, the popover, the preview section and the overview (`tests/e2e/pull-requests`). E2E never
  runs `gh`: `POST /repositories/{id}/pull-requests` sets what `get_pull_requests` answers (an empty `headRefOid` means
  the local tip; `availablePullRequests` answers for `JetBrains/ultimate`), and `GET` returns how often the app asked. The opener plugin is mocked and records the opened URLs in
  `window.__OPENED_URLS__`. The E2E repositories have no remote: a push answered in the page (`routePush`,
  `pushBranchFromRow`) gives a branch its remote tip.

## Open Questions / Risks

- `--author @me` uses the GitHub search, which can lag a few seconds behind a newly opened pull request.
- Only the 50 most recent pull requests of the user are read; an old pull request of a long-lived branch may be missed.
- A branch pushed to a fork as `origin`, with the pull request on the upstream repository, is not matched.
- The Space settings file is a snapshot committed by the repository's admins: a mirror configured only in Space, not in
  the file, needs `branchdeck.githubRepository`.

## References

- [Branch Row](branch-row.spec.md): the Status cell and the anatomy of the chip details
- [Outgoing Commits](../repository/outgoing.spec.md)
- [Branch Preview](branch-preview.spec.md)
- [Patronus](patronus.spec.md): the other review status, shown next to the chip
- [Remote Status for Virtual Branches](../../docs/design/remote-status.md)
