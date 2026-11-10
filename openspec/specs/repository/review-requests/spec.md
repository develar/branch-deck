---
name: Review Requests
description: Requirements for "Your Turn", the island of the GitHub pull requests of others that wait for the user's review (requested by name), above the branches, waiting the longest first, with the user's own pull requests a reviewer handed back; GitHub polled every five minutes through its API, a toast or an OS notification for a new request, the Dock badge; the in-app review of a pull request in its own window (an overview, its description whole beside its shape, else its files, and its commits, then the review of base..head with the file tree and all diffs, led by the summary of the description), approving it (the primary decision) or requesting changes on the commit the user saw, with the same actions in the overview, the review and the row's menu; and the Notifications page of Settings, the notifications and the badge.
style: plain-1
targets:
  - crates/domain-types/src/review_request.rs
  - src/lib/approval-news.ts
  - crates/remote-facts/src/review_requests.rs
  - crates/domain-policy/src/news.rs
  - crates/repository-model/src/review_request_facts.rs
  - crates/remote-facts/src/scheduler_review_requests.rs
  - crates/domain-policy/src/refresh.rs
  - crates/sync-core/src/review_fetch.rs
  - crates/operations/src/review_fetch.rs
  - crates/remote-facts/src/pull_requests.rs
  - crates/remote-facts/src/pull_request_writes.rs
  - crates/operations/src/review.rs
  - crates/operations/src/robot_start.rs
  - crates/github/src/service.rs
  - crates/sync-core/src/remote_command.rs
  - crates/git-ops/src/commit_files.rs
  - crates/git-executor/src/git_command_executor.rs
  - crates/sync-utils/src/process_tree.rs
  - crates/test-server/src/review_fixtures.rs
  - crates/app-api/src/reads/diffs.rs
  - crates/app-api/src/remote/pull_requests.rs
  - src-tauri/src/services.rs
  - crates/app-api/src/app/windows.rs
  - src-tauri/src/lib.rs
  - src-tauri/capabilities/sub-windows.json
  - src-tauri/src/menu.rs
  - src-tauri/capabilities/desktop.json
  - src/lib/review-requests.ts
  - src/lib/review-request-actions.ts
  - src/lib/pull-request-review.ts
  - src/lib/review-window.ts
  - src/lib/review-runs.ts
  - src/lib/event-toast.ts
  - src/lib/sub-window-link.ts
  - src/hooks/useMainWindowLink.ts
  - src/lib/range-review.ts
  - src/components/review/RangeReview.tsx
  - src/components/review/DescriptionLead.tsx
  - src/components/files/FilePathLabel.tsx
  - src/presenters/commit-body.ts
  - src/lib/errors.ts
  - src/lib/preview-keys.ts
  - src/lib/os-notification.ts
  - src/lib/dock.ts
  - src/lib/commit-files.ts
  - src/lib/outgoing-attention.ts
  - src/stores/review-requests.ts
  - src/stores/patronus.ts
  - src/stores/review-fetch.ts
  - src/stores/settings/appSettings.ts
  - src/stores/settings/repositorySettings.ts
  - src/stores/activity.ts
  - src/hooks/useReviewStatusRefresh.ts
  - src/hooks/useSessionFactStores.ts
  - src/hooks/useDockStatus.ts
  - src/hooks/useReviewRuns.ts
  - src/components/repository/YourTurnCard.tsx
  - src/hooks/useSettledLayout.ts
  - src/components/repository/ReviewRequestRow.tsx
  - src/components/repository/ReviewStatusLine.tsx
  - src/components/review/PullRequestReview.tsx
  - src/components/review/ReviewOwnerChips.tsx
  - src/hooks/useFileOwners.ts
  - src/hooks/useReviewOwnerFilter.ts
  - src/lib/review-position.ts
  - src/presenters/reviews.ts
  - src/lib/review-owners.ts
  - src/components/review/ReviewHeader.tsx
  - src/components/review/ReviewDecisions.tsx
  - src/lib/review-draft.ts
  - src/components/review/ReviewRail.tsx
  - src/pages/PullRequestReviewWindow.tsx
  - src/pages/ReviewPage.tsx
  - src/components/shared/MenuItemButton.tsx
  - src/components/shared/PreviewDialogFrame.tsx
  - src/components/commit/CommitReview.tsx
  - src/components/shared/CollapsibleCard.tsx
  - src/components/settings/NotificationsPage.tsx
  - src/components/settings/SettingRow.tsx
  - src/components/branch/BranchCreator.tsx
  - src/App.tsx
  - src/model/intents.ts
---

# Review Requests

Status: Implemented
Date: 2026-10-15

## Purpose

Branch Deck showed GitHub status only for the user's own pull requests ([Pull Requests](../../branch/pull-requests/spec.md)).
Whose turn it is to review was told by github.com/pulls alone. "Your Turn" is an island above the branches with the open
pull requests that ask for the user's review by name, waiting the longest first, how long each waits, and the user's own
pull requests a reviewer handed back. GitHub is polled every five minutes; a new request is a toast, or an OS
notification while Branch Deck is in the background. A row opens the review of the pull request in the app, in a window
of its own: an overview of what GitHub said and of the fetched files and commits, then the review of
`base..head`, the file tree beside all diffs,
as the review of a commit ([Commit Review](../../commit/commit-review/spec.md)). The user approves it or requests changes
there, in the preview and in the review alike, on the commit the review shows; the row's menu has the same actions.

## Goals

- See without leaving the app which pull requests wait for the user, the oldest first, and which have waited long.
- Learn of a new request when it comes, also in the background, once.
- Read a pull request (its description, its commits, its diffs) in the app, without a checkout and without touching
  the working tree, also from a Space checkout mirrored to GitHub.
- Answer it where it was read: approve, or request changes with a comment, without opening GitHub.

## Non-goals

- Requests of a team the user is in (CODEOWNERS fan-out): only direct requests (`user-review-requested:@me`); the
  team requests, the reviewed and the own pull requests are [Reviews](../reviews/spec.md).
- Drafts and archived repositories.
- Comments on lines, a review with a comment only, and dismissing a review: GitHub does that (Open Pull Request).
- Repositories other than the selected one: only it is polled.
- "Hide Until Updated" on a row (not chosen for the first version).
- A click action on an OS notification.

## Concepts

### Approval news

The other side of the review: the user waits for the code owners of their own pull request
([Next Step](../../branch/next-step/spec.md), "Awaiting Review", whom in its hover). An approval that makes it ready is news, with the
Safe Push it allows.

## Requirements

### Requirement: Reading the requests: One GraphQL search of the direct requests per refresh

Each refresh MUST be one GraphQL request (2 points of the rate limit), with the token of [Pull
Requests](../../branch/pull-requests/spec.md#the-token): the search `repo:<owner>/<name> is:pr is:open archived:false
draft:false user-review-requested:@me`, `first: 50` with the `issueCount`, and `repository(owner, name)`, whose null
is "no access" as for the pull requests.

#### Scenario: The query of a refresh
- **WHEN** the session refreshes the review requests of a GitHub repository
- **THEN** it sends one GraphQL request: the search of the direct requests, `first: 50` with the `issueCount`, and `repository(owner, name)`
- **AND** a null `repository(owner, name)` is "no access" as for the pull requests
  [@test] crates/remote-facts/src/review_requests_test.rs (`asks_for_the_direct_requests_of_the_repository`)

### Requirement: Reading the requests: The repository is the one of the pull requests

The repository MUST be the one of [Pull Requests](../../branch/pull-requests/spec.md#reading-pull-requests)
(configured, `origin`, or the Space mirror). When the listing of the pull requests named it, the session passes
`owner/name`, and it MUST NOT be resolved again. A repository outside GitHub MUST never ask GitHub.

#### Scenario: A repository the listing of the pull requests named
- **GIVEN** the listing of the pull requests named the GitHub repository
- **WHEN** the review requests are read
- **THEN** the session passes `owner/name`, and it is not resolved again
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_known_github_repository_skips_resolving_it`)

#### Scenario: A repository outside GitHub
- **WHEN** the repository is outside GitHub
- **THEN** it never asks GitHub

### Requirement: Reading the requests: The requests are a fact of the session

The requests MUST be a fact of the session of the repository (`facts.reviewRequests`,
`repository_model::review_request_facts`), which its scheduler reads (`remote_facts::scheduler`); the page only
shows them.

#### Scenario: The page shows the fact of the session
- **WHEN** the scheduler of the session reads the review requests
- **THEN** they are the fact `facts.reviewRequests` of the session of the repository
- **AND** the page only shows them

### Requirement: Reading the requests: A request waits since the request event of the user

A request MUST wait since the newest review request event whose reviewer is the user, else since the pull request was
opened; who asked MUST be that event's actor, not the author.

#### Scenario: The time and the asker of a request
- **WHEN** the search returns a request
- **THEN** it waits since the newest review request event whose reviewer is the user, else since the pull request was opened
- **AND** who asked is that event's actor, not the author
  [@test] crates/remote-facts/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)

### Requirement: Reading the requests: A review since the request event drops the request

A request MUST count as re-requested when the user's latest submitted review is older than its request event; a
pending review is no review. A request whose latest review is not older than the event MUST be dropped (the search
index lags behind a review).

#### Scenario: A re-request and a lagging review
- **WHEN** the user's latest submitted review is older than the request event
- **THEN** the request counts as re-requested, a pending review being no review
- **AND** a request whose latest review is not older than the event is dropped (the search index lags behind a review)
  [@test] crates/remote-facts/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)

### Requirement: Reading the requests: A request lists its reviewers

A request MUST list its reviewers (`reviewers`): the latest review of each (`latestReviews`; a pending one is none),
then the users asked without one (`reviewRequests`; a team needs `read:org`, and is not read). They are what the
readiness of the request is asked with ([Patronus](../../branch/patronus/spec.md#the-readiness-of-a-request)): a new
review MUST ask it again.

#### Scenario: The reviewers of a request
- **WHEN** the search returns a request
- **THEN** its `reviewers` are the latest review of each reviewer, a pending one none, then the users asked without one
- **AND** a team asked is not read
  [@test] crates/remote-facts/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)

#### Scenario: A new review asks the readiness again
- **WHEN** a new review of the request comes
- **THEN** the readiness of the request is asked again with its reviewers

### Requirement: Reading the requests: A request the user answered with a comment is dropped

A comment in the conversation is no review, and GitHub keeps the request: a request the user answered with a comment
after the newest request event of the user MUST be dropped until the other side moves.

#### Scenario: The user answered with a comment
- **GIVEN** the user commented in the conversation after the newest request event of the user
- **WHEN** the requests are read
- **THEN** the request is dropped until the other side moves
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_request_the_user_answered_with_a_comment_is_not_their_turn`)

### Requirement: Reading the requests: A second timeline of the query tells the answer

A second timeline of the same query, `activity` (`last: 30` comments, reviews, commits and force pushes; its own
connection, so bot comments don't push the request events out; the query still costs 2 points), MUST tell it: the
user's newest comment (`viewerDidAuthor`) not older than the event; a command such as `/patronus dry-run` (which the
app posts as the user) MUST be no answer.

#### Scenario: A comment and a command of the user
- **WHEN** the user's newest comment (`viewerDidAuthor`) in `activity` is not older than the request event
- **THEN** it is the answer
- **AND** a command such as `/patronus dry-run` is no answer
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_request_the_user_answered_with_a_comment_is_not_their_turn`)

### Requirement: Reading the requests: A move of the other side gives the turn back

A move after the answer MUST give the turn back: a commit (by its date, of anyone), a force push, or a comment or a
submitted review of someone else than the user; a force push, a comment or a review of a bot (`Bot`) MUST never move.
The request waits since the first move then, and says what it was (`backToYou`: who replied, or new commits). Only the
times are compared, not the order of the timeline; a commit made before the answer and pushed after it is missed, as a
plain push has no event.

#### Scenario: Someone else moves after the answer
- **WHEN** a commit, a force push, a comment or a submitted review of someone else than the user comes after the answer
- **THEN** the request waits again, since the first move
- **AND** it says what the move was (`backToYou`: who replied, or new commits)
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_move_of_the_other_side_after_the_answer_makes_it_the_user_s_turn_again`)

#### Scenario: A bot comments after the answer
- **WHEN** a bot (`Bot`) comments after the answer
- **THEN** nothing moves and the request stays dropped

#### Scenario: A commit pushed after the answer
- **WHEN** a commit made before the answer is pushed after it
- **THEN** it is missed, as only the times are compared and a plain push has no event

### Requirement: Reading the requests: A comment hides only a request by name

Without a request event of the user (a team request, an event beyond the last 20) a comment MUST hide nothing.

#### Scenario: No request event of the user
- **WHEN** the user commented on a pull request without a request event of the user (a team request, an event beyond the last 20)
- **THEN** the comment hides nothing
  [@test] crates/remote-facts/src/review_requests_test.rs (`an_answer_counts_only_after_a_request_by_name`)

### Requirement: Reading the requests: The count leaves out the requests that don't wait

The count MUST be GitHub's `issueCount` without the requests of the answer that don't wait (a lagging review, an
answer by comment); GitHub's own search, "Open All on GitHub", still lists the answered ones.

#### Scenario: A lagging review and an answer by comment
- **WHEN** the answer holds a request with a lagging review or one answered by comment
- **THEN** the count is the `issueCount` without them
- **AND** "Open All on GitHub" still lists them
  [@test] crates/remote-facts/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`, `a_request_the_user_answered_with_a_comment_is_not_their_turn`)

### Requirement: Reading the requests: A dropped request says so in its review window

A request dropped while its review window is open there MUST say "No longer waiting for your review".

#### Scenario: The request is dropped while its review window is open
- **GIVEN** the review window of a request is open
- **WHEN** the request is dropped
- **THEN** the window says "No longer waiting for your review"

### Requirement: Reading the requests: A failure of GitHub is classified, never thrown

A partial answer (`data` with `errors`) MUST be read; GitHub's rate limit MUST be `rateLimited`; every other failure
MUST be classified as for the pull requests and never thrown.

#### Scenario: A partial answer
- **WHEN** GitHub answers `data` with `errors`
- **THEN** the data is read
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_partial_answer_is_read`)

#### Scenario: The rate limit
- **WHEN** GitHub's rate limit refuses the request
- **THEN** the answer is `rateLimited`
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_rate_limit_is_rate_limited`)

#### Scenario: Another failure
- **WHEN** the request fails otherwise
- **THEN** the failure is classified as for the pull requests and never thrown
  [@test] crates/remote-facts/src/review_requests_test.rs (`classifies_the_other_failures`)

### Requirement: Reading the requests: The requests wait the longest first

The session MUST keep the requests in memory, sorted by how long they wait, then by number.

#### Scenario: The order of the requests
- **WHEN** the session holds several requests
- **THEN** they are sorted by how long they wait, then by number
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_requests_wait_the_longest_first_and_a_new_one_is_news_once`)

### Requirement: Reading the requests: A failure keeps the rows, a rejected token drops them

A failure of GitHub MUST keep the rows of the last listing (`Stale` with the reason); no token, a token GitHub
rejects and a repository off GitHub MUST drop them.

#### Scenario: GitHub fails
- **WHEN** GitHub fails
- **THEN** the rows of the last listing stay, `Stale` with the reason
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_failure_keeps_the_rows_and_a_rejected_token_drops_them`)

#### Scenario: No token, a rejected token, a repository off GitHub
- **WHEN** there is no token, GitHub rejects the token, or the repository is off GitHub
- **THEN** the rows are dropped
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_failure_keeps_the_rows_and_a_rejected_token_drops_them`)

### Requirement: Reading the requests: An answer that changes nothing re-renders nothing

An answer that changes nothing MUST send no patch, and the page MUST keep the objects of the rows a patch doesn't
change, so a poll re-renders nothing.

#### Scenario: A poll with the same answer
- **WHEN** a poll brings an answer that changes nothing
- **THEN** the session sends no patch, and the page keeps the objects of the rows and the list
  [@test] src/lib/review-requests.test.ts (`keeps the objects and the list of an answer that changed nothing`)
  [@test] src/stores/review-requests.test.ts (`keeps the list and the rows of an answer that changed nothing`)

### Requirement: Polling: The session asks after a sync, on a focus, by Refresh and by a poll

The session of the selected repository MUST ask after a sync (at most once a minute), on a focus when its requests are
older than two minutes (at least a minute after the last request), by the Refresh button always, and by a poll five
minutes after each answer, focused or not, on the clock of the session; a poll closer than 4.5 min to the last
request MUST ask nothing.

#### Scenario: A sync, a focus and the Refresh button
- **WHEN** a sync ends (at most once a minute), the window gets the focus while the requests are older than two minutes (at least a minute after the last request), or the user presses the Refresh button
- **THEN** the session asks GitHub, by the Refresh button always
  [@test] crates/remote-facts/src/scheduler_test.rs (`lists_the_review_requests_after_a_sync_on_a_focus_and_by_the_refresh_button`)
  [@test] crates/domain-policy/src/refresh_test.rs (`the_review_requests_are_polled_every_five_minutes_and_listed_after_a_sync_every_minute`)

#### Scenario: The poll of five minutes
- **WHEN** five minutes passed on the clock of the session since the last answer, focused or not
- **THEN** a poll asks, unless it is closer than 4.5 min to the last request
  [@test] crates/remote-facts/src/scheduler_test.rs (`polls_every_five_minutes_focused_or_not_and_backs_off_while_github_fails`)
  [@test] crates/domain-policy/src/refresh_test.rs (`the_review_requests_are_polled_every_five_minutes_and_listed_after_a_sync_every_minute`)

### Requirement: Polling: A poll is skipped offline

A poll MUST be skipped offline (`online` of the window's demand, `navigator.onLine`): it waits a whole interval more.

#### Scenario: The window is offline at the time of a poll
- **GIVEN** the window's demand says it is not `online` (`navigator.onLine`)
- **WHEN** the time of a poll comes
- **THEN** the poll asks nothing and waits a whole interval more
  [@test] crates/remote-facts/src/scheduler_test.rs (`skips_a_poll_while_offline`)

### Requirement: Polling: One request is in flight

One request MUST be in flight; a Refresh during it MUST ask once more after it.

#### Scenario: Refresh while a request is in flight
- **GIVEN** a request of the review requests is in flight
- **WHEN** the user presses Refresh
- **THEN** no second request starts, and the session asks once more after it

### Requirement: Polling: A poll lists the pull requests while a subject waits for approvals

While the last answer of a subject waits for approvals in its pull request (`domain_policy::readiness::awaits_approval`),
each poll MUST list the pull requests too, whose new reviews ask its readiness again ([Approval news](#approval-news)).

#### Scenario: A subject waits for approvals
- **GIVEN** the last answer of a subject waits for approvals in its pull request
- **WHEN** a poll of the review requests comes
- **THEN** it lists the pull requests too, whose new reviews ask the readiness again
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_poll_lists_the_pull_requests_while_a_subject_waits_for_approvals`)
  [@test] crates/domain-policy/src/readiness_test.rs (`a_subject_awaits_approvals_while_patronus_misses_them_or_github_waits_for_a_reviewer`)

### Requirement: Polling: No store of the page polls on a timer of its own

A store of the page MUST NOT poll on a timer of its own (`poll-timer` of `pnpm lint:arch`).

#### Scenario: A timer in a store or a hook
- **WHEN** a store or a hook of the page sets a timer
- **THEN** `poll-timer` of `pnpm lint:arch` fails on it as a poll, and not on a timer of its test or another module
  [@test] scripts/lint-arch.test.mjs (`a timer of a store or a hook is a poll, of its test or another module not`)

### Requirement: Polling: A failure backs off, the rate limit is waited out

A failure MUST double the interval, up to 30 min; the rate limit MUST wait 15 min at least (nothing but Refresh asks
before).

#### Scenario: GitHub fails
- **WHEN** GitHub fails again
- **THEN** the interval doubles, up to 30 min
  [@test] crates/domain-policy/src/refresh_test.rs (`the_review_requests_poll_backs_off_after_failures_waits_out_a_rate_limit_and_stops_without_access`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`polls_every_five_minutes_focused_or_not_and_backs_off_while_github_fails`)

#### Scenario: The rate limit
- **WHEN** GitHub answers with its rate limit
- **THEN** the next poll waits 15 min at least, and only Refresh asks before
  [@test] crates/remote-facts/src/scheduler_test.rs (`waits_out_the_rate_limit_but_the_refresh_button_asks`)

### Requirement: Polling: Without a token or access nothing is polled

Without a token, with one GitHub rejects, or without access to the repository (no access, SAML SSO) nothing MUST be
polled until a focus or Refresh; a repository not on GitHub MUST be asked again only by Refresh.

#### Scenario: No token, a rejected token, no access
- **WHEN** there is no token, GitHub rejects the token, or the user has no access to the repository (no access, SAML SSO)
- **THEN** nothing is polled until a focus or Refresh
  [@test] crates/domain-policy/src/refresh_test.rs (`the_review_requests_poll_backs_off_after_failures_waits_out_a_rate_limit_and_stops_without_access`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`stops_while_github_rejects_the_token_until_a_focus_asks_again`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden while GitHub rejects the token or nothing waits, and show once something does`)

#### Scenario: A repository not on GitHub
- **WHEN** the repository is not on GitHub
- **THEN** it is asked again only by Refresh, not on a focus
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden off GitHub, and not ask again on focus`)

### Requirement: Polling: A session no window watches asks nothing

A session no window watches MUST ask nothing: another repository selected leaves the session of the last one without
the main window, and its polls stop (a review window of it keeps them).

#### Scenario: Another repository is selected
- **WHEN** the user selects another repository in the main window
- **THEN** the polls of the session of the last one stop
- **AND** a review window of that repository keeps them

### Requirement: Polling: Opening a pull request on GitHub tells the session

Opening a pull request on GitHub from the card or the review MUST tell the session (`refresh_facts { reviewRequests,
afterWrite }`): its next focus asks at once, an approved one leaves the list.

#### Scenario: The page tells the session
- **WHEN** the user opens a pull request on GitHub from the card or the review
- **THEN** the page tells the session with `refresh_facts { reviewRequests, afterWrite }`
  [@test] src/stores/review-requests.test.ts (`tells the session a request was opened on GitHub: the next focus asks at once`)

#### Scenario: The next focus asks at once
- **GIVEN** a request was opened on GitHub
- **WHEN** the window gets the focus again
- **THEN** the session asks at once, and an approved one leaves the list
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_request_opened_on_github_asks_on_the_next_focus_at_once`)

### Requirement: New requests: A request is new by its number and the time of its request

A request MUST be new when its number and the time of its request event (`requestedAt`) are not in the last answer: a
re-request is new. A request back to the user after their answer MUST NOT be new, and keeps its key, read or unread as
the user left it: the user commented, they know it.

#### Scenario: A request the last answer lacks, and a re-request
- **WHEN** a listing holds a request whose number and `requestedAt` are not in the last answer
- **THEN** the request is new, a re-request too
  [@test] crates/domain-policy/src/news_test.rs (`a_listing_tells_the_requests_it_adds_and_re_requests_never_one_back_to_the_user`)

#### Scenario: A request back to the user after their answer
- **WHEN** a request comes back to the user after their answer by comment
- **THEN** it is not new, and keeps its key, read or unread as the user left it
  [@test] crates/domain-policy/src/news_test.rs (`a_listing_tells_the_requests_it_adds_and_re_requests_never_one_back_to_the_user`)
  [@test] src/lib/review-requests.test.ts (`keeps the key of a request back to the user after their answer`)

### Requirement: New requests: The session tells the new requests as its news

The session MUST tell the new requests as news (`reviewRequested`, the numbers, `domain_policy::news::review_requested`);
its first listing, and the first one after a window watches it anew (a start, the repository selected again), MUST be
silent, and so MUST the news of a view the window subscribes to.

#### Scenario: A new request is news once
- **WHEN** a listing adds a request
- **THEN** the session tells `reviewRequested` with its number, once
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_requests_wait_the_longest_first_and_a_new_one_is_news_once`)

#### Scenario: The first listing after a window watches anew
- **WHEN** the session lists the requests for the first time, or the first time after a window watches it anew (a start, the repository selected again)
- **THEN** it tells no news, nor does a view the window subscribes to
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_first_listing_after_a_window_watches_anew_is_silent`)

### Requirement: New requests: One toast tells the new requests of an answer, with Review

While a Branch Deck window has the focus (a conflict window counts), one toast MUST tell the new requests of an answer,
with the action "Review", which opens the first one that still waits, for 10 s.

#### Scenario: One request, several requests
- **WHEN** an answer brings one new request
- **THEN** the toast says "dave requested your review" / "#104 Document the CLI"
- **AND** several new requests are one toast "3 reviews requested" / "#1 … · #2 … · #3 …" (four of four named, from five on the first three and "2 more", `cutList`)
  [@test] src/lib/review-requests.test.ts (`tells one request by who asked, several in one notice`)

#### Scenario: Review opens the first request that still waits
- **GIVEN** a Branch Deck window has the focus
- **WHEN** a poll brings new requests and the user presses "Review" of the toast
- **THEN** the first one that still waits opens, unread until opened
  [@test] src/stores/review-requests.test.ts (`tells the requests the session tells new in one toast with Review, unread until opened, recorded in Activity`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)

### Requirement: New requests: The toast of one request is the toast of its pull request

The toast of one request MUST be the toast of its pull request
([One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event)): the review the user submits
takes its place. A toast of several MUST close once none of them waits, and so MUST either while the main window shows
another repository, where Review would open nothing.

#### Scenario: The user reviews the request of the toast
- **WHEN** the user submits the review of the request a toast tells
- **THEN** the toast of the review ("Approved #104") takes its place
  [@test] src/stores/review-requests.test.ts (`replaces the toast of a request with the review of it, and closes a toast of several once none of them waits`)

#### Scenario: None of the requests of a toast waits any more
- **WHEN** none of the requests of a toast of several waits any more
- **THEN** the toast closes
  [@test] src/stores/review-requests.test.ts (`replaces the toast of a request with the review of it, and closes a toast of several once none of them waits`)

#### Scenario: The main window shows another repository
- **WHEN** the main window shows another repository
- **THEN** the toast of one request and the toast of several close, as Review would open nothing

### Requirement: New requests: In the background an OS notification tells them

In the background an OS notification MUST say it at once, with the words of the toast, when "Review requests" is on in
Settings; the toast MUST show on the next focus.

#### Scenario: A new request in the background
- **GIVEN** no Branch Deck window has the focus
- **WHEN** a new request comes
- **THEN** an OS notification says it at once, with the words of the toast
- **AND** the toast shows on the next focus
  [@test] src/stores/review-requests.test.ts (`tells in the background by an OS notification, and in a toast on focus`)
  [@test] src/lib/os-notification.test.ts (`shows the toast at once while focused, else notifies and shows it on focus`)

#### Scenario: "Review requests" is off in Settings
- **WHEN** "Review requests" is off in Settings and a new request comes in the background
- **THEN** no OS notification is sent, and the toast waits for the focus all the same
  [@test] src/lib/os-notification.test.ts (`sends no OS notification of a kind Settings turned off; the toast waits for the focus all the same`)

### Requirement: New requests: The permission is asked on the first notification

The permission MUST be asked on the first notification, never at start. A notification MUST have no click action.

#### Scenario: The first notification
- **WHEN** the first OS notification is to be sent
- **THEN** the permission is asked then, never at start
- **AND** the notification has no click action

### Requirement: New requests: Each new request is an Activity entry

Each new request MUST be an Activity entry "dave requested your review on #104" / its title, with "Open Pull Request"
([Activity](../activity/spec.md)).

#### Scenario: The entry of a new request
- **WHEN** a new request comes
- **THEN** Activity gets the entry "dave requested your review on #104" / its title, with "Open Pull Request"

### Requirement: New requests: A new request is unread until it is opened

A new request MUST be unread (a `bg-primary` dot) until it is opened, in the app or on GitHub.

#### Scenario: The user opens a new request
- **GIVEN** a new request shows its `bg-primary` dot
- **WHEN** the user opens it, in the app or on GitHub
- **THEN** it is read and the dot goes

### Requirement: Approval news: Each poll lists the own pull requests while approvals are missing

While the last answer of a subject waits for approvals (`missing`, or Patronus can't judge them while GitHub waits
for a reviewer: `domain_policy::readiness::awaits_approval`), each poll of the review requests of its session (every
five minutes, later after failures) MUST list the user's pull requests too (the refresh button's reason): they bring
the latest reviews, no other query.

#### Scenario: A subject waits for approvals
- **GIVEN** the last answer of a subject is `missing`, or Patronus can't judge the approvals while GitHub waits for a reviewer
- **WHEN** a poll of the review requests of its session comes
- **THEN** it lists the user's pull requests too, with the reason of the refresh button, and no other query
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_poll_lists_the_pull_requests_while_a_subject_waits_for_approvals`)
  [@test] crates/domain-policy/src/readiness_test.rs (`a_subject_awaits_approvals_while_patronus_misses_them_or_github_waits_for_a_reviewer`)

### Requirement: Approval news: A new review asks the readiness once

A new review MUST change the input of the readiness, which asks Patronus once.

#### Scenario: An approval arrives with a poll
- **WHEN** a poll brings a new review of the pull request of a waiting subject
- **THEN** the readiness asks Patronus once with it
  [@test] tests/e2e/next-step/next-step.spec.ts (`should tell the approval that arrives with a poll, with Safe Push of the branch first`)

### Requirement: Approval news: An approval that makes the pull request ready is news

An approval MUST be news when the readiness of a branch goes from `missing` to `approved` (or `notRequired`) for the
same tip and the same pull request with other reviews (`approval_news`): a new review, not a new commit of the user,
nor a recheck of a kept answer; an unavailable answer between the two MUST hide nothing (the last answer Patronus gave
counts).

#### Scenario: The readiness goes from missing to approved for the same tip
- **WHEN** the readiness of a branch goes from `missing` to `approved` (or `notRequired`) for the same tip and the same pull request with other reviews
- **THEN** the approval is news
- **AND** an unavailable answer between the two hides nothing
  [@test] crates/domain-policy/src/news_test.rs (`tells_the_approvals_that_made_a_waiting_pull_request_ready_for_the_same_tip`)
  [@test] src/lib/approval-news.test.ts (`tells the approvals that made a waiting pull request ready, for the same tip`)

#### Scenario: The same reviews, or a readiness still missing
- **WHEN** the readiness is asked again with the same reviews (a recheck of a kept answer), or stays `missing`
- **THEN** there is no news
  [@test] crates/domain-policy/src/news_test.rs (`is_no_news_for_another_tip_a_start_through_the_api_the_same_reviews_or_a_readiness_still_missing`)

### Requirement: Approval news: An approval with a new tip or at the first answer is no news

An approval that arrives together with a new tip (a sync after a push) MUST NOT be told: the row shows it. The first
answer of a session MUST be no news: an approval made while the app was closed brings none, the row shows it.

#### Scenario: An approval together with a new tip
- **WHEN** an approval arrives together with a new tip (a sync after a push)
- **THEN** it is not told, and the row shows it
  [@test] crates/domain-policy/src/news_test.rs (`is_no_news_for_another_tip_a_start_through_the_api_the_same_reviews_or_a_readiness_still_missing`)

#### Scenario: An approval made while the app was closed
- **WHEN** the first answer of a session is approved
- **THEN** it is no news, and the row shows it

### Requirement: Approval news: Each approval is told once, in the main window

Each approval MUST be told once (by the branch, the pull request and the approvers), decided by the session of the
repository (`domain_policy::news::approval_news`, told as its news `approvalsArrived`) and told in the main window only.

#### Scenario: The approvals of the news of the session
- **WHEN** the session tells `approvalsArrived`
- **THEN** each approval is told once, with its subject, in the main window only, and none of the robots' news is one
  [@test] crates/repository-model/src/readiness_facts_test.rs (`the_approvals_that_arrived_are_news_once`)
  [@test] src/lib/approval-news.test.ts (`takes the approvals of the session's news, each with its subject, and none of the robots'`)

### Requirement: Approval news: While Patronus can't judge, GitHub's wait is the news

While Patronus can't judge the approvals (`unknown`, `unavailable` on both answers), GitHub's wait MUST be the news:
the pull request waited for requested reviewers ([Next Step](../../branch/next-step/spec.md) row 10′), now it waits
for nobody and some of them approved.

#### Scenario: A reviewer GitHub waited for approved
- **GIVEN** Patronus answers `unknown` or `unavailable` on both answers
- **WHEN** the pull request waited for requested reviewers and now waits for nobody, some of them having approved
- **THEN** the news says "ikorennoy approved wsm-skip-wa-2", "#1078"
  [@test] crates/domain-policy/src/news_test.rs (`tells_the_approval_of_a_reviewer_github_waited_for_while_patronus_cannot_judge_the_approvals`)

### Requirement: Approval news: The news is a success toast with Safe Push first

The news MUST be a `success` toast "Ilya Korennoy approved wsm-skip-wa-2" / "#1078 · IJPL Code Platform", with
"Safe Push" first and "Open Pull Request" below the text, for 10 s.

#### Scenario: The approvers of the groups that waited
- **WHEN** an approval is news
- **THEN** the toast names every approver of the groups that waited, each once ("Ilya Korennoy and Anna Ivanova approved …"), and a group approved before is no news
  [@test] src/lib/approval-news.test.ts (`names every approver of the groups that waited, each once; a group approved before isn't news`)
  [@test] crates/domain-policy/src/news_test.rs (`names_every_approver_of_the_groups_that_waited_each_once_a_group_approved_before_is_not_news`)

#### Scenario: Patronus names nobody
- **WHEN** Patronus needs no approval any more and names nobody
- **THEN** the toast says "The code owners approved …"
  [@test] src/lib/approval-news.test.ts (`says the code owners approved when Patronus needs no approval any more and names nobody`)

### Requirement: Approval news: Safe Push of the toast starts the Safe Push of the row

"Safe Push" of the toast MUST be the start of the row's Safe Push (`requestSafePush`, the row revealed: it starts at
once or opens its popover at the row's slot), decided again at the click: a reason that came since is an `info`
toast. Its Safe Push MUST be offered only while the main window shows its repository.

#### Scenario: The user presses Safe Push
- **WHEN** the user presses "Safe Push" of the toast
- **THEN** the row is revealed and its Safe Push starts at once or opens its popover at the row's slot
- **AND** a reason that came since is an `info` toast
  [@test] tests/e2e/next-step/next-step.spec.ts (`should tell the approval that arrives with a poll, with Safe Push of the branch first`)

#### Scenario: The main window shows another repository
- **WHEN** the main window shows another repository
- **THEN** the toast offers no Safe Push

### Requirement: Approval news: A start of the subject takes the place of the toast

A start of the subject from any surface MUST take the place of the toast ("Safe Push started", the toast of the robot).

#### Scenario: The subject starts from another surface
- **GIVEN** the toast of an approval shows
- **WHEN** the Safe Push of its subject starts from any surface
- **THEN** the toast of the robot "Safe Push started" takes its place

### Requirement: Approval news: In the background an OS notification tells the approval

In the background an OS notification MUST say it with the words of the toast while "Approvals of my pull requests" is
on in Settings (`notifyApprovals`); the toast MUST show on the next focus. An Activity entry, kind `review`, MUST keep
it with "Open Pull Request".

#### Scenario: An approval in the background
- **GIVEN** no Branch Deck window has the focus and "Approvals of my pull requests" is on
- **WHEN** an approval is news
- **THEN** an OS notification says it with the words of the toast, and the toast shows on the next focus
- **AND** an Activity entry of kind `review` keeps it with "Open Pull Request"

### Requirement: Approval news: Changes requested stay the returned line of the card

Changes requested by a reviewer MUST stay the "Returned to you" line of the card (below): its own pull
requests with the review decision `changesRequested`.

#### Scenario: A reviewer requests changes
- **WHEN** a reviewer requests changes on a pull request of the user
- **THEN** the pull request stays a "Returned to you" line of the card

### Requirement: Card: Your Turn shows after the first completed sync, above Unassigned Commits

"Your Turn" MUST show in the main window after the first completed sync of the repository (never with the Welcome or
the Ready to Sync card), after the error alert, before Unassigned Commits.

#### Scenario: The place of the island
- **WHEN** the first sync of the repository completed and something waits for the user in a review
- **THEN** "Your Turn" shows after the error alert, before Unassigned Commits
- **AND** never with the Welcome or the Ready to Sync card

### Requirement: Card: It holds the requests, then the returned pull requests

The card MUST hold what waits for the user in a review: the pull requests of others that ask for the user's review,
then the user's own pull requests a reviewer handed back.

#### Scenario: A returned pull request alone
- **WHEN** no request waits, and a reviewer handed back a pull request of the user
- **THEN** the island shows, with that pull request alone
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the island for a returned pull request alone`)

### Requirement: Card: It is hidden off GitHub, without a token, and while nothing waits

The card MUST be hidden off GitHub, without a token, with one GitHub rejects, and while neither waits; without access
to the repository it MUST keep its rows with the hint.

#### Scenario: A repository off GitHub
- **WHEN** the repository is off GitHub
- **THEN** the card is hidden
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden off GitHub, and not ask again on focus`)

#### Scenario: A rejected token, nothing waits
- **WHEN** GitHub rejects the token, or neither a request nor a returned pull request waits
- **THEN** the card is hidden, and shows once something does
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should stay hidden while GitHub rejects the token or nothing waits, and show once something does`)

#### Scenario: No access to the repository
- **WHEN** the user has no access to the repository
- **THEN** the card keeps its rows with the hint

### Requirement: Card: Its arrival moves no row

Its arrival MUST move no row: GitHub answers after the first rows, so until the facts of the remote are in
([Facts without an answer](../../branch/next-step/spec.md#facts-without-an-answer), the review requests among them) the
island MUST keep the height it had once they were in the last time (`settledLayout` of the repository state,
`YourTurnSlot`).

#### Scenario: A start before GitHub answers
- **GIVEN** the facts of the remote are not in yet
- **WHEN** the main window starts
- **THEN** the island keeps the height it had once they were in the last time: its header with "Checking GitHub…" where it will be, or the island itself once it renders
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the rows and the columns where the last start left them while Your Turn, the pull requests and the approvals come back`)

#### Scenario: A repository without that height
- **WHEN** a repository has no such height (its first start)
- **THEN** no place is held

### Requirement: Card: Once the facts are in, the island takes its own height

Once the facts of the remote are in, the island MUST take its own height (a request answered since closes the place,
once), and that height MUST be kept for the next start and a switch back.

#### Scenario: The facts come in
- **WHEN** the facts of the remote come in
- **THEN** the island takes its own height, a request answered since closing the place once
- **AND** that height is kept for the next start and a switch back
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the rows and the columns where the last start left them while Your Turn, the pull requests and the approvals come back`)

### Requirement: Card: A collapsible card, collapsed per repository

The card MUST be a `CollapsibleCard`, open by default; collapsed per repository (`reviewRequestsCollapsed` of the
recent projects).

#### Scenario: The user collapses the card
- **WHEN** the user collapses the card of a repository
- **THEN** it stays collapsed for that repository, kept in `reviewRequestsCollapsed` of the recent projects

### Requirement: Card: The header holds the count, open and collapsed alike

The header MUST hold `UserCheckIcon`, "Your Turn", the count (all that wait: the count of the requests, and the
returned pull requests), open and collapsed alike, as every island keeps its count; collapsed, "oldest waiting 2 days
· 1 returned" after it ("1 returned" alone), the age in `warning` when old.

#### Scenario: The header of the open card
- **WHEN** the card is open
- **THEN** its header shows `UserCheckIcon`, "Your Turn" and the count of all that wait
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`, `should show the island for a returned pull request alone`)

#### Scenario: The header of the collapsed card
- **WHEN** the card is collapsed
- **THEN** its header keeps the count, then "oldest waiting 2 days · 1 returned" ("1 returned" alone), the age in `warning` when old
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say 50 of 63 when GitHub has more than it returns, expanded or not, and keep the count in the collapsed header`)

### Requirement: Card: The help popover and the menu of the header

The help popover after the count MUST say what is shown (direct requests only, not an answered one, then the returned
pull requests), how the ages colour, and where from; the "⋯" menu MUST have "Refresh", "Open All on GitHub" (the
search of the repository, while requests wait), "Show All Reviews" (⌘2, [Reviews](../reviews/spec.md)) and
"Notification Settings…".

#### Scenario: The menu of the header
- **WHEN** the user opens the "⋯" menu of the card
- **THEN** it has "Refresh" (when GitHub was asked on its hover: "Checked 2 minutes ago"), "Open All on GitHub" while requests wait, "Show All Reviews" and "Notification Settings…"

### Requirement: Card: A row shows the unread dot, the title and the meta

A row, 36 px, MUST show the unread dot, the title (`font-medium`, the issue key it starts with muted and regular, where
Rust split the title, `titleLead`), and the meta "#101 · carol · 2 files · +12 −3", which gives way before the title:
the title is cut only once the meta has no room left (muted line counts, as in every row); the meta is hidden below
`sm`.

#### Scenario: Someone else than the author asked
- **WHEN** someone else than the author asked for the review, as for a pull request a bot opened
- **THEN** the meta says "#830 · agent-bot · requested by dave · …": that one waits for the user
  [@test] src/lib/review-requests.test.ts (`names who asked for the review when it is not the author`)

#### Scenario: Another base than the default branch
- **WHEN** the base of the pull request is not the default branch
- **THEN** the meta ends with "· → release/253"

### Requirement: Card: A row shows the dots of the checks

A row MUST show a `destructive` dot for failing and a `warning` dot for running checks.

#### Scenario: Failing and running checks
- **WHEN** the checks of a request fail, or run
- **THEN** its row shows a `destructive` dot, or a `warning` one

### Requirement: Card: A row says how long the request waits

A row MUST say "waiting 2 days", "re-requested 3 hours ago", or, back to the user after their comment, "carol
replied 30 minutes ago" / "new commits 30 minutes ago".

#### Scenario: A request, a re-request, a request back to the user
- **WHEN** a row shows a request
- **THEN** it says "waiting 2 days", or "re-requested 3 hours ago" when the user was asked again
- **AND** back to the user after their comment, "carol replied 30 minutes ago" / "new commits 30 minutes ago"
  [@test] src/lib/review-requests.test.ts (`says how long a request waits, or when the user was asked again`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`)

### Requirement: Card: The age is warning after one working day

The age MUST be muted, `warning` after one working day (Saturdays and Sundays don't count); never `destructive`. The
age is a leaf on the shared clock.

#### Scenario: A request over a weekend
- **WHEN** a request waits over a weekend
- **THEN** the weekend is skipped: the age is `warning` only after one working day, and never `destructive`
  [@test] src/lib/review-requests.test.ts (`is old after a day of working time, the weekend skipped`)

### Requirement: Card: Three rows, then Show N More

The card MUST show three rows, then "Show N More" ("Show Less" back), "Show 1 More" too: the named exception to the cut
rule of [UX Writing](../../../../docs/contributing/ux-writing.md), a fourth row would come and go with the polls.

#### Scenario: More than three requests
- **WHEN** more than three requests wait
- **THEN** the oldest three show, the rest behind "Show N More" ("Show 1 More" for one), and "Show Less" goes back
  [@test] src/lib/review-requests.test.ts (`shows the oldest three and every unread request, an unread one in the place of a read one`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`)

### Requirement: Card: An unread request is always shown

An unread request MUST always be shown, in the place of a read one while the rows are full: a poll MUST never grow the
list or insert a row above the others.

#### Scenario: A poll brings an unread request while the rows are full
- **WHEN** a poll brings an unread request while three rows show
- **THEN** it takes the place of a read one, and the list does not grow
  [@test] src/lib/review-requests.test.ts (`shows the oldest three and every unread request, an unread one in the place of a read one`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)

### Requirement: Card: Rows fade in, only the first entrance of the card slides

Rows MUST fade in by opacity; only the first entrance of the card SHALL slide.

#### Scenario: A row comes
- **WHEN** a row comes with a poll
- **THEN** it fades in by opacity, and the card does not slide again

### Requirement: Card: With more than 50 the list says 50 of 63

GitHub returns 50: with more, the list MUST say "50 of 63" with "Open All on GitHub", expanded or not, beside "Show N
More" when there is one, so the rows add up to the count of the header.

#### Scenario: GitHub has more than it returns
- **WHEN** GitHub counts 63 requests and returns 50
- **THEN** the list says "50 of 63" with "Open All on GitHub", expanded or not
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say 50 of 63 when GitHub has more than it returns, expanded or not, and keep the count in the collapsed header`)

### Requirement: Card: A click reviews, a ⌘-click opens GitHub

A click MUST review the pull request in the review window ([Review window](#review-window)); ⌘-click (Ctrl elsewhere),
`O` and the button on hover ("Open Pull Request") MUST open it on GitHub.

#### Scenario: The button, a ⌘-click and the menu
- **WHEN** the user ⌘-clicks a row, presses `O`, or the button on hover ("Open Pull Request")
- **THEN** the pull request opens on GitHub
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open a pull request on GitHub from its button, a ⌘-click and its menu`)

### Requirement: Card: The menu of a row holds the actions of the review

The right-click, the context menu key and Shift+F10 MUST open the menu, the actions of the review
(`getReviewRequestActions`): Review (↵), Approve…, Request Changes…, Open Pull Request (O), Copy Pull Request Link,
Copy Branch Name (copies without a toast).

#### Scenario: The menu from the mouse and the keyboard
- **WHEN** the user right-clicks a row, or presses the context menu key or Shift+F10 on it
- **THEN** the menu opens with Review (↵), Approve…, Request Changes…, Open Pull Request (O), Copy Pull Request Link and Copy Branch Name
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open a pull request on GitHub from its button, a ⌘-click and its menu`, `should move with the arrows, review with Enter and open the menu from the keyboard`)

### Requirement: Card: A write of the menu opens the review window for it

A write MUST open the review window for it, since a review is on the commit the user saw: Request Changes… with its
comment form, Approve and Safe Push… with its confirmation, Approve… with the focus on Approve once the pull request
is fetched.

#### Scenario: Request Changes… of the menu
- **WHEN** the user chooses Request Changes… in the menu of a row
- **THEN** the review window opens with its comment form
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)

#### Scenario: Approve and Safe Push… of the menu
- **WHEN** the user chooses Approve and Safe Push… in the menu of a row
- **THEN** the review window opens with its confirmation
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should not Safe Push a pull request that got commits since the user reviewed it`)

#### Scenario: Approve… of the menu
- **WHEN** the user chooses Approve… in the menu of a row
- **THEN** the review window opens with the focus on Approve once the pull request is fetched

### Requirement: Card: The actions of a row are those Rust decides

The actions MUST be those Rust decides of the pull request (`pull:<number>` of `RepositoryView.actions`,
[Reviews](../reviews/spec.md#the-actions-of-a-pull-request)).

#### Scenario: An approved head, another base
- **WHEN** the user approved the head GitHub shows (or the app submitted the approval)
- **THEN** Approve… is disabled with "You approved <hash>"
- **AND** Approve and Safe Push… is offered where Patronus merges, and disabled for another base than its target

### Requirement: Card: The keys of the rows

↑/↓ MUST move the focus between the rows (one tab stop), Enter reviews, `D` reviews its changes (the window opens in
Changes once the pull request is fetched, as D in the review shows them); the main window keeps the focus on the row.
Space MUST do nothing: it is Quick Look elsewhere, and a pull request has none.

#### Scenario: The arrows and Enter
- **WHEN** the user presses ↑/↓ and Enter on the rows
- **THEN** the focus moves between the rows, one tab stop, and Enter reviews the focused one
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should move with the arrows, review with Enter and open the menu from the keyboard`)

#### Scenario: D on a row
- **WHEN** the user presses `D` on a row
- **THEN** the window opens in Changes once the pull request is fetched, and the main window keeps the focus on the row
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review the changes of a row with D`)

#### Scenario: Space on a row
- **WHEN** the user presses Space on a row
- **THEN** nothing happens

### Requirement: Card: The own pull requests a reviewer handed back are rows after the requests

The user's own open pull requests with changes requested (`collectReturnedToYou`, in the table order; failing checks
are the chip's and the landing overview's) MUST be rows after the requests, 36 px as theirs: the node of the branch in
the place of the unread dot (its hover the branch row's), the title (its issue key muted), the meta and the branch,
muted.

#### Scenario: The pull requests handed back
- **WHEN** reviewers handed back open pull requests of the user
- **THEN** only those are rows, in the table order
  [@test] src/lib/outgoing-attention.test.ts (`names only the open pull requests a reviewer handed back, in the table order`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the island for a returned pull request alone`)

#### Scenario: The meta names the reviewers
- **WHEN** a returned row shows
- **THEN** its meta says "#760 · changes requested by alice": the reviewers whose latest review asked for changes; it gives way before the title
  [@test] src/lib/outgoing-attention.test.ts (`names the reviewers who asked for changes`)

### Requirement: Card: A returned row shows its branch and reviews it with D

A click and Enter on a returned row MUST show the branch row, where the changes are made; `D` reviews its branch in the
review window of the branch ([Branch Review](../../branch/branch-review/spec.md)); ⌘-click, `O` and the button on hover
open the pull request. ↑/↓ move between them.

#### Scenario: A click, D and the menu of a returned row
- **WHEN** the user clicks a returned row, or presses Enter or `D` on it
- **THEN** a click and Enter show the branch row, and `D` reviews the branch in the review window of the branch
- **AND** the menu has Review Changes (D, disabled with the reason of a branch it can't review), Open Pull Request (O) and Copy Pull Request Link
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list the own pull requests a reviewer handed back after the requests, show the branch and review it`)

### Requirement: Card: No footer while GitHub answers

While GitHub answers there MUST be no footer: the island is its rows, Refresh and when it was checked are in "⋯".

#### Scenario: Refresh from the menu
- **WHEN** the user chooses Refresh in "⋯"
- **THEN** the session is asked, the card refreshing until its answer is in, and no footer shows while GitHub answers
  [@test] src/stores/review-requests.test.ts (`asks the session on the refresh button, refreshing until its answer is in`)

### Requirement: Card: A failed refresh keeps the rows with the reason in a footer

A failed refresh MUST keep the rows, and a footer MUST say why and how old the rows are, with Refresh
(`ReviewStatusLine`): "GitHub: offline · checked 5 minutes ago". The card MUST never turn into an error.

#### Scenario: A refresh fails
- **WHEN** a refresh fails
- **THEN** the rows stay, and the footer says "GitHub: offline · checked 5 minutes ago" with Refresh
- **AND** the card does not turn into an error
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_failure_keeps_the_rows_and_a_rejected_token_drops_them`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should refresh from the menu, and keep the rows when a refresh fails, with the reason in the footer`)

### Requirement: Badge: The Dock badge counts the review requests

The Dock badge MUST add the review requests to what needs the user
([Robots in the Dock](../robots-indicator/spec.md#dock)), while "Count review requests" is on; Windows has no badge.

#### Scenario: The sum of the sources of the badge
- **WHEN** review requests wait beside the other sources of the badge
- **THEN** the badge shows their sum, set only on a change, none at 0
  [@test] src/lib/dock.test.ts (`shows the sum of its sources, set only on a change, none at 0`)

#### Scenario: Behind its switch
- **WHEN** "Count review requests" is off
- **THEN** the badge does not count the review requests
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the review requests, behind its switch`)

#### Scenario: Windows
- **WHEN** the app runs on Windows
- **THEN** there is no badge

### Requirement: Review window: A pull request is reviewed only in the review window

The review of a pull request MUST be only in the review window
([Review Window](../../review/review-window/spec.md#one-window), the subject `kind: "pullRequest"`, its part a chunk of
its own the page loads for it; [Title Bar](../title-bar/spec.md) for how it opens): there is no review dialog, no Open
in Window, no `W` and no ⇧-click.

#### Scenario: The native title of the window
- **WHEN** the review window shows a pull request, also when it is reused or ↑/↓ shows another request
- **THEN** the native title says "Review #101 <title>", as "Review login" of a branch and "Review a1b2c3d <subject>" of a commit
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### Requirement: Review window: A click, Enter, D, Review and a write open the window

A click on a row, Enter, `D`, the toast's Review and a write of the row's menu MUST open the review window. Another
request, or an open with an intent (a write of the menu, `D`), MUST start the window over; a plain open of the request
it was opened on MUST only bring it to the front, where the review is (also after ↑/↓ showed another one: that request
again).

#### Scenario: A plain open of the request the window was opened on
- **GIVEN** the window was opened on a request
- **WHEN** the user opens that request again plainly, also after ↑/↓ showed another one
- **THEN** the window comes to the front with its review, on that request again
  [@test] src/lib/review-reuse.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`)

#### Scenario: Another request, or an open with an intent
- **WHEN** the user opens another request, or opens with an intent (a write of the menu, `D`)
- **THEN** the window starts over
  [@test] src/lib/review-reuse.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`)

### Requirement: Review window: Back goes to the review shown before and fetches it again

Back (⌘[) MUST go to the review the window showed before, and back to a pull request MUST fetch it again (the window
stops the fetch of a pull request it leaves, and drops its range).

#### Scenario: Back to a pull request after the review of a branch
- **GIVEN** the window showed a pull request, then the review of a branch
- **WHEN** the user presses ⌘[
- **THEN** the window goes back to the pull request and fetches it again
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should go back to the pull request with ⌘[ after the review of a branch in the same window, and fetch it again`)

### Requirement: Review window: The title bar of the window

The window MUST open at once with what the search returned. Its title bar ([Title Bar](../title-bar/spec.md)) MUST
hold ↑ ↓ (in the tooltips) and the window title on the left; GitHub (`O`), "⋯" (Copy Pull Request Link, Copy Branch
Name) and the decisions ([Reviews](#reviews)) on the right, Approve last, in the corner.

#### Scenario: The window title
- **WHEN** the window opens from Your Turn, or from [Reviews](../reviews/spec.md), whose rows it walks
- **THEN** the window title says "Your Turn › 2 of 5", or "Reviews › 3 of 14"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

#### Scenario: The decisions on the right
- **WHEN** the window shows a pull request
- **THEN** GitHub, "⋯" and the decisions are on the right of its title bar, Approve last, in the corner
  [@test] tests/e2e/repository/title-bar.spec.ts (`should hold the decisions of a pull request review on its one line, and its diff view at the end of the tabs`)

### Requirement: Review window: A narrow title bar keeps one line

In a title bar narrower than 48rem GitHub MUST be its icon; narrower than 56rem the decisions MUST drop their keys and
Request Changes… MUST be its icon, named on hover; one 40px line down to the window's minimum width.

#### Scenario: A narrow window
- **WHEN** the title bar is narrower than 56rem, or 48rem
- **THEN** the decisions drop their keys and Request Changes… is its icon, named on hover; below 48rem GitHub is its icon too
- **AND** the title bar stays one line down to the window's minimum width
  [@test] tests/e2e/repository/title-bar.spec.ts (`should hold the decisions of a pull request review on its one line, and its diff view at the end of the tabs`)

### Requirement: Review window: The island names who asks and the chips of the pull request

Below the title bar, in one island, the window MUST say who asks and for how long, then the title (`text-base
font-medium`, as the subject of a commit preview), then the chips of the pull request.

#### Scenario: Who asks and for how long
- **WHEN** the window shows a request
- **THEN** it shows an initials avatar, "dave asks for your review of carol's pull request" (the author named when someone else asked) · "waiting 2 days" (`warning` when old, the clock leaf of the card)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

#### Scenario: The chips
- **WHEN** the window shows a request
- **THEN** the chips are `#101` (opens GitHub), `master ← alice/feature` (copies the branch), "2 files +12 −3", "● checks failing" (`StatusChipDot`: `destructive`, `warning` running, `success` passed), GitHub's decision ("approved", "changes requested"), and "approved by you" / "changes requested by you"

### Requirement: Review window: The arrows show the previous or next request

↑/↓ MUST show the previous or next request; the window MUST ask the main window to mark it read in Your Turn
(`review/run`, the item `read` of `pull:<number>`), and its session keeps its refs (`Demand::review_windows`).

#### Scenario: ↓ shows the next request
- **WHEN** the user presses ↓ in the window
- **THEN** it shows the next request, which the main window marks read in Your Turn
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review a commit of the pull request in the changes, step through the commits, and show the next request with ↓`)

### Requirement: Review window: The tabs Overview and Changes

Below them the window MUST have the tabs Overview and Changes (`D`, with the file count) on the left; while the
changes show, the sidebar toggle and the Split/Unified/Inline toggle with Whitespace at the end of the tabs, in the
header of the block they switch.

#### Scenario: The tabs while the changes show
- **WHEN** the changes show
- **THEN** the sidebar toggle and the Split/Unified/Inline toggle with Whitespace stand at the end of the tabs
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### Requirement: Review window: The overview is the cover of the pull request

The overview, in an island of its own, MUST be the cover of the pull request: its description whole (below), as wide
as it holds, the measure of reading at most (a body at `max-w-prose`), the rail right after it, the two in the middle
of the island; never a second copy of the document the changes tell above their diffs.

#### Scenario: The overview and the document of the changes
- **WHEN** the window shows the overview
- **THEN** it holds the description whole and the rail, not the document the changes tell above their diffs ([Review Tour](../../branch/review-tour/spec.md#in-the-review-window), `ReviewPrologue`), whose map, What Changes, steps and Possible Breakage stand there alone
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### Requirement: Review window: The rail shows the shape of the plan

Once the plan of all changes is read and has a shape ([Review Tour → The Map](../../branch/review-tour/spec.md#the-map)),
the rail MUST show "Shape" with the count of the files and lines of the change, then the rows of the map, the one view
of them (`ShapeRows`); a click on a row reviews all changes at its first file, as the same row of the map there shows
it.

#### Scenario: A narrow rail
- **WHEN** the rail is narrower than 24rem
- **THEN** the rows have no bars, and the badge of a module the change adds or deletes leads the muted line under its row in its tone, its label in that tone, so a label and its counts keep their line
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list the shape of the plan in the rail of the overview, a module showing its first file in the changes`)

#### Scenario: A click on a module
- **WHEN** the user clicks a row of the shape
- **THEN** the window reviews all changes at its first file
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list the shape of the plan in the rail of the overview, a module showing its first file in the changes`)

### Requirement: Review window: Without a shape the rail lists the files

Else (the plan off, being read, or a change too small for a shape) the rail MUST show "Files 4" with the muted sum of
the line counts, then the files of the range (status letter, name, directory, `+a −d`; a click reviews all changes at
that file, the right click and ⌘C copy its path, [Commit Files](../../commit/commit-files/spec.md#the-actions-on-a-file)).

#### Scenario: A name before its directory
- **WHEN** the rail lists a file whose path does not fit
- **THEN** the name comes first and stays whole while the directory, right after it, gives way from its start down to its last folder, and wraps out of sight below the name when even that does not fit, as every list of changed files names a file (`FilePathLabel`); its path in its hover
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list the files of the pull request in the rail without a plan, a name whole before its directory, which gives way`)

### Requirement: Review window: The rail lists the commits

The rail MUST list "Commits 4", newest first as in the list of commits of the changes, each the one commit row of a
sidebar ([Commit Row → Commit card](../../commit/commit-row/spec.md#commit-card): its title as the changes name it, its
hash in its card, the mark of its card; a click and Enter review the changes at that commit).

#### Scenario: A commit in the rail
- **WHEN** the rail lists the commits of the pull request
- **THEN** each is named alike in the rail, the commit column and the tree, opening its card by the hover of its subject and by I
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should name a commit alike in the rail, the commit column and the tree, each opening its card by the hover of its subject and by I`)

### Requirement: Review window: Below md the rail goes under the description

Below `md` the rail MUST go under the description.

#### Scenario: A narrow window
- **WHEN** the window is narrower than `md`
- **THEN** the rail goes under the description

### Requirement: Review window: The key hints below the islands

The key hints below the islands MUST be ↑↓ Previous/next, Esc Close; Space scrolls.

#### Scenario: The hints of the overview
- **WHEN** the window shows a pull request
- **THEN** the key hints below the islands say ↑↓ Previous/next, Esc Close
- **AND** Space scrolls

### Requirement: Review window: The changes open on the summary of the description

The document of the changes MUST open, once the repository turns the plan on, on the description as its lead, no
second headline under the title of the island: its summary alone (`CommitBody.summary`, below), then "Description",
muted, which shows the overview at the description.

#### Scenario: The lead of the changes
- **WHEN** the window shows the changes with the plan on
- **THEN** they open on the summary of the description, then "Description", which shows it whole in the overview
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should lead the changes of the pull request with the summary of its description, whose Description shows it whole in the overview`)
  [@test] src/presenters/commit-body.test.ts (`leads with the block Rust names the summary, its code kept, and says more of a description beside it`)

#### Scenario: A summary alone, no summary, no description
- **WHEN** the description is its summary alone, has no summary, or is empty
- **THEN** a description of its summary alone has no "Description", one of no summary the link alone, an empty one "No description"
  [@test] src/presenters/commit-body.test.ts (`says no more of a summary under its heading alone, more of one with trailers`, `leads with nothing of a body without a summary, which the overview shows whole`)

### Requirement: Review window: A link of the document reviews all changes at its place

A link of the document MUST review all changes at its place: a code name or an entry at its declaration, its file
selected and its name selected in the diffs, a use left outside opening its file over the changes, a step at its
opening.

#### Scenario: A link of a breakage
- **WHEN** the user follows a breakage link in the changes of a pull request
- **THEN** the review shows the declaration, or a use outside the changes over them
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show the place of a breakage link in the changes of a pull request: the declaration, a use outside over the changes`)

### Requirement: Review window: The description is the pull request as its author wrote it

The description MUST be GitHub's `body` (GitHub Markdown), never its plain text, read by the grammar of a commit body
([Commit Row → Message body](../../commit/commit-row/spec.md#message-body)), so its headings stay headings, its lists
lists, its tables tables (`role=table`) and its code code; the hints of its template (HTML comments) are no text, an
image reads as its alt text, a link as its text.

#### Scenario: A description with headings and tables
- **WHEN** the window shows a description with headings and tables
- **THEN** they show as headings and as tables (`role=table`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the description of the pull request as its Markdown: its headings and its tables`)
  [@test] crates/remote-facts/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)

### Requirement: Review window: A description that opens with a heading leads with its summary

A description that opens with a heading MUST have no lead, but a summary (`CommitBody.summary`: the first paragraph of
its first plain section).

#### Scenario: A description that opens with a heading
- **WHEN** the description opens with a heading
- **THEN** its summary is the first paragraph of its first plain section
  [@test] crates/sync-utils/src/commit_body_test.rs (`the_summary_is_the_lead_else_the_first_paragraph_of_a_plain_section`)

### Requirement: Review window: The fetch is an operation of the session of the window

The fetch is the window's: it MUST start when the window shows the request, as an operation of the session of its
repository (the intent `fetchReview { number }` of `execute`, `operations::review_fetch`: the head and the base as the
review requests of the session name them, its review refs held, a second fetch of the same pull request joining it).

#### Scenario: A pull request the session lists
- **WHEN** the window shows a request the session lists
- **THEN** the fetch answers the range of the head and the base the session names
  [@test] crates/operations/src/review_fetch_test.rs (`a_review_fetch_answers_the_range_of_the_pull_request_the_session_lists`)

#### Scenario: A pull request the session never listed
- **WHEN** the fetch names a pull request the session never listed
- **THEN** it is refused
  [@test] crates/operations/src/review_fetch_test.rs (`a_review_fetch_refuses_a_pull_request_the_session_never_listed`)

### Requirement: Review window: The progress of the fetch is in the rail

The rail MUST show "Fetching #101 from origin… 42%" (`tabular-nums`, "from GitHub" for a fork), its only progress;
Changes MUST be disabled, without its file count, until it is done, and after a failed fetch the alert says why.

#### Scenario: A fetch in progress
- **WHEN** the window fetches a pull request
- **THEN** the rail shows "Fetching #101 from origin… 42%", and Changes is disabled without its file count until it is done
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### Requirement: Review window: One fetch at a time per window

There MUST be one fetch at a time per window: another request stops the one in flight (`cancel_operation` of its
operation), the close of the window stops the fetches it started (the app, by its label); a fetch that reports nothing
for 120 s MUST be killed too ("git reported nothing for 120 s").

#### Scenario: Another request, or the close of the window
- **WHEN** the window shows another request, or closes, while a fetch runs
- **THEN** the fetch is stopped and its git killed
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_cancel_kills_the_fetch`)
  [@test] crates/git-executor/src/git_command_executor_test.rs (`a_cancelled_git_is_killed`)

#### Scenario: A fetch that reports nothing
- **WHEN** the git of a fetch reports nothing for 120 s
- **THEN** it is killed, "git reported nothing for 120 s", and progress resets that time
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_stalled_fetch_is_killed`)
  [@test] crates/git-executor/src/git_command_executor_test.rs (`a_git_that_reports_nothing_for_the_idle_timeout_is_killed`, `progress_resets_the_idle_timeout`)

### Requirement: Review window: A killed fetch ends every process its git started

On Windows the kill MUST end the git that the launcher of Git for Windows started too, with its ssh.

#### Scenario: The kill of a git that started others
- **WHEN** a fetch is stopped
- **THEN** every process its git started ends, on Windows the git the launcher of Git for Windows started too, with its ssh
  [@test] crates/git-executor/src/git_command_executor_test.rs (`a_stopped_git_ends_every_process_it_started`)

### Requirement: Review window: After the answer the source is the one the range names

After the answer, the source MUST be the one the range names; the journal MUST keep the range without its commits.

#### Scenario: The answer of the fetch
- **WHEN** the fetch answers
- **THEN** the source is the one the range names, and the journal keeps the range without its commits
  [@test] crates/operations/src/review_fetch_test.rs (`a_review_fetch_answers_the_range_of_the_pull_request_the_session_lists`)

### Requirement: Review window: Changes reviews the range as the range review of a branch

Changes (`D`, toggling) MUST review the range in the island, the range review of a branch
([Branch Review](../../branch/branch-review/spec.md)): the commits ("All Changes" `base..head`, then the commits newest
first) above the file tree in the sidebar, beside all diffs. The header shrinks to the title and the tabs with the diff
view.

#### Scenario: Stepping through the commits
- **WHEN** the user reviews the changes
- **THEN** J/K move between the files, N/P the changes, ⌥↑/⌥↓ the items of the list, and the footer says "All Changes · File 3 / 12" or "Commit 2 / 5 · File 1 / 3"
- **AND** a chosen commit shows its subject, author, time and message above its diffs, never a nested commit preview
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review a commit of the pull request in the changes, step through the commits, and show the next request with ↓`)

### Requirement: Review window: Since Your Review shows what changed since the review

A re-requested review whose last review is on a commit of the pull request below its head MUST offer "Since Your
Review" (`<reviewed>..head`) in the list, the reviewed commit under the line "Your review · a1b2c3d"; once there is
nothing after it (the user approved the head) or a commit left the range, the review MUST show all changes, at the same
file.

#### Scenario: A review of a commit below the head
- **WHEN** the last review of the user is on a commit of the pull request below its head
- **THEN** the list offers "Since Your Review", the reviewed commit under the line "Your review · a1b2c3d"
  [@test] src/lib/range-review.test.ts (`offers what changed since the user's review, the reviewed commit under the boundary`)

#### Scenario: A review of the head, or of no commit of the range
- **WHEN** the user approved the head, or the reviewed commit left the range
- **THEN** there is nothing since, and the review shows all changes at the same file
  [@test] src/lib/range-review.test.ts (`has nothing since a review of the head, or of no commit of the range`)

### Requirement: Review window: Escape closes a form, then the changes, then the window

Escape MUST close a form first, then go back from Changes to the overview, then close the window; while a write it
asked for runs, the window MUST stay. ⌘W closes it too.

#### Scenario: Escape step by step
- **WHEN** the user presses Escape in Changes with no form open
- **THEN** the window goes back to the overview, and the next Escape closes it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

#### Scenario: Escape while a write runs
- **WHEN** the user presses Escape while a write the window asked for runs
- **THEN** the window stays
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the window while a write runs, and tell its result in the window, which runs it`)

### Requirement: Review window: O opens the pull request on GitHub

`O` MUST open the pull request on GitHub, from the overview and from Changes.

#### Scenario: O in the overview and in Changes
- **WHEN** the user presses `O` in the overview or in Changes
- **THEN** the pull request opens on GitHub
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### Requirement: Review window: A failed fetch says why, with Retry

A failed fetch MUST be a `destructive` callout "Cannot fetch pull request #101" with the reason, Retry and Open Pull
Request. A fetch killed for a newer one ("canceled") MUST be no failure.

#### Scenario: GitHub asks for credentials
- **WHEN** the fetch fails as GitHub asks for credentials
- **THEN** the reason says "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)"
  [@test] crates/sync-core/src/review_fetch_test.rs (`refused_github_credentials_are_told_how_to_fix`)

#### Scenario: Another failure, and Retry
- **WHEN** the fetch fails otherwise
- **THEN** the callout says "<remote>: <git's last line>", and Retry fetches again
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say why a fetch failed and fetch again on Retry`)

#### Scenario: A fetch killed for a newer one
- **WHEN** a fetch is killed for a newer one ("canceled")
- **THEN** no failure is shown

### Requirement: Review window: A commit that could not be fetched is a warning

When the commit the pull request shows could not be fetched (`missing`), a `warning` callout MUST say "origin has
commits the pull request does not have yet; the review shows them." A source ahead of the pull request (a mirror) MUST
still show the pull request's commit; a force-pushed source MUST be `diverged`.

#### Scenario: A head that cannot be got
- **WHEN** the head of the pull request cannot be got
- **THEN** it is `missing`, and the review shows the source branch with the `warning` callout
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_head_that_cannot_be_got_shows_the_source_branch`)

#### Scenario: A mirror ahead of the pull request
- **WHEN** the source is ahead of the pull request (a mirror)
- **THEN** the review shows the pull request's commit
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_mirror_ahead_of_the_pull_request_shows_the_pull_request`)

#### Scenario: A force-pushed source
- **WHEN** the source branch was force-pushed
- **THEN** it is `diverged`
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_force_pushed_source_branch_diverged`)

### Requirement: Review window: A withdrawn request stays with a quiet line

A request withdrawn while the window shows it MUST stay, with the quiet line "No longer waiting for your review" ("No
longer in Reviews" of one opened from Reviews).

#### Scenario: The request is withdrawn meanwhile
- **GIVEN** the window shows a request
- **WHEN** the request is withdrawn
- **THEN** the review and its refs stay, with the quiet line "No longer waiting for your review"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)

### Requirement: Review window: The window reads the requests from its own session

The window MUST read the requests from its own session (the stores of its facts,
[Review Window](../../review/review-window/spec.md#one-window)): its data names the request, its place, the intent and
the list it was opened from (`PullRequestReviewWindowData`).

#### Scenario: What the window reads of its session
- **WHEN** the window shows a request
- **THEN** the rows of the list it was opened from, how the user takes part in each pull request of Reviews, the actions Rust decides of each (`pull:<number>`; the window's pull request among them while it shows it, `Demand::review_windows` of the window, also once no listing has it), the reviewer groups of the pull request it shows ([Owner chips](#owner-chips)), the baseline and what runs are those of its session

### Requirement: Review window: Approve and Request Changes run in the window

Approve and Request Changes MUST run in the window, as intents of its session (`SubmitReview`,
`crates/operations/src/review.rs`, holding the review of the pull request: one at a time, a second one waits with its
busy state, the same one joins it): their toast is in the window (its action opens the pull request), Activity records
them.

#### Scenario: A review submitted in the window
- **WHEN** the user approves in the window
- **THEN** the window runs it and tells its result there, and keeps the review once the request left the list
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve the commit the review shows, in the review too, and keep its place once it left the list`, `should keep the window while a write runs, and tell its result in the window, which runs it`)

#### Scenario: What runs for a pull request
- **WHEN** a review, then its Safe Push, runs for a pull request
- **THEN** the window says what runs from the reviews of the session
  [@test] src/stores/review-requests.test.ts (`says what runs for a pull request from the reviews of the session: the review, then its Safe Push`)

### Requirement: Review window: Approve and Safe Push runs in the main window

Approve and Safe Push MUST ask the main window (`review/run`, the item `safe-push-pull-request` of `pull:<number>` with
the whole request, an approved one having left its list): Patronus answers its comment there, which switches to the
request's repository first (`showRepository`, it waits for its sync) and watches the robot; `review/reply` ends the
wait (no timeout: Patronus may take a minute).

#### Scenario: The main window shows another repository
- **WHEN** the window asks the main window for the Safe Push of a request of another repository
- **THEN** the main window switches to the request's repository first, waits for its sync, and tells the window that asked
  [@test] src/stores/review-requests.test.ts (`tells the window that asked and switches the main window to the request's repository first`)

### Requirement: Review window: The toast of the Safe Push goes back to the window

The toast of the Safe Push MUST go back to the window (`review/feedback`, its action opens the robot or the pull
request) and the toast of the request in the main window MUST close; the toasts of one pull request are one
(`toastKeys.review`), so the start takes the place of its approval.

#### Scenario: The Safe Push starts
- **WHEN** Patronus starts the Safe Push asked from the window
- **THEN** its toast shows in the window, in the place of the toast of its approval
- **AND** the toast of the request in the main window closes
  [@test] src/stores/review-requests.test.ts (`tells the window that asked and switches the main window to the request's repository first`)

### Requirement: Review window: Open Pull Request of the window marks the request read

Open Pull Request MUST open GitHub from the window and ask the main window to mark the request read.

#### Scenario: Open Pull Request in the window
- **WHEN** the user chooses Open Pull Request in the window
- **THEN** GitHub opens and the main window marks the request read

### Requirement: Review window: The close of the window prunes its refs and its stores

The close of the window MUST stop its fetch, and the session MUST prune the refs of the request it showed (the demand
no longer names it); the stores of the files and the diffs of a range are the window's and go with it.

#### Scenario: The stores of a closed review
- **WHEN** the window closes
- **THEN** the files of its range, loaded by its id `base..head`, and its diffs are forgotten
  [@test] src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)
  [@test] src/stores/file-diffs.test.ts (`keeps the last subjects only, and forgets the subjects of a closed review under every policy`)

### Requirement: Owner chips: Changes has the owner chips of the review of a branch

Changes MUST have the owner chips of the review of a branch
([Branch Review](../../branch/branch-review/spec.md#code-owners)), in the header of the files: All; the reviewer
groups of the pull request, the user's first; the owners by the `OWNERSHIP` files of the baseline (of the base of the
pull request until the main window names its baseline), most files first; No Owner.

#### Scenario: The chips of the groups
- **WHEN** the pull request has reviewer groups
- **THEN** the user's groups come first, with "· You" (a member whose approval can count) and a `success` check once the group has its approvals ("Approved" on hover; the warning shield and "Needs approval for a Safe Push" before)
  [@test] src/lib/review-owners.test.ts (`lists the user's groups first, each with whether it has its approvals`)

### Requirement: Owner chips: A chosen chip filters the tree and the diffs

A chosen chip MUST filter the tree and diffs to its paths, and J/K/N/P MUST move through that same set; a partial step
shows its files out of the step total. A chip at 0 MUST NOT filter and says so.

#### Scenario: The user chooses a chip
- **WHEN** the user chooses a chip
- **THEN** the tree and the diffs show its files, marked by its name, and J/K/N/P move through them
  [@test] src/lib/review-owners.test.ts (`marks the files of the chosen chip by its name`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open on the user's group that needs an approval, then on the chip the user chose, which All forgets`)

#### Scenario: A chip at 0
- **WHEN** a chip has no file of the shown changes
- **THEN** it does not filter and says so

### Requirement: Owner chips: The chip stays across the scopes, its counts of the shown changes

All Changes, Since Your Review and a commit MUST keep the chip; its counts MUST be of the files of the shown changes,
of a new head of the request too, which keeps the chip.

#### Scenario: Another scope, a new head
- **WHEN** the user switches between All Changes, Since Your Review and a commit, or the request gets a new head
- **THEN** the chip stays, and its counts are of the files of the shown changes
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the chips by the files of the scope shown, Since Your Review too, and keep the chip on a new head of the request`)

### Requirement: Owner chips: The groups are those of the session

The groups MUST be the session's (`reviewGroups` of `pull:<number>`, `domain_policy::reviewer_groups`): Patronus's
readiness of the head the window fetched ([Patronus](../../branch/patronus/spec.md#the-readiness-of-a-request)). A
group that comes later MUST take its place among the chips.

#### Scenario: The groups come later
- **WHEN** the readiness of the fetched head comes after the chips show
- **THEN** its groups take their place among the chips

### Requirement: Owner chips: Without groups the chips are the owners alone

There MUST be no groups while they are not known (a new head before its fetch), where Patronus doesn't answer or the
repository is not on Space, where no approval is needed, and for a pull request into another branch: the chips are the
owners by the `OWNERSHIP` files alone.

#### Scenario: Patronus doesn't answer
- **WHEN** Patronus doesn't answer
- **THEN** the chips are the owners by the `OWNERSHIP` files alone, opening on All
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should mark by the OWNERSHIP files alone where Patronus doesn't answer, opening on All`)

### Requirement: Owner chips: A new head keeps the file on screen by its path

A new head MUST keep the file on screen by its path, not by its place in the files: until the groups of the new head
are known the chosen group is the owner of its name by the `OWNERSHIP` files, whose files may lack the file on screen.
The review of a branch keeps it the same way ([Branch Review](../../branch/branch-review/spec.md#the-stack-and-the-commits)).

#### Scenario: The files of the chip lack the file on screen
- **WHEN** the files of the chip lack the file on screen after a new head
- **THEN** the first of them stands in for it (no place of the reader: a review read again starts at the file on screen, and Approve does not count the stand-in)
- **AND** the review selects the file on screen again once the files of the chip show it, unless the reader moved
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the opened files again from zero on a new head, and say when the approval counts for no group that needs one`)

#### Scenario: A new head adds a file before the file on screen
- **WHEN** a new head adds `ui/button.css`
- **THEN** `ui/login.js` is File 3 / 3 of Design, and K goes to `ui/login.css`

### Requirement: Owner chips: Each request opens on the remembered chip, else the user's group

The window MUST open each request (each open from the main window, the same request again too, and each request ↑/↓
shows) on the chip the user last chose in a review window of the repository when it marks at least one file of the
pull request, else on the user's first group that still needs an approval and has files there, else on All, as
remembered when it opened.

#### Scenario: The chip of an open
- **WHEN** the window opens a request
- **THEN** it opens on the remembered chip where it marks files, else the user's group that still needs an approval, else All ([Branch Review](../../branch/branch-review/spec.md#code-owners): remembered for the repository, All forgets it)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open on the user's group that needs an approval, then on the chip the user chose, which All forgets`)
  [@test] src/lib/review-owners.test.ts (`opens on the remembered chip where it marks files, else the user's group that still needs an approval, else All`)

#### Scenario: A chip chosen in another review meanwhile
- **WHEN** the main window opens the same request again, a chip chosen in the review of a branch in between
- **THEN** the request opens again on the chip remembered when it opened: a chip chosen in another review meanwhile does not move it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open again on the remembered chip when the main window opens the same request again, a chip chosen in the review of a branch in between`)

### Requirement: Owner chips: Groups that come later choose again until the user chooses

While the window opened on All and the user chose nothing, the groups and the owners that come later MUST choose again
(the user's group once Patronus answers), and so MUST the user's group while the remembered chip may still come (the
owners by the `OWNERSHIP` files come after the groups, `awaitsRememberedChip`); then the first chip other than All
holds, and a chip the user chose, All too (a press of the chosen All), MUST never be moved.

#### Scenario: The groups come later
- **GIVEN** the window opened on All and the user chose nothing
- **WHEN** the groups of the request come later
- **THEN** the window moves to the user's group, unless the user chose All
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should move from All to the user's group when the groups of the request come later, unless the user chose All`)

#### Scenario: The user chose a chip
- **WHEN** the groups of the request come after the user chose a chip
- **THEN** the chip the user chose holds
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the chip the user chose when the groups of the request come later`)

#### Scenario: The remembered chip may still come
- **WHEN** the remembered chip is not among the chips yet, the owners coming after the groups
- **THEN** the window waits for it (`awaitsRememberedChip`)
  [@test] src/lib/review-owners.test.ts (`waits for a remembered chip the chips don't have yet: the owners come after the groups`)

### Requirement: Owner chips: A group and an owner of one name are one chip

A group and an owner of the `OWNERSHIP` files of one name MUST be one owner, and one chip: the group's, without the
owner chip of its name. The chips MUST be told apart by their filter, not their name (`ownerChipKey`).

#### Scenario: An owner chip held before the groups came
- **WHEN** the groups come while a chip chosen, or opened on (remembered as the group), as the owner is held
- **THEN** it becomes the group's chip: one chip, pressed, with "· You", the group's count and the group's files marked
- **AND** a press of it keeps it as the group (remembered so), and the user can always choose it again
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should make the owner chip chosen or remembered before the groups came the group of its name once they come, one chip the user can choose`)
  [@test] src/lib/review-owners.test.ts (`makes an owner chip held before the groups came the group of its name: one chip, pressed, with the group's files`)

### Requirement: Owner chips: The footer names the chosen chip

While a chip is chosen, the footer MUST add its name without duplicating its count: "All Changes · File 1 / 2 ·
Design"; the file index and total are those of the filtered files. All MUST add no owner name.

#### Scenario: A chosen chip
- **WHEN** the user chooses the chip Design
- **THEN** the footer says "All Changes · File 1 / 2 · Design", of the filtered files
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show owner-filtered file positions and say in Approve what it counts for and what the user opened`)

#### Scenario: A branch-wide group
- **WHEN** the chosen chip is a branch-wide group
- **THEN** every file stays visible, as its paths include all files, but its name remains in the footer
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep every file visible for a branch-wide group and count it in Approve`)

### Requirement: Owner chips: Approve counts the files shown while a chip narrows the review

Approve MUST still say how many of the user's group files the window has shown, including while a chip narrows the
review.

#### Scenario: A chip narrows the review
- **WHEN** a chip narrows the review
- **THEN** Approve still says how many of the user's group files the window has shown
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show owner-filtered file positions and say in Approve what it counts for and what the user opened`, `should keep every file visible for a branch-wide group and count it in Approve`)

### Requirement: Reviews: The decisions stand in the title bar, the primary last

The decisions MUST be in the title bar, in the overview and in Changes alike, the primary last: Request Changes…
(`R`, outline), Approve and Safe Push… (`M`, outline, where Patronus merges, see below), and **Approve** (`A`, the
primary).

#### Scenario: The same actions in the row menu and the review
- **WHEN** the review window shows a pull request
- **THEN** its decisions are Request Changes…, Approve and Safe Push… where Patronus merges, and Approve last, the same actions as the row menu, whose writes open the review
  [@test] src/lib/review-request-actions.test.ts (`lists the same actions in the row menu and the review, the writes of the row opening the review`)

### Requirement: Reviews: Once approved, Approve is done and Safe Push the primary

Once the user approved the shown commit, Approve MUST be "✓ Approved" (`success`, "You approved <hash>" on hover;
`A` does nothing), and Safe Push…, the next step, MUST be the primary while it can start.

#### Scenario: The user approved the shown commit
- **WHEN** the user approves the commit the review shows
- **THEN** Approve is "✓ Approved", and `A` does nothing
- **AND** Safe Push… is the primary while it can start
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve the commit the review shows, in the review too, and keep its place once it left the list`)

### Requirement: Reviews: A key of the review acts as its button

The keys MUST work in both views; a key typed into a text field MUST be no key of the review (`isTypingTarget`). A key
MUST act as its button: while the button is disabled (a write runs, the commit of another request loads) the key does
nothing.

#### Scenario: Letters typed into the comment
- **WHEN** the user types the comment of Request Changes…
- **THEN** its letters are no keys of the review
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)

#### Scenario: The key of a disabled button
- **WHEN** a write runs, or the commit of another request loads, and the user presses the key of a decision
- **THEN** the key does nothing, as its button is disabled

### Requirement: Reviews: A disabled decision keeps the focus

A disabled decision MUST keep the focus (`aria-disabled`): its form gives the focus back to it as it closes, also
while its write ends, else the focus fell to the page and the keys of the review stopped until a click.

#### Scenario: The form of a decision closes while its write ends
- **WHEN** the form of a decision closes while its write ends
- **THEN** the focus goes back to its button, disabled, and the keys of the review keep working

### Requirement: Reviews: A review is on the commit the review shows

A review MUST be on the commit the review shows, the head of the fetched range (`commit_id`), also when the pull
request got commits since. Until it is fetched, after a failed fetch, and when the commit the pull request shows could
not be fetched (`missing`), Approve and Request Changes MUST be disabled with the reason.

#### Scenario: The commit the user saw is not there
- **WHEN** the pull request is not fetched yet, its fetch failed, or its commit is `missing`
- **THEN** Approve and Request Changes are disabled with the reason
  [@test] src/lib/review-request-actions.test.ts (`disables the writes of the review until the commit the user saw is there, an approval of the same commit, and an own pull request`)

### Requirement: Reviews: New commits of a poll can be approved

An approval of the same commit again MUST be disabled ("You approved <hash>"); a poll that brings new commits MUST
fetch them, the header says "#101 got new commits while it was open: the review shows <hash> now", and they can be
approved.

#### Scenario: The same commit, then new commits
- **WHEN** the user approved the shown commit and a poll brings new commits
- **THEN** Approve of the same commit is disabled with "You approved <hash>"
- **AND** the new commits are fetched, the header says "#101 got new commits while it was open: the review shows <hash> now", and they can be approved
  [@test] src/lib/review-request-actions.test.ts (`disables the writes of the review until the commit the user saw is there, an approval of the same commit, and an own pull request`)

### Requirement: Reviews: Approve says which groups it counts for

Approve MUST say what it counts for, in its description (on hover, after the reason it is disabled) and in the
confirmation of Approve and Safe Push…: "Counts for Design · You opened 1 of its 2 files" ("Design and Air Plugin
· … of their 4 files"), the user's groups that still need an approval (`yours`, not `approved`), whatever chip is
chosen.

#### Scenario: The groups of the user that still need an approval
- **WHEN** the user's groups still need an approval
- **THEN** Approve names them and how many of their files the window showed
- **AND** it leaves out the groups of others and the approved ones
  [@test] src/presenters/reviews.test.ts (`names the user's groups that still need an approval and how many of their files the window showed`, `leaves out the groups of others and the approved ones`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show owner-filtered file positions and say in Approve what it counts for and what the user opened`)

### Requirement: Reviews: Approve counts the files the window showed for the head

Approve MUST count how many of their files among all changes the window showed (the file on screen, as the footer
counts it; none while the overview of the plan is at the top or the plan is still read) for the head it shows; a
branch-wide group's files are all of them.

#### Scenario: The overview of the plan at the top
- **WHEN** the overview of the plan is at the top of the changes
- **THEN** Approve counts no file, and the first file J shows counts
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count no file for Approve while the overview of the plan is at the top of the changes, and the first file J shows`)

#### Scenario: A branch-wide group
- **WHEN** the user's group is branch-wide
- **THEN** its files are all of them
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep every file visible for a branch-wide group and count it in Approve`)

### Requirement: Reviews: Approve says when it counts for no group, or nothing

Approve MUST say "Counts for no group that still needs an approval" when the user, known by the login of the sign-in,
is in none of them; just "Counts for Design" when nobody tells their files; nothing without groups (also while a new
head is fetched), while the sign-in names no login (nobody knows whose the groups are: the token of `gh` or the
environment), and once the shown commit is approved.

#### Scenario: The user is in no group that needs an approval
- **WHEN** the user, known by the login of the sign-in, is in none of the groups that still need an approval
- **THEN** Approve says "Counts for no group that still needs an approval"
  [@test] src/presenters/reviews.test.ts (`says when the approval counts for no group that still needs one`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the opened files again from zero on a new head, and say when the approval counts for no group that needs one`)

#### Scenario: Nobody tells their files, no groups
- **WHEN** nobody tells the files of the groups, or there are no groups
- **THEN** Approve says just "Counts for Design", or nothing
  [@test] src/presenters/reviews.test.ts (`names the groups alone when their files are unknown or none here, and says nothing without groups`)

#### Scenario: The sign-in names no login
- **WHEN** the sign-in names no login (the token of `gh` or the environment)
- **THEN** Approve says nothing of what it counts for, rather than no group
  [@test] src/presenters/reviews.test.ts (`says nothing while the sign-in names no login, rather than no group`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say nothing of what Approve counts for while the GitHub sign-in names no login`)

### Requirement: Reviews: The window keeps the opened files of each request

The window MUST keep the opened files of each request it showed for as long as it is open: ↓ to another request and ↑
back keeps them, and only a new head of a request MUST count its opened files from zero.

#### Scenario: A new head of the request
- **WHEN** the request gets a new head
- **THEN** its opened files count from zero
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the opened files again from zero on a new head, and say when the approval counts for no group that needs one`)

#### Scenario: Another request and back
- **WHEN** the user presses ↓ to another request and ↑ back
- **THEN** the opened files of the request are kept

### Requirement: Reviews: What Approve counts for never disables it

What Approve counts for is information only: Approve MUST never be disabled by it.

#### Scenario: The description of Approve
- **WHEN** Approve says what it counts for
- **THEN** it says it after the reason it waits, never disabling it, and nothing once approved or in the row menu
  [@test] src/lib/review-request-actions.test.ts (`says in Approve what it counts for after the reason it waits, never disabling it, and nothing once approved or in the row menu`)

### Requirement: Reviews: Approve is one POST of the review of the commit

Approve MUST be one `POST repos/<owner>/<name>/pulls/<n>/reviews` with `{"event": "APPROVE", "commit_id": <id>}`
(`SubmitReview`, its effect `submitReview`, the token of [Pull Requests](../../branch/pull-requests/spec.md#the-token)),
in the repository the requests were read from; anything but an object id MUST be refused.

#### Scenario: The approval of the reviewed commit
- **WHEN** the user approves
- **THEN** one `POST` approves the commit the user reviewed
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`approves_the_commit_the_user_reviewed`)
  [@test] crates/operations/src/review_test.rs (`a_review_submits_its_verdict_then_the_safe_push_of_an_approval_comments`)

#### Scenario: No commit id
- **WHEN** the review names no object id
- **THEN** it is refused
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`refuses_a_review_without_a_commit_id_or_a_request_for_changes_without_a_comment`)

### Requirement: Reviews: Request Changes… opens a form whose comment is required

Request Changes… MUST open a form below its button with the focus in the comment, also when it opens again while it
still closes: the comment is required, ⌘↵ (Ctrl+Enter) sends it as `{"event": "REQUEST_CHANGES", "body": <comment>}`,
Escape closes the form and keeps the draft, the focus goes back to the button.

#### Scenario: A comment sent
- **WHEN** the user writes a comment and presses ⌘↵
- **THEN** the request for changes posts its comment
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_request_for_changes_posts_its_comment`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)

#### Scenario: No comment
- **WHEN** a request for changes has no comment
- **THEN** it is refused
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`refuses_a_review_without_a_commit_id_or_a_request_for_changes_without_a_comment`)

### Requirement: Reviews: The draft of the comment is the pull request's

The draft MUST be the pull request's (by its URL, in the browser storage every window shares, `review-draft.ts`): the
window showing another request, or closed, gives it back when the pull request is reviewed again; a sent comment MUST
forget it.

#### Scenario: The pull request is reviewed again
- **GIVEN** the user closed the form with a draft
- **WHEN** the window shows another request, or closes, and the pull request is reviewed again
- **THEN** the draft comes back
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)

### Requirement: Reviews: A review is a toast and an Activity entry

A review MUST be a toast "Approved #101" / "Requested changes on #101" with the title and "Open Pull Request", and an
Activity entry in the same words; the chips say "approved by you" (`success`) or "changes requested by you". GitHub
MUST be asked again at once.

#### Scenario: An approval
- **WHEN** the user approves the commit they saw
- **THEN** the toast "Approved #101" says so with "Open Pull Request", and GitHub is asked again
  [@test] src/stores/review-requests.test.ts (`approves the commit the user saw, says so with Open Pull Request, and asks GitHub again`)

### Requirement: Reviews: The reviewed request leaves the card, the review keeps its place

The reviewed request MUST leave the card, and the review MUST keep it without the "No longer waiting" line, at its
place: ↓ shows the request now there, ↑ the one before.

#### Scenario: The approved request leaves the list
- **WHEN** GitHub answers after the review
- **THEN** the request leaves the card, and the review keeps it at its place, without the "No longer waiting" line
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve the commit the review shows, in the review too, and keep its place once it left the list`)

### Requirement: Reviews: A refused review says what GitHub said

A refused review MUST be the error toast "Cannot approve pull request #101" ("Cannot request changes on pull request
#101") with what GitHub said (e.g. "User can only have one pending review per pull request"); the action stays.

#### Scenario: GitHub refuses a review
- **WHEN** GitHub refuses a review
- **THEN** the error toast says what GitHub said, and the action stays
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_refused_review_says_what_github_said`)
  [@test] src/stores/review-requests.test.ts (`requests changes with the comment, and says why GitHub refused`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say why GitHub refused a review`)

### Requirement: Safe Push of a reviewed pull request: Approve and Safe Push where Patronus guards

Where Patronus guards the repository, the title bar MUST have Approve and Safe Push… (`M`), an outline button before
Approve, the primary (the decision comes first, the merge after it), and the row's menu MUST have it after Approve….

#### Scenario: A repository Patronus guards
- **WHEN** Patronus guards the repository
- **THEN** the title bar has Approve and Safe Push… before Approve, and the row's menu has it after Approve…
  [@test] src/lib/review-request-actions.test.ts (`offers Approve and Safe Push where Rust offers it, disabled with its reason, the Safe Push alone once approved`)

### Requirement: Safe Push of a reviewed pull request: Rust decides it on the branch Patronus pushes into

Rust MUST decide it ([Reviews](../reviews/spec.md#the-actions-of-a-pull-request),
`domain_policy::actions::review_actions`) on the branch Patronus pushes into for the baseline, which the sync read
(`TableHeader.targetBranch`): enabled only for a pull request into it. The row's menu and the review MUST decide
alike, by the same actions.

#### Scenario: A pull request into the Patronus target
- **WHEN** the pull request is into the branch Patronus pushes into
- **THEN** Approve and Safe Push… is enabled, and an approval of the head leaves the Safe Push alone
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] src/lib/review-request-actions.test.ts (`offers Approve and Safe Push where Rust offers it, disabled with its reason, the Safe Push alone once approved`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve and Safe Push a pull request into the Patronus target with a plain /patronus merge, after a confirmation`)

#### Scenario: Another base
- **WHEN** the pull request is into another base
- **THEN** it is disabled with "#103 merges into release/253: Patronus would merge it there, not into master" (a stacked pull request would land in its parent)
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)

#### Scenario: Before the first sync, a baseline that is no branch of origin
- **WHEN** the sync has not read the target yet, or the baseline is no branch of origin
- **THEN** it is disabled with "Reading the branch Patronus pushes into…" before the first sync, or "Patronus pushes into a branch of origin: the baseline is none"
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)

### Requirement: Safe Push of a reviewed pull request: It opens a confirmation below its button

It MUST open a confirmation below its button: "Approves 1a2b3c4d, then comments /patronus merge on #101: Patronus
rebases alice/feature onto master, runs the checks, merges it and deletes the branch. It reports in the pull
request.", with "Approve and Safe Push" (↵, focused on each open) and Esc Cancel; the keys of the review MUST wait
while it is open.

#### Scenario: After an approval of the commit
- **WHEN** the user approved the commit
- **THEN** the button is "Safe Push…" and the text starts with "Comments"

#### Scenario: Before the approval
- **WHEN** the user has not approved the commit
- **THEN** a muted line below says what it counts for, as Approve does ("Counts for Design · You opened 2 of its 2 files")
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show owner-filtered file positions and say in Approve what it counts for and what the user opened`)

### Requirement: Safe Push of a reviewed pull request: The approval first, then a check of the head

It MUST be one operation (`SubmitReview` with `thenLand`; once the user approved that commit, without a verdict: the
Safe Push alone): the approval first, said as soon as it is in; then one `GET repos/<owner>/<name>/pulls/<n>` for its
`head.sha`.

#### Scenario: The head GitHub shows
- **WHEN** the approval is in
- **THEN** one `GET` reads the `head.sha` of the pull request
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`reads_the_head_of_a_pull_request`)

#### Scenario: Another head than the approved commit
- **WHEN** the head is other than the approved commit
- **THEN** the start is skipped (`plan_changed`): "Cannot start the Safe Push" / "#101 has new commits since you reviewed 1a2b3c4d: review 5e6f7a8b first", and nothing is commented
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should not Safe Push a pull request that got commits since the user reviewed it`)
  [@test] src/stores/review-requests.test.ts (`refuses a pull request that got commits since the review, and says the reason of the bot`)

### Requirement: Safe Push of a reviewed pull request: The comment /patronus merge asks Patronus

Then it MUST comment `/patronus merge` (without `--keep-source-branch`: the branch of someone else goes once merged,
as a merge on GitHub does) in the repository the requests were read from, and read the bot's answer as for an own
branch ([Patronus](../../branch/patronus/spec.md#pull-requests)), "Asking Patronus on pull request #101…" in the
confirmation meanwhile, on the clock of the session (`robot_start::start_by_comment`, never a timer of the page).

#### Scenario: A plain merge
- **WHEN** the head is the approved commit
- **THEN** the comment is a plain `/patronus merge`, and "Asking Patronus on pull request #101…" shows in the confirmation until the bot answers
  [@test] crates/operations/src/robot_start_test.rs (`a_reviewed_pull_request_is_merged_plainly_and_its_robot_looked_for_only_a_while_after_the_reply`)
  [@test] src/stores/review-requests.test.ts (`approves, checks the head GitHub shows and asks Patronus; its thumbs up is the start, in the toast of the approval`)

### Requirement: Safe Push of a reviewed pull request: The thumbs up is the start, its robot watched

The 👍 MUST be the start: the robot is looked for in the user's robots for 10 s more (`PATRONUS_REVIEWED_ROBOT_WAIT`),
and goes into the session; found, it is announced as any robot ("Safe Push started" with Open in Patronus) and watched
for the session (`watchRobot`). Either toast of the start MUST take the place of the toast "Approved #101".

#### Scenario: The robot is found
- **WHEN** the bot answers 👍 and the robot is found among the user's robots
- **THEN** it is announced as any robot, "Safe Push started" with Open in Patronus, in the place of the toast of the approval
- **AND** it is polled while it runs, listed in the landing overview by "#101 <title>", its merge told ("#101 <title> merged into master", [Landing Overview](../landing-overview/spec.md#overview))
  [@test] crates/operations/src/robot_start_test.rs (`a_reviewed_pull_request_is_merged_plainly_and_its_robot_looked_for_only_a_while_after_the_reply`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should watch the robot of a reviewed pull request: listed by what it tests, its merge told`)

#### Scenario: The robot is not found
- **WHEN** the robot is not found within the 10 s (Patronus may count it as the author's)
- **THEN** the toast "Safe Push started" / "#101 <title>" opens the pull request, and Activity says "Safe Push started on #101"
  [@test] src/stores/review-requests.test.ts (`approves, checks the head GitHub shows and asks Patronus; its thumbs up is the start, in the toast of the approval`)

### Requirement: Safe Push of a reviewed pull request: A failure counts only of the newest robot

The failure of a watched robot MUST be a move (and in the Dock badge) only while it is the newest robot of "#101
<title>": once the pull request is started again, the old failure is in the line of the last day of the overview.

#### Scenario: The pull request was started again
- **GIVEN** a watched robot of the pull request failed
- **WHEN** the pull request is started again
- **THEN** the old failure asks nothing, and is in the line of the last day of the overview
  [@test] src/lib/robot-overview.test.ts (`asks nothing for the failure of a watched robot once its pull request was started again: the newest robot is the one to act on`)

### Requirement: Safe Push of a reviewed pull request: A thumbs down says the reason of the bot

A 👎 MUST be "Cannot start the Safe Push" with the bot's reason; the approval stays.

#### Scenario: The bot refuses
- **WHEN** the bot answers 👎
- **THEN** the toast "Cannot start the Safe Push" says the bot's reason, and the approval stays
  [@test] src/stores/review-requests.test.ts (`refuses a pull request that got commits since the review, and says the reason of the bot`)

### Requirement: Fetch and refs: Commits that are local need no fetch

When both commits are local, nothing MUST be fetched and no ref MUST be written.

#### Scenario: The head and the base are local
- **WHEN** the head and the base of the pull request are both local
- **THEN** nothing is fetched and no ref is written
  [@test] crates/sync-core/src/review_fetch_test.rs (`commits_that_are_local_need_no_fetch_and_no_ref`)

### Requirement: Fetch and refs: One fetch with the flags of the baseline fetch

Otherwise there MUST be one fetch with the flags of the baseline fetch plus `--refmap=` (no `refs/remotes/origin/*`
moves), `--no-prune`, `--no-show-forced-updates`, `fetch.writeCommitGraph=false`; it MUST never take the repository
lock, nor touch the working tree or the index ([Git Operations](../../../../docs/design/git-operations.md)).

#### Scenario: The remote-tracking refs stay
- **WHEN** the review fetch runs
- **THEN** no `refs/remotes/origin/*` ref moves
  [@test] crates/sync-core/src/review_fetch_test.rs (`fetches_the_branch_from_the_mirror_without_touching_its_remote_tracking_refs`)

### Requirement: Fetch and refs: The source of the fetch

The fetch MUST take GitHub's `refs/pull/<n>/head` from a GitHub origin, the branch from a mirror origin, GitHub (a
remote of that repository, else its URL) for a fork or a head the mirror lacks; the base too when it is not local, so a
stale `origin/master` adds nothing to the range.

#### Scenario: A GitHub origin
- **WHEN** origin is GitHub
- **THEN** the fetch takes `refs/pull/<n>/head`
  [@test] crates/sync-core/src/review_fetch_test.rs (`fetches_the_pull_ref_from_a_github_origin`)

#### Scenario: A mirror origin
- **WHEN** origin is a mirror that has the head
- **THEN** the fetch takes the branch from the mirror
  [@test] crates/sync-core/src/review_fetch_test.rs (`fetches_the_branch_from_the_mirror_without_touching_its_remote_tracking_refs`)

#### Scenario: A mirror behind the pull request, a fork
- **WHEN** the mirror lacks the head, or the pull request comes from a fork
- **THEN** the fetch takes it from GitHub, a remote of that repository, else its URL
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_mirror_behind_the_pull_request_falls_back_to_github`, `a_fork_is_fetched_from_github`)

#### Scenario: A base that is not local
- **WHEN** the base is not local
- **THEN** it is fetched in the same fetch
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_missing_base_is_fetched_in_the_same_fetch`)

### Requirement: Fetch and refs: The head and the base land in the review refs

The head and base MUST land in `refs/branch-deck/review/<n>/{head,base}` (visible in `git log --all`). Ids and names
that could be options MUST be refused.

#### Scenario: The refs of a fetch
- **WHEN** the review fetch fetched a pull request
- **THEN** its head and base are `refs/branch-deck/review/<n>/{head,base}`
  [@test] crates/sync-core/src/review_fetch_test.rs (`fetches_the_pull_ref_from_a_github_origin`)

#### Scenario: An id or a name that could be an option
- **WHEN** an id or a name could be an option
- **THEN** the fetch refuses it
  [@test] crates/sync-core/src/review_fetch_test.rs (`refuses_ids_and_names_that_could_be_options`)

### Requirement: Fetch and refs: The range is from the merge base to the head

The range MUST be `merge-base(head, base)..head`; no merge base MUST be "The pull request has no common history with
master".

#### Scenario: No common history
- **WHEN** the head and the base have no merge base
- **THEN** the error says "The pull request has no common history with master"
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_pull_request_without_common_history_is_an_error`)

### Requirement: Fetch and refs: The files and the diffs of the range

The files of the range MUST be those of all its commits, by one `diff-tree` of the two commits, and its diffs those of
`get_file_diffs` for the range; anything but an object id MUST be refused.

#### Scenario: The files of a range and of a commit
- **WHEN** the files of a range are read
- **THEN** they are the changes of all its commits, and the files of a commit subject those of the commit
  [@test] crates/git-ops/src/commit_files_test.rs (`range_files_are_the_changes_of_all_its_commits`, `the_files_of_a_commit_subject_are_those_of_the_commit`)

#### Scenario: Not an object id
- **WHEN** the range names what is not an object id
- **THEN** it is refused
  [@test] crates/git-ops/src/commit_files_test.rs (`a_range_refuses_what_is_not_an_object_id`)

### Requirement: Fetch and refs: The session prunes the review refs of the others

The review refs MUST be those of the pull requests the review windows show: the session deletes the others at each
change of the review windows of its demand (a window shows one, another one, none, or goes; `prune_review_refs`, one
CAS transaction, on the blocking pool), the refs of an earlier start with the first one; a demand that names the same
windows, and a poll, MUST prune nothing.

#### Scenario: A change of the review windows
- **WHEN** a window shows another pull request, none, or goes
- **THEN** the session deletes the review refs of the pull requests no review window shows
  [@test] crates/repository-model/src/session_test.rs (`a_change_of_the_review_windows_prunes_the_review_refs_of_the_others`)
  [@test] crates/sync-core/src/review_fetch_test.rs (`prune_deletes_the_review_refs_not_kept`)

#### Scenario: The same windows, a poll
- **WHEN** a demand names the same windows, or a poll comes, while a request shown was withdrawn
- **THEN** nothing is pruned, and the review keeps its refs
  [@test] crates/repository-model/src/session_test.rs (`a_change_of_the_review_windows_prunes_the_review_refs_of_the_others`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)

### Requirement: Fetch and refs: A failed prune is tried again at the next change

A CAS failure MUST be tried again at the next change; across app instances it is best effort.

#### Scenario: A ref moved meanwhile
- **WHEN** a review ref moved meanwhile
- **THEN** the prune deletes nothing, and the next change tries again
  [@test] crates/sync-core/src/review_fetch_test.rs (`prune_deletes_nothing_when_a_ref_moved_meanwhile`)

### Requirement: Settings: Notification Settings… opens Settings at Notifications

"Notification Settings…" of the card MUST open the Settings window at Notifications ([Settings](../settings/spec.md));
the window MUST write the switches through the main window, which saves them.

#### Scenario: From the card menu
- **WHEN** the user chooses "Notification Settings…" in the menu of the card
- **THEN** the Settings window opens at Notifications, and the main window keeps its switches
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`)

### Requirement: Settings: The switches of the notifications and of the Dock icon

The Notifications page MUST hold the switches of the notifications and of "Dock icon" (not on Windows), all on by
default, in the app settings, synced across windows.

#### Scenario: The switches
- **WHEN** the user opens Notifications
- **THEN** it has "Review requests" (`notifyReviewRequests`), "Safe Push merges" (`notifyPatronusMerges`, the merge notification of [Patronus](../../branch/patronus/spec.md#merged)), "Safe Push failures" (`notifyPatronusFailures`, a required check of a running robot failed, or a robot failed, [Patronus](../../branch/patronus/spec.md#failing), [Failed](../../branch/patronus/spec.md#failed)) and "Approvals of my pull requests" (`notifyApprovals`, [above](#approval-news))
- **AND** "Dock icon" has "Count review requests" (`reviewBadge`) and "Count failed robots and conflicts" (`attentionBadge`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`, `should count the review requests, behind its switch`)

### Requirement: Settings: The permission of the OS to notify

The permission of the OS MUST be checked when the page shows, on each open of the window and on focus: not asked,
"Allow Notifications"; refused, a one-line `warning` callout "Notifications are off in System Settings" with "Open
System Settings"; granted, "Send Test Notification".

#### Scenario: The window gets the focus
- **WHEN** the Settings window gets the focus
- **THEN** it asks the OS for the permission again
  [@test] tests/e2e/settings/settings.spec.ts (`should ask the OS for the permission to notify again when the window gets the focus`)

#### Scenario: Not asked, refused, granted
- **WHEN** the page shows
- **THEN** it offers "Allow Notifications" while the permission is not asked
- **AND** a refused one is the callout "Notifications are off in System Settings" with "Open System Settings", a granted one "Send Test Notification"

### Requirement: Settings: The other pages of Settings

The other pages of Settings SHALL hold "Suggest branch names" on General, the branch prefix on Branches, the GitHub
token and sign-in on Accounts ([GitHub Sign-In](../github-sign-in/spec.md#settings)).

#### Scenario: The other pages
- **WHEN** the user opens General, Branches or Accounts
- **THEN** General has "Suggest branch names", Branches the branch prefix, Accounts the GitHub token and sign-in

## User Experience

- The card is a `Card` island (`CollapsibleCard`), its rows `divide-y`, the hover `bg-muted`; the focus outline of a row is
  inside it (`bd-focus-inset`), without the hover tint. The unread dot is `size-1.5 bg-primary`, the checks dots are `StatusChipDot`. The status line of a failure is
  the footer, `text-xs`.
- `UserCheckIcon` means "a review for the user" (the card, the Activity entry, the Review menu item).
- The decisions on the pull request are in the title bar, as the actions of every review window are on its object
  ([Title Bar](../title-bar/spec.md)): the window-level actions as `sm` ghost buttons with icon, label and key, then the
  decisions as `sm` buttons (`MenuItemButton size="sm"`: the icon, the label, the key last, the reason of a disabled
  action in a tooltip; `narrow` drops the key or the label in a narrow title bar), Approve the primary, last button,
  in the corner; the island below holds the pull request and the tabs, the view of the diffs at their end; the footer
  is key hints only. A key inside a primary button is `PRIMARY_KBD_CLASS_NAME` (it would vanish on the primary colour). The tabs are
  `TabsList variant="line"`. The window lies on `bg-canvas` with two `Card` islands.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/review-requests.test.ts` (ages, order, news, backoff), `src/stores/review-requests.test.ts`
  (polling, notifications, pruning), `src/lib/os-notification.test.ts`, `src/lib/dock.test.ts`, `src/stores/commit-files.test.ts`,
  `src/stores/file-diffs.test.ts`.
- `cargo nextest run`: `crates/remote-facts/src/review_requests_test.rs` (a fake GitHub),
  `crates/repository-model/src/review_request_facts_test.rs`, `crates/remote-facts/src/scheduler_test.rs` (the cadence),
  `crates/sync-core/src/review_fetch_test.rs` (local bare remotes), `crates/git-ops/src/commit_files_test.rs`, `crates/git-executor/src/git_command_executor_test.rs`,
  `crates/test-server/src/review_requests_test.rs`.
  [@test] crates/test-server/src/review_requests_test.rs (`review_routes_answer_the_fixture_without_a_network`)
- `pnpm e2e`: `tests/e2e/review-requests/review-requests.spec.ts` with the `issue_links` template.
  `POST /repositories/{id}/review-requests` sets what the session of the repository reads of GitHub
  (`availableReviewRequests` for `acme/monorepo`) and, with `fetchError`, makes the review fetch fail; `GET` returns
  `{ calls, fetchCalls, prunes }`; a poll comes when the test moves the clock of its sessions
  (`repo.advanceBackendClock`).
  Its pull requests are in the fake GitHub of the test server too: the `SubmitReview` of `execute` runs the real
  engine against it (its review, the head, the `/patronus merge` comment and the reads of the reply, on the clock of
  the session), the review held there by `holdRemoteAnswers({ hold: ["submitReview"] })`, and a test asserts the
  requests GitHub got (`gitHubRequests`: the review with its event and commit, the `/patronus merge`
  comment); `answerGitHub` scripts a refused review or another head (e2e-testing.md, Remote services).
  The review fetch runs the real operation on the template (`RewriteHost::review_fetch` of the server fills the
  commits: an empty head is the branch the request names, an empty base the baseline, the source `local`); a test holds
  its `execute` in the page for the progress (`routeReviewFetch`), and the prunes of the session are recorded (`prunes`). The Dock and
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

- [Pull Requests](../../branch/pull-requests/spec.md): the user's own pull requests, the repository and the token.
- [Commit Review](../../commit/commit-review/spec.md), [Commit Files](../../commit/commit-files/spec.md): the review and the
  range subject.
- [Robots in the Dock](../robots-indicator/spec.md#dock): the badge.
- [Activity](../activity/spec.md), [Patronus](../../branch/patronus/spec.md): the notification of a merge.
- [Git Operations](../../../../docs/design/git-operations.md): the review fetch.
