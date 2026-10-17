---
name: Review Requests
description: Requirements for "Your Turn", the island of the GitHub pull requests of others that wait for the user's review (requested by name), above the branches, waiting the longest first, with the user's own pull requests a reviewer handed back; GitHub polled every five minutes through its API, a toast or an OS notification for a new request, the Dock badge; the in-app review of a pull request in its own window (an overview with the files and commits while it is fetched, then the review of base..head with the file tree and all diffs), approving it (the primary decision) or requesting changes on the commit the user saw, with the same actions in the overview, the review and the row's menu; and the Settings dialog of the notifications and the badge.
style: plain-1
targets:
  - ../../crates/sync-types/src/review_request.rs
  - ../../crates/sync-core/src/review_requests.rs
  - ../../crates/sync-core/src/review_fetch.rs
  - ../../crates/sync-core/src/pull_requests.rs
  - ../../crates/sync-core/src/pull_request_writes.rs
  - ../../crates/github/src/service.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/git-ops/src/commit_files.rs
  - ../../crates/git-executor/src/git_command_executor.rs
  - ../../crates/test-server/src/tauri_command_bridge.rs
  - ../../src-tauri/src/commands/review_requests.rs
  - ../../src-tauri/src/commands/pull_requests.rs
  - ../../src-tauri/src/commands/window_management.rs
  - ../../src-tauri/src/lib.rs
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src-tauri/src/menu.rs
  - ../../src-tauri/capabilities/desktop.json
  - ../../src/lib/review-requests.ts
  - ../../src/lib/review-request-actions.ts
  - ../../src/lib/pull-request-review.ts
  - ../../src/lib/pull-request-review-window.ts
  - ../../src/lib/sub-window-link.ts
  - ../../src/hooks/useMainWindowLink.ts
  - ../../src/lib/range-review.ts
  - ../../src/components/review/RangeReview.tsx
  - ../../src/lib/errors.ts
  - ../../src/lib/preview-keys.ts
  - ../../src/lib/os-notification.ts
  - ../../src/lib/dock.ts
  - ../../src/lib/commit-files.ts
  - ../../src/lib/outgoing-attention.ts
  - ../../src/stores/review-requests.ts
  - ../../src/stores/patronus.ts
  - ../../src/stores/review-fetch.ts
  - ../../src/stores/settings-dialog.ts
  - ../../src/stores/settings/appSettings.ts
  - ../../src/stores/settings/repositorySettings.ts
  - ../../src/stores/activity.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/hooks/useDockStatus.ts
  - ../../src/hooks/useReviewWindowBridge.ts
  - ../../src/components/repository/YourTurnCard.tsx
  - ../../src/components/repository/ReviewRequestRow.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/components/review/PullRequestReview.tsx
  - ../../src/components/review/ReviewHeader.tsx
  - ../../src/components/review/ReviewDecisions.tsx
  - ../../src/lib/review-draft.ts
  - ../../src/components/review/ReviewRail.tsx
  - ../../src/pages/PullRequestReviewPage.tsx
  - ../../src/components/shared/MenuItemButton.tsx
  - ../../src/components/shared/PreviewDialogFrame.tsx
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
notification while Branch Deck is in the background. A row opens the review of the pull request in the app, in a window
of its own: an overview of what GitHub said and of the fetched files and commits, then the review of
`base..head`, the file tree beside all diffs,
as the review of a commit ([Commit Review](../commit/commit-review.spec.md)). The user approves it or requests changes
there, in the preview and in the review alike, on the commit the review shows; the row's menu has the same actions.

## Goals

- See without leaving the app which pull requests wait for the user, the oldest first, and which have waited long.
- Learn of a new request when it comes, also in the background, once.
- Read a pull request (its description, its commits, its diffs) in the app, without a checkout and without touching
  the working tree, also from a Space checkout mirrored to GitHub.
- Answer it where it was read: approve, or request changes with a comment, without opening GitHub.

## Non-goals

- Requests of a team the user is in (CODEOWNERS fan-out): only direct requests (`user-review-requested:@me`).
- Drafts and archived repositories.
- Comments on lines, a review with a comment only, and dismissing a review: GitHub does that (Open Pull Request).
- Repositories other than the selected one: only it is polled.
- "Hide Until Updated" on a row (not chosen for the first version).
- A click action on an OS notification.

## Requirements

### Reading the requests

- One GraphQL request per refresh (1 point of the rate limit), with the token of [Pull
  Requests](../branch/pull-requests.spec.md#the-token): the search `repo:<owner>/<name> is:pr is:open archived:false
  draft:false user-review-requested:@me`, `first: 50` with the `issueCount`, and `repository(owner, name)`, whose null
  is "no access" as for the pull requests; the repository is the one of [Pull
  Requests](../branch/pull-requests.spec.md#reading-pull-requests) (configured, `origin`, or the Space mirror). When the
  pull requests already named it, the frontend passes `owner/name`, and it is not resolved again. A repository outside
  GitHub never asks GitHub.
  [@test] ../../crates/sync-core/src/review_requests_test.rs (`asks_for_the_direct_requests_of_the_repository`, `a_known_github_repository_skips_resolving_it`)
- A request waits since the newest review request event whose reviewer is the user, else since the pull request was
  opened; who asked is that event's actor, not the author. It is re-requested when the user's latest submitted review
  is older than that; a pending review is no review. A request whose latest review is newer than the event is dropped
  (the search index lags behind a review).
  [@test] ../../crates/sync-core/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)
- A partial answer (`data` with `errors`) is read; GitHub's rate limit is `rateLimited`; every other failure is
  classified as for the pull requests and never thrown.
  [@test] ../../crates/sync-core/src/review_requests_test.rs (`a_partial_answer_is_read`, `a_rate_limit_is_rate_limited`, `classifies_the_other_failures`)
- The requests are kept in memory per repository, sorted by how long they wait, then by number. An answer that changes
  nothing keeps the objects and the list, so a poll re-renders nothing.
  [@test] ../../src/lib/review-requests.test.ts (`sorts waiting the longest first`, `keeps the objects and the list of an answer that changed nothing`)
  [@test] ../../src/stores/review-requests.test.ts (`tells a re-request, keeps the list of an answer that changed nothing`)

### Polling

- The selected repository is asked after a sync (at most once a minute), on a focus when its requests are older than
  two minutes, by the Refresh button always, and by a poll every five minutes, focused or not; the poll timer restarts
  after any answer, and a poll closer than 4.5 min to the last request asks nothing. A poll is skipped offline
  (`navigator.onLine`). One request per repository is in flight; a Refresh during it asks once more after it.
  [@test] ../../src/stores/review-requests.test.ts (`polls every five minutes, focused or not, and backs off while GitHub fails`, `skips a poll while offline`)
- A failure doubles the interval, up to 30 min; the rate limit waits 15 min at least (nothing but Refresh asks before);
  without a token, with one GitHub rejects, or without access to the repository (no access, SAML SSO) nothing is
  polled until a focus or Refresh; a repository not on GitHub is asked again only by Refresh.
  [@test] ../../src/lib/review-requests.test.ts (`polls every five minutes, backs off on errors, waits out the rate limit and stops without a token`)
  [@test] ../../src/stores/review-requests.test.ts (`stops while GitHub rejects the token, until a focus asks again`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden off GitHub, and not ask again on focus`, `should stay hidden while GitHub rejects the token or nothing waits, and show once something does`)
- Another repository selected cancels the poll of the last one.
- Opening a pull request on GitHub from the card or the review makes the next focus ask at once: an approved one
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
- Each new request is an Activity entry "dave requested your review on #104" / its title, with "Open Pull Request"
  ([Activity](activity.spec.md)).
- A new request is unread (a `bg-primary` dot) until it is opened, in the app or on GitHub.

### Card

- "Your Turn" shows in the main window after the first completed sync of the repository (never with the Welcome or the
  Ready to Sync card), after the error alert, before Unassigned Commits. It holds what waits for the user in a review:
  the pull requests of others that ask for the user's review, then the user's own pull requests a reviewer handed back.
  It is hidden off GitHub, without a token, with one GitHub rejects, and while neither waits; without access to the
  repository it keeps its rows with the hint.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden off GitHub, and not ask again on focus`, `should stay hidden while GitHub rejects the token or nothing waits, and show once something does`, `should show the island for a returned pull request alone`)
- A `CollapsibleCard`, open by default; collapsed per repository (`reviewRequestsCollapsed` of the recent projects).
  The header: `UserCheckIcon`, "Your Turn", the count (all that wait, and the returned pull requests); collapsed, "5
  reviews · oldest waiting 2 days · 1 returned" instead of the count ("1 returned" alone), the age in `warning` when
  old. The help popover after the count says what is shown (direct requests only, then the returned pull requests),
  how the ages colour, and where from; the "⋯" menu has
  "Refresh" (when GitHub was asked on its hover: "Checked 2 minutes ago"), "Open All on GitHub" (the search of the
  repository, while requests wait) and "Notification Settings…".
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`)
- A row, 36 px: the unread dot, the title (`font-medium`), the meta "#101 · carol · 2 files · +12 −3" (coloured line
  counts; "#830 · agent-bot · requested by dave · …" when someone else than the author asked, as for a pull request a
  bot opened: that one waits for the user; "· → release/253" when the base is not the default branch; hidden below `sm`), a `destructive` dot for failing
  and a `warning` dot for running checks, and "waiting 2 days" or "re-requested 3 hours ago". The age is muted, `warning`
  after one working day (Saturdays and Sundays don't count); never `destructive`. The age is a leaf on the shared clock.
  [@test] ../../src/lib/review-requests.test.ts (`is old after a day of working time, the weekend skipped`, `says how long a request waits, or when the user was asked again`, `names who asked for the review when it is not the author`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`)
- Three rows, then "Show N More" ("Show Less" back). An unread request is always shown, in the place of a read one
  while the rows are full: a poll never grows the list or inserts a row above the others. Rows fade in by opacity;
  only the first entrance of the card slides.
  [@test] ../../src/lib/review-requests.test.ts (`shows the oldest three and every unread request, an unread one in the place of a read one`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)
- GitHub returns 50: with more, the expanded list says "50 of 63" with "Open All on GitHub".
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should say 50 of 63 when GitHub has more than it returns`)
- A click reviews the pull request in the review window ([Review window](#review-window)); ⌘-click (Ctrl elsewhere),
  `O` and the button on hover ("Open Pull Request") open it on GitHub. The right-click, the context menu key and
  Shift+F10 open the menu, the actions of the review (`getReviewRequestActions`): Review (↵), Approve…, Request
  Changes…, Open Pull Request (O), Copy Pull Request Link, Copy Branch Name (copies without a toast). A write opens the
  review window for it, since a review is on the commit the user saw: Request Changes… with its comment form, Approve
  and Safe Push… with its confirmation, Approve… with the focus on Approve once the pull request is fetched.
  Approve… is disabled with "You approved <hash>" after an approval of the head GitHub shows. ↑/↓ move the focus between
  the rows (one tab stop), Enter reviews, `D` reviews its changes (the window opens in Changes once the pull request is
  fetched, as D in the review shows them); the main window keeps the focus on the row. Space does nothing: it is Quick
  Look elsewhere, and a pull request has none.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open a pull request on GitHub from its button, a ⌘-click and its menu`, `should move with the arrows, review with Enter and open the menu from the keyboard`, `should review the changes of a row with D`, `should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`, `should not Safe Push a pull request that got commits since the user reviewed it`)
- The user's own open pull requests with changes requested (`collectReturnedToYou`, in the table order; failing checks
  are the chip's and the outgoing dot's) are rows after the requests, 36 px as theirs: the title, the meta "#760 ·
  changes requested by alice" (the reviewers whose latest review asked for changes) and the branch, muted. A click and
  Enter show the branch row, where the changes are made; ⌘-click, `O` and the button on hover open the pull request;
  the menu has Open Pull Request (O) and Copy Pull Request Link. ↑/↓ move between them.
  [@test] ../../src/lib/outgoing-attention.test.ts (`names only the open pull requests a reviewer handed back, in the table order`, `names the reviewers who asked for changes`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should list the own pull requests a reviewer handed back after the requests, and show the branch`)
- No footer while GitHub answers: the island is its rows, Refresh and when it was checked are in "⋯". A failed refresh
  keeps the rows, and a footer says why and how old the rows are, with Refresh (`ReviewStatusLine`): "GitHub: offline
  · checked 5 minutes ago". The card never turns into an error.
  [@test] ../../src/stores/review-requests.test.ts (`keeps the rows when a refresh fails, drops them without a token`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should refresh from the menu, and keep the rows when a refresh fails, with the reason in the footer`)

### Badge

- The Dock badge adds the review requests to what needs the user ([Robots Indicator](robots-indicator.spec.md#dock)),
  while "Count review requests" is on; Windows has no badge.
  [@test] ../../src/lib/dock.test.ts (`shows the sum of its sources, set only on a change, none at 0`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should count the review requests, behind its switch`)

### Review window

- The review of a pull request is only in the window `pull-request-review` (`/pull-request-review`, one window, reused;
  [Title Bar](title-bar.spec.md) for how it opens; the native title says "Review #101 <title>", as "Review login" of a
  branch and "Review a1b2c3d <subject>" of a commit, also when it is reused or ↑/↓ shows another request). A click on a
  row, Enter, `D`, the toast's Review and a write of the row's menu open it; there is no review dialog, no Open in
  Window, no `W` and no ⇧-click. Another request, or an open with an intent (a write of the menu, `D`), starts the
  window over; a plain open of the request it was opened on only brings it to the front, where the review is (also
  after ↑/↓ showed another one: that request again).
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)
  [@test] ../../src/lib/pull-request-review.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`)
- The window opens at once with what the search returned. Its title bar ([Title Bar](title-bar.spec.md)): ↑ ↓ (in the
  tooltips) and "2 of 5 · Your Turn" on the left; the Unified/Split toggle while the changes show, GitHub (`O`) and "⋯"
  (Copy Pull Request Link, Copy Branch Name) on the right (`ActionButton`s: in a title bar narrower than 48rem only
  their icons). Then, in one island:
  - who asks and for how long: an initials avatar, "dave asks for your review of carol's pull request" (the author named
    when someone else asked) · "waiting 2 days" (`warning` when old, the clock leaf of the card);
  - the title (`text-lg font-semibold`), then the chips: `#101` (opens GitHub), `master ← alice/feature` (copies the
    branch), "2 files +12 −3", "● checks failing" (`StatusChipDot`: `destructive`, `warning` running, `success` passed),
    GitHub's decision ("approved", "changes requested"), and "approved by you" / "changes requested by you".
  ↑/↓ show the previous or next request; the window tells the main window (`shown`: the request is read, its refs stay).
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`, `should review a commit of the pull request in the changes, step through the commits, and show the next request with ↓`)
- Below them the tabs Overview and Changes (`D`, with the file count) on the left and the decisions on the right
  ([Reviews](#reviews)). The overview, in an island of its own, has the description (as a commit body) beside a rail:
  the files of the range (status letter, name, directory, `+a −d`; the name first, the directory giving way; a click
  reviews all changes at that file, the right click and ⌘C copy its path, [Commit Files](../commit/commit-files.spec.md#the-actions-on-a-file))
  and its commits, newest first as in the list of commits of the changes (subject, author, age, hash; a click reviews
  the changes at that commit). Below `md` the rail goes under the description. The key hints below the islands: ↑↓
  Previous/next, Esc Close; Space scrolls.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)
- The fetch (`fetch_review_head`) is the window's: it starts when the window shows the request: "Fetching #101 from
  origin… 42%" in the rail (`tabular-nums`, "from GitHub" for a fork), its only progress; Changes is disabled, without
  its file count, until it is done, and after a failed fetch the alert says why. One fetch at a time per window
  (`fetch_review_head` and `cancel_review_fetch` count per webview label): another request, or the close of the window,
  kills the one in flight; a fetch that reports nothing for 120 s is killed too ("git reported nothing for 120 s").
  After the answer, the source is the one the range names.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`a_cancel_kills_the_fetch`, `a_stalled_fetch_is_killed`)
  [@test] ../../crates/git-executor/src/git_command_executor_test.rs (`a_git_that_reports_nothing_for_the_idle_timeout_is_killed`, `progress_resets_the_idle_timeout`, `a_cancelled_git_is_killed`)
  [@test] ../../src-tauri/src/commands/review_requests.rs (`a_window_kills_only_its_own_fetch`, `a_closed_window_kills_its_fetch`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)
- Changes (`D`, toggling) reviews the range in the island, the range review of a branch
  ([Branch Review](../branch/branch-review.spec.md)): the commits ("All Changes" `base..head`, then the commits newest
  first) above the file tree in the sidebar, beside all diffs, J/K files, N/P changes, ⌥↑/⌥↓ the items of the list, "All Changes
  · File 3 / 12" or "Commit 2 / 5 · File 1 / 3" in the footer; a chosen commit shows its subject, author, time and
  message above its diffs, never a nested commit preview. The header shrinks to the title, the tabs and the decisions.
  A re-requested review whose last review is on a commit of the pull request below its head offers "Since Your
  Review" (`<reviewed>..head`) in the list, the reviewed commit under the line "Your review · a1b2c3d"; once there is
  nothing after it (the user approved the head) or a commit left the range, the review shows all changes, at the same
  file.
  [@test] ../../src/lib/range-review.test.ts (`offers what changed since the user's review, the reviewed commit under the boundary`, `has nothing since a review of the head, or of no commit of the range`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should review a commit of the pull request in the changes, step through the commits, and show the next request with ↓`)
- Escape closes a form first, then goes back from Changes to the overview, then closes the window; while a write it
  asked for runs, the window stays. ⌘W closes it too.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`, `should keep the window while a write runs, and tell its result in the main window once the window closed`)
- `O` opens the pull request on GitHub, from the overview and from Changes.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)
- A failed fetch is a `destructive` callout "Cannot fetch pull request #101" with the reason (e.g. "GitHub asked for
  credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)", "<remote>: <git's last line>"), Retry and Open Pull Request. A fetch killed for a
  newer one ("canceled") is no failure.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`refused_github_credentials_are_told_how_to_fix`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should say why a fetch failed and fetch again on Retry`)
- When the commit the pull request shows could not be fetched (`missing`), a `warning` callout says "origin has commits
  the pull request does not have yet; the review shows them." A source ahead of the pull request (a mirror) still shows
  the pull request's commit; a force-pushed source is `diverged`.
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`a_mirror_ahead_of_the_pull_request_shows_the_pull_request`, `a_force_pushed_source_branch_diverged`, `a_head_that_cannot_be_got_shows_the_source_branch`)
- A request withdrawn while the window shows it stays, with the quiet line "No longer waiting for your review".
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)
- The main window owns the requests: the window gets them in its data (`PullRequestReviewWindowData`: the request, its
  place, the intent, and the state of the requests of its repository: the list, the reviews of the user, what runs,
  Patronus, whether the main window shows that repository), and `pull-request-review/state` whenever that changed
  (deduplicated), over the link of the window (`sub-window-link.ts`, the events of a window prefixed with its label).
- A write of the window (Approve, Request Changes, Approve and Safe Push, Open Pull Request) is a
  `pull-request-review/request` with the whole request; the main window runs it on the window's repository, the toast
  goes back to the window (`pull-request-review/feedback`, its action opens the pull request or the robot) while it
  still shows the request, else (it closed, or shows another one) the main window shows it; Activity records it in the
  main window, and `pull-request-review/reply` ends the wait (no timeout: Patronus may take a minute). A Safe Push asks
  Patronus of the repository the main window shows: for another one it is disabled with "Select <repo> in the main
  window: Patronus is asked for the repository it shows" (and refused by the main window).
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should approve the commit the review shows, in the review too, and keep its place once it left the list`, `should keep the window while a write runs, and tell its result in the main window once the window closed`)
  [@test] ../../src/stores/review-requests.test.ts (`tells the window that asked, records Activity in the main window, and asks Patronus for the selected repository only`)
  [@test] ../../src/lib/sub-window-link.test.ts (`knows what the window shows until it is destroyed, and only by its own label`, `sends the state only when it changed`, `answers each request by its id, with the feedback in between`, `forgets a window that failed to open`)
- The close of the window kills its fetch and prunes the refs of the request it showed; the stores of the files and
  the patches of a range are the window's and go with it.
  [@test] ../../src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)
  [@test] ../../src/stores/commit-patch.test.ts (`reads the range of a review with its own command, keyed base..head, until its review drops it`)

### Reviews

- The decisions are in the header, in the overview and in Changes alike, the primary last: Request Changes… (`R`,
  outline), Approve and Safe Push… (`M`, outline, where Patronus merges, see below), and **Approve** (`A`, the
  primary). Once the user approved the shown commit, Approve is "✓ Approved" (`success`, "You approved <hash>" on hover;
  `A` does nothing), and Safe Push…, the next step, is the primary while it can start. The keys work in both views; a
  key typed into a text field is no key of the review (`isTypingTarget`).
  [@test] ../../src/lib/review-request-actions.test.ts (`lists the same actions in the row menu and the review, the writes of the row opening the review`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should approve the commit the review shows, in the review too, and keep its place once it left the list`)
- A review is on the commit the review shows, the head of the fetched range (`commit_id`), also when the pull request
  got commits since. Until it is fetched, after a failed fetch, and when the commit the pull request shows could not be
  fetched (`missing`), Approve and Request Changes are disabled with the reason. An approval of the same commit again
  is disabled ("You approved <hash>"); a poll that brings new commits fetches them, the header says "#101 got new
  commits while it was open: the review shows <hash> now", and they can be approved.
  [@test] ../../src/lib/review-request-actions.test.ts (`disables the writes of the review until the commit the user saw is there, and an approval of the same commit`)
- Approve is one `POST repos/<owner>/<name>/pulls/<n>/reviews` with `{"event": "APPROVE", "commit_id": <id>}`
  (`submit_pull_request_review`, the token of [Pull Requests](../branch/pull-requests.spec.md#the-token)), in the
  repository the requests were read from; anything but an object id is refused. Request Changes… opens a form below
  its button: the comment is required, ⌘↵ (Ctrl+Enter) sends it as `{"event": "REQUEST_CHANGES", "body": <comment>}`,
  Escape closes the form and keeps the draft, the focus goes back to the button. The draft is the pull request's (by
  its URL, in the browser storage every window shares, `review-draft.ts`): the window showing another request, or
  closed, gives it back when the pull request is reviewed again; a sent comment forgets it.
  [@test] ../../crates/sync-core/src/pull_request_writes_test.rs (`approves_the_commit_the_user_reviewed`, `a_request_for_changes_posts_its_comment`, `refuses_a_review_without_a_commit_id_or_a_request_for_changes_without_a_comment`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)
- A review is a toast "Approved #101" / "Requested changes on #101" with the title and "Open Pull Request", and an
  Activity entry in the same words; the chips say "approved by you" (`success`) or "changes requested by you".
  GitHub is asked again at once: the reviewed request leaves the card, the review keeps it without the "No longer
  waiting" line, at its place: ↓ shows the request now there, ↑ the one before.
  [@test] ../../src/stores/review-requests.test.ts (`approves the commit the user saw, says so with Open Pull Request, and asks GitHub again`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should approve the commit the review shows, in the review too, and keep its place once it left the list`)
- A refused review is the error toast "Cannot approve pull request #101" ("Cannot request changes on pull request
  #101") with what GitHub said (e.g. "User can only have one pending review per pull request"); the action stays.
  [@test] ../../crates/sync-core/src/pull_request_writes_test.rs (`a_refused_review_says_what_github_said`)
  [@test] ../../src/stores/review-requests.test.ts (`requests changes with the comment, and says why GitHub refused`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should say why GitHub refused a review`)

### Safe Push of a reviewed pull request

- Where Patronus guards the repository, the header has Approve and Safe Push… (`M`), an outline button before Approve,
  the primary (the decision comes first, the merge after it), and the row's menu has it after Approve…. It is enabled only for a pull request into the branch
  Patronus pushes into for the baseline (`get_patronus_target_branch`); another base is disabled with "#103 merges into
  release/253: Patronus would merge it there, not into master" (a stacked pull request would land in its parent).
  [@test] ../../src/lib/review-request-actions.test.ts (`offers Approve and Safe Push where Patronus merges into the base, the Safe Push alone once approved`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should approve and Safe Push a pull request into the Patronus target with a plain /patronus merge, after a confirmation`)
- It opens a confirmation below its button: "Approves 1a2b3c4d, then comments /patronus merge on #101: Patronus rebases
  alice/feature onto master, runs the checks, merges it and deletes the branch. It reports in the pull request.", with
  "Approve and Safe Push" (↵, focused) and Esc Cancel; the keys of the review wait while it is open. After an approval
  of the commit it is "Safe Push…" and the text starts with "Comments".
- Approve first, unless the user approved that commit; then one `GET repos/<owner>/<name>/pulls/<n>` for its
  `head.sha` (`get_pull_request_head`): a head other than the approved commit is refused, "Cannot start the Safe Push" / "#101 has
  new commits since you reviewed 1a2b3c4d: review 5e6f7a8b first", and nothing is commented.
  [@test] ../../crates/sync-core/src/pull_request_writes_test.rs (`reads_the_head_of_a_pull_request`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should not Safe Push a pull request that got commits since the user reviewed it`)
- Then the comment `/patronus merge` (without `--keep-source-branch`: the branch of someone else goes once merged, as a
  merge on GitHub does) in the repository the requests were read from, and the bot's answer as for an own branch
  ([Patronus](../branch/patronus.spec.md#pull-requests)), "Asking Patronus on pull request #101…" in the confirmation
  meanwhile. The 👍 is the start: the robot is looked for in the user's robots for 10 s more; found, it is announced as
  any robot ("Safe Push started" with Open in Patronus) and watched for the session (`watchRobot`): polled while it
  runs, listed in the robots indicator by "#101 <title>", its merge told ("#101 <title> merged into master",
  [Robots Indicator](robots-indicator.spec.md#list)); not found (Patronus may count it as the author's),
  the toast "Safe Push started" / "#101 <title>" opens the pull request, and Activity says "Safe Push started on #101".
  A 👎 is "Cannot start the Safe Push" with the bot's reason; the approval stays.
  [@test] ../../src/stores/review-requests.test.ts (`approves, checks the head GitHub shows and asks Patronus; its thumbs up is the start`, `refuses a pull request that got commits since the review, and says the reason of the bot`)
  [@test] ../../src/stores/patronus.test.ts (`starts a reviewed pull request of someone else with its thumbs up, the robot only if the user's robots list it`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should watch the robot of a reviewed pull request: listed by what it tests, its merge told`)

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
  repository per start and after a fetch wrote one, and when the review window closes; never after a failed answer;
  the refs of the request the window shows stay. A CAS failure is tried again the next time; across app instances it is best effort.
  [@test] ../../src/stores/review-requests.test.ts (`prunes once after an answer with the requests, never after a failure, and keeps the window's`)
  [@test] ../../crates/sync-core/src/review_fetch_test.rs (`prune_deletes_the_review_refs_not_kept`, `prune_deletes_nothing_when_a_ref_moved_meanwhile`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)

### Settings

- "Settings…" (⌘, in the app menu on macOS; File → "Settings…", Ctrl+, elsewhere; the focused window opens it) and
  "Notification Settings…" of the card open the Settings dialog (`md`), a home for later settings too.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open from the card menu and keep the switches`, `should open from the app menu`)
- Branches: the branch prefix of every repository ([Onboarding](onboarding.spec.md)). Notifications: "Review requests" (`notifyReviewRequests`), "Safe Push merges" (`notifyPatronusMerges`, the merge
  notification of [Patronus](../branch/patronus.spec.md#merged)), "Safe Push failures" (`notifyPatronusFailures`, a
  required check of a running robot failed, or a robot failed, [Patronus](../branch/patronus.spec.md#failing),
  [Failed](../branch/patronus.spec.md#failed)). "Dock icon" (not on Windows): "Count review requests"
  (`reviewBadge`), "Count failed robots and conflicts" (`attentionBadge`). All on by default, in the app settings,
  synced across windows.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open from the card menu and keep the switches`, `should count the review requests, behind its switch`)
- GitHub: where the token comes from, and the app's own sign-in ([GitHub Sign-In](github-sign-in.spec.md#settings)).
- The permission of the OS, checked when the dialog opens and on focus: not asked, "Allow Notifications"; refused, a
  `warning` callout "Notifications are off for Branch Deck in System Settings" with "Open System Settings"; granted,
  "Send Test Notification".
- "AI": "Suggest branch names", the consent of the AI branch names
  ([Branch Name Suggestions](../branch/branch-name-suggestions.spec.md)).

## User Experience

- The card is a `Card` island (`CollapsibleCard`), its rows `divide-y`, the hover `bg-muted`; the focus ring of a row is
  inset. The unread dot is `size-1.5 bg-primary`, the checks dots are `StatusChipDot`. The status line of a failure is
  the footer, `text-xs`.
- `UserCheckIcon` means "a review for the user" (the card, the Activity entry, the Review menu item).
- The review is a "decision header" (GitHub, Graphite, Linear): the window-level actions in a toolbar of `sm` ghost
  buttons with icon, label and key; the decisions as `sm` buttons (`MenuItemButton size="sm"`: the icon, the label, the
  key last, the reason of a disabled action in a tooltip), Approve the primary, last button; the footer is key hints
  only. A key inside a primary button is `PRIMARY_KBD_CLASS_NAME` (it would vanish on the primary colour). The tabs are
  `TabsList variant="line"`. The window lies on `bg-canvas` with two `Card` islands.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/review-requests.test.ts` (ages, order, news, backoff), `src/stores/review-requests.test.ts`
  (polling, notifications, pruning), `src/lib/os-notification.test.ts`, `src/lib/dock.test.ts`, `src/stores/commit-files.test.ts`,
  `src/stores/commit-patch.test.ts`.
- `cargo nextest run`: `crates/sync-core/src/review_requests_test.rs` (a fake GitHub), `review_fetch_test.rs` (local
  bare remotes), `crates/git-ops/src/commit_files_test.rs`, `crates/git-executor/src/git_command_executor_test.rs`,
  `crates/test-server/src/review_requests_test.rs`.
  [@test] ../../crates/test-server/src/review_requests_test.rs (`review_routes_answer_the_fixture_without_a_network`)
- `pnpm e2e`: `tests/e2e/review-requests/review-requests.spec.ts` with the `issue_links` template.
  `submit_pull_request_review` is answered in the page (`routeCommand`, `routeCommandError`), which records the calls.
  `POST /repositories/{id}/review-requests` sets what `get_review_requests` answers (`availableReviewRequests` for
  `acme/monorepo`) and, with `fetchError`, makes `fetch_review_head` fail; `GET` returns `{ calls, fetchCalls, prunes }`.
  `fetch_review_head` runs the real function on the template: an empty head is the branch the request names, an empty
  base the baseline, the source `local`; a test holds it in the page for the progress (`routeReviewFetch`). The Dock and
  ⌘-click tests take the user agent of macOS. The review window is the page `/pull-request-review` with the recorded
  data (`openSubWindowPage`); the label stays `main`, so Escape closing the window and Activity staying in the main
  window are not covered there; what the window emits is read with `getEmittedEvents`, the main window's answers are
  fired with `emitTauriEvent`.
- In the app: a repository with direct review requests; the background notification (another app in front); a review
  fetch from a Space checkout; afterwards `git for-each-ref refs/branch-deck/review/` is empty and `git branch -r`
  unchanged.

## Open Questions / Risks

- Whether Patronus starts a robot for a `/patronus merge` of someone who is not the author, and whose robot it is
  (the list asks for `owner=me`), is to be checked in a live run; a robot not among the user's is not watched, so its
  merge is told by the pull request, not by the app.
- GitHub refuses a review while the user has a pending one (started on github.com and not submitted): the toast says
  GitHub's words; the app does not submit or delete the pending review.
- The search counts a request that was answered by a review moments ago until GitHub's index catches up; the review
  time drops it.
- A request of a team the user is in is not shown, by design; a CODEOWNERS-heavy repository may show little.
- Whether macOS shows a notification of the app in front ("Send Test Notification" while Branch Deck is frontmost) is
  to be checked in the app.

## References

- [Pull Requests](../branch/pull-requests.spec.md): the user's own pull requests, the repository and the token.
- [Commit Review](../commit/commit-review.spec.md), [Commit Files](../commit/commit-files.spec.md): the review and the
  range subject.
- [Robots Indicator](robots-indicator.spec.md#dock): the badge.
- [Activity](activity.spec.md), [Patronus](../branch/patronus.spec.md): the notification of a merge.
- [Git Operations](../../docs/design/git-operations.md): the review fetch.
