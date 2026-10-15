---
name: Review Requests
description: Requirements for "Your Turn", the island of the GitHub pull requests of others that wait for the user's review (requested by name), above the branches, waiting the longest first, with the user's own pull requests a reviewer handed back; GitHub polled every five minutes through gh, a toast or an OS notification for a new request, the Dock badge; the in-app review of a pull request (a preview while it is fetched, then the review of base..head with the file tree and all diffs); and the Settings dialog of the notifications and the badge.
style: plain-1
targets:
  - ../../crates/sync-types/src/review_request.rs
  - ../../crates/sync-core/src/review_requests.rs
  - ../../crates/sync-core/src/review_fetch.rs
  - ../../crates/sync-core/src/pull_requests.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/git-ops/src/commit_files.rs
  - ../../crates/git-executor/src/git_command_executor.rs
  - ../../crates/test-server/src/tauri_command_bridge.rs
  - ../../src-tauri/src/commands/review_requests.rs
  - ../../src-tauri/src/menu.rs
  - ../../src-tauri/capabilities/desktop.json
  - ../../src/lib/review-requests.ts
  - ../../src/lib/os-notification.ts
  - ../../src/lib/dock.ts
  - ../../src/lib/commit-files.ts
  - ../../src/lib/outgoing-attention.ts
  - ../../src/stores/review-requests.ts
  - ../../src/stores/review-fetch.ts
  - ../../src/stores/settings-dialog.ts
  - ../../src/stores/settings/appSettings.ts
  - ../../src/stores/settings/repositorySettings.ts
  - ../../src/stores/activity.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/hooks/useDockStatus.ts
  - ../../src/components/repository/YourTurnCard.tsx
  - ../../src/components/repository/ReviewRequestRow.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/components/review/PullRequestReviewDialog.tsx
  - ../../src/components/commit/CommitReview.tsx
  - ../../src/components/shared/CollapsibleCard.tsx
  - ../../src/components/settings/SettingsDialog.tsx
  - ../../src/components/branch/BranchCreator.tsx
  - ../../src/App.tsx
---

# Review Requests

Status: Implemented
Date: 2026-10-15

## Summary

Branch Deck showed GitHub status only for the user's own pull requests ([Pull Requests](../branch/pull-requests.spec.md)).
Whose turn it is to review was told by github.com/pulls alone. "Your Turn" is an island above the branches with the open
pull requests that ask for the user's review by name, waiting the longest first, how long each waits, and the user's own
pull requests a reviewer handed back. GitHub is polled every five minutes; a new request is a toast, or an OS
notification while Branch Deck is in the background. A row opens the review of the pull request in the app: a preview
of what GitHub said while the pull request is fetched, then the review of `base..head`, the file tree beside all diffs,
as the review of a commit ([Commit Review](../commit/commit-review.spec.md)). Approving stays on GitHub: "Review on
GitHub" opens its files there.

## Goals

- See without leaving the app which pull requests wait for the user, the oldest first, and which have waited long.
- Learn of a new request when it comes, also in the background, once.
- Read a pull request (its description, its commits, its diffs) in the app, without a checkout and without touching
  the working tree, also from a Space checkout mirrored to GitHub.

## Non-goals

- Requests of a team the user is in (CODEOWNERS fan-out): only direct requests (`user-review-requested:@me`).
- Drafts and archived repositories.
- Approving, commenting or requesting changes: GitHub does that.
- Repositories other than the selected one: only it is polled.
- "Hide Until Updated" on a row (not chosen for the first version).
- A click action on an OS notification.

## Requirements

### Reading the requests

- One `gh api graphql` per refresh (1 point of the rate limit), the search `repo:<owner>/<name> is:pr is:open
  archived:false draft:false user-review-requested:@me`, `first: 50` with the `issueCount`; the repository is the one of
  [Pull Requests](../branch/pull-requests.spec.md#reading-pull-requests) (configured, `origin`, or the Space mirror).
  When the pull requests already named it, the frontend passes `owner/name`, and it is not resolved again. A repository
  outside GitHub never runs `gh`.
  [@test] ../../crates/sync-core/src/review_requests_test.rs (`asks_for_the_direct_requests_of_the_repository`, `a_known_github_repository_skips_resolving_it`, `a_repository_outside_github_never_runs_gh`)
- A request waits since the newest review request event whose reviewer is the user, else since the pull request was
  opened; who asked is that event's actor, not the author. It is re-requested when the user's latest submitted review
  is older than that; a pending review is no review. A request whose latest review is newer than the event is dropped
  (the search index lags behind a review).
  [@test] ../../crates/sync-core/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)
- A partial answer (`data` with `errors`) is read; GitHub's rate limit is `rateLimited`; every other failure is
  classified as for the pull requests and never thrown.
  [@test] ../../crates/sync-core/src/review_requests_test.rs (`a_partial_answer_is_read`, `a_rate_limit_is_rate_limited`, `classifies_the_other_failures_as_gh_does`)
- The requests are kept in memory per repository, sorted by how long they wait, then by number. An answer that changes
  nothing keeps the objects and the list, so a poll re-renders nothing.
  [@test] ../../src/lib/review-requests.test.ts (`sorts waiting the longest first`, `keeps the objects and the list of an answer that changed nothing`)
  [@test] ../../src/stores/review-requests.test.ts (`tells a re-request, keeps the list of an answer that changed nothing`)

### Polling

- The selected repository is asked after a sync (at most once a minute), on a focus when its requests are older than
  two minutes, by the Refresh button always, and by a poll every five minutes, focused or not; the poll timer restarts
  after any answer, and a poll closer than 4.5 min to the last request asks nothing. A poll is skipped offline
  (`navigator.onLine`). One request per repository is in flight; a Refresh during it asks once more after it.
  [@test] ../../src/stores/review-requests.test.ts (`polls every five minutes, focused or not, and backs off while gh fails`, `skips a poll while offline`)
- A failure doubles the interval, up to 30 min; the rate limit waits 15 min at least (nothing but Refresh asks before);
  without `gh` or logged out nothing is polled until a focus or Refresh; a repository not on GitHub is asked again only
  by Refresh.
  [@test] ../../src/lib/review-requests.test.ts (`polls every five minutes, backs off on errors, waits out the rate limit and stops without gh`)
  [@test] ../../src/stores/review-requests.test.ts (`stops while gh is logged out, until a focus asks again`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden off GitHub, and not ask again on focus`, `should stay hidden while gh is logged out or nothing waits, and show once something does`)
- Another repository selected cancels the poll of the last one.
- Opening a pull request on GitHub from the card or the dialog makes the next focus ask at once: an approved one
  leaves the list.

### New requests

- A request is new when its number and the time it waits since are not in the last answer: a re-request is new. The
  first answer after a start, and the first one after a repository switch, are silent.
  [@test] ../../src/lib/review-requests.test.ts (`is silent for the first answer after a start or a repository switch`, `tells the requests not known yet, a re-request too`)
  [@test] ../../src/stores/review-requests.test.ts (`is silent for the first answer after a repository switch`)
- While a Branch Deck window has the focus (a conflict window counts), one toast tells the new requests of an answer:
  "dave requested your review" / "#104 Document the CLI", or "3 reviews requested" / "#1 … · #2 … · #3 …", with the
  action "Review", which opens the first one, for 10 s.
  [@test] ../../src/lib/review-requests.test.ts (`tells one request by who asked, several in one notice`)
  [@test] ../../src/stores/review-requests.test.ts (`tells nothing for the first answer, then one toast with Review for what is new, recorded in Activity`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)
- In the background an OS notification says it at once, with the words of the toast, when "Review requests" is on in
  Settings; the toast shows on the next focus. The permission is asked on the first notification, never at start. A
  notification has no click action.
  [@test] ../../src/stores/review-requests.test.ts (`tells in the background by an OS notification, and in a toast on focus`)
  [@test] ../../src/lib/os-notification.test.ts (`shows the toast at once while focused, else notifies and shows it on focus`, `sends no OS notification of a kind Settings turned off; the toast waits for the focus all the same`)
- Each new request is an Activity entry "dave requested your review on #104" / its title, with "Open on GitHub"
  ([Activity](activity.spec.md)).
- A new request is unread (a `bg-primary` dot) until it is opened, in the app or on GitHub.

### Card

- "Your Turn" shows in the main window after the first completed sync of the repository (never with the Welcome or the
  Ready to Sync card), after the error alert, before Unassigned Commits. It is hidden off GitHub, without `gh`, logged
  out, and while nothing waits.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden off GitHub, and not ask again on focus`, `should stay hidden while gh is logged out or nothing waits, and show once something does`)
- A `CollapsibleCard`, open by default; collapsed per repository (`reviewRequestsCollapsed` of the recent projects).
  The header: `UserCheckIcon`, "Your Turn", the count (all that wait); collapsed, "5 reviews · oldest waiting 2 days"
  instead of the count, the age in `warning` when old. The help popover says what is shown (direct requests only), how
  the ages colour, and where from; the "⋯" menu has "Open All on GitHub" (the search of the repository) and
  "Notification Settings…".
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`)
- A row, 36 px: the unread dot, the title (`font-medium`), the meta "#101 · carol · 2 files · +12 −3" (coloured line
  counts; "· → release/253" when the base is not the default branch; hidden below `sm`), a `destructive` dot for failing
  and a `warning` dot for running checks, and "waiting 2 days" or "re-requested 3 hours ago". The age is muted, `warning`
  after one working day (Saturdays and Sundays don't count); never `destructive`. The age is a leaf on the shared clock.
  [@test] ../../src/lib/review-requests.test.ts (`is old after a day of working time, the weekend skipped`, `says how long a request waits, or when the user was asked again`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`)
- Three rows, then "Show N More" ("Show Less" back). An unread request is always shown, in the place of a read one
  while the rows are full: a poll never grows the list or inserts a row above the others. Rows fade in by opacity;
  only the first entrance of the card slides.
  [@test] ../../src/lib/review-requests.test.ts (`shows the oldest three and every unread request, an unread one in the place of a read one`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)
- GitHub returns 50: with more, the expanded list says "50 of 63" with "Open All on GitHub".
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should say 50 of 63 when GitHub has more than it returns`)
- A click reviews the pull request; ⌘-click (Ctrl elsewhere) and the button on hover, "Open #101 on GitHub", open it on
  GitHub. The right-click, the context menu key and Shift+F10 open the menu: Review, Open on GitHub, Copy Link, Copy
  Branch Name (copies without a toast). ↑/↓ move the focus between the rows (one tab stop), Enter and Space review.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open a pull request on GitHub from its button, a ⌘-click and its menu`, `should move with the arrows, review with Enter and open the menu from the keyboard`)
- "Returned to you: JIRA-456 (changes requested) · …" names the user's own open pull requests with changes requested,
  in the words of the outgoing attention ([Outgoing Commits](outgoing.spec.md)); failing checks are the chip's. A name
  reveals its row.
  [@test] ../../src/lib/outgoing-attention.test.ts (`names only the open pull requests a reviewer handed back, in the table order`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should name the own pull requests a reviewer handed back, and reveal the branch`)
- The footer is "Checked just now" with Refresh (`ReviewStatusLine`). A failed refresh keeps the rows; the footer says
  why and how old the rows are: "GitHub: offline · checked 5 minutes ago". The card never turns into an error.
  [@test] ../../src/stores/review-requests.test.ts (`keeps the rows when a refresh fails, drops them without gh`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should keep the rows when a refresh fails, with the reason in the footer`)

### Badge

- The Dock badge adds the review requests to what needs the user ([Robots Indicator](robots-indicator.spec.md#dock)),
  while "Count review requests" is on; Windows has no badge.
  [@test] ../../src/lib/dock.test.ts (`shows the sum of its sources, set only on a change, none at 0`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should count the review requests, behind its switch`)

### Review dialog

- A `PreviewDialogFrame`, lazy and kept mounted after its first open. It opens at once at `lg` with what the search
  returned: the title, "#101 · carol · master ← alice/feature · 2 files · +12 −3 · checks failing", the description (as
  a commit body), and the commits once fetched (subject, author, age, hash). ↑/↓ show the previous or next request,
  the footer says "2 / 5".
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`, `should open a commit of the pull request in the commit preview, and show the next request with ↓`)
- The fetch (`fetch_review_head`) starts when it opens: "Fetching #101 from origin… 42%" (`tabular-nums`, "from
  GitHub" for a fork); `D` Review is disabled until it is done. One fetch at a time: another request, or the close,
  kills the one in flight (`cancel_review_fetch`); a fetch that reports nothing for 120 s is killed too ("git reported
  nothing for 120 s"). After the answer, the source is the one the range names.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`a_cancel_kills_the_fetch`, `a_stalled_fetch_is_killed`)
  [@test] ../../crates/git-executor/src/git_command_executor_test.rs (`a_git_that_reports_nothing_for_the_idle_timeout_is_killed`, `progress_resets_the_idle_timeout`, `a_cancelled_git_is_killed`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)
- `D` reviews `base..head` at full width: the file tree beside all diffs, J/K files, N/P changes, "File 3 / 12"; Escape
  goes back to the preview, which Escape (or Space) closes. A re-requested review whose last review is on a commit of
  the pull request offers "All Changes · Since Your Review" (`<reviewed>..head`). A commit opens the commit preview and
  its review, ⌥↑/⌥↓ between the commits.
- "Review on GitHub" opens `<url>/files`: approving happens there.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)
- A failed fetch is a `destructive` callout "Cannot fetch pull request #101" with the reason (e.g. "GitHub asked for
  credentials: run gh auth setup-git", "<remote>: <git's last line>"), Retry and Open on GitHub. A fetch killed for a
  newer one ("canceled") is no failure.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`refused_github_credentials_are_told_how_to_fix`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should say why a fetch failed and fetch again on Retry`)
- When the commit the pull request shows could not be fetched (`missing`), a `warning` callout says "origin has commits
  the pull request does not have yet; the review shows them." A source ahead of the pull request (a mirror) still shows
  the pull request's commit; a force-pushed source is `diverged`.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`a_mirror_ahead_of_the_pull_request_shows_the_pull_request`, `a_force_pushed_source_branch_diverged`, `a_head_that_cannot_be_got_shows_the_source_branch`)
- A request withdrawn while the dialog is open keeps the dialog, with the quiet line "No longer waiting for your
  review".
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should keep the dialog when the request is withdrawn meanwhile, and its refs`)
- The close returns the focus to the row and forgets the range, its files and its patch.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should move with the arrows, review with Enter and open the menu from the keyboard`)
  [@test] ../../src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)
  [@test] ../../src/stores/commit-patch.test.ts (`reads the range of a review with its own command, keyed base..head, until its review drops it`)

### Fetch and refs

- Both commits local: nothing is fetched and no ref is written. Otherwise one fetch with the flags of the baseline
  fetch plus `--refmap=` (no `refs/remotes/origin/*` moves), `--no-prune`, `--no-show-forced-updates`,
  `fetch.writeCommitGraph=false`: GitHub's `refs/pull/<n>/head` from a GitHub origin, the branch from a mirror origin,
  GitHub (a remote of that repository, else its URL) for a fork or a head the mirror lacks; the base too when it is not
  local, so a stale `origin/master` adds nothing to the range. The head and base land in
  `refs/branch-deck/review/<n>/{head,base}` (visible in `git log --all`); never the repository lock, the working tree
  or the index ([Git Operations](../../docs/design/git-operations.md)). Ids and names that could be options are refused.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`fetches_the_pull_ref_from_a_github_origin`, `commits_that_are_local_need_no_fetch_and_no_ref`, `fetches_the_branch_from_the_mirror_without_touching_its_remote_tracking_refs`, `a_mirror_behind_the_pull_request_falls_back_to_github`, `a_fork_is_fetched_from_github`, `a_missing_base_is_fetched_in_the_same_fetch`, `refuses_ids_and_names_that_could_be_options`)
- The range is `merge-base(head, base)..head`; no merge base is "The pull request has no common history with master".
  Its files and its patch are those of all its commits, by one `diff-tree` of the two commits; anything but an object id
  is refused.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`a_pull_request_without_common_history_is_an_error`)
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`range_files_are_the_changes_of_all_its_commits`, `the_files_of_a_commit_subject_are_those_of_the_commit`, `range_patch_has_a_section_per_file_of_the_range`, `a_range_refuses_what_is_not_an_object_id`)
- The review refs are pruned (`prune_review_refs`, one CAS transaction) after an answer with the requests, once per
  repository per start and after a fetch wrote one, and when the dialog closes; never after a failed answer; the open
  dialog's refs stay. A CAS failure is tried again the next time; across app instances it is best effort.
  [@test] ../../src/stores/review-requests.test.ts (`prunes once after an answer with the requests, never after a failure, and keeps the open dialog's`)
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`prune_deletes_the_review_refs_not_kept`, `prune_deletes_nothing_when_a_ref_moved_meanwhile`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should keep the dialog when the request is withdrawn meanwhile, and its refs`)

### Settings

- "Settings…" (⌘, in the app menu on macOS; File → "Settings…", Ctrl+, elsewhere; the focused window opens it) and
  "Notification Settings…" of the card open the Settings dialog (`md`), a home for later settings too.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open from the card menu and keep the switches`, `should open from the app menu`)
- Notifications: "Review requests" (`notifyReviewRequests`), "Safe Push merges" (`notifyPatronusMerges`, the merge
  notification of [Patronus](../branch/patronus.spec.md)). "Dock icon" (not on Windows): "Count review requests"
  (`reviewBadge`), "Count failed robots and conflicts" (`attentionBadge`). All on by default, in the app settings,
  synced across windows.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open from the card menu and keep the switches`, `should count the review requests, behind its switch`)
- The permission of the OS, checked when the dialog opens and on focus: not asked, "Allow Notifications"; refused, a
  `warning` callout "Notifications are off for Branch Deck in System Settings" with "Open System Settings"; granted,
  "Send Test Notification".

## User Experience

- The card is a `Card` island (`CollapsibleCard`), its rows `divide-y`, the hover `bg-muted`; the focus ring of a row is
  inset. The unread dot is `size-1.5 bg-primary`, the checks dots are `StatusChipDot`. "Returned to you" and the status
  line share the footer, `text-xs`.
- `UserCheckIcon` means "a review for the user" (the card, the Activity entry, the Review menu item).
- The dialog follows the Quick Look previews: header, scrolling body, footer of key hints with `xs` buttons; "Review on
  GitHub" is the primary, last button.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/review-requests.test.ts` (ages, order, news, backoff), `src/stores/review-requests.test.ts`
  (polling, notifications, pruning), `src/lib/os-notification.test.ts`, `src/lib/dock.test.ts`, `src/stores/commit-files.test.ts`,
  `src/stores/commit-patch.test.ts`.
- `cargo nextest run`: `crates/sync-core/src/review_requests_test.rs` (a fake `gh`), `review_fetch_test.rs` (local
  bare remotes), `crates/git-ops/src/commit_files_test.rs`, `crates/git-executor/src/git_command_executor_test.rs`,
  `crates/test-server/src/review_requests_test.rs`.
  [@test] ../../crates/test-server/src/review_requests_test.rs (`review_routes_answer_the_fixture_without_a_network`)
- `pnpm e2e`: `tests/e2e/review-requests/review-requests.spec.ts` with the `issue_links` template.
  `POST /repositories/{id}/review-requests` sets what `get_review_requests` answers (`availableReviewRequests` for
  `acme/monorepo`) and, with `fetchError`, makes `fetch_review_head` fail; `GET` returns `{ calls, fetchCalls, prunes }`.
  `fetch_review_head` runs the real function on the template: an empty head is the branch the request names, an empty
  base the baseline, the source `local`; a test holds it in the page for the progress (`routeReviewFetch`). The Dock and
  ⌘-click tests take the user agent of macOS.
- In the app: a repository with direct review requests; the background notification (another app in front); a review
  fetch from a Space checkout; afterwards `git for-each-ref refs/branch-deck/review/` is empty and `git branch -r`
  unchanged.

## Open Questions / Risks

- The search counts a request that was answered by a review moments ago until GitHub's index catches up; the review
  time drops it.
- A request of a team the user is in is not shown, by design; a CODEOWNERS-heavy repository may show little.
- A commit of a pull request shows its subject only in the commit preview: the range lists no bodies.
- Whether macOS shows a notification of the app in front ("Send Test Notification" while Branch Deck is frontmost) is
  to be checked in the app.

## References

- [Pull Requests](../branch/pull-requests.spec.md): the user's own pull requests, the repository and `gh`.
- [Commit Review](../commit/commit-review.spec.md), [Commit Files](../commit/commit-files.spec.md): the review and the
  range subject.
- [Robots Indicator](robots-indicator.spec.md#dock): the badge.
- [Activity](activity.spec.md), [Patronus](../branch/patronus.spec.md): the notification of a merge.
- [Git Operations](../../docs/design/git-operations.md): the review fetch.
