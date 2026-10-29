---
name: Pull Requests
description: Requirements for the GitHub pull request status of the virtual branches, read through the GitHub API with the token of GH_TOKEN, gh or git (also for branches pushed to Space and mirrored to GitHub, as its committed settings say) and shown as a chip in the branch row, the branch preview and the landing overview; a pull request Patronus merged reads merged; and "Create Pull Request…" in the branch menu, the branch preview and the push toast, a dialog that creates the pull request into the base of the branch through the GitHub API, with the reviewers (the code owners Patronus suggests, a search of the repository's users) and the user as its assignee, or opens GitHub's form with its text; "Request Review…" asks the suggested code owners on an open pull request.
style: plain-1
targets:
  - ../../crates/domain-types/src/pull_request.rs
  - ../../crates/operations/src/pull_request.rs
  - ../../src/model/intents.ts
  - ../../crates/github/src/client.rs
  - ../../crates/github/src/service.rs
  - ../../crates/github/src/token.rs
  - ../../crates/remote-facts/src/pull_requests.rs
  - ../../crates/sync-core/src/github_repository.rs
  - ../../src-tauri/src/services.rs
  - ../../crates/app-api/src/remote/pull_requests.rs
  - ../../crates/app-api/src/repository_state.rs
  - ../../src/stores/pull-requests.ts
  - ../../src/lib/pull-request.ts
  - ../../src/lib/check-counts.ts
  - ../../src/lib/review-commit.ts
  - ../../src/components/branch/PullRequestChip.tsx
  - ../../src/components/branch/ReviewCommitNotice.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/repository/LandingIndicator.tsx
  - ../../src/components/repository/LandingPopover.tsx
  - ../../src/components/repository/PullRequestStatusLine.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../crates/repository-model/src/pull_request_facts.rs
  - ../../crates/remote-facts/src/scheduler_github.rs
  - ../../crates/domain-policy/src/refresh.rs
  - ../../src/components/branch/StatusChip.tsx
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/WholeStackSlot.tsx
  - ../../src/lib/push-run.ts
  - ../../src/presenters/pull-request-url.ts
  - ../../src/lib/git/push-toast.tsx
  - ../../src/lib/create-pull-request.ts
  - ../../src/lib/pull-request-dialog.ts
  - ../../src/lib/pull-request-run.ts
  - ../../src/stores/create-pull-request-dialog.ts
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../src/components/branch/BranchPreviewActions.tsx
  - ../../crates/remote-facts/src/pull_request_create.rs
  - ../../crates/remote-facts/src/pull_request_text.rs
  - ../../crates/sync-core/src/issue_navigation.rs
  - ../../crates/remote-facts/src/pull_request_writes.rs
  - ../../crates/bd-remote/src/push_command.rs
  - ../../crates/bd-remote/src/request_review_command.rs
  - ../../src/hooks/useReviewerSearch.ts
  - ../../src/components/shared/ReviewerPicker.tsx
  - ../../src/presenters/review.ts
  - ../../src/components/branch/RequestReviewPopover.tsx
  - ../../src/components/branch/RequestReviewForm.tsx
  - ../../crates/domain-policy/src/pull_request.rs
  - ../../crates/domain-policy/src/review.rs
  - ../../crates/repository-model/src/derive.rs
---

# Pull Requests

Status: Implemented
Date: 2026-10-14

## Summary

Reviews happen on GitHub pull requests of the pushed virtual branches. Branch Deck shows, per branch, the pull request
the user opened for it: its number, the review decision, the checks and whether it shows the branch as it is. The
status is read through the GitHub API, with the first token the user already has: `GH_TOKEN`, `gh` or the credentials
git stores for github.com; the `gh` CLI is not needed. Opening a pull request opens the browser. A branch without one gets "Create
Pull Request…", on the remote or not: a dialog that pushes the branch and creates the pull request into its base, asks its reviewers (the code owners
Patronus suggests, and whom the user finds) and assigns it to the user; "Request Review…" asks them later, on the open
pull request. The branches may be pushed to Space, which mirrors them to GitHub: the committed Space settings name the
GitHub repository then. The app writes to GitHub in six places only: the creation of a pull request with its assignee
and reviewers (below), the review request of an open pull request (below), the `/patronus merge` comment
Patronus merges a pull request from ([Patronus](patronus.spec.md#pull-requests)), the retarget of a pull request into a
virtual branch to the baseline (before a push of its branch, or the next step's "Retarget #7 to master"), the close of the pull requests a Safe Push of a stacked branch landed
([Stacked Branches](stacked-branches.spec.md#pull-requests)), and the review the user submits
([Review Requests](../repository/review-requests.spec.md)).

## Goals

- See which branches are approved, which wait for a review and which wait for the author (changes requested, failing
  checks) without leaving the app.
- Notice that the branch changed since the commit its pull request shows, so a push is due, without a false alarm after
  a sync that only rebuilt the branch on a newer baseline.
- Open the pull request of a branch in one click.
- Open a review after a plain push without leaving the app: the pull request into its base, its text from the branch,
  the code owners whose approval Patronus needs asked to review it, the user its assignee.

## Non-goals

- Updating or merging pull requests. Approvals are final and Patronus merges; the app reads, and its one comment is
  the Patronus command ([Patronus](patronus.spec.md#pull-requests)).
- Draft pull requests, labels, milestones and team reviewers in the dialog and in Request Review…: a draft can't be merged by Patronus
  (the `draft` blocker of `domain_policy::land::decide`), and "Continue on GitHub" opens GitHub's form for the rest.
- Pull requests of forks and of branches that are not virtual branches of the user, here. The pull requests of others
  that wait for the user's review are [Review Requests](../repository/review-requests.spec.md) ("Your Turn").
- GitHub Enterprise and other hosts: only github.com.
- Polling the user's own pull requests: GitHub is asked for them after a sync, a push, a focus and a manual refresh
  only. The review requests of others are polled ([Review Requests](../repository/review-requests.spec.md#polling)).

## Requirements

### Reading pull requests

- The repository is `git config branchdeck.githubRepository` (`owner/name`; Settings → Repository, GitHub repository,
  which shows the detected one as its placeholder, [Settings](../repository/settings.spec.md#repository)), if set;
  else the one `origin` pushes to
  (`git remote get-url --push origin`), as the virtual branches are pushed to `origin`; else the GitHub mirror that the Space
  settings committed in the repository name (`mirror.url` of `.space/repository-settings.json`, read from `HEAD` with
  one `git cat-file`, never from the working tree). SSH (`git@github.com:o/r.git`, `ssh://git@github.com/o/r.git`, `ssh.github.com:443`),
  HTTPS (with or without `.git` and credentials) and `git://` URLs name a github.com repository. No repository host or
  name is built into the app, and nothing searches GitHub for a mirror.
  [@test] ../../crates/sync-core/src/github_repository_test.rs (`parses_github_remote_urls`)
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
  [@test] ../../crates/sync-core/src/github_repository_test.rs (`the_configured_github_repository_wins_over_origin`)
  [@test] ../../crates/sync-core/src/github_repository_test.rs (`reads_the_mirror_from_the_committed_space_settings`)
  [@test] ../../crates/sync-core/src/github_repository_test.rs (`a_space_mirror_must_be_on_github`)
- The app must resolve it once per repository and keep it while the repository's `config` and the worktree's
  `config.worktree` stay as they are, at most 10 minutes (the global config and the Space settings may name it too):
  a refresh of the pull requests, the review requests and the user search runs no git for it, and a change in
  Settings → Repository is read at the next one.
  [@test] ../../crates/app-api/src/repository_state_test.rs (`the_github_repository_is_kept_until_the_config_changes`)
- Otherwise a remote on another host, a local path or no `origin` must report "not on GitHub" without asking GitHub.
  [@test] ../../crates/sync-core/src/github_repository_test.rs (`rejects_remote_urls_outside_github`)
  [@test] ../../crates/sync-core/src/github_repository_test.rs (`a_remote_outside_github_has_no_github_repository`)
- One GraphQL request per refresh (`PULL_REQUESTS_QUERY`): the search `repo:<owner>/<name> is:pr author:@me
  sort:updated-desc`, first 50, with the fields `gh pr list --json` reads, and `repository(owner, name)`. Newest first,
  unlike `gh pr list`: the virtual branches under review are among the 50 most recently updated.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
- An answer that lists the pull requests names the repository it asked, its owner and name (`acme/monorepo`), for
  links to GitHub that no pull request carries; the app knows the repository only while GitHub lists its pull requests
  (`getGitHubRepository()`).
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
  [@test] ../../src/stores/pull-requests.test.ts (`names the GitHub repository while GitHub lists its pull requests`)
- A null `repository` is "no access" ("no repository acme/monorepo"), not an empty list: the token can't see it (a fine-grained token of other
  repositories, an organization that restricts OAuth apps, which answers a search without an error). A null one with a
  SAML failure, or an answer with `X-GitHub-SSO: partial-results`, is "SSO required".
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`a_repository_the_token_can_not_see_is_no_access_not_an_empty_list`)
- A token without `read:org` can't read a team's slug: an `INSUFFICIENT_SCOPES` answer is asked again once without the
  team reviewers (`PULL_REQUESTS_QUERY_WITHOUT_TEAMS`). The missing scope is remembered for the token until it changes:
  the next refreshes ask once, without the teams.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`a_token_without_read_org_is_asked_again_without_teams`)

### The token

- The token is the first of: `GH_TOKEN` (else `GITHUB_TOKEN`) of the process; `gh auth token --hostname github.com`
  when `gh` is installed (found as git is: `/opt/homebrew/bin/gh`, `/usr/local/bin/gh` on macOS, then `PATH`); the
  token of the app's own sign-in ([GitHub Sign-In](../repository/github-sign-in.spec.md)); the password git stores for
  `https://github.com/<owner>/<name>.git` (`git credential fill`, see [Git
  Operations](../../docs/design/git-operations.md#github-credentials)). `gh` comes before the app's sign-in:
  organizations that restrict OAuth apps have usually approved it, and the Branch Deck app only once an admin did.
  [@test] ../../crates/github/src/token_test.rs (`the_chain_takes_the_first_source_and_skips_rejected_tokens`)
  [@test] ../../crates/github/src/token_test.rs (`gh_gives_its_token_and_nothing_when_logged_out`)
  [@test] ../../crates/github/src/token_test.rs (`the_git_credential_of_github_is_its_password`)
- Nothing may ask the user: `gh` runs with `GH_PROMPT_DISABLED`; git with `credential.interactive=false`,
  `GIT_TERMINAL_PROMPT=0`, an empty `GIT_ASKPASS` (git would run an askpass program before it looks at the terminal
  setting), `GCM_INTERACTIVE=never` and `GCM_GUI_PROMPT=false`. Both are killed after 10 s; no answer is no token.
  [@test] ../../crates/github/src/token_test.rs (`git_asks_its_helper_for_the_repository_and_without_one_asks_nobody`)
  [@test] ../../crates/git-executor/src/git_command_executor_test.rs (`a_timed_command_is_killed_after_its_timeout`)
- The token GitHub accepted is kept per repository. A token GitHub rejects (401) is never sent again and the next one is
  tried in the same request; one that can't see the repository is skipped for it for 10 minutes. A new `gh auth login`
  takes effect without a restart, within 30 s.
  [@test] ../../crates/github/src/service_test.rs (`a_rejected_token_is_followed_by_the_next_source_and_not_sent_again`)
  [@test] ../../crates/github/src/service_test.rs (`a_listing_without_access_tries_the_next_token_and_remembers_why_all_failed`)
- A source that starts a process (`gh`, git) must run once for every request that asks while it runs, and its answer, a
  token or none, must be kept 30 s, so the first requests of every repository at start run `gh` once and a signed-out
  chain asks no process on every refresh; the app's sign-in and sign-out forget the answers at once, the answer of a
  read in flight too. No request waits for a source it does not ask: a repository whose token GitHub accepted answers
  while `git credential fill` of another waits for a keychain.
  [@test] ../../crates/github/src/token_test.rs (`five_first_requests_at_once_run_gh_once`)
  [@test] ../../crates/github/src/token_test.rs (`a_signed_out_chain_asks_its_sources_once_a_while_and_again_after_a_sign_in`)
  [@test] ../../crates/github/src/token_test.rs (`a_read_in_flight_during_a_sign_in_keeps_nothing_of_its_answer`)
  [@test] ../../crates/github/src/token_test.rs (`an_accepted_repository_answers_while_the_credential_of_another_is_read`)

### Requests

- Every request has a 5 s connect and a 30 s total timeout. A read (a GET, a GraphQL query) is asked again once after a
  transient failure (5xx, timeout, network), a write never.
  [@test] ../../crates/github/src/client_test.rs (`a_get_is_asked_again_once_after_a_server_error_and_a_post_never`)
- A GET the same token asked before is conditional (`If-None-Match`): a `304 Not Modified` does not count against the
  rate limit.
  [@test] ../../crates/github/src/client_test.rs (`a_get_escapes_its_segments_and_asks_conditionally_the_second_time`)
- Failures must be classified, never thrown:
  - no token from any source: "signed out", and GitHub is not asked;
  - 401: "not authenticated", with the source of the last token tried;
  - the primary rate limit (403/429 with `x-ratelimit-remaining: 0`, a GraphQL `RATE_LIMITED`) and a secondary one
    (`retry-after`, or "secondary rate limit" in the message): "rate limited" until the reset;
  - a 403 with `X-GitHub-SSO: required; url=…`: "SSO required" with the URL that authorizes the token;
  - another 403 or a 404: "no access", with GitHub's message;
  - anything else: an error with GitHub's message (and the messages of its `errors`), or the transport failure.
  [@test] ../../crates/github/src/client_test.rs (`a_401_is_a_rejected_token`)
  [@test] ../../crates/github/src/client_test.rs (`the_primary_rate_limit_waits_for_its_reset`)
  [@test] ../../crates/github/src/client_test.rs (`a_secondary_rate_limit_waits_for_retry_after_or_a_minute`)
  [@test] ../../crates/github/src/client_test.rs (`a_403_of_saml_sso_carries_the_authorization_url`)
  [@test] ../../crates/github/src/client_test.rs (`a_403_or_404_is_no_access_with_the_message`)
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`without_a_token_github_is_not_asked`)
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`classifies_failures`)
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`rejects_output_that_is_not_a_pull_request_list`)
- After a rate limit, a listing (the pull requests, Your Turn) must not ask GitHub with that token before the reset: the
  refresh after every sync, push and focus answers "rate limited" at once instead of extending the penalty. A write
  the user asked for (Create, Retarget, Request Review) still goes.
  [@test] ../../crates/github/src/service_test.rs (`after_a_rate_limit_no_listing_is_asked_before_its_retry_at`, `a_rate_limit_that_ended_lets_the_listing_ask_again`)

### Matching pull requests to branches

- A pull request belongs to the virtual branch whose pushed name, `{prefix}/virtual/{name}`, is its head branch, and
  only when the head repository owner is the owner of the push remote (case-insensitive). A pull request from a fork
  (`isCrossRepository`) is never matched.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`parses_and_matches_pull_requests_by_virtual_branch`)
- Of several pull requests of one branch, an open one wins over a merged one over a closed one, then the most recently
  updated.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`closed_pull_requests_prefer_the_newest_and_empty_decisions_are_none`)
- A closed pull request with the label `merged` is merged: Patronus rebases, so GitHub can't mark the pull request it
  merged as merged; it closes it and adds the label. The labels are read in the same search.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`a_pull_request_patronus_closed_with_the_merged_label_is_merged`)
- The checks of the head commit are summarized from `statusCheckRollup`:
  - a check run that is not completed, or a status context `PENDING`/`EXPECTED`, is running;
  - a completed check run concluding `FAILURE`, `TIMED_OUT`, `CANCELLED`, `ACTION_REQUIRED` or `STARTUP_FAILURE`, or a
    status context `FAILURE`/`ERROR`, failed; every other completed check passed (`SKIPPED` and `NEUTRAL` too);
  - the summary is failing with any failure, else pending with any running check, else passing; none without checks.
  The names of the first three failed checks are kept.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`pending_checks_without_failures_are_pending`)
- The reviewers are the latest submitted review of each reviewer (approved, changes requested, commented; pending and
  dismissed reviews say nothing), then the requested reviewers and teams without one (`requested`). A reviewer asked
  again after a review keeps its state and is `requested` too: a re-requested approval still counts for Patronus. The
  details say "carol approved · re-requested". The pull request names its `author` (`author { login }`). The reviewers
  and the author are the input of the readiness of the branch, so a new review asks Patronus once, and the chip keeps
  its own state while the slot waits for an approval ([Next Step](next-step.spec.md)).
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`parses_and_matches_pull_requests_by_virtual_branch`)
  [@test] ../../src/lib/pull-request.test.ts (`names the latest review, and a request after it`)

### Refreshing

- The pull requests are a fact of the session of the repository and branch prefix (`facts.pullRequests` of its view,
  `repository_model::pull_request_facts`), kept in memory: the session lists them by the rules of
  `domain_policy::refresh` (`remote_facts::scheduler`) while a window shows it, and the page only stores what its view
  says. Nothing is persisted.
- At most one listing per session is in flight. A push, a write of the app or the refresh button during a listing asks
  once more after it; a sync or a focus during a listing adds nothing.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`a_retarget_asks_the_pull_requests_again`)
- The listing also tells whether GitHub can merge each pull request (`mergeStateStatus`), its auto-merge and the merge
  methods of the repository, which the merge of the pull-request route reads
  ([Landing Routes](landing-routes.spec.md#merge)); an open pull request GitHub answered `UNKNOWN` of makes it list once
  more a moment later, as a push whose pull request still shows an old head does.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`the_merge_state_the_auto_merge_and_the_merge_methods_come_with_the_listing`)
  [@test] ../../crates/domain-policy/src/refresh_test.rs (`an_open_pull_request_whose_merge_state_github_computes_asks_the_list_again`)
- A pull request the app wrote (a retarget, a close, the reviewers it asked, one it created) is in the session at once
  (`Sessions::write_pull_request`, from the command of the write), numbered when it applies: a listing that started
  before it keeps the pull request as written, so a retarget during a listing never comes back as "Retarget #7" and the
  next push does not retarget again. A retarget, the reviewers and a creation list them once more, after the listing in
  flight, whose answer then is the truth; a close lists nothing. The command answers once the view has the write, so
  the toast of the push that follows reads it. A pull request the app created stays until GitHub's search lists it, at
  most five minutes. The retarget the user asks (row 5 of the next step, its "⋯", the Safe Push form, the push of a
  branch whose pull request goes into a virtual branch) is an intent of the engine (`Retarget`,
  `operations::pull_request`): planned over the pull requests of the session, it moves an open one into the target
  the baseline tracks (none of one into the target already; a merged one is refused), holds the base of the pull
  request (a review request of it runs beside, a second retarget joins it), and writes the new base into the session.
  [@test] ../../crates/repository-model/src/pull_request_facts_test.rs (`a_late_listing_keeps_the_retarget_the_reviewers_and_the_close_written_after_it_started`, `a_created_pull_request_stays_until_the_search_lists_it`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)
  [@test] ../../src/stores/pull-requests.test.ts (`retargets the pull request of a branch through the engine, in the repository of its listing`)
  [@test] ../../crates/operations/src/robot_test.rs (`a_retarget_moves_an_open_pull_request_into_the_target_and_nothing_else`, `a_retarget_during_a_review_request_of_the_same_pull_request_runs_beside_it`)
  [@test] ../../crates/test-server/src/github_host_test.rs (`pull_requests_are_listed_created_retargeted_and_closed_through_the_real_service`)
- A repository that answered "not on GitHub" is asked again only by the refresh button: a sync or a focus does not run
  `git remote get-url` for it again. After GitHub's rate limit nothing but the refresh button asks before its
  `retry_at`, which the GitHub service holds per token too.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`after_a_rate_limit_nothing_is_asked_before_retry_at_but_manual`)
- After every committed table of a sync, the pull requests are listed if the last listing started at least 60 s ago; on
  a focus of a window if they are older than 2 minutes (the time on the session's clock); a push of the app that pushed
  (the remote did not move) and the refresh button always. Nothing polls GitHub.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`the_pull_requests_are_listed_by_a_sync_a_focus_a_push_and_the_refresh_button`)
  [@test] ../../src/stores/pull-requests.test.ts (`asks the session for the pull requests, the refresh button showing it runs`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should refresh after a push`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should refresh from the overview`)
- GitHub can show the old head for a moment after a push: when the listing after the push still shows another head of a
  pushed branch (or whole stack), it is listed once more 5 s later on the session clock; that listing asks no further
  one.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`a_push_github_shows_on_an_old_head_is_looked_at_again_five_seconds_later_once`)
  [@test] ../../crates/repository-model/src/pull_request_facts_test.rs (`a_listing_shows_the_pushed_head_or_an_old_one`)
- A robot linked to a pull request that merged ([Patronus](patronus.spec.md#pull-requests)) asks again right away: the
  pull request is closed with the label `merged` now.
  [@test] ../../src/stores/patronus.test.ts (`keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged`)
- A failed listing is never an empty one: before any listing it is unavailable with its reason, so no pull request of a
  branch is known; after a listing, a failure GitHub recovers from (an error, the rate limit) keeps the listing, stale
  with the reason, and only the availability changes; once GitHub refuses the token no listing stands. A listing that
  answers the same pull requests changes nothing, and a patch keeps every pull request it doesn't name as the same
  object.
  [@test] ../../crates/repository-model/src/pull_request_facts_test.rs (`a_failed_listing_is_unavailable_never_empty`, `a_failed_refresh_after_a_listing_keeps_it_stale`, `a_listing_of_the_same_pull_requests_changes_nothing`)
  [@test] ../../src/stores/pull-requests.test.ts (`stores the pull requests of the view as they are, the record kept while a patch names none`)

### Chip

- The Status cell of a branch row shows the chip in its second column, after the status text
  ([Branch Row](branch-row.spec.md)): the pull request icon and `#760`. A branch without a pull request shows nothing.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show each pull request state in the status cell`)
- The hue of an open pull request follows the review decision: approved `success`, changes requested `destructive`,
  review required and no decision muted. A draft is muted with the draft icon; a merged one `info`
  with the merge icon; a closed one muted with the closed icon. A draft, merged or closed pull request shows its state,
  not the decision. The hue colours the icon only; the number is muted. The node of the row has the colour of the next
  step, so a fact gets one colour: a red node, a red icon and a red "#409" said one fact three times.
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
  running, 9 passed", "All 4 passed"; the failed ones named as a cut list, "5 failing (a, b, c and 2 more)", one
  without a name said as such, never "and 1 more": "2 failing (tests and a check without a name)"), the reviewers, the
  last update, the notice about the head (below), "Open Pull Request" and "Copy Pull Request Link" (which says
  "Copied" in place), named as in the branch menu.
  [@test] ../../src/lib/pull-request.test.ts (`lists the failing checks first`)
  [@test] ../../src/lib/check-counts.test.ts (`never counts one failed check left out: one without a name is said as such, in the counts of the checks too`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
- With the dot, the details say "Shows 3264f30f; the branch has changed since: push to update the pull request.", or,
  when the remote branch has the change already and GitHub doesn't show it yet, "Shows 3264f30f; GitHub doesn't show the
  pushed 88de421d yet." A branch only rebuilt on a newer baseline gets no dot; the details say quietly "Shows an earlier
  build of the same commits."
  [@test] ../../src/lib/pull-request.test.ts (`asks for a push when the branch changed, waits for GitHub after one, and says quietly that it shows an earlier build`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say when the branch changed since the pull request's head`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say only quietly that the pull request shows an earlier build of the same commits`)
- When someone else pushed commits to the branch of the pull request that the branch lacks (`othersBehind`, the head the
  listing showed fetched, [Outgoing](../repository/outgoing.spec.md#remote-commits-of-others)), the details never ask for
  a push, which would drop them: "Shows d708d5b8; someone else pushed commits your branch doesn't have: a push drops
  them.", with the dot.
  [@test] ../../src/lib/pull-request.test.ts (`says that a push drops the commits someone else pushed to the remote branch, never that a push is due`)
- A click on the chip opens the pull request in the browser, leaves its details as they are
  ([hover descriptions](../../docs/design/ui-theme.md)) and does not toggle the row.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
- The branch context menu has "Open Pull Request" in its history group when the branch has a pull request, also when the
  branch can't be modified, and "Copy Pull Request Link" in its copy group. "Open Pull Request" shows `O`, which opens
  the pull request on the focused row as in the branch preview ([Branch Row](branch-row.spec.md#keyboard)).
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)

### Creating a pull request

- "Create Pull Request…" opens the dialog "Create pull request" of the branch (`CreatePullRequestDialog` in the main
  window, loaded on the first open; the review window has its own): the title "Create pull request", the line
  "feature-auth → master", Title, Description, Reviewers, "Assign to me", and the footer "Esc Cancel", "Continue on
  GitHub", "Create Pull Request ⌘↵". The base is the baseline as the remote names it (`master`) for every branch,
  stacked or not ([Stacked Branches](stacked-branches.spec.md#pull-requests)): a pull request holds what the Safe Push
  of its branch lands. A stacked branch's dialog says so under the line: "Includes the commits of auth and login, which
  oauth is built on: 3 branches · 12 commits" (`formatPullRequestIncludes`; up to four branches below it are named,
  five or more are "auth, login, core and 2 more").
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
  [@test] ../../src/lib/pull-request-dialog.test.ts (`creates the pull request of a stacked branch into the baseline, with the commits of the branches it is built on`, `includes nothing more for a branch built on the baseline`)
- "Create Pull Request…" is offered to any branch without a live pull request, on the remote or not (Rust offers `create-pull-request`
  by the GitHub repository and the pull request alone): the push is the creation's first step, never a step the user
  takes or reads about. Opening the dialog pushes nothing; Create (and "Continue on GitHub") is one intent of the engine
  (`CreatePullRequest`, `operations::pull_request`; "Continue on GitHub" with `onGitHub`), whose plan pushes first what
  the push button would (`derive::action_push_of`: the branch and the ancestors with work, root first; the pull requests
  of what it pushes into a virtual branch are retargeted into the target before, D3a; the stack branch of a whole stack;
  no push toast; Activity records it), its progress in the button ("Pushing 45%"), Escape and a click outside don't
  close the dialog meanwhile. A push the user decides is never made for them: one that drops commits of the remote
  branch refuses the plan, and the dialog stays with "Cannot create pull request" / "The remote branch of feature-auth
  has 2 commits that feature-auth lacks: push it from its row, which shows them"; nothing is created. Right after its
  push GitHub may not have the branch yet (a Space mirror copies it a moment later): the creation asks again every 2 s
  for 30 s (`wait_for_mirror`), "Continue on GitHub" waits until GitHub has the branch (the effect `WaitForMirror` of its
  plan) before the page opens GitHub's form; bd's `safe-push --create-pull-request` waits the same way after its push.
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`, `should open GitHub's form of a branch not on the remote yet from its preview, after the push`, `should keep the dialog, with the reason, when the push would drop commits of the remote branch`)
  [@test] ../../crates/remote-facts/src/pull_request_create_test.rs (`right_after_a_push_the_creation_waits_for_the_mirror_to_copy_the_branch`, `the_wait_for_the_mirror_ends_with_the_reason`, `waits_until_github_has_the_branch`)
  [@test] ../../crates/operations/src/pull_request_test.rs (`a_creation_pushes_first_what_the_remote_lacks_then_opens_the_pull_request`, `continue_on_github_pushes_and_waits_until_github_has_the_branch`)
  [@test] ../../src/lib/pull-request-run.test.ts (`opens GitHub's form once the run pushed and GitHub has the branch, of the whole stack too`)
- The text starts as `bd push` and `bd safe-push --create-pull-request` write it (`get_new_pull_request`,
  `pull_request_text`): the `bd describe` description of the branch, else the text of the commits the pull request
  will show, those of the local branch the creation pushes (`<remote>/<base>..<head>`). A field the user typed in is
  not replaced by the text that arrives later.
  [@test] ../../crates/remote-facts/src/pull_request_create_test.rs (`the_text_is_the_description_else_the_commits_the_pull_request_will_show`)
  [@test] ../../crates/remote-facts/src/pull_request_text_test.rs (`the_description_wins`)
- The title of one commit is its subject. With several, a branch of one issue (its name is the key, or every subject
  carries the same key) is titled by the key and the issue's summary, `IJAI-4390 Agent CLIs render without colors`,
  never by a key cut into words; without the summary, by the key and the summary of the branch the row shows (the
  first subject, oldest first, that is no cleanup, without its key). A branch of several keys or of none is titled by
  the summary of the branch, else by its name as `gh pr create --fill` names it (`feature auth`). The summary comes
  from YouTrack only when the link of the key in `.idea/vcs.xml` is an issue page of the YouTrack signed in to, as the
  issue popover asks it, within 3 s for all the keys; so the title depends on the sign-in.
  [@test] ../../crates/remote-facts/src/pull_request_text_test.rs (`an_issue_branch_keeps_its_key_and_takes_the_issue_summary`, `without_an_issue_summary_the_title_is_the_key_and_the_branch_summary`, `a_branch_of_several_keys_names_each_key_and_is_titled_by_its_summary`)
  [@test] ../../crates/remote-facts/src/pull_request_text_test.rs (`an_issue_is_asked_from_youtrack_only_when_the_repository_links_it_there`)
  [@test] ../../crates/sync-core/src/issue_navigation.rs (`a_key_links_as_the_first_pattern_that_matches_all_of_it_names_it`)
- The body starts with each issue of the branch once, `[IJAI-4390](<its page>)` joined by ` · ` (the bare key without
  a link). One commit then gives its body; several a numbered item each, oldest first: the subject in bold, without
  the key the title carries when the subject starts with it, the body under it indented by the width of the marker
  (`10.` by 4). `fixup!`, `squash!` and `amend!` commits have no item.
  [@test] ../../crates/remote-facts/src/pull_request_text_test.rs (`an_issue_branch_keeps_its_key_and_takes_the_issue_summary`, `trailers_and_autosquash_commits_are_left_out`)
- A commit body reads on GitHub as the commit preview shows it: the blocks of the one grammar of a commit body
  (`sync_utils::commit_body`, [Commit Row](../commit/commit-row.spec.md#message-body)) written as Markdown, where a
  newline is a line break. A paragraph is a line per line it reads as (its hard-wrapped lines joined; a line after a
  hard break, a quote line and a table row each their own; a heading and a rule a paragraph of their own), a list `-`
  or the numbers from its first, its wrapped and indented lines joined, code (fenced or indented) in a fence, the
  blocks apart by a blank line. The trailers are left out, a one-word `Note:` alone is prose. Outside code spans, a
  word GitHub would read as a mention (`@Nullable`) or as an HTML tag (`<T>`, `List<String>`) goes into a code span.
  [@test] ../../crates/remote-facts/src/pull_request_text_test.rs (`each_commit_is_an_item_with_its_body_reflowed`, `trailers_and_autosquash_commits_are_left_out`, `mentions_and_angle_brackets_stay_literal`)
- A stacked branch's pull request holds the commits of the branches it is built on: its title and items come from its
  own commits, and a last section says "Built on auth, then login: their commits are in this pull request too." with
  the subjects of each ("Built on auth: its commits …" for one). The app sends the chain of the row
  (`stack_ancestors`), and one `log` splits the commits by the tips of those branches; `bd` takes them from the sync.
  [@test] ../../crates/remote-facts/src/pull_request_text_test.rs (`a_stacked_branch_is_titled_by_its_own_commits_and_names_its_ancestors`)
  [@test] ../../crates/remote-facts/src/pull_request_create_test.rs (`the_commits_up_to_the_tip_of_each_ancestor_are_the_ancestor_s`)
  [@test] ../../crates/remote-facts/src/process_budget_test.rs (`the_text_of_a_new_pull_request`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`parents_go_first_and_a_child_alone_carries_its_parents_commits`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
- GitHub takes a body of 65,536 characters: the items that don't fit are named in a last line, "_Too long for a pull
  request: the text of 2 commits is left out (…)._", as in the pull request of the whole stack.
  [@test] ../../crates/remote-facts/src/pull_request_text_test.rs (`a_body_too_long_for_github_is_cut_and_names_the_commits_left_out`)
- Reviewers: chips of GitHub logins with a search of the users of the repository a review can be asked from (GitHub's
  `assignableUsers`, from 2 characters, after a pause in the typing; an older answer never replaces a newer one); the
  user is never offered, GitHub refuses a review request of the author. Where Patronus is, "Suggested by code owners:"
  lists the suggested and available reviewers of each group whose approval is missing (the readiness of the branch
  from its one reader, `review-readiness`, [Next Step](next-step.spec.md#the-readiness-store), a start without a pull
  request, the kept answer, never a recheck; for a stacked branch the groups of the
  stack Patronus counts), and one suggested reviewer with a GitHub login per group is chosen when the dialog opens: the user sees
  the chips before creating and removes what doesn't fit. An answer that comes after the opening (asked, failed, then
  answered) chooses them too, the choice following the answer it shows, until the user changes the reviewers. A suggestion without a GitHub login (Patronus sends
  `githubUsername` behind its flag `review.precheck.github.usernames.enabled`) searches the reviewer's name; the user
  picks the person, the app never guesses. While Patronus answers the row says "Asking Patronus for the code owners…",
  a failure "Cannot read the code owners: …", a skipped check "The code owners of so many commits are named when the Safe Push starts".
  [@test] ../../src/lib/create-pull-request.test.ts (`are the suggested and available reviewers, without the user`, `start with one reviewer with a login per group, each once`, `shows the users not chosen yet, without the user`, `takes only the answer of the latest request`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should create the pull request with the reviewers it searched and the user as its assignee`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should ask the code owner Patronus names, and the row then awaits the review in the pull request`, `should offer to create the pull request the approvals count in, and no start Patronus would fail`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should offer Create Pull Request… with the code owner when approvals count only in one`)
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should preselect the code owners in Create Pull Request… from the answer that comes after the dialog opened`)
- "Assign to me" is on until the user turns it off; the choice is kept (`appSettings.createPullRequestAssignSelf`).
- ⌘↵ (Ctrl+Enter) or the button creates it (the last effect of `CreatePullRequest`), in one request of the token chain: `GET /user`
  first (a rejected token is replaced before anything is written), `POST pulls`, then `POST issues/<n>/assignees`
  and `POST pulls/<n>/requested_reviewers`. After the pull request exists nothing fails the creation: an assignee
  GitHub leaves out and a reviewer it refuses are warnings; a refused list of reviewers is asked again one by one, to
  name whom it refuses. Escape and a click outside don't close the dialog while it runs; a click outside never closes
  it (the text stays). The creation and "Continue on GitHub" are operations of the session of the repository
  (`RepositoryView.operations`, [Running Operations](../../docs/design/operations.md)): each holds the head of its
  subject, so a second submit of the same joins the first, another text (or a push of the branch) waits for it and is
  decided again (GitHub then finds the pull request it opened); a creation asked while the push of its branch sends
  waits for that push and runs on what it left, nothing more to push; the dialog waits from the click on and while the
  operation runs, the review window for its own request. So is "Request Review…": its button shows the spinner, a
  second click joins it.
  [@test] ../../crates/operations/src/pull_request_test.rs (`a_review_request_waits_for_a_retarget_of_its_pull_request_only_when_its_push_retargets_it`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should create the pull request asked for while the push of its branch sends, on what that push left`)
  [@test] ../../src/lib/pull-request-run.test.ts (`is the running operation of its row while the session runs it`)
  [@test] ../../crates/remote-facts/src/pull_request_create_test.rs (`creates_the_pull_request_assigns_the_user_and_asks_the_reviewers_except_the_author`, `without_the_assignee_and_reviewers_only_the_pull_request_is_created`, `a_refused_list_of_reviewers_is_asked_one_by_one_and_who_github_refuses_is_a_warning`, `an_assignee_github_leaves_out_or_refuses_is_a_warning_of_the_created_pull_request`, `a_rejected_token_writes_nothing`, `a_title_of_several_lines_is_refused_before_github_is_asked`)
- The creation, "Continue on GitHub" and "Request Review…" belong to the repository they were asked in (the dialog of
  the main window or of the review window, the popover): its path goes with the request, and Rust resolves the branch
  prefix of that repository for each command: the intent is planned and run in the session of that repository, so the
  push, the pull request, its reviewers, the store and Activity are of that repository also when the main window opens
  another during the push.
  [@test] ../../src/lib/pull-request-run.test.ts (`runs the intent in the repository of the dialog, and toasts the pull request GitHub opened`, `pushes first and answers whom GitHub asked and refused, also when it asked nobody`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should create the pull request asked from the review window in its repository when the main window opens another during its push`)
- The result is one toast with "Open Pull Request", recorded in Activity: "Created pull request #123" / "feature-auth
  → master · review requested from alice · assigned to you", a `warning` toast whose description names what GitHub
  refused ("cannot request review from bob: …"). The pull request is in the row at once and stays there until
  GitHub's search lists it (5 min at most), then a refresh follows.
  [@test] ../../src/lib/create-pull-request.test.ts (`says the base, who reviews and the assignee`, `is a warning that names what GitHub refused`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say whom GitHub refused, and keep the dialog when GitHub creates nothing`)
- A pull request of the branch that already exists is found and left as it is: "feature-auth already has pull
  request #7" / "The title, the reviewers and the assignee weren't applied to it". One a refresh brings while the
  dialog is open (open or merged) turns it into "feature-auth already has pull request #7." with "Open Pull Request";
  Create and "Continue on GitHub" are disabled, and their hover says why ("feature-auth already has pull request #7",
  "Pull request #7 of feature-auth is merged"): GitHub's form would open nothing. A disabled Create always says why on
  hover: "Enter a title", "The title must be one line", "Reading the commits of the branch…". The existing one is
  looked up by its head (`GET pulls?head=<owner>:<branch>&state=open`), not by the search, which lists a new pull
  request only a while later: a retry after a timeout, or the app and `bd safe-push --create-pull-request` at once,
  find the one just created.
  [@test] ../../src/presenters/pull-request-url.test.ts (`says why GitHub's form and Create can't make a new pull request: a live one; a branch not on the remote can, it is pushed first`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say why it can't create, and offer only Open Pull Request once the branch has one`)
  [@test] ../../crates/remote-facts/src/pull_request_create_test.rs (`a_pull_request_that_already_exists_is_found_and_left_as_it_is`)
  [@test] ../../crates/operations/src/pull_request_test.rs (`a_creation_again_finds_the_pull_request_the_search_does_not_list_yet`)
  [@test] ../../src/lib/pull-request-run.test.ts (`answers the one GitHub had of the head already, and throws why nothing was created`)
  [@test] ../../src/lib/create-pull-request.test.ts (`says that one which existed got nothing of the dialog`)
- A creation that fails is "Cannot create pull request" with GitHub's reason, and the dialog stays with its text. A
  branch GitHub doesn't have yet (a mirror copies a pushed branch a moment later) says so: "GitHub doesn't have
  user/virtual/feature-auth yet: a mirror copies a pushed branch a moment later; try again"; a base it doesn't have
  "GitHub doesn't have the base … : push it first".
  [@test] ../../crates/remote-facts/src/pull_request_create_test.rs (`a_branch_github_does_not_have_yet_says_so`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say whom GitHub refused, and keep the dialog when GitHub creates nothing`)
- Without a token GitHub accepts (the availability of the pull requests: signed out, rejected, SAML SSO, no access,
  rate limited) the dialog says why, with what fixes it ("Sign In…", "Authorize…", as the status line), and
  "Continue on GitHub" is its only action; it has no Reviewers (GitHub's form takes none, nobody can be searched) and
  "Assign to me" only while it knows the user's login.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open GitHub's form without a token GitHub accepts`)
- "Continue on GitHub" opens GitHub's form of a new pull request in the browser with the dialog's title, description
  (cut at 6000 characters) and the user as the assignee, which GitHub takes as query parameters; reviewers it doesn't
  (its tooltip says so). `https://github.com/<owner>/<name>/compare/<base>...<prefix>/virtual/<branch>?expand=1&title=…&body=…&assignees=…`.
  The repository is the one GitHub lists the pull requests of (`getGitHubRepository()`), else the one Patronus knows
  the branches are mirrored to (the Space settings, no token needed for a link). A branch name keeps its slashes,
  anything else that a URL path can't hold is encoded. GitHub creates nothing until the form is submitted.
  [@test] ../../src/lib/pull-request.test.ts (`opens the pull request form of the pushed branch into its base`, `fills in the title, the description and the assignee of the dialog, and cuts a long description`)
- The history group of the branch menu (the right-click and "⋯") has "Create Pull Request…" when the branch has no
  open or merged pull request and the GitHub repository is known, on the remote or not (the dialog pushes it); a branch
  with one doesn't. A closed pull request doesn't count: the branch gets a new one, and "Open Pull Request" (its hover
  "#7 is closed") still opens the closed one, beside it in the menu, the preview and the review window alike.
  [@test] ../../src/lib/push-run.test.ts (`offers a new pull request when the branch has only a closed one`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`, `should open the pull request of a pushed branch from the push toast`)
- Once nothing of the branch is left to land, as its [next step](next-step.spec.md#the-table) says (`landed`, row 1:
  every commit in the fetched baseline, a merged Safe Push landed all of it, its own robot merged it), "Create Pull
  Request…" stays in the menu, the footer of the preview and the title bar of the review window, disabled with that
  reason ("Landed in master with oauth", "Merged by Patronus"): a pull request of commits that are in the target has
  nothing to review. A branch with commits after the landed ones offers it, for them.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should disable Create Pull Request… of a branch a merged Safe Push landed, in its menu, its preview and its review window: nothing of it is left to review`)
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
  [@test] ../../src/presenters/next-step.test.ts (`row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed`)
- The toast of a push ("Pushed branch feature-auth", [Outgoing Commits](../repository/outgoing.spec.md#push)) of any branch,
  stacked or not, with stacked branches on or off, has the action "Create Pull Request…" under the same conditions as
  the menu, else "Open Pull Request" when the branch has a pull request, and stays 10 s; off GitHub it has no action.
  "Create Pull Request…" closes the toast as it opens the dialog: a toast lies above the dialogs and would cover its
  footer ([UX Writing](../../docs/contributing/ux-writing.md), an action with "…" of a toast).
  The toast reads the branch after the push: the push that created the remote branch offers its pull request. It names
  no base, every branch has the same; an open pull request into a virtual branch whose retarget failed gets "Copy gh
  pr edit" instead ([Stacked Branches](stacked-branches.spec.md#pull-requests)). A push that is a step of another action has no toast
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)).
  [@test] ../../src/lib/push-run.test.ts (`offers to create the pull request of a branch without one, into the baseline`, `opens the pull request the branch has`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`, `should open the pull request of a pushed branch from the push toast`)
- The review window of a branch ([Branch Review](branch-review.spec.md#the-create-pull-request-dialog)) has "Create
  Pull Request…" too: the dialog opens in the window, the main window sends what it shows and creates the pull request
  it asks for, or says why it can't ("Cannot create the pull request of auth"); the toast is in the window.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window it was asked in, through the main window`)
- At row 7 of the [next step](next-step.spec.md) the footer has "Safe Push Without Review…" at its left: it closes the
  dialog and opens the Safe Push popover at the row's slot, its profile one that skips the code-owner review
  ([Patronus](patronus.spec.md#safe-push-without-review)).
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`)
- The dialog, in the main window and in the review window, is wide enough for its footer in one line, "Safe Push
  Without Review…" included (`form`, 48rem); in a narrower window the footer wraps, and nothing leaves the dialog.
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should keep the pull request dialog's footer in one line inside the dialog`)
- A copied or printed `gh` command (`gh pr edit` of the toast and of the menu's "Copy Retarget Command", the
  `gh pr create … --assignee @me` of `bd push`) gets `--repo <owner>/<name>` when the GitHub repository is not the one
  `origin` pushes to (a mirror): `gh` finds no GitHub repository among the remotes of a mirrored checkout.

### Request Review

Patronus counts the approvals of the code owners in the pull request the robot starts from
([Patronus](patronus.spec.md#code-owner-review)): a pull request that asks no member of a group whose approval is
missing waits for nobody. "Request Review…" asks them, on the pull request that exists.

- `getRequestReviewItem` (`src/presenters/review.ts`) builds the item, id `request-review`, "Request Review…", for
  every surface that offers it (the row's next step, the branch menu): there only while the branch
  has an open pull request (else "Create Pull Request…" asks the reviewers); disabled with the reason while the pull
  request is a draft ("#1078 is a draft: mark it ready for review on GitHub first"), as the session reads the open pull
  request of the subject (`RepositoryView.openPullRequests`, `PullRequestView`: its number, where it goes by
  `domain_policy::pull_request::base_of`, a draft, the candidates below). Work the remote lacks by content is
  no reason: the request (the intent `RequestReview` with `pushFirst`, D5) pushes it first, the `Push` of its plan (the
  push of the push button, the stack branch of a whole stack, no push toast; one only rebuilt on a newer baseline is
  up to date, an unknown remote status pushes nothing), so the reviewers see the commits. A push that dismisses the
  approvals the pull request has (the rules of its base as the session read them, `useBranchRules` answers `dismiss`) is said in
  the popover, a consequence the user decides by Request: "The push of the new commits dismisses the approvals of
  #1078: the rules of the repository dismiss stale reviews when new commits are pushed." Rules that can't be read are
  said, never read as keeping the approvals, and are asked again by the next form; the request still goes (it counts
  no approvals): "Cannot check whether the push of the new commits keeps the approvals of #1078: GitHub answered 500."
  A refused push ("Cannot request a review on
  #1078" with its reason) asks nobody. Its hover names whom it asks: "Ask Ilya Korennoy to review #1078".
  [@test] ../../src/presenters/review.test.ts (`asks the chosen code owners on the open pull request`, `is not there without an open pull request`, `is disabled with the reason Rust gives for a draft: work the remote lacks is pushed by the request`)
  [@test] ../../crates/operations/src/pull_request_test.rs (`a_review_request_pushes_first_only_when_told_to_and_asks_the_preselected_code_owner`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
  [@test] ../../crates/repository-model/src/derive_test.rs (`the_view_reads_where_each_open_pull_request_goes_and_whom_request_review_offers`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should push the commits the remote lacks as the first step of Request Review…`)
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should say in Request Review… while Patronus names no code owners, when it fails to, and when the rules of the branch can't be read`)
- It opens `RequestReviewPopover`, mounted by the surface around its own trigger (Base UI `render`), with the branch,
  its pull request and its review readiness: "Request a review on #1078" over the title of the pull request; "Code
  owners", a checkbox per suggested code owner with a GitHub login of the groups whose approval is missing (the full name,
  the login beside it, the groups below), never the author, a member asked already or one whose approval counts, each
  once; checked when it opens: one per missing group nobody is asked for yet, each once, as the Create Pull Request
  dialog chooses them. One rule decides them for the popover and `bd request-review` alike
  (`domain_policy::review::candidates`, `ReviewCandidate.preselected`), which the view of the session sends with the
  open pull request. A readiness that comes after the opening checks them too, until the
  user checks or unchecks one. While Patronus names none the popover says why, never nothing as if none were needed
  (`describeCodeOwnersFact`): "Asking Patronus which code owners must approve…" before its answer, "Patronus didn't
  answer which code owners must approve (…): ask the reviewers yourself." after a failed check, "Patronus doesn't tell
  yet which code owners must approve: ask the reviewers yourself." for a check that doesn't judge them. "Others" (or "Reviewers" without code owners): the reviewer search of
  the dialog (`ReviewerPicker`, `useReviewerSearch`). "Request Review" asks the checked ones and the chosen ones.
  [@test] ../../crates/domain-policy/src/review_test.rs (`are_the_suggested_ones_of_the_missing_groups_one_per_group_nobody_is_asked_for_chosen_each_once`, `leave_out_the_author_whoever_is_asked_already_and_an_approval_that_counts_a_group_someone_is_asked_for_chooses_nobody`, `are_none_when_no_approval_is_missing_or_the_readiness_is_unknown`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should choose the code owner of a readiness of the whole stack that comes after its Request Review… opened`)
  [@test] ../../src/presenters/review.test.ts (`says while Patronus checks them, when its check failed or doesn't tell, never nothing as if none were needed`)
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should say in Request Review… while Patronus names no code owners, when it fails to, and when the rules of the branch can't be read`)
- The surfaces ([Next Step](next-step.spec.md), row 9): the row's slot mounts the popover around its step, or, for
  another step, around an anchor at its end, which "Request Review…" of the branch menu opens
  (`request-review-popover`). The Safe Push popover has no Request Review…: a start whose approvals are missing is
  refused, and the row's step asks for them. The pull request of the whole stack has its own, in the actions of the
  whole stack ([Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)): its popover at the header of the island,
  else at an anchor of the root's slot (`WholeStackRequestReviewAnchor`).
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should ask the code owner nobody asked yet from Request Review…, then wait for him from the click on`, `should open Request Review… from the branch menu, at the row's next step`)
- Request Review… asks GitHub, not Patronus: in a repository without Patronus, where a row and the header of an island
  have no next step, it must still open from every menu that offers it (the branch menu, "⋯" of the row and of the
  preview, "Whole Stack ›" of the root, "⋯" of the island), never a dead item: its popover opens at anchors that take
  no room in the step column of the Actions cell, so "⋯" of the row stands where it stands in every other row
  (`RequestReviewAnchors`, `GaplessAnchors`); no code owners without Patronus, the search finds whom to ask.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should ask for a review from the branch menu, the row's ⋯ and the preview, the popover at the row`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should ask for a review of the pull request of a small stack without Patronus from Whole Stack › of the root's menu, the popover at the root's row`, `should ask for a review of the pull request of the whole stack without Patronus from the island's ⋯, the popover at the header`)
- The request is the last effect of `RequestReview` (`RequestReviewers`, after its push:
  `POST pulls/<n>/requested_reviewers`, users only, no team), each login once: one request, and one per reviewer only when GitHub refuses the list with a 422 (it names nobody), to tell
  whom it refuses (the author, a user without access); a login it accepts but doesn't list is refused too ("GitHub left
  the reviewer out"). Another refusal (no right to write, no such pull request), also of one reviewer after the split,
  fails the request; the token chain tries the next token after a 401. The creation of a pull request asks its
  reviewers the same way. The request holds the reviewers of its pull request: a retarget of it runs beside, unless the
  push of the request retargets the pull request too (it goes into a virtual branch), which holds its base: then it
  waits for the retarget and is planned again over the base it left, the PATCH of the base before the request.
  [@test] ../../crates/operations/src/pull_request_test.rs (`a_review_request_retargets_and_pushes_first_then_asks_whom_its_plan_names`, `a_review_request_waits_for_a_retarget_of_its_pull_request_only_when_its_push_retargets_it`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should retarget a pull request while a review is requested in it: both reach GitHub, neither refuses the other`)
  [@test] ../../crates/remote-facts/src/pull_request_writes_test.rs (`asks_the_reviewers_of_an_open_pull_request_once_each`, `a_refused_list_is_asked_one_by_one_to_name_whom_github_refuses`, `a_request_github_refuses_as_a_whole_fails_and_no_login_asks_nothing`, `a_rejected_token_after_the_split_fails_the_request`)
- The result is a toast with "Open Pull Request": "Requested a review from Ilya Korennoy on #1078", a `warning` whose
  description names whom GitHub refused ("GitHub refused Bob: …"). A request GitHub asked nobody in, or one that
  failed, is "Cannot request a review on #1078" with the reason, and the popover stays. Whom GitHub asked is requested
  in the pull request of the row at once, and a refresh of the pull requests follows: the review readiness reads the
  new request.
  [@test] ../../src/presenters/review.test.ts (`names whom GitHub asked, and whom it refused as a warning`, `is none when GitHub asked nobody: the error says why`)
- `bd request-review <branch>` runs the same intent from the agent CLI: the same code owners, `--reviewer <login>`
  others; work the remote lacks refuses it (`needs_push`) until `--push` pushes it first
  ([Agent CLI](../cli/agent-cli.spec.md#bd-request-review)).

### Branch preview

- The meta line of the [branch preview](branch-preview.spec.md) ends with the chip; the body starts with a pull request
  section (the details of the chip popover) above the commits; `O` and the footer button "Open Pull Request" open it.
  A branch without a pull request has neither. Where the menu offers "Create Pull Request…", the footer has it instead
  of "Open Pull Request": the preview closes and the dialog opens; `O` keeps opening a closed pull request.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request in the branch preview and open it with O`, `should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)

### Landing overview

- The [landing overview](../repository/landing-overview.spec.md#overview) shows the chip of each row after its reason,
  before its next step.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request chips of the rows, and a pull request that waits for its author as a fix`)
- Off Space, a branch lands through its pull request: an open one with changes requested or failing checks is a fix,
  a move of the user, and the row says why: "bob requested changes on #761", "Changes requested on #800", "Checks fail
  on #761" (`landing-node.ts`). A pushed branch stays on the way to master until it lands.
  [@test] ../../crates/domain-policy/src/landing_node_test.rs (`off_space_a_branch_lands_through_its_pull_request`)
  [@test] ../../src/presenters/landing-node.test.ts (`lands by its pull request in a repository that is not on Space`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request chips of the rows, and a pull request that waits for its author as a fix`, `should keep a pushed branch on the way to master while its pull request waits for its author`)
- The footer of the overview has a line "Pull requests checked just now" (it ages while shown, as every relative time:
  [Update Project](../repository/update-project.spec.md)) with a Refresh button, named "Refresh pull requests" (the
  Patronus line has a Refresh button too). When GitHub can't be asked, the line is a quiet hint instead: "No GitHub
  token to see pull requests", "GitHub rejected the token from gh" (the source of the last token tried: `GH_TOKEN`,
  `gh`, the Branch Deck sign-in, the git credentials), both with "Sign In…" ("GitHub Settings…" in a build without the
  sign-in), which opens Settings at GitHub ([GitHub Sign-In](../repository/github-sign-in.spec.md#where-it-is-offered)); "GitHub: the organization requires SAML SSO,
  authorize the token for it", with "Authorize…" when GitHub gave the URL; "GitHub: <message> (the token from gh)" (no
  access; for the token of the Branch Deck sign-in it adds that an organization that restricts OAuth apps has to
  approve Branch Deck, with "Request Access…": [GitHub Sign-In](../repository/github-sign-in.spec.md#where-it-is-offered));
  or "GitHub: <the error>". It is never a toast. A repository that is not on GitHub has no line.
  [@test] ../../src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should hint quietly when GitHub rejects the token`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should say nothing for a repository that is not on GitHub`)

## User Experience

- The chip is text-sized (`text-xs`, `size-3.5` icon in the hue, the number `text-muted-foreground` in tabular
  figures), a plain button that underlines on hover, with the `rounded-2xl` focus ring of buttons and badges. No chip has an outline: a draft shows by its icon and its muted
  hue. The dots are `size-1.5 rounded-full shrink-0`, as the status dot of the row.
- The notice of a changed branch is `text-xs text-warning` after the warning triangle; the line of a rebuilt one is
  `text-xs text-muted-foreground`, without an icon (`ReviewCommitNotice`, shared with the Patronus details).
- `info` is the merged colour; push states never use it ([Outgoing Commits](../repository/outgoing.spec.md)).
- The details popover is `w-80`, aligned to the start of the chip; in the preview the same details sit in a nested
  `rounded-lg` panel.

## Testing / Local Run

- `cargo nextest run --workspace` covers the parsing, matching and URL parsing (`crates/remote-facts/src/pull_requests_test.rs`),
  the requests against an in-process fake GitHub (`github::fake_github`, which records them), the classification
  (`crates/github/src/client_test.rs`) and the token chain with a fake `gh` and a credential helper script
  (`token_test.rs`, `service_test.rs`). No test asks the developer's `gh` or git credentials.
- `pnpm test:unit` covers the refresh policy and the chip mapping.
- `pnpm e2e` covers the chip, the popover, the preview section, the overview and the creation (`tests/e2e/pull-requests`).
  E2E never asks GitHub: the test server runs the app's service and cores against a fake GitHub per test repository
  ([Remote services](../../docs/design/e2e-testing.md#remote-services)); `POST /repositories/{id}/pull-requests` sets
  what it holds (an empty `headRefOid` means the local tip; `availablePullRequests` answers for `acme/monorepo`), and
  `GET` returns how often the app asked and every request GitHub got. The opener plugin is mocked and records the
  opened URLs in `window.__OPENED_URLS__`. A push goes to the remote of the test repository, a bare
  `.git/upstream.git` ([The remote of a repository](../../docs/design/e2e-testing.md#the-remote-of-a-repository)): a
  branch pushed before is a push there behind the app's back (`pushInTerminal`, an earlier build of its commits for a
  pull request that shows one), and a test asserts the pushes the remote took (`remotePushes`).
  No test answers a GitHub write in the page (`routeCommand`, a fulfilling `page.route`): the registry test fails it,
  and the page answers of the reads are pinned and only go down (`ROUTED_IN_PAGE`).
  [@test] ../../crates/test-server/src/command_registry_test.rs (`no_e2e_test_answers_a_write_in_the_page`)
  [@test] ../../crates/test-server/src/command_registry_test.rs (`the_page_answers_of_the_e2e_tests_are_pinned`)
  [@test] ../../crates/test-server/src/github_host_test.rs (`pull_requests_are_listed_created_retargeted_and_closed_through_the_real_service`)
  [@test] ../../crates/test-server/src/github_host_test.rs (`a_fixture_pull_request_reads_back_through_the_real_parser`)

## Open Questions / Risks

- `author:@me` uses the GitHub search, which can lag a few seconds behind a newly opened pull request.
- A token of git may be a fine-grained token without access to pull requests: GitHub answers 403, and the hint names
  the git credentials as the source.
- Only the 50 most recent pull requests of the user are read; an old pull request of a long-lived branch may be missed.
- A branch pushed to a fork as `origin`, with the pull request on the upstream repository, is not matched.
- The Space settings file is a snapshot committed by the repository's admins: a mirror configured only in Space, not in
  the file, needs `branchdeck.githubRepository` (Settings → Repository).

## References

- [Branch Row](branch-row.spec.md): the Status cell and the anatomy of the chip details
- [Outgoing Commits](../repository/outgoing.spec.md)
- [Branch Preview](branch-preview.spec.md)
- [Patronus](patronus.spec.md): the other review status, shown next to the chip
- [Review Requests](../repository/review-requests.spec.md): the pull requests of others that wait for the user
- [Remote Status for Virtual Branches](../../docs/design/remote-status.md)
