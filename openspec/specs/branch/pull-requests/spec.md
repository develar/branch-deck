---
name: Pull Requests
description: Requirements for the GitHub pull request status of the virtual branches, read through the GitHub API with the token of GH_TOKEN, gh or git (also for branches pushed to Space and mirrored to GitHub, as its committed settings say) and shown as a chip in the branch row, the branch preview and the landing overview; a pull request Patronus merged reads merged; and "Create Pull Request…" in the branch menu, the branch preview and the push toast, a dialog that creates the pull request into the base of the branch through the GitHub API, with the reviewers (the code owners Patronus suggests, a search of the repository's users) and the user as its assignee, or opens GitHub's form with its text; "Request Review…" asks the suggested code owners on an open pull request.
style: plain-1
targets:
  - crates/domain-types/src/pull_request.rs
  - crates/operations/src/pull_request.rs
  - src/model/intents.ts
  - crates/github/src/client.rs
  - crates/github/src/service.rs
  - crates/github/src/token.rs
  - crates/remote-facts/src/pull_requests.rs
  - crates/sync-core/src/github_repository.rs
  - crates/sync-core/src/repo_facts.rs
  - src-tauri/src/services.rs
  - crates/app-api/src/remote/pull_requests.rs
  - crates/app-api/src/repository_state.rs
  - src/stores/pull-requests.ts
  - src/lib/pull-request.ts
  - src/lib/pull-request-now.ts
  - src/lib/check-counts.ts
  - src/lib/review-commit.ts
  - src/components/branch/PullRequestChip.tsx
  - src/components/branch/ReviewCommitNotice.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchPreviewDialog.tsx
  - src/components/repository/LandingIndicator.tsx
  - src/components/repository/LandingPopover.tsx
  - src/components/repository/PullRequestStatusLine.tsx
  - src/components/repository/ReviewStatusLine.tsx
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useBranchRowKeys.ts
  - src/hooks/useReviewStatusRefresh.ts
  - src/hooks/useSessionFactStores.ts
  - crates/repository-model/src/pull_request_facts.rs
  - crates/remote-facts/src/scheduler_github.rs
  - crates/domain-policy/src/refresh.rs
  - src/components/branch/StatusChip.tsx
  - src/components/branch/NextStepSlot.tsx
  - src/components/branch/WholeStackSlot.tsx
  - src/lib/push-run.ts
  - src/presenters/pull-request-url.ts
  - src/lib/git/push-toast.tsx
  - src/lib/create-pull-request.ts
  - src/lib/pull-request-dialog.ts
  - src/lib/pull-request-run.ts
  - src/stores/create-pull-request-dialog.ts
  - src/components/branch/CreatePullRequestDialog.tsx
  - src/components/branch/BranchPreviewActions.tsx
  - crates/remote-facts/src/pull_request_create.rs
  - crates/remote-facts/src/pull_request_text.rs
  - crates/sync-core/src/issue_navigation.rs
  - crates/remote-facts/src/pull_request_writes.rs
  - crates/bd-remote/src/push_command.rs
  - crates/bd-remote/src/request_review_command.rs
  - src/hooks/useReviewerSearch.ts
  - src/components/shared/ReviewerPicker.tsx
  - src/presenters/review.ts
  - src/components/branch/RequestReviewPopover.tsx
  - src/components/branch/RequestReviewForm.tsx
  - crates/domain-policy/src/pull_request.rs
  - crates/domain-policy/src/review.rs
  - crates/repository-model/src/derive.rs
  - crates/sync-core/src/remote_url.rs
---

# Pull Requests

Status: Implemented
Date: 2026-10-14

## Purpose

Reviews happen on GitHub pull requests of the pushed virtual branches. Branch Deck shows, per branch, the pull request
the user opened for it: its number, the review decision, the checks and whether it shows the branch as it is. The
status is read through the GitHub API, with the first token the user already has: `GH_TOKEN`, `gh` or the credentials
git stores for github.com; the `gh` CLI is not needed. Opening a pull request opens the browser. A branch without one gets "Create
Pull Request…", on the remote or not: a dialog that pushes the branch and creates the pull request into its base, asks its reviewers (the code owners
Patronus suggests, and whom the user finds) and assigns it to the user; "Request Review…" asks them later, on the open
pull request. The branches may be pushed to Space, which mirrors them to GitHub: the committed Space settings name the
GitHub repository then. The app writes to GitHub in six places only: the creation of a pull request with its assignee
and reviewers (below), the review request of an open pull request (below), the `/patronus merge` comment
Patronus merges a pull request from ([Patronus](../patronus/spec.md#pull-requests)), the retarget of a pull request into a
virtual branch to the baseline (before a push of its branch, or the next step's "Retarget #7 to master"), the close of the pull requests a Safe Push of a stacked branch landed
([Stacked Branches](../stacked-branches/spec.md#pull-requests)), and the review the user submits
([Review Requests](../../repository/review-requests/spec.md)).

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
  the Patronus command ([Patronus](../patronus/spec.md#pull-requests)).
- Draft pull requests, labels, milestones and team reviewers in the dialog and in Request Review…: a draft can't be merged by Patronus
  (the `draft` blocker of `domain_policy::land::decide`), and "Continue on GitHub" opens GitHub's form for the rest.
- Pull requests of forks and of branches that are not virtual branches of the user, here. The pull requests of others
  that wait for the user's review are [Review Requests](../../repository/review-requests/spec.md) ("Your Turn").
- GitHub Enterprise and other hosts: only github.com.
- Polling the user's own pull requests: GitHub is asked for them after a sync, a push, a focus and a manual refresh
  only. The review requests of others are polled ([Review Requests](../../repository/review-requests/spec.md#polling)).

## Concepts

### Request Review

Patronus counts the approvals of the code owners in the pull request the robot starts from
([Patronus](../patronus/spec.md#code-owner-review)): a pull request that asks no member of a group whose approval is
missing waits for nobody. "Request Review…" asks them, on the pull request that exists.

## Requirements

### Requirement: Reading pull requests: The repository is the configured one, origin's or the Space mirror

The repository MUST be `git config branchdeck.githubRepository` (`owner/name`), if set; else the one `origin` pushes to
(`git remote get-url --push origin`), as the virtual branches are pushed to `origin`; else, of an `origin` on Space, the
GitHub mirror that the Space settings committed in the repository name (`mirror.url` of
`.space/repository-settings.json`, read from `HEAD` with one `git cat-file`, never from the working tree).

#### Scenario: The configured repository wins over origin
- **GIVEN** `git config branchdeck.githubRepository` is set (Settings → Landing, the tab of the repository, GitHub
  repository, which shows the detected one as its placeholder, [Settings](../../repository/settings/spec.md#landing))
- **WHEN** the repository is resolved
- **THEN** it is the configured one, not the one `origin` pushes to
  [@test] crates/sync-core/src/github_repository_test.rs (`the_configured_github_repository_wins_over_origin`)

#### Scenario: The repository origin pushes to
- **GIVEN** no configured repository
- **WHEN** the pull requests are listed
- **THEN** they are those of the repository `origin` pushes to
  [@test] crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)

#### Scenario: The GitHub mirror of the committed Space settings
- **GIVEN** no configured repository and `origin` on Space
- **WHEN** `.space/repository-settings.json` of `HEAD` names a GitHub mirror in `mirror.url`
- **THEN** the repository is that mirror, read with one `git cat-file`, never from the working tree
- **AND** a mirror that is not on GitHub names no repository, and the settings of an `origin` elsewhere are not read
  [@test] crates/sync-core/src/github_repository_test.rs (`reads_the_mirror_from_the_committed_space_settings`, `a_space_mirror_must_be_on_github`)

#### Scenario: The sync reads it with the remotes
- **WHEN** a sync reads the remotes
- **THEN** the GitHub repository is in the view from its first frame (`LandingRemote::github`), with no process for a configured repository or an `origin` elsewhere, one `cat-file` for an `origin` on Space
- **AND** a command of `brd` takes it from the report of its sync (`SyncReport::github`), reading nothing again
  [@test] crates/sync-core/src/repo_facts_test.rs (`the_landing_remote_is_read_with_the_facts_of_a_sync`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_a_space_origin_reads_its_github_mirror`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_push`, `a_safe_push_check`)

### Requirement: Reading pull requests: SSH, HTTPS and git URLs name a github.com repository

SSH, HTTPS (with or without `.git` and credentials) and `git://` URLs SHALL name a github.com repository.

#### Scenario: The URL forms of a github.com repository
- **WHEN** the URL is SSH (`git@github.com:o/r.git`, `ssh://git@github.com/o/r.git`, `ssh.github.com:443`), HTTPS
  (with or without `.git` and credentials) or `git://`
- **THEN** it names a github.com repository
  [@test] crates/sync-core/src/github_repository_test.rs (`parses_github_remote_urls`)

### Requirement: Reading pull requests: No repository is built in and none is searched for

The app MUST NOT have a repository host or name built in, and it MUST NOT search GitHub for a mirror.

#### Scenario: The repository comes only from the repository's own facts
- **WHEN** the repository is resolved
- **THEN** no host or name built into the app takes part
- **AND** nothing searches GitHub for a mirror

### Requirement: Reading pull requests: The repository is resolved once while its config stays

The app MUST resolve the repository once per repository and keep it while the repository's `config` and the
worktree's `config.worktree` stay as they are, at most 10 minutes (the global config and the Space settings may name
it too): a refresh of the pull requests, the review requests and the user search MUST run no git for it.

#### Scenario: A refresh runs no git for the repository
- **GIVEN** the repository was resolved and `config` and `config.worktree` stay as they are, for at most 10 minutes
- **WHEN** the pull requests, the review requests or the user search refresh
- **THEN** no git runs for the repository
  [@test] crates/app-api/src/repository_state_test.rs (`the_github_repository_is_kept_until_the_config_changes`)

#### Scenario: A change in Settings is read at the next refresh
- **WHEN** the repository changes in Settings → Landing
- **THEN** the next refresh reads it
  [@test] crates/app-api/src/repository_state_test.rs (`the_github_repository_is_kept_until_the_config_changes`)

### Requirement: Reading pull requests: A repository outside GitHub is not on GitHub

Otherwise a remote on another host, a local path or no `origin` MUST report "not on GitHub" without asking GitHub.

#### Scenario: A remote outside GitHub
- **WHEN** the remote is on another host, a local path, or there is no `origin`
- **THEN** the repository is "not on GitHub"
- **AND** GitHub is not asked
  [@test] crates/sync-core/src/github_repository_test.rs (`rejects_remote_urls_outside_github`, `a_remote_outside_github_has_no_github_repository`)

### Requirement: Reading pull requests: One GraphQL request per refresh, newest first

A refresh MUST make one GraphQL request (`PULL_REQUESTS_QUERY`): the search
`repo:<owner>/<name> is:pr author:@me sort:updated-desc`, first 50, with the fields `gh pr list --json` reads, and
`repository(owner, name)`. Newest first, unlike `gh pr list`: the virtual branches under review are among the 50 most
recently updated.

#### Scenario: The request of a refresh
- **WHEN** the pull requests refresh
- **THEN** one GraphQL request searches the user's pull requests of the repository, newest first, first 50
- **AND** it asks `repository(owner, name)` in the same request
  [@test] crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)

### Requirement: Reading pull requests: Each pull request comes with its place in a stack of GitHub

Each pull request MUST come with its place in a stack of GitHub, the stack's number, its base and size and the layer's
position (`stack` and `stackEntry` of the same node, `PullRequest.stack`), none outside a stack; the listing MUST say
whether the repository can have stacks of GitHub (`githubStacks`,
[Stacks of GitHub](../../../../docs/design/github-stacks.md)).

#### Scenario: A pull request in a stack and one outside
- **WHEN** the pull requests are listed
- **THEN** a pull request in a stack of GitHub carries the stack's number, base and size and its layer's position
- **AND** one outside a stack carries none
- **AND** the listing says whether the repository can have stacks of GitHub
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_stack_of_github_of_each_pull_request_comes_with_the_listing`)

### Requirement: Reading pull requests: A schema without stacks is asked again without them

A schema of GitHub that refuses the fields of the stacks (`undefinedField`) MUST be asked again without them
(`PULL_REQUESTS_QUERY_WITHOUT_STACKS`), remembered for the process: the next refresh asks once, without them, and the
repository has no stacks of GitHub.

#### Scenario: GitHub refuses the fields of the stacks
- **WHEN** GitHub answers `undefinedField` for the fields of the stacks
- **THEN** the listing is asked again without them
- **AND** the next refresh of the process asks once, without them, and the repository has no stacks of GitHub
  [@test] crates/remote-facts/src/pull_requests_test.rs (`a_schema_without_stacks_is_asked_again_without_them`)

### Requirement: Reading pull requests: The listing names the repository it asked

An answer that lists the pull requests MUST name the repository it asked, its owner and name (`acme/monorepo`), for
links to GitHub that no pull request carries; the app MUST know the repository only while GitHub lists its pull
requests (`getGitHubRepository()`).

#### Scenario: The answer names the repository
- **WHEN** GitHub lists the pull requests of `acme/monorepo`
- **THEN** the answer names its owner and name
  [@test] crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)

#### Scenario: The app knows the repository only while GitHub lists its pull requests
- **WHEN** GitHub lists the pull requests, and later does not
- **THEN** `getGitHubRepository()` names the repository only while it does
  [@test] src/stores/pull-requests.test.ts (`names the GitHub repository while GitHub lists its pull requests`)

### Requirement: Reading pull requests: A repository the token can not see is no access

A null `repository` MUST be "no access" ("no repository acme/monorepo"), not an empty list: the token can't see it (a
fine-grained token of other repositories, an organization that restricts OAuth apps, which answers a search without an
error).

#### Scenario: A null repository
- **WHEN** GitHub answers a null `repository` and an empty search
- **THEN** the pull requests are "no access" ("no repository acme/monorepo"), not an empty list
  [@test] crates/remote-facts/src/pull_requests_test.rs (`a_repository_the_token_can_not_see_is_no_access_not_an_empty_list`)

### Requirement: Reading pull requests: A SAML failure or partial results is SSO required

A null `repository` with a SAML failure, or an answer with `X-GitHub-SSO: partial-results`, MUST be "SSO required".

#### Scenario: An answer with partial results of SSO
- **WHEN** GitHub answers a null `repository` with `X-GitHub-SSO: partial-results`
- **THEN** the pull requests are "SSO required"
  [@test] crates/remote-facts/src/pull_requests_test.rs (`a_repository_the_token_can_not_see_is_no_access_not_an_empty_list`)

#### Scenario: A null repository with a SAML failure
- **WHEN** GitHub answers a null `repository` with a SAML failure
- **THEN** the pull requests are "SSO required"

### Requirement: Reading pull requests: A token without read:org is asked again without teams

A token without `read:org` can't read a team's slug: an `INSUFFICIENT_SCOPES` answer MUST be asked again once without
the team reviewers (`PULL_REQUESTS_QUERY_WITHOUT_TEAMS`). The missing scope MUST be remembered for the token until it
changes: the next refreshes ask once, without the teams.

#### Scenario: GitHub answers INSUFFICIENT_SCOPES
- **WHEN** GitHub answers `INSUFFICIENT_SCOPES` for the team reviewers
- **THEN** the listing is asked again once without them
- **AND** the next refreshes with the same token ask once, without the teams
  [@test] crates/remote-facts/src/pull_requests_test.rs (`a_token_without_read_org_is_asked_again_without_teams`)

### Requirement: The token: The token is the first of its sources

The token MUST be the first of: `GH_TOKEN` (else `GITHUB_TOKEN`) of the process; `gh auth token --hostname github.com`
when `gh` is installed; the token of the app's own sign-in; the password git stores for
`https://github.com/<owner>/<name>.git`. `gh` MUST come before the app's sign-in: organizations that restrict OAuth
apps have usually approved it, and the Branch Deck app only once an admin did.

#### Scenario: The chain takes the first source
- **WHEN** the token is read
- **THEN** it is the first of `GH_TOKEN` (else `GITHUB_TOKEN`), `gh`, the app's own sign-in
  ([GitHub Sign-In](../../repository/github-sign-in/spec.md)) and the password git stores
  [@test] crates/github/src/token_test.rs (`the_chain_takes_the_first_source_and_skips_rejected_tokens`)

#### Scenario: The token of gh
- **GIVEN** `gh` is installed, found as git is: `/opt/homebrew/bin/gh`, `/usr/local/bin/gh` on macOS, then `PATH`
- **WHEN** `gh auth token --hostname github.com` runs
- **THEN** its token is the token of `gh`, and a logged-out `gh` gives nothing
  [@test] crates/github/src/token_test.rs (`gh_gives_its_token_and_nothing_when_logged_out`)

#### Scenario: The password git stores
- **WHEN** the token comes from git
- **THEN** it is the password `git credential fill` gives for `https://github.com/<owner>/<name>.git` (see [Git
  Operations](../../../../docs/design/git-operations.md#github-credentials))
  [@test] crates/github/src/token_test.rs (`the_git_credential_of_github_is_its_password`)

### Requirement: The token: Reading a token never asks the user

Reading a token MUST NOT ask the user: `gh` runs with `GH_PROMPT_DISABLED`; git with `credential.interactive=false`,
`GIT_TERMINAL_PROMPT=0`, an empty `GIT_ASKPASS` (git would run an askpass program before it looks at the terminal
setting), `GCM_INTERACTIVE=never` and `GCM_GUI_PROMPT=false`. Both MUST be killed after 10 s; no answer is no token.

#### Scenario: git asks its helper and nobody else
- **WHEN** git reads the credential of the repository
- **THEN** it asks its helper for the repository, and without one it asks nobody
  [@test] crates/github/src/token_test.rs (`git_asks_its_helper_for_the_repository_and_without_one_asks_nobody`)

#### Scenario: A source that does not answer
- **WHEN** `gh` or git does not answer within 10 s
- **THEN** it is killed and gives no token
  [@test] crates/git-executor/src/git_command_executor_test.rs (`a_timed_command_is_killed_after_its_timeout`)

### Requirement: The token: An accepted token is kept and a rejected one is never sent again

The token GitHub accepted MUST be kept per repository. A token GitHub rejects (401) MUST NOT be sent again and the next
one is tried in the same request; one that can't see the repository is skipped for it for 10 minutes. A new
`gh auth login` MUST take effect without a restart, within 30 s.

#### Scenario: A rejected token
- **WHEN** GitHub rejects a token (401)
- **THEN** the next source is tried in the same request
- **AND** the rejected token is never sent again
  [@test] crates/github/src/service_test.rs (`a_rejected_token_is_followed_by_the_next_source_and_not_sent_again`)

#### Scenario: A token that can't see the repository
- **WHEN** a token can't see the repository
- **THEN** the next token is tried, and that one is skipped for the repository for 10 minutes
  [@test] crates/github/src/service_test.rs (`a_listing_without_access_tries_the_next_token_and_remembers_why_all_failed`)

#### Scenario: A new login of gh
- **WHEN** the user runs a new `gh auth login`
- **THEN** it takes effect without a restart, within 30 s

### Requirement: The token: A source that starts a process runs once and its answer is kept 30 s

A source that starts a process (`gh`, git) MUST run once for every request that asks while it runs, and its answer, a
token or none, MUST be kept 30 s, so the first requests of every repository at start run `gh` once and a signed-out
chain asks no process on every refresh; the app's sign-in and sign-out MUST forget the answers at once, the answer of
a read in flight too.

#### Scenario: The first requests at start
- **WHEN** five first requests ask at once
- **THEN** `gh` runs once
  [@test] crates/github/src/token_test.rs (`five_first_requests_at_once_run_gh_once`)

#### Scenario: A signed-out chain
- **GIVEN** no source has a token
- **WHEN** the refreshes ask for one
- **THEN** the sources are asked once a while, not on every refresh
- **AND** asked again after a sign-in
  [@test] crates/github/src/token_test.rs (`a_signed_out_chain_asks_its_sources_once_a_while_and_again_after_a_sign_in`)

#### Scenario: A sign-in during a read in flight
- **WHEN** the user signs in while a read of a source is in flight
- **THEN** nothing of its answer is kept
  [@test] crates/github/src/token_test.rs (`a_read_in_flight_during_a_sign_in_keeps_nothing_of_its_answer`)

### Requirement: The token: No request waits for a source it does not ask

A request MUST NOT wait for a source it does not ask: a repository whose token GitHub accepted answers while
`git credential fill` of another waits for a keychain.

#### Scenario: Another repository's credential waits for a keychain
- **GIVEN** GitHub accepted the token of a repository
- **WHEN** `git credential fill` of another repository waits for a keychain
- **THEN** the first repository answers meanwhile
  [@test] crates/github/src/token_test.rs (`an_accepted_repository_answers_while_the_credential_of_another_is_read`)

### Requirement: Requests: Timeouts, and a read asked again once after a failure

Every request MUST have a 5 s connect and a 30 s total timeout. A read (a GET, a GraphQL query) MUST be asked again once
after a failure of the server or the connection (5xx, refused, reset), never after a timeout, which already waited its
full time; a write MUST NOT be asked again.

#### Scenario: A failure of the server or the connection
- **WHEN** a read fails with a failure of the server or the connection (5xx, refused, reset)
- **THEN** it is asked again once
- **AND** a write that fails so is never asked again
  [@test] crates/github/src/client_test.rs (`a_get_is_asked_again_once_after_a_server_error_and_a_post_never`)

#### Scenario: A read that timed out
- **WHEN** a read times out
- **THEN** it is not asked again
  [@test] crates/github/src/client_test.rs (`a_get_that_timed_out_is_not_asked_again`)

### Requirement: Requests: A GET the same token asked before is conditional

A GET the same token asked before MUST be conditional (`If-None-Match`): a `304 Not Modified` does not count against
the rate limit.

#### Scenario: The second GET of a token
- **WHEN** the same token asks a GET a second time
- **THEN** it asks with `If-None-Match`, and a `304 Not Modified` answers it
- **AND** another token asks without it
  [@test] crates/github/src/client_test.rs (`a_get_escapes_its_segments_and_asks_conditionally_the_second_time`)

### Requirement: Requests: Failures are classified, never thrown

Failures MUST be classified, never thrown.

#### Scenario: No token from any source
- **WHEN** no source gives a token
- **THEN** the failure is "signed out", and GitHub is not asked
  [@test] crates/remote-facts/src/pull_requests_test.rs (`without_a_token_github_is_not_asked`)

#### Scenario: A 401
- **WHEN** GitHub answers 401
- **THEN** the failure is "not authenticated", with the source of the last token tried
  [@test] crates/github/src/client_test.rs (`a_401_is_a_rejected_token`)
  [@test] crates/remote-facts/src/pull_requests_test.rs (`classifies_failures`)

#### Scenario: The primary rate limit
- **WHEN** GitHub answers 403/429 with `x-ratelimit-remaining: 0`, or a GraphQL `RATE_LIMITED`
- **THEN** the failure is "rate limited" until the reset
  [@test] crates/github/src/client_test.rs (`the_primary_rate_limit_waits_for_its_reset`)
  [@test] crates/remote-facts/src/pull_requests_test.rs (`classifies_failures`)

#### Scenario: A secondary rate limit
- **WHEN** GitHub answers with `retry-after`, or "secondary rate limit" in the message
- **THEN** the failure is "rate limited" until the reset
  [@test] crates/github/src/client_test.rs (`a_secondary_rate_limit_waits_for_retry_after_or_a_minute`)

#### Scenario: A 403 of SSO
- **WHEN** GitHub answers a 403 with `X-GitHub-SSO: required; url=…`
- **THEN** the failure is "SSO required" with the URL that authorizes the token
  [@test] crates/github/src/client_test.rs (`a_403_of_saml_sso_carries_the_authorization_url`)

#### Scenario: Another 403 or a 404
- **WHEN** GitHub answers another 403 or a 404
- **THEN** the failure is "no access", with GitHub's message
  [@test] crates/github/src/client_test.rs (`a_403_or_404_is_no_access_with_the_message`)

#### Scenario: Anything else
- **WHEN** a request fails otherwise
- **THEN** the failure is an error with GitHub's message (and the messages of its `errors`), or the transport failure
  [@test] crates/remote-facts/src/pull_requests_test.rs (`classifies_failures`, `rejects_output_that_is_not_a_pull_request_list`)

### Requirement: Requests: After a rate limit no listing asks before the reset

After a rate limit, a listing (the pull requests, Your Turn) MUST NOT ask GitHub with that token before the reset: the
refresh after every sync, push and focus answers "rate limited" at once instead of extending the penalty. A write the
user asked for (Create, Retarget, Request Review) MUST still go.

#### Scenario: A refresh before the reset
- **GIVEN** a rate limit of the token
- **WHEN** a listing refreshes before the reset, after a sync, a push or a focus
- **THEN** it answers "rate limited" at once without asking GitHub
  [@test] crates/github/src/service_test.rs (`after_a_rate_limit_no_listing_is_asked_before_its_retry_at`)

#### Scenario: A rate limit that ended
- **WHEN** the reset of the rate limit passed
- **THEN** the listing asks GitHub again
  [@test] crates/github/src/service_test.rs (`a_rate_limit_that_ended_lets_the_listing_ask_again`)

#### Scenario: A write the user asked for
- **GIVEN** a rate limit of the token
- **WHEN** the user asks for a write (Create, Retarget, Request Review)
- **THEN** the write still goes

### Requirement: Matching pull requests to branches: A pull request belongs to the branch of its head

A pull request MUST belong to the virtual branch whose pushed name, `{prefix}/virtual/{name}`, is its head branch, and
only when the head repository owner is the owner of the push remote (case-insensitive). A pull request from a fork
(`isCrossRepository`) MUST NOT be matched.

#### Scenario: The head branch of the push remote's owner, and a fork
- **WHEN** a pull request's head branch is `{prefix}/virtual/{name}` of the push remote's owner, in any case
- **THEN** it belongs to that virtual branch
- **AND** a pull request from a fork (`isCrossRepository`) is never matched
  [@test] crates/remote-facts/src/pull_requests_test.rs (`parses_and_matches_pull_requests_by_virtual_branch`)

### Requirement: Matching pull requests to branches: Open wins over merged over closed, then the newest

Of several pull requests of one branch, an open one MUST win over a merged one over a closed one, then the most
recently updated.

#### Scenario: Several pull requests of one branch
- **WHEN** one branch has several pull requests
- **THEN** an open one wins over a merged one over a closed one
- **AND** among equal ones the most recently updated wins
  [@test] crates/remote-facts/src/pull_requests_test.rs (`closed_pull_requests_prefer_the_newest_and_empty_decisions_are_none`)

### Requirement: Matching pull requests to branches: A closed pull request labelled merged is merged

A closed pull request with the label `merged` MUST be merged: Patronus rebases, so GitHub can't mark the pull request
it merged as merged; it closes it and adds the label. The labels MUST be read in the same search.

#### Scenario: A pull request Patronus closed with the label
- **WHEN** a closed pull request carries the label `merged`
- **THEN** it is merged
- **AND** its labels came in the same search
  [@test] crates/remote-facts/src/pull_requests_test.rs (`a_pull_request_patronus_closed_with_the_merged_label_is_merged`)

### Requirement: Matching pull requests to branches: The checks of the head commit are summarized

The checks of the head commit MUST be summarized from `statusCheckRollup`: the summary is failing with any failure,
else pending with any running check, else passing; none without checks. The names of the first three failed checks
MUST be kept.

#### Scenario: The checks and their summary
- **WHEN** the checks of the head commit are read
- **THEN** a check run that is not completed, or a status context `PENDING`/`EXPECTED`, is running
- **AND** a completed check run concluding `FAILURE`, `TIMED_OUT`, `CANCELLED`, `ACTION_REQUIRED` or
  `STARTUP_FAILURE`, or a status context `FAILURE`/`ERROR`, failed
- **AND** every other completed check passed (`SKIPPED` and `NEUTRAL` too)
- **AND** the summary is failing with any failure, else pending with any running check, else passing; none without
  checks
- **AND** the names of the first three failed checks are kept
  [@test] crates/remote-facts/src/pull_requests_test.rs (`pending_checks_without_failures_are_pending`)

### Requirement: Matching pull requests to branches: The reviewers are each one's latest review, then the requested

The reviewers MUST be the latest submitted review of each reviewer (approved, changes requested, commented; pending and
dismissed reviews say nothing), then the requested reviewers and teams without one (`requested`). A reviewer asked
again after a review MUST keep its state and be `requested` too: a re-requested approval still counts for Patronus.

#### Scenario: The latest review of each reviewer and the requested ones
- **WHEN** the reviews of a pull request are read
- **THEN** each reviewer has the state of its latest submitted review; pending and dismissed reviews say nothing
- **AND** the requested reviewers and teams without a review are `requested`
  [@test] crates/remote-facts/src/pull_requests_test.rs (`parses_and_matches_pull_requests_by_virtual_branch`)

#### Scenario: A reviewer asked again after a review
- **WHEN** a reviewer who approved is asked again
- **THEN** it keeps its state and is `requested` too
- **AND** the details say "carol approved · re-requested"
  [@test] src/lib/pull-request.test.ts (`names the latest review, and a request after it`)

### Requirement: Matching pull requests to branches: The reviewers and the author are the readiness input

The pull request MUST name its `author` (`author { login }`). The reviewers and the author MUST be the input of the
readiness of the branch, so a new review asks Patronus once, and the chip keeps its own state while the slot waits for
an approval ([Next Step](../next-step/spec.md)).

#### Scenario: The author of a pull request
- **WHEN** a pull request is read
- **THEN** it names its `author`
  [@test] crates/remote-facts/src/pull_requests_test.rs (`parses_and_matches_pull_requests_by_virtual_branch`)

#### Scenario: A new review
- **WHEN** a new review comes
- **THEN** Patronus is asked once for the readiness of the branch
- **AND** the chip keeps its own state while the slot waits for an approval

### Requirement: Refreshing: The pull requests are a fact of the session, never persisted

The pull requests MUST be a fact of the session of the repository and branch prefix (`facts.pullRequests` of its view,
`repository_model::pull_request_facts`), kept in memory: the session lists them by the rules of
`domain_policy::refresh` (`remote_facts::scheduler`) while a window shows it, and the page only stores what its view
says. The pull requests MUST NOT be persisted.

#### Scenario: A window shows the session
- **WHEN** a window shows the session of a repository and branch prefix
- **THEN** the session lists the pull requests by the rules of `domain_policy::refresh` and keeps them in memory
- **AND** the page stores only what its view says, and nothing is persisted

### Requirement: Refreshing: At most one listing per session is in flight

At most one listing per session MUST be in flight. A push, a write of the app or the refresh button during a listing
MUST ask once more after it; a sync or a focus during a listing adds nothing.

#### Scenario: A write during a listing
- **GIVEN** a listing in flight
- **WHEN** a push, a write of the app (a retarget) or the refresh button asks
- **THEN** the pull requests are asked once more after the listing
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_retarget_asks_the_pull_requests_again`)

#### Scenario: A sync or a focus during a listing
- **GIVEN** a listing in flight
- **WHEN** a sync or a focus asks
- **THEN** it adds nothing

### Requirement: Refreshing: The listing tells the merge state, the auto-merge and the merge methods

The listing MUST also tell whether GitHub can merge each pull request (`mergeStateStatus`), its auto-merge and the
merge methods of the repository, which the merge of the pull-request route reads
([Landing Routes](../landing-routes/spec.md#merge)).

#### Scenario: A listing of the pull requests
- **WHEN** the pull requests are listed
- **THEN** each one comes with its `mergeStateStatus` and its auto-merge
- **AND** the listing comes with the merge methods of the repository
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_merge_state_the_auto_merge_and_the_merge_methods_come_with_the_listing`)

### Requirement: Refreshing: A merge state GitHub still computes lists once more

An open pull request GitHub answered `UNKNOWN` of MUST make it list once more a moment later, as a push whose pull
request still shows an old head does.

#### Scenario: An open pull request of an UNKNOWN merge state
- **WHEN** GitHub answers `UNKNOWN` of the merge state of an open pull request
- **THEN** the pull requests are listed once more a moment later
  [@test] crates/domain-policy/src/refresh_test.rs (`an_open_pull_request_whose_merge_state_github_computes_asks_the_list_again`)

### Requirement: Refreshing: A pull request the app wrote is in the session at once

A pull request the app wrote (a retarget, a close, the reviewers it asked, one it created) MUST be in the session at
once (`Sessions::write_pull_request`, from the command of the write), numbered when it applies: a listing that started
before it keeps the pull request as written, so a retarget during a listing never comes back as "Retarget #7" and the
next push does not retarget again.

#### Scenario: A listing that started before the write
- **GIVEN** a listing started
- **WHEN** the app retargets, closes or asks the reviewers of a pull request before the listing answers
- **THEN** the write is in the session at once
- **AND** the late listing keeps the pull request as written, never "Retarget #7" again
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_late_listing_keeps_the_retarget_the_reviewers_and_the_close_written_after_it_started`)
  [@test] crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)

### Requirement: Refreshing: A write lists once more, a close lists nothing

A retarget, the reviewers and a creation MUST list the pull requests once more, after the listing in flight, whose
answer then is the truth; a close MUST list nothing.

#### Scenario: A creation and a retarget list again
- **WHEN** the app creates or retargets a pull request, or asks its reviewers
- **THEN** the pull requests are listed once more, after the listing in flight
  [@test] crates/test-server/src/github_host_test.rs (`pull_requests_are_listed_created_retargeted_and_closed_through_the_real_service`)

#### Scenario: A close lists nothing
- **WHEN** the app closes a pull request
- **THEN** nothing is listed
  [@test] crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)
  [@test] crates/test-server/src/github_host_test.rs (`pull_requests_are_listed_created_retargeted_and_closed_through_the_real_service`)

### Requirement: Refreshing: The command of a write answers once the view has it

The command of a write MUST answer once the view has the write, so the toast of the push that follows reads it.

#### Scenario: The toast of the push after a write
- **WHEN** the command of a write answers
- **THEN** the view already has the write, and the toast of the push that follows reads it
  [@test] crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)

### Requirement: Refreshing: A created pull request stays until the search lists it

A pull request the app created MUST stay until GitHub's search lists it, at most five minutes.

#### Scenario: A search that does not list it yet
- **WHEN** a listing after the creation does not list the pull request yet
- **THEN** the created pull request stays, at most five minutes
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_created_pull_request_stays_until_the_search_lists_it`)

### Requirement: Refreshing: The retarget the user asks is an intent of the engine

The retarget the user asks (row 5 of the next step, its "⋯", the Safe Push form, the push of a branch whose pull request
goes into a virtual branch) MUST be an intent of the engine (`Retarget`, `operations::pull_request`): planned over the
pull requests of the session, it moves an open one into the target the baseline tracks, holds the base of the pull
request, and writes the new base into the session.

#### Scenario: The page asks the retarget through the engine
- **WHEN** the user retargets the pull request of a branch
- **THEN** the page asks the engine, in the repository of its listing
  [@test] src/stores/pull-requests.test.ts (`retargets the pull request of a branch through the engine, in the repository of its listing`)

#### Scenario: An open pull request moves into the target and nothing else
- **WHEN** the retarget runs
- **THEN** it moves an open pull request into the target the baseline tracks
- **AND** none of one into the target already; a merged one is refused
  [@test] crates/operations/src/robot_test.rs (`a_retarget_moves_an_open_pull_request_into_the_target_and_nothing_else`)

#### Scenario: A review request of the same pull request
- **WHEN** a review request of the pull request runs during the retarget
- **THEN** it runs beside, and a second retarget joins the first
  [@test] crates/operations/src/robot_test.rs (`a_retarget_during_a_review_request_of_the_same_pull_request_runs_beside_it`)

### Requirement: Refreshing: A repository not on GitHub is asked again only by the refresh button

A repository that answered "not on GitHub" MUST be asked again only by the refresh button: a sync or a focus does not
run `git remote get-url` for it again.

#### Scenario: A sync or a focus of a repository not on GitHub
- **GIVEN** the repository answered "not on GitHub"
- **WHEN** a sync or a focus comes
- **THEN** `git remote get-url` does not run for it again
- **AND** the refresh button asks again

### Requirement: Refreshing: After a rate limit only the refresh button asks before retry_at

After GitHub's rate limit nothing but the refresh button MUST ask before its `retry_at`, which the GitHub service holds
per token too.

#### Scenario: A rate limit of GitHub
- **GIVEN** GitHub answered a rate limit
- **WHEN** a sync, a focus or a push comes before its `retry_at`
- **THEN** nothing is asked
- **AND** the refresh button asks
  [@test] crates/remote-facts/src/scheduler_test.rs (`after_a_rate_limit_nothing_is_asked_before_retry_at_but_manual`)

### Requirement: Refreshing: The pull requests are listed by a sync, a focus, a push and the button

After every committed table of a sync, the pull requests MUST be listed if the last listing started at least 60 s ago;
on a focus of a window if they are older than 2 minutes and the last listing started at least 60 s ago (the time on
the session's clock); a push of the app that pushed (the remote did not move) and the refresh button always. The app
MUST NOT poll GitHub.

#### Scenario: A sync, a focus, a push and the refresh button
- **WHEN** a sync commits its table, a window gets the focus, the app pushes, or the user presses the refresh button
- **THEN** the pull requests are listed after a sync if the last listing started at least 60 s ago, on a focus if they
  are older than 2 minutes and the last listing started at least 60 s ago, after a push of the app that pushed and on the refresh button always
- **AND** nothing polls GitHub
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_pull_requests_are_listed_by_a_sync_a_focus_a_push_and_the_refresh_button`)

#### Scenario: The refresh button
- **WHEN** the user presses the refresh button
- **THEN** the page asks the session for the pull requests, the button showing it runs
  [@test] src/stores/pull-requests.test.ts (`asks the session for the pull requests, the refresh button showing it runs`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should refresh from the overview`)

#### Scenario: A push of the app
- **WHEN** the app pushes a branch
- **THEN** the pull requests are listed after it
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should refresh after a push`)

### Requirement: Refreshing: An old head after a push is looked at again 5 s later, once

GitHub can show the old head for a moment after a push: when the listing after the push still shows another head of a
pushed branch (or whole stack), it MUST be listed once more 5 s later on the session clock; that listing asks no
further one.

#### Scenario: The listing after a push shows an old head
- **WHEN** the listing after a push shows another head of a pushed branch (or whole stack)
- **THEN** the pull requests are listed once more 5 s later on the session clock
- **AND** that listing asks no further one
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_push_github_shows_on_an_old_head_is_looked_at_again_five_seconds_later_once`)
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_listing_shows_the_pushed_head_or_an_old_one`)

### Requirement: Refreshing: A robot linked to a merged pull request asks again right away

A robot linked to a pull request that merged ([Patronus](../patronus/spec.md#pull-requests)) MUST ask again right away:
the pull request is closed with the label `merged` now.

#### Scenario: A robot of a pull request merged
- **WHEN** a poll of Patronus says a robot linked to a pull request merged
- **THEN** the pull requests are listed right away
- **AND** the GitHub repository of the listing is kept through the poll
  [@test] src/stores/patronus.test.ts (`keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged`)

### Requirement: Refreshing: A failed listing is never an empty one

A failed listing MUST NOT be an empty one: before any listing it is unavailable with its reason, so no pull request of
a branch is known; after a listing, a failure GitHub recovers from (an error, the rate limit) keeps the listing, stale
with the reason, and only the availability changes; once GitHub refuses the token no listing stands.

#### Scenario: A failure before any listing
- **WHEN** the first listing fails
- **THEN** the pull requests are unavailable with the reason, and no pull request of a branch is known
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_failed_listing_is_unavailable_never_empty`)

#### Scenario: A failure after a listing
- **GIVEN** a listing
- **WHEN** a refresh fails with a failure GitHub recovers from (an error, the rate limit)
- **THEN** the listing is kept, stale with the reason, and only the availability changes
- **AND** once GitHub refuses the token no listing stands
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_failed_refresh_after_a_listing_keeps_it_stale`)

### Requirement: Refreshing: A listing of the same pull requests changes nothing

A listing that answers the same pull requests MUST change nothing, and a patch MUST keep every pull request it doesn't
name as the same object.

#### Scenario: The same pull requests again
- **WHEN** a listing answers the same pull requests
- **THEN** nothing changes
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_listing_of_the_same_pull_requests_changes_nothing`)

#### Scenario: A patch that names none
- **WHEN** a patch of the view names no pull request
- **THEN** the page keeps the record of every pull request as the same object
  [@test] src/stores/pull-requests.test.ts (`stores the pull requests of the view as they are, the record kept while a patch names none`)

### Requirement: Chip: The Status cell shows the chip after the status text

The Status cell of a branch row MUST show the chip in its second column, after the status text
([Branch Row](../branch-row/spec.md)): the pull request icon and `#760`. A branch without a pull request MUST show
nothing.

#### Scenario: A branch with a pull request and one without
- **WHEN** the branch table shows a branch with a pull request and one without
- **THEN** the Status cell of the first shows the pull request icon and `#760` after the status text
- **AND** the second shows nothing
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show each pull request state in the status cell`)

### Requirement: Chip: The hue follows the review decision or the state

The hue of an open pull request MUST follow the review decision; a draft, merged or closed pull request MUST show its
state, not the decision. The hue colours the icon only; the number is muted. The node of the row has the colour of the
next step, so a fact gets one colour: a red node, a red icon and a red "#409" said one fact three times.

#### Scenario: An open pull request
- **WHEN** an open pull request is approved, has changes requested, or has review required or no decision
- **THEN** its icon is approved `success`, changes requested `destructive`, review required and no decision muted
- **AND** its number is muted
  [@test] src/lib/pull-request.test.ts (`colours an open pull request by its review decision`)

#### Scenario: A draft, a merged and a closed pull request
- **WHEN** the pull request is a draft, merged or closed
- **THEN** a draft is muted with the draft icon; a merged one `info` with the merge icon; a closed one muted with the
  closed icon
- **AND** none shows the decision
  [@test] src/lib/pull-request.test.ts (`shows the state of a draft, a merged and a closed pull request instead of the decision`)

### Requirement: Chip: A dot marks failing checks or a head the branch changed since

An open pull request with failing checks MUST get a small `destructive` dot. An open one MUST get a `warning` dot when
the branch changed since its head: the head is not the commit the remote branch is at now (`remoteTip`), or the branch
is not up to date with the remote by content, as the
[remote status](../../../../docs/design/remote-status.md) counts it.

#### Scenario: Failing checks and a changed branch
- **WHEN** an open pull request has failing checks, or its head is not `remoteTip` or the branch is not up to date by
  content
- **THEN** it gets a `destructive` dot, or a `warning` dot
  [@test] src/lib/pull-request.test.ts (`marks failing checks and a head the branch changed since, for open pull requests only`)
  [@test] src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`)

### Requirement: Chip: A head at the tip, a merged or closed one get no dot

A head at the local tip MUST get no dot, nor one at the remote tip of a branch that is up to date: every sync rebuilds
the virtual branches on the newer baseline, so their hashes change while their commits don't. A merged or closed pull
request MUST get no dot, nor any pull request until the remote status of the branch is loaded.

#### Scenario: A head at the local tip or at the remote tip of a branch up to date
- **WHEN** the head is the local tip, or the remote tip of a branch that is up to date
- **THEN** the pull request gets no dot
  [@test] src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`, `takes the branch as it is for its own tip, and while nothing says otherwise`)

#### Scenario: Before the remote status is loaded
- **WHEN** the remote status of the branch is not loaded yet
- **THEN** the pull request gets no dot
  [@test] src/lib/review-commit.test.ts (`takes the branch as it is for its own tip, and while nothing says otherwise`)

#### Scenario: A merged or closed pull request
- **WHEN** the pull request is merged or closed
- **THEN** it gets no dot
  [@test] src/lib/pull-request.test.ts (`marks failing checks and a head the branch changed since, for open pull requests only`)

### Requirement: Chip: The accessible name of the chip

Its accessible name MUST be "Pull request #760: Approved".

#### Scenario: The chip of an approved pull request
- **WHEN** the chip shows an approved pull request #760
- **THEN** its accessible name is "Pull request #760: Approved"

### Requirement: Chip: Hovering the chip opens the details

Hovering the chip MUST open the details: the number and the state, the title, the checks, the reviewers, the last
update, the notice about the head, "Open Pull Request" and "Copy Pull Request Link" (which says "Copied" in place),
named as in the branch menu.

#### Scenario: The details on hover
- **WHEN** the pointer hovers the chip
- **THEN** the details show the number and the state, the title, the checks, the reviewers, the last update and the
  notice about the head
- **AND** "Open Pull Request" and "Copy Pull Request Link", which says "Copied" in place
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`, `should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)

#### Scenario: The checks in the details
- **WHEN** the details show the checks
- **THEN** they say "1 failing (tests), 2 running, 9 passed" or "All 4 passed"
- **AND** the failed ones are named as a cut list, "5 failing (a, b, c and 2 more)", one without a name said as such,
  never "and 1 more": "2 failing (tests and a check without a name)"
  [@test] src/lib/pull-request.test.ts (`lists the failing checks first`)
  [@test] src/lib/check-counts.test.ts (`never counts one failed check left out: one without a name is said as such, in the counts of the checks too`)

### Requirement: Chip: The details say why the head differs

With the dot, the details MUST say "Shows 3264f30f; the branch has changed since: push to update the pull request.",
or, when the remote branch has the change already and GitHub doesn't show it yet, "Shows 3264f30f; GitHub doesn't
show the pushed 88de421d yet." A branch only rebuilt on a newer baseline gets no dot; the details MUST say quietly
"Shows an earlier build of the same commits."

#### Scenario: The branch changed since the head
- **WHEN** the branch changed since the head of its pull request
- **THEN** the details say "Shows 3264f30f; the branch has changed since: push to update the pull request."
  [@test] src/lib/pull-request.test.ts (`asks for a push when the branch changed, waits for GitHub after one, and says quietly that it shows an earlier build`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say when the branch changed since the pull request's head`)

#### Scenario: GitHub doesn't show the push yet
- **WHEN** the remote branch has the change already and GitHub doesn't show it yet
- **THEN** the details say "Shows 3264f30f; GitHub doesn't show the pushed 88de421d yet."
  [@test] src/lib/pull-request.test.ts (`asks for a push when the branch changed, waits for GitHub after one, and says quietly that it shows an earlier build`)

#### Scenario: A branch only rebuilt on a newer baseline
- **WHEN** the branch was only rebuilt on a newer baseline
- **THEN** the chip gets no dot
- **AND** the details say quietly "Shows an earlier build of the same commits."
  [@test] src/lib/pull-request.test.ts (`asks for a push when the branch changed, waits for GitHub after one, and says quietly that it shows an earlier build`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say only quietly that the pull request shows an earlier build of the same commits`)

### Requirement: Chip: Commits someone else pushed are never pushed away

When someone else pushed commits to the branch of the pull request that the branch lacks (`othersBehind`, the head the
listing showed fetched, [Outgoing](../../repository/outgoing/spec.md#remote-commits-of-others)), the details MUST NOT
ask for a push, which would drop them.

#### Scenario: Others pushed commits the branch lacks
- **WHEN** someone else pushed commits the branch doesn't have
- **THEN** the chip has the dot
- **AND** the details say "Shows d708d5b8; someone else pushed commits your branch doesn't have: a push drops them."
  [@test] src/lib/pull-request.test.ts (`says that a push drops the commits someone else pushed to the remote branch, never that a push is due`)

### Requirement: Chip: A click opens the pull request and keeps the row

A click on the chip MUST open the pull request in the browser, leave its details as they are
([hover descriptions](../../../../docs/design/ui-theme.md)) and not toggle the row.

#### Scenario: A click on the chip
- **WHEN** the user clicks the chip
- **THEN** the pull request opens in the browser
- **AND** its details stay as they are and the row does not toggle
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)

### Requirement: Chip: The branch menu opens the pull request and copies its link

The branch context menu MUST have "Open Pull Request #7" in its history group when the branch has a pull request, the
number saying which one it opens, also when the branch can't be modified, and "Copy Pull Request Link" in its copy
group: one item for every surface, the menu, the footer of the preview and the title bar of the review. "Open Pull
Request #7" shows `O`, which
opens the pull request on the focused row as in the branch preview
([Branch Row](../branch-row/spec.md#keyboard)).

#### Scenario: Open Pull Request from the branch menu
- **WHEN** the user chooses "Open Pull Request #7" in the branch context menu
- **THEN** the pull request opens
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)

#### Scenario: O on the focused row and Copy Pull Request Link
- **WHEN** the user presses `O` on the focused row, or chooses "Copy Pull Request Link"
- **THEN** the pull request opens, or its link is copied
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)

### Requirement: Creating a pull request: Create Pull Request opens the dialog of the branch

"Create Pull Request…" MUST open the dialog "Create pull request" of the branch (`CreatePullRequestDialog` in the main
window, loaded on the first open; the review window has its own): the title "Create pull request", the line
"feature-auth → master", Title, Description, Reviewers, "Assign to me", and the footer "Esc Cancel", "Continue on
GitHub", "Create Pull Request ⌘↵".

#### Scenario: The dialog of a branch
- **WHEN** the user chooses "Create Pull Request…" of a branch
- **THEN** the dialog "Create pull request" opens with the line "feature-auth → master", Title, Description,
  Reviewers and "Assign to me"
- **AND** its footer says "Esc Cancel", "Continue on GitHub", "Create Pull Request ⌘↵"
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)

### Requirement: Creating a pull request: The base is the baseline for every branch

The base MUST be the baseline as the remote names it (`master`) for every branch, stacked or not
([Stacked Branches](../stacked-branches/spec.md#pull-requests)): a pull request holds what the Safe Push of its branch
lands.

#### Scenario: The pull requests of a stack
- **WHEN** the pull requests of a stack are created
- **THEN** each goes into `master`
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
  [@test] src/lib/pull-request-dialog.test.ts (`creates the pull request of a stacked branch into the baseline, with the commits of the branches it is built on`)

### Requirement: Creating a pull request: A stacked branch's dialog names what it includes

A stacked branch's dialog MUST say so under the line: "Includes the commits of auth and login, which oauth is built on:
3 branches · 12 commits" (`formatPullRequestIncludes`; up to four branches below it are named, five or more are "auth,
login, core and 2 more").

#### Scenario: A stacked branch
- **WHEN** the dialog of a stacked branch opens
- **THEN** it says "Includes the commits of auth and login, which oauth is built on: 3 branches · 12 commits"
- **AND** up to four branches below it are named, five or more are "auth, login, core and 2 more"
  [@test] src/lib/pull-request-dialog.test.ts (`creates the pull request of a stacked branch into the baseline, with the commits of the branches it is built on`)

#### Scenario: A branch built on the baseline
- **WHEN** the dialog of a branch built on the baseline opens
- **THEN** it includes nothing more
  [@test] src/lib/pull-request-dialog.test.ts (`includes nothing more for a branch built on the baseline`)

### Requirement: Creating a pull request: It is offered to any branch without a live pull request

"Create Pull Request…" MUST be offered to any branch without a live pull request, on the remote or not (Rust offers
`create-pull-request` by the GitHub repository and the pull request alone): the push is the creation's first step,
never a step the user takes or reads about.

#### Scenario: A branch not on the remote yet
- **WHEN** a branch without a live pull request is not on the remote
- **THEN** "Create Pull Request…" is offered to it
  [@test] tests/e2e/next-step/next-step.spec.ts (`should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`)

### Requirement: Creating a pull request: Create is one intent of the engine that pushes first

Opening the dialog MUST push nothing; Create (and "Continue on GitHub") MUST be one intent of the engine
(`CreatePullRequest`, `operations::pull_request`; "Continue on GitHub" with `onGitHub`), whose plan pushes first what the
push button would (`derive::action_push_of`), its progress in the button ("Pushing 45%"); Escape and a click outside
MUST NOT close the dialog meanwhile.

#### Scenario: Create pushes first, Cancel pushes nothing
- **WHEN** the user opens the dialog of a branch not on the remote and cancels it, or creates the pull request
- **THEN** Cancel pushes nothing
- **AND** Create pushes the branch, its progress in the button ("Pushing 45%"), then creates the pull request
  [@test] tests/e2e/next-step/next-step.spec.ts (`should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`)

#### Scenario: What the plan pushes
- **WHEN** the plan of `CreatePullRequest` is made
- **THEN** it pushes first what the remote lacks: the branch and the ancestors with work, root first; the pull requests
  of what it pushes into a virtual branch are retargeted into the target before, D3a; the stack branch of a whole stack
- **AND** no push toast shows; Activity records it
  [@test] crates/operations/src/pull_request_test.rs (`a_creation_pushes_first_what_the_remote_lacks_then_opens_the_pull_request`)

### Requirement: Creating a pull request: A push the user decides is never made for them

A push the user decides MUST NOT be made for them: one that drops commits of the remote branch refuses the plan, and
the dialog stays; nothing is created.

#### Scenario: The push would drop commits of the remote branch
- **WHEN** the push of the creation would drop commits of the remote branch
- **THEN** the plan is refused and nothing is created
- **AND** the dialog stays with "Cannot create pull request" / "The remote branch of feature-auth has 2 commits that
  feature-auth lacks: push it from its row, which shows them"
  [@test] tests/e2e/next-step/next-step.spec.ts (`should keep the dialog, with the reason, when the push would drop commits of the remote branch`)

### Requirement: Creating a pull request: After its push the creation waits for GitHub to have the branch

Right after its push GitHub may not have the branch yet (a Space mirror copies it a moment later): the creation MUST ask
again every 2 s for 30 s (`wait_for_mirror`), "Continue on GitHub" MUST wait until GitHub has the branch (the effect
`WaitForMirror` of its plan) before the page opens GitHub's form; brd's `safe-push --create-pull-request` waits the same
way after its push.

#### Scenario: The mirror copies the branch a moment later
- **WHEN** GitHub does not have the branch right after the push
- **THEN** the creation asks again every 2 s for 30 s
- **AND** a wait that ends without the branch ends with the reason
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`right_after_a_push_the_creation_waits_for_the_mirror_to_copy_the_branch`, `the_wait_for_the_mirror_ends_with_the_reason`)

#### Scenario: Continue on GitHub of a branch not on the remote yet
- **WHEN** the user chooses "Continue on GitHub" of a branch not on the remote yet
- **THEN** the plan pushes and waits until GitHub has the branch
- **AND** the page then opens GitHub's form, of the whole stack too
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`waits_until_github_has_the_branch`)
  [@test] crates/operations/src/pull_request_test.rs (`continue_on_github_pushes_and_waits_until_github_has_the_branch`)
  [@test] src/lib/pull-request-run.test.ts (`opens GitHub's form once the run pushed and GitHub has the branch, of the whole stack too`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should open GitHub's form of a branch not on the remote yet from its preview, after the push`)

### Requirement: Creating a pull request: The text starts as brd writes it

The text MUST start as `brd push` and `brd safe-push --create-pull-request` write it (`get_new_pull_request`,
`pull_request_text`): the `brd describe` description of the branch, else the text of the commits the pull request will
show, those of the local branch the creation pushes (`<remote>/<base>..<head>`).

#### Scenario: The description, else the commits
- **WHEN** the dialog asks for the text
- **THEN** it is the `brd describe` description of the branch
- **AND** without one, the text of the commits the pull request will show
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`the_text_is_the_description_else_the_commits_the_pull_request_will_show`)
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`the_description_wins`)

### Requirement: Creating a pull request: A field the user typed in is kept

A field the user typed in MUST NOT be replaced by the text that arrives later.

#### Scenario: The text arrives after the user typed
- **WHEN** the text arrives after the user typed in a field
- **THEN** that field keeps what the user typed

### Requirement: Creating a pull request: The title of one commit, an issue branch and another branch

The title of one commit MUST be its subject. With several, a branch of one issue (its name is the key, or every subject
carries the same key) MUST be titled by the key and the issue's summary, never by a key cut into words; without the
summary, by the key and the summary of the branch the row shows. A branch of several keys or of none MUST be titled by
the summary of the branch, else by its name as `gh pr create --fill` names it (`feature auth`).

#### Scenario: A branch of one issue
- **WHEN** a branch of several commits is of one issue
- **THEN** its title is the key and the issue's summary, `IJAI-4390 Agent CLIs render without colors`
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`an_issue_branch_keeps_its_key_and_takes_the_issue_summary`)

#### Scenario: A branch of one issue without its summary
- **WHEN** the summary of the issue is not known
- **THEN** the title is the key and the summary of the branch the row shows (the first subject, oldest first, that is
  no cleanup, without its key)
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`without_an_issue_summary_the_title_is_the_key_and_the_branch_summary`)

#### Scenario: A branch of several keys
- **WHEN** the branch is of several keys or of none
- **THEN** its title is the summary of the branch, else its name as `gh pr create --fill` names it
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`a_branch_of_several_keys_names_each_key_and_is_titled_by_its_summary`)

### Requirement: Creating a pull request: The summary of an issue comes from the YouTrack signed in to

The summary MUST come from YouTrack only when the link of the key in `.idea/vcs.xml` is an issue page of the YouTrack
signed in to, as the issue popover asks it, every key at once, within 3 s for all of them; so the title depends on the
sign-in.

#### Scenario: A key linked to the YouTrack signed in to
- **WHEN** the link of a key in `.idea/vcs.xml` is an issue page of the YouTrack signed in to
- **THEN** its summary is asked from YouTrack, every key at once, within 3 s for all of them
- **AND** a key linked elsewhere is not asked
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`an_issue_is_asked_from_youtrack_only_when_the_repository_links_it_there`, `the_issues_are_asked_at_once`)
  [@test] crates/sync-core/src/issue_navigation.rs (`a_key_links_as_the_first_pattern_that_matches_all_of_it_names_it`)

### Requirement: Creating a pull request: The body names the issues, then the commits

The body MUST start with each issue of the branch once, `[IJAI-4390](<its page>)` joined by ` · ` (the bare key without
a link). One commit then gives its body; several a numbered item each, oldest first: the subject in bold, without the
key the title carries when the subject starts with it, the body under it indented by the width of the marker (`10.` by
4). `fixup!`, `squash!` and `amend!` commits MUST have no item.

#### Scenario: The issues of the branch and the items of its commits
- **WHEN** the body of a branch of one issue and several commits is written
- **THEN** it starts with the issue once, `[IJAI-4390](<its page>)`, several joined by ` · `
- **AND** each commit is a numbered item, oldest first, its subject in bold without the key the title carries
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`an_issue_branch_keeps_its_key_and_takes_the_issue_summary`)

#### Scenario: Autosquash commits
- **WHEN** the branch has `fixup!`, `squash!` or `amend!` commits
- **THEN** they have no item
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`trailers_and_autosquash_commits_are_left_out`)

### Requirement: Creating a pull request: A commit body reads on GitHub as the commit preview shows it

A commit body MUST read on GitHub as the commit preview shows it: the blocks of the one grammar of a commit body
(`sync_utils::commit_body`, [Commit Row](../../commit/commit-row/spec.md#message-body)) written as Markdown, where a
newline is a line break, the blocks apart by a blank line. A link, an image and strong emphasis stay as the author
wrote them, an HTML comment is left out.

#### Scenario: Paragraphs, lists, code, headings and rules
- **WHEN** a commit body is written into the pull request
- **THEN** a paragraph is a line per line it reads as (its hard-wrapped lines joined; a line after a hard break, a
  quote line and a line that starts with `|` each their own)
- **AND** a list `-` or the numbers from its first, its wrapped and indented lines joined
- **AND** code (fenced or indented) in a fence, a heading with the `#` of its level, a rule `---`
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`each_commit_is_an_item_with_its_body_reflowed`)

#### Scenario: A table of a commit body
- **WHEN** a commit body holds a table
- **THEN** it is a table with its header, a delimiter row of the alignment of each column (`---`, `:---`, `:---:`,
  `---:`) and its rows, a pipe in a cell `\|`
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`a_table_of_a_commit_body_is_a_table_of_the_pull_request`)

### Requirement: Creating a pull request: A label of a commit body stays as the author wrote it

A label MUST stay as the author wrote it: a run-in label on the line of its paragraph (`Why: the text`), a label
written on a line of its own a `Label:` paragraph of its own, so a list, a table or a quote after it keeps its first
line. The trailers MUST be left out; a one-word `Note:` alone is prose.

#### Scenario: Labels and trailers
- **WHEN** a commit body has labels and trailers
- **THEN** a run-in label stays on the line of its paragraph, a label on a line of its own is a paragraph of its own
- **AND** the trailers are left out, and a one-word `Note:` alone is prose
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`each_commit_is_an_item_with_its_body_reflowed`, `trailers_and_autosquash_commits_are_left_out`)

### Requirement: Creating a pull request: A mention or a tag outside code goes into a code span

Outside code spans, a word GitHub would read as a mention (`@Nullable`) or as an HTML tag (`<T>`, `List<String>`) MUST
go into a code span.

#### Scenario: Mentions and angle brackets
- **WHEN** a commit body has `@Nullable`, `<T>` or `List<String>` outside a code span
- **THEN** the pull request shows it literally, in a code span
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`mentions_and_angle_brackets_stay_literal`)
  [@test] crates/bd-remote/src/push_test.rs (`the_printed_body_keeps_angle_brackets_literal`)

### Requirement: Creating a pull request: A stacked branch's pull request names the branches it is built on

A stacked branch's pull request MUST hold the commits of the branches it is built on: its title and items come from its
own commits, and a last section says "Built on auth, then login: their commits are in this pull request too." with the
subjects of each ("Built on auth: its commits …" for one). The app sends the chain of the row (`stack_ancestors`), and
one `log` splits the commits by the tips of those branches; `brd` takes them from the sync.

#### Scenario: The text of a stacked branch
- **WHEN** the text of a stacked branch's pull request is written
- **THEN** its title and items come from its own commits
- **AND** a last section says "Built on auth, then login: their commits are in this pull request too." with the
  subjects of each
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`a_stacked_branch_is_titled_by_its_own_commits_and_names_its_ancestors`)

#### Scenario: The commits of each ancestor in the app
- **WHEN** the app sends the chain of the row
- **THEN** one `log` splits the commits by the tips of those branches
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`the_commits_up_to_the_tip_of_each_ancestor_are_the_ancestor_s`)
  [@test] crates/remote-facts/src/process_budget_test.rs (`the_text_of_a_new_pull_request`)

#### Scenario: The commits of each ancestor in brd
- **WHEN** `brd` pushes a stack
- **THEN** it takes the commits of each ancestor from the sync, and a child alone carries its parents' commits
  [@test] crates/bd-remote/src/push_test.rs (`parents_go_first_and_a_child_alone_carries_its_parents_commits`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)

### Requirement: Creating a pull request: A body too long for GitHub names the commits left out

GitHub takes a body of 65,536 characters: the items that don't fit MUST be named in a last line, "_Too long for a pull
request: the text of 2 commits is left out (…)._", as in the pull request of the whole stack.

#### Scenario: A body longer than GitHub takes
- **WHEN** the body is longer than 65,536 characters
- **THEN** the items that don't fit are left out
- **AND** a last line says "_Too long for a pull request: the text of 2 commits is left out (…)._"
  [@test] crates/remote-facts/src/pull_request_text_test.rs (`a_body_too_long_for_github_is_cut_and_names_the_commits_left_out`)

### Requirement: Creating a pull request: Reviewers are chips with a search of the repository's users

Reviewers MUST be chips of GitHub logins with a search of the users of the repository a review can be asked from
(GitHub's `assignableUsers`, from 2 characters, after a pause in the typing; an older answer never replaces a newer
one); the user MUST NOT be offered, GitHub refuses a review request of the author.

#### Scenario: A search of the users
- **WHEN** the user types 2 characters or more in Reviewers and pauses
- **THEN** the users a review can be asked from are searched, those not chosen yet shown, without the user
  [@test] src/lib/create-pull-request.test.ts (`shows the users not chosen yet, without the user`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should create the pull request with the reviewers it searched and the user as its assignee`)

#### Scenario: An older answer of the search
- **WHEN** the answer of an older search comes after a newer one
- **THEN** it does not replace the newer one
  [@test] src/lib/create-pull-request.test.ts (`takes only the answer of the latest request`)

### Requirement: Creating a pull request: The code owners Patronus suggests are listed

Where Patronus is, "Suggested by code owners:" MUST list the suggested and available reviewers of each group whose
approval is missing (the readiness of the branch from its one reader, `review-readiness`,
[Next Step](../next-step/spec.md#the-readiness-store), a start without a pull request, the kept answer, never a
recheck; for a stacked branch the groups of the stack Patronus counts).

#### Scenario: The suggested reviewers
- **WHEN** the dialog opens where Patronus is
- **THEN** "Suggested by code owners:" lists the suggested and available reviewers of each group whose approval is
  missing, without the user
  [@test] src/lib/create-pull-request.test.ts (`are the suggested and available reviewers, without the user`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and no start Patronus would fail`)

#### Scenario: Approvals that count only in one pull request
- **WHEN** the approvals count only in one pull request
- **THEN** "Create Pull Request…" is offered with the code owner
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer Create Pull Request… with the code owner when approvals count only in one`)

### Requirement: Creating a pull request: One suggested reviewer per group is chosen

One suggested reviewer with a GitHub login per group MUST be chosen when the dialog opens: the user sees the chips
before creating and removes what doesn't fit. An answer that comes after the opening (asked, failed, then answered)
MUST choose them too, the choice following the answer it shows, until the user changes the reviewers.

#### Scenario: The dialog opens with the answer of Patronus
- **WHEN** the dialog opens and Patronus answered
- **THEN** one reviewer with a login per group is chosen, each once
  [@test] src/lib/create-pull-request.test.ts (`start with one reviewer with a login per group, each once`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should ask the code owner Patronus names, and the row then awaits the review in the pull request`)

#### Scenario: The answer comes after the dialog opened
- **WHEN** the answer of Patronus comes after the dialog opened (asked, failed, then answered)
- **THEN** it chooses the reviewers too, until the user changes them
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should preselect the code owners in Create Pull Request… from the answer that comes after the dialog opened`)

### Requirement: Creating a pull request: A suggestion without a login searches the name

A suggestion without a GitHub login (Patronus sends `githubUsername` behind its flag
`review.precheck.github.usernames.enabled`) MUST search the reviewer's name; the user picks the person, the app MUST
NOT guess.

#### Scenario: A suggested reviewer without a GitHub login
- **WHEN** Patronus suggests a reviewer without `githubUsername`
- **THEN** the dialog searches the reviewer's name
- **AND** the user picks the person; the app does not guess

### Requirement: Creating a pull request: The row of the code owners says while Patronus answers

While Patronus answers the row MUST say "Asking Patronus for the code owners…", a failure "Cannot read the code owners:
…", a skipped check "The code owners of so many commits are named when the Safe Push starts".

#### Scenario: Patronus answers, fails or skips the check
- **WHEN** Patronus has not answered yet, failed, or skipped the check
- **THEN** the row says "Asking Patronus for the code owners…", "Cannot read the code owners: …", or "The code owners
  of so many commits are named when the Safe Push starts"

### Requirement: Creating a pull request: Assign to me is kept

"Assign to me" MUST be on until the user turns it off; the choice MUST be kept
(`appSettings.createPullRequestAssignSelf`).

#### Scenario: The user turns Assign to me off
- **WHEN** the user turns "Assign to me" off
- **THEN** it stays off in the next dialog

### Requirement: Creating a pull request: Create writes in one request of the token chain

⌘↵ (Ctrl+Enter) or the button MUST create it (the last effect of `CreatePullRequest`), in one request of the token
chain: `GET /user` first (a rejected token is replaced before anything is written), `POST pulls`, then
`POST issues/<n>/assignees` and `POST pulls/<n>/requested_reviewers`. A title of several lines MUST be refused ("A
pull request needs a title of one line") before GitHub is asked.

#### Scenario: The creation with the assignee and the reviewers
- **WHEN** the user presses ⌘↵ or the button
- **THEN** `GET /user`, `POST pulls`, `POST issues/<n>/assignees` and `POST pulls/<n>/requested_reviewers` run in turn
- **AND** the author is not asked to review
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`creates_the_pull_request_assigns_the_user_and_asks_the_reviewers_except_the_author`)

#### Scenario: Without the assignee and reviewers
- **WHEN** neither the assignee nor reviewers are chosen
- **THEN** only the pull request is created
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`without_the_assignee_and_reviewers_only_the_pull_request_is_created`)

#### Scenario: A rejected token
- **WHEN** GitHub rejects the token at `GET /user`
- **THEN** nothing is written with it
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`a_rejected_token_writes_nothing`)

#### Scenario: A title of several lines
- **WHEN** a title of several lines reaches the creation
- **THEN** it is refused with "A pull request needs a title of one line", and GitHub is not asked
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`a_title_of_several_lines_is_refused_before_github_is_asked`)

### Requirement: Creating a pull request: Nothing fails a creation after the pull request exists

After the pull request exists the creation MUST NOT fail: an assignee GitHub leaves out and a reviewer it refuses
are warnings; a refused list of reviewers MUST be asked again one by one, to name whom it refuses.

#### Scenario: GitHub refuses the list of reviewers
- **WHEN** GitHub refuses the list of reviewers
- **THEN** they are asked one by one, and whom GitHub refuses is a warning
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`a_refused_list_of_reviewers_is_asked_one_by_one_and_who_github_refuses_is_a_warning`)

#### Scenario: GitHub leaves out or refuses the assignee
- **WHEN** GitHub leaves out or refuses the assignee
- **THEN** it is a warning of the created pull request
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`an_assignee_github_leaves_out_or_refuses_is_a_warning_of_the_created_pull_request`)

### Requirement: Creating a pull request: The dialog stays open while the creation runs

Escape and a click outside MUST NOT close the dialog while it runs; a click outside MUST NOT close it ever (the text
stays).

#### Scenario: Escape or a click outside during the creation
- **WHEN** the user presses Escape or clicks outside while the creation runs
- **THEN** the dialog stays
- **AND** a click outside never closes it, also when nothing runs

### Requirement: Creating a pull request: A creation is an operation of the session that holds its head

The creation and "Continue on GitHub" MUST be operations of the session of the repository (`RepositoryView.operations`,
[Running Operations](../../../../docs/design/operations.md)): each holds the head of its subject, so a second submit
of the same joins the first, another text (or a push of the branch) waits for it and is decided again (GitHub then
finds the pull request it opened).

#### Scenario: A second submit and another text
- **GIVEN** a creation runs, holding the ref of its branch
- **WHEN** the same creation is submitted again, or one with another text
- **THEN** the same one joins the running creation
- **AND** the one with another text waits for it
  [@test] crates/operations/src/pull_request_test.rs (`a_review_request_waits_for_a_retarget_of_its_pull_request_only_when_its_push_retargets_it`)

### Requirement: Creating a pull request: A creation waits for the push or the sync in flight

A creation asked while the push of its branch sends MUST wait for that push and run on what it left, nothing more to
push; one asked during a sync MUST wait for the table of that sync, then push and create once; the dialog waits from
the click on and while the operation runs, the review window for its own request.

#### Scenario: A creation asked while the push of its branch sends
- **WHEN** the user creates the pull request while the push of its branch sends
- **THEN** the creation waits for that push and runs on what it left, nothing more to push
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should create the pull request asked for while the push of its branch sends, on what that push left`)

#### Scenario: A creation asked during a sync
- **WHEN** the user creates the pull request during a held sync
- **THEN** the creation waits for the table of that sync, then pushes and creates once, the sync ending once
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should create the pull request asked for during a held sync once that sync ended, its push first, the sync ending once`)

### Requirement: Creating a pull request: Request Review is an operation of the session too

"Request Review…" MUST be an operation of the session as well: its button shows the spinner, a second click joins it.

#### Scenario: Request Review while it runs
- **WHEN** the session runs a review request of a branch
- **THEN** it is the running operation of its row, its button showing the spinner
- **AND** a second click joins it
  [@test] src/lib/pull-request-run.test.ts (`is the running operation of its row while the session runs it`)

### Requirement: Creating a pull request: A creation belongs to the repository it was asked in

The creation, "Continue on GitHub" and "Request Review…" MUST belong to the repository they were asked in (the dialog of
the main window or of the review window, the popover): its path goes with the request, and Rust resolves the branch
prefix of that repository for each command: the intent is planned and run in the session of that repository, so the
push, the pull request, its reviewers, the store and Activity are of that repository also when the main window opens
another during the push.

#### Scenario: The intent runs in the repository of the dialog
- **WHEN** the user creates the pull request in a dialog
- **THEN** the intent runs in the repository of the dialog, and the toast names the pull request GitHub opened
- **AND** it pushes first and answers whom GitHub asked and refused, also when it asked nobody
  [@test] src/lib/pull-request-run.test.ts (`runs the intent in the repository of the dialog, and toasts the pull request GitHub opened`, `pushes first and answers whom GitHub asked and refused, also when it asked nobody`)

#### Scenario: The main window opens another repository during the push
- **WHEN** the main window opens another repository during the push of a creation asked in the review window
- **THEN** the pull request is created in the repository of the review window
  [@test] tests/e2e/next-step/next-step.spec.ts (`should create the pull request of the dialog of the review window in its repository when the main window opens another during its push`)

### Requirement: Creating a pull request: The result is one toast recorded in Activity

The result MUST be one toast with "Open Pull Request", recorded in Activity: "Created pull request #123" /
"feature-auth → master · review requested from alice · assigned to you", a `warning` toast whose description names what
GitHub refused ("cannot request review from bob: …"). The pull request MUST be in the row at once and stay there until
GitHub's search lists it (5 min at most), then a refresh follows.

#### Scenario: The toast of a creation
- **WHEN** the pull request is created
- **THEN** the toast says "Created pull request #123" / "feature-auth → master · review requested from alice · assigned
  to you" with "Open Pull Request"
  [@test] src/lib/create-pull-request.test.ts (`says the base, who reviews and the assignee`)

#### Scenario: GitHub refused a reviewer
- **WHEN** GitHub refused a reviewer or the assignee
- **THEN** the toast is a `warning` whose description names it ("cannot request review from bob: …")
  [@test] src/lib/create-pull-request.test.ts (`is a warning that names what GitHub refused`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say whom GitHub refused, and keep the dialog when GitHub creates nothing`)

### Requirement: Creating a pull request: An existing pull request is found and left as it is

A pull request of the branch that already exists MUST be found and left as it is: "feature-auth already has pull
request #7" / "The title, the reviewers and the assignee weren't applied to it". It MUST be looked up by its head
(`GET pulls?head=<owner>:<branch>&state=open`), not by the search, which lists a new pull request only a while later.

#### Scenario: The branch has a pull request already
- **WHEN** the creation finds a pull request of the branch
- **THEN** it is left as it is
- **AND** the app says "feature-auth already has pull request #7" / "The title, the reviewers and the assignee
  weren't applied to it"
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`a_pull_request_that_already_exists_is_found_and_left_as_it_is`)
  [@test] src/lib/pull-request-run.test.ts (`answers the one GitHub had of the head already, and throws why nothing was created`)
  [@test] src/lib/create-pull-request.test.ts (`says that one which existed got nothing of the dialog`)

#### Scenario: One just created that the search does not list yet
- **WHEN** a retry after a timeout, or the app and `brd safe-push --create-pull-request` at once, create the pull
  request of the branch
- **THEN** the second finds the one just created by its head
  [@test] crates/operations/src/pull_request_test.rs (`a_creation_again_finds_the_pull_request_the_search_does_not_list_yet`)

### Requirement: Creating a pull request: A pull request a refresh brings disables the creation

One a refresh brings while the dialog is open (open, or merged so that the branch landed by it: into the target with
its commits, as Rust's next step says; one merged into a parent's branch is none) MUST turn it into "feature-auth
already has pull request #7." with "Open Pull Request"; Create and "Continue on GitHub" MUST be disabled, their hover
saying why: GitHub's form would open nothing.

#### Scenario: A refresh brings an open pull request
- **WHEN** a refresh brings an open pull request of the branch while the dialog is open
- **THEN** the dialog says "feature-auth already has pull request #7." with "Open Pull Request"
- **AND** Create and "Continue on GitHub" are disabled, their hover "feature-auth already has pull request #7"
  [@test] src/presenters/pull-request-url.test.ts (`says why GitHub's form and Create can't make a new pull request: a live one; a branch not on the remote can, it is pushed first`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say why it can't create, and offer only Open Pull Request once the branch has one`)

#### Scenario: A merged pull request
- **WHEN** the pull request of the branch is merged and the branch landed by it
- **THEN** Create and "Continue on GitHub" are disabled, their hover "Pull request #7 of feature-auth is merged"
- **AND** one merged into a parent's branch lets a new pull request repeat it
  [@test] src/lib/pull-request-dialog.test.ts (`lets a new pull request repeat a merged one only while the branch did not land by it, as into a parent's branch`)

### Requirement: Creating a pull request: A disabled Create always says why

A disabled Create MUST always say why on hover: "Enter a title", "The title must be one line", "Reading the commits of
the branch…".

#### Scenario: Create without a title or while the commits are read
- **WHEN** the title is empty, of several lines, or the commits of the branch are still read
- **THEN** Create is disabled and its hover says "Enter a title", "The title must be one line", or "Reading the
  commits of the branch…"
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say why it can't create, and offer only Open Pull Request once the branch has one`)

### Requirement: Creating a pull request: A failed creation keeps the dialog with the reason

A creation that fails MUST be "Cannot create pull request" with GitHub's reason, and the dialog MUST stay with its
text. A branch GitHub doesn't have yet (a mirror copies a pushed branch a moment later) MUST say so; a base it doesn't
have "GitHub doesn't have the base … : push it first".

#### Scenario: GitHub creates nothing
- **WHEN** GitHub creates nothing
- **THEN** the dialog stays with its text and "Cannot create pull request" with GitHub's reason
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say whom GitHub refused, and keep the dialog when GitHub creates nothing`)

#### Scenario: A branch or a base GitHub doesn't have yet
- **WHEN** GitHub doesn't have the branch yet, or the base
- **THEN** the reason is "GitHub doesn't have user/virtual/feature-auth yet: a mirror copies a pushed branch a moment
  later; try again", or "GitHub doesn't have the base … : push it first"
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`a_branch_github_does_not_have_yet_says_so`)

### Requirement: Creating a pull request: Without a token GitHub accepts only Continue on GitHub is left

Without a token GitHub accepts (the availability of the pull requests: signed out, rejected, SAML SSO, no access, rate
limited) the dialog MUST say why, with what fixes it ("Sign In…", "Authorize…", as the status line), and "Continue on
GitHub" MUST be its only action; it has no Reviewers (GitHub's form takes none, nobody can be searched) and "Assign to
me" only while it knows the user's login.

#### Scenario: The dialog without a token GitHub accepts
- **GIVEN** no token GitHub accepts
- **WHEN** the dialog opens
- **THEN** it says why with what fixes it ("Sign In…", "Authorize…"), and "Continue on GitHub" is its only action
- **AND** it has no Reviewers, and "Assign to me" only while it knows the user's login
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open GitHub's form without a token GitHub accepts`)

### Requirement: Creating a pull request: Continue on GitHub opens GitHub's form with the dialog's text

"Continue on GitHub" MUST open GitHub's form of a new pull request in the browser with the dialog's title, description
(cut at 6000 characters) and the user as the assignee, which GitHub takes as query parameters; reviewers it doesn't
(its tooltip says so). GitHub creates nothing until the form is submitted.

#### Scenario: The form of the pushed branch into its base
- **WHEN** the user chooses "Continue on GitHub"
- **THEN** the browser opens
  `https://github.com/<owner>/<name>/compare/<base>...<prefix>/virtual/<branch>?expand=1&title=…&body=…&assignees=…`
- **AND** a branch name keeps its slashes, anything else that a URL path can't hold is encoded
  [@test] src/lib/pull-request.test.ts (`opens the pull request form of the pushed branch into its base`)

#### Scenario: The text and the assignee of the dialog
- **WHEN** the form opens
- **THEN** it has the dialog's title, description (cut at 6000 characters) and the user as the assignee
  [@test] src/lib/pull-request.test.ts (`fills in the title, the description and the assignee of the dialog, and cuts a long description`)

### Requirement: Creating a pull request: The form's repository is the listed one, else the mirror

The repository of GitHub's form MUST be the one GitHub lists the pull requests of (`getGitHubRepository()`), else the
one Patronus knows the branches are mirrored to (the Space settings, no token needed for a link).

#### Scenario: No listing of GitHub
- **WHEN** GitHub lists no pull requests of the repository and Patronus knows the mirror
- **THEN** the form opens in the mirror the Space settings name, without a token

### Requirement: Creating a pull request: The branch menu offers it without an open or merged pull request

The history group of the branch menu (the right-click and "⋯") MUST have "Create Pull Request…" when the branch has no
open or merged pull request and the GitHub repository is known, on the remote or not (the dialog pushes it); a branch
with one MUST NOT. A closed pull request doesn't count: the branch gets a new one, and "Open Pull Request #7" (its hover
"#7 is closed") still opens the closed one, beside it in the menu, the preview and the review window alike.

#### Scenario: A branch without a pull request
- **WHEN** the user opens the menu of a branch without a pull request
- **THEN** its history group has "Create Pull Request…"
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`, `should open the pull request of a pushed branch from the push toast`)

#### Scenario: A branch with only a closed pull request
- **WHEN** the branch has only a closed pull request
- **THEN** "Create Pull Request…" is offered
- **AND** "Open Pull Request #7", its hover "#7 is closed", opens the closed one beside it
  [@test] src/lib/push-run.test.ts (`offers a new pull request when the branch has only a closed one`)

### Requirement: Creating a pull request: A landed branch keeps Create disabled with the reason

Once nothing of the branch is left to land, as its [next step](../next-step/spec.md#the-table) says (`landed`, row 1:
every commit in the fetched baseline, a merged Safe Push landed all of it, its own robot merged it), "Create Pull
Request…" MUST stay in the menu, the footer of the preview and the title bar of the review window, disabled with that
reason: a pull request of commits that are in the target has nothing to review.

#### Scenario: A branch a merged Safe Push landed
- **WHEN** nothing of the branch is left to land
- **THEN** "Create Pull Request…" is disabled in its menu, its preview and its review window
- **AND** it says the reason ("Landed in master with oauth", "Merged by Patronus")
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should disable Create Pull Request… of a branch a merged Safe Push landed, in its menu, its preview and its review window: nothing of it is left to review`)
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
  [@test] src/presenters/next-step.test.ts (`row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed`)

#### Scenario: Commits after the landed ones
- **WHEN** the branch has commits after the landed ones
- **THEN** it offers "Create Pull Request…", for them

### Requirement: Creating a pull request: The push toast offers Create or Open Pull Request

The toast of a push ("Pushed branch feature-auth", [Outgoing Commits](../../repository/outgoing/spec.md#push)) of any
branch, stacked or not, MUST have the action "Create Pull Request…" under the same conditions as the menu, else "Open
Pull Request" when the branch has a pull request, and stay 10 s; off GitHub it has no action. The toast reads the
branch after the push: the push that created the remote branch offers its pull request.

#### Scenario: A push of a branch without a pull request
- **WHEN** the push of a branch without a pull request ends
- **THEN** the toast "Pushed branch feature-auth" offers "Create Pull Request…", into the baseline, for 10 s
  [@test] src/lib/push-run.test.ts (`offers to create the pull request of a branch without one, into the baseline`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)

#### Scenario: A push of a branch with a pull request
- **WHEN** the push of a branch with a pull request ends
- **THEN** the toast offers "Open Pull Request", which opens it
  [@test] src/lib/push-run.test.ts (`opens the pull request the branch has`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request of a pushed branch from the push toast`)

#### Scenario: A push off GitHub
- **WHEN** a branch of a repository not on GitHub is pushed
- **THEN** the toast has no action

### Requirement: Creating a pull request: Create Pull Request closes the toast as it opens the dialog

"Create Pull Request…" of the toast MUST close the toast as it opens the dialog: a toast lies above the dialogs and
would cover its footer ([UX Writing](../../../../docs/contributing/ux-writing.md), an action with "…" of a toast).

#### Scenario: Create Pull Request from the push toast
- **WHEN** the user chooses "Create Pull Request…" in the push toast
- **THEN** the toast closes and the dialog opens

### Requirement: Creating a pull request: The push toast names no base

The push toast MUST name no base, every branch has the same; an open pull request into a virtual branch whose retarget
failed gets "Copy gh pr edit" instead ([Stacked Branches](../stacked-branches/spec.md#pull-requests)). A push that is a
step of another action MUST have no toast ([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: A retarget that failed
- **WHEN** the pull request of the pushed branch goes into a virtual branch and its retarget failed
- **THEN** the toast has "Copy gh pr edit" instead

#### Scenario: A push that is a step of another action
- **WHEN** a push runs as a step of another action
- **THEN** it has no toast

### Requirement: Creating a pull request: The review window creates in its own repository

The review window of a branch ([Branch Review](../branch-review/spec.md#the-create-pull-request-dialog)) MUST have
"Create Pull Request…" too: the same dialog opens in the window, with what the window's own session holds, and creates
the pull request in the repository of the review; the toast is in the window.

#### Scenario: Create Pull Request in the review window
- **WHEN** the user chooses "Create Pull Request…" in the review window
- **THEN** the dialog opens in the window with the context of its own session
- **AND** the pull request is created in the repository of the review, the toast in the window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window, with the context of its own session`)

### Requirement: Creating a pull request: Safe Push Without Review at row 7

At row 7 of the [next step](../next-step/spec.md) the footer MUST have "Safe Push Without Review…" at its left: it
closes the dialog and opens the Safe Push popover at the row's slot, its profile one that skips the code-owner review
([Patronus](../patronus/spec.md#safe-push-without-review)).

#### Scenario: Safe Push Without Review from the dialog
- **GIVEN** the branch is at row 7 of the next step
- **WHEN** the user chooses "Safe Push Without Review…" in the footer
- **THEN** the dialog closes and the Safe Push popover opens at the row's slot with the profile that skips the
  code-owner review
  [@test] tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`)

### Requirement: Creating a pull request: The footer fits in one line

The dialog, in the main window and in the review window, MUST be wide enough for its footer in one line, "Safe Push
Without Review…" included (`form`, 48rem); in a narrower window the footer wraps, and nothing leaves the dialog.

#### Scenario: The footer of the dialog
- **WHEN** the dialog shows its footer with "Safe Push Without Review…"
- **THEN** the footer is one line inside the dialog
- **AND** in a narrower window it wraps, and nothing leaves the dialog
  [@test] tests/e2e/next-step/next-step.spec.ts (`should keep the pull request dialog's footer in one line inside the dialog`)

### Requirement: Creating a pull request: A gh command of a mirror names its repository

A copied or printed `gh` command (`gh pr edit` of the toast and of the menu's "Copy Retarget Command", the
`gh pr create … --assignee @me` of `brd push`) MUST get `--repo <owner>/<name>` when the GitHub repository is not the one
`origin` pushes to (a mirror): `gh` finds no GitHub repository among the remotes of a mirrored checkout.

#### Scenario: A mirrored checkout
- **GIVEN** the GitHub repository is not the one `origin` pushes to
- **WHEN** a `gh` command is copied or printed
- **THEN** it has `--repo <owner>/<name>`

### Requirement: Request Review: One builder makes the item of every surface

`getRequestReviewItem` (`src/presenters/review.ts`) MUST build the item, id `request-review`, "Request Review…", of the
open pull request and the actions of the subject its caller selected, for every surface that offers it (the row's next
step, the branch menu, `getRequestReviewItemNow`): there only while the branch has an open pull request (else "Create
Pull Request…" asks the reviewers).

#### Scenario: A branch with an open pull request
- **WHEN** the branch has an open pull request and Rust offers the action
- **THEN** the item "Request Review…" asks the chosen code owners on it, its press opening the popover
  [@test] src/presenters/review.test.ts (`asks the chosen code owners on the open pull request, its press opens the popover`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)

#### Scenario: A branch without an open pull request
- **WHEN** the branch has no open pull request, or Rust doesn't offer the action
- **THEN** the item is not there
  [@test] src/presenters/review.test.ts (`is not there without an open pull request, nor while Rust doesn't offer it`)

### Requirement: Request Review: A draft disables it with the reason

The item MUST be disabled with the reason while the pull request is a draft ("#1078 is a draft: mark it ready for
review on GitHub first"), as the session reads the open pull request of the subject (`RepositoryView.openPullRequests`,
`PullRequestView`: its number, where it goes by `domain_policy::pull_request::base_of`, a draft, the candidates below).

#### Scenario: A draft pull request
- **WHEN** the open pull request is a draft
- **THEN** "Request Review…" is disabled with "#1078 is a draft: mark it ready for review on GitHub first"
  [@test] src/presenters/review.test.ts (`is disabled with the reason Rust gives for a draft: work the remote lacks is pushed by the request`)

#### Scenario: The view of the open pull requests
- **WHEN** the session derives the view
- **THEN** it reads where each open pull request goes and whom Request Review offers
  [@test] crates/repository-model/src/derive_test.rs (`the_view_reads_where_each_open_pull_request_goes_and_whom_request_review_offers`)

### Requirement: Request Review: Work the remote lacks is pushed by the request first

Work the remote lacks by content MUST be no reason to disable it: the request (the intent `RequestReview` with
`pushFirst`) pushes it first, the `Push` of its plan (the push of the push button, the stack branch of a whole
stack, no push toast; one only rebuilt on a newer baseline is up to date, an unknown remote status pushes nothing), so
the reviewers see the commits.

#### Scenario: A branch with commits the remote lacks
- **WHEN** the user requests a review of a branch with commits the remote lacks
- **THEN** the commits are pushed as the first step, then the code owner is asked
  [@test] tests/e2e/next-step/next-step.spec.ts (`should push the commits the remote lacks as the first step of Request Review…`)
  [@test] src/presenters/review.test.ts (`is disabled with the reason Rust gives for a draft: work the remote lacks is pushed by the request`)

#### Scenario: The plan pushes only when told to
- **WHEN** the plan of `RequestReview` is made
- **THEN** it pushes first only with `pushFirst`, and asks the preselected code owner
  [@test] crates/operations/src/pull_request_test.rs (`a_review_request_pushes_first_only_when_told_to_and_asks_the_preselected_code_owner`)

### Requirement: Request Review: A push that dismisses the approvals is said in the popover

A push that dismisses the approvals the pull request has (the rules of its base as the session read them,
`useBranchRules` answers `dismiss`) MUST be said in the popover, a consequence the user decides by Request: "The push
of the new commits dismisses the approvals of #1078: the rules of the repository dismiss stale reviews when new commits
are pushed."

#### Scenario: The rules of the base dismiss stale reviews
- **WHEN** the push of the request would dismiss the approvals of the pull request
- **THEN** the popover says "The push of the new commits dismisses the approvals of #1078: the rules of the repository
  dismiss stale reviews when new commits are pushed."

### Requirement: Request Review: Rules that can't be read are said, never read as keeping

Rules that can't be read MUST be said, never read as keeping the approvals, and asked again by the next form; the
request still goes (it counts no approvals): "Cannot check whether the push of the new commits keeps the approvals of
#1078: GitHub answered 500."

#### Scenario: The rules of the branch can't be read
- **WHEN** the rules of the branch can't be read
- **THEN** the popover says "Cannot check whether the push of the new commits keeps the approvals of #1078: GitHub
  answered 500."
- **AND** the request still goes
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should say in Request Review… while Patronus names no code owners, when it fails to, and when the rules of the branch can't be read`)

### Requirement: Request Review: A refused push asks nobody

A refused push ("Cannot request a review on #1078" with its reason) MUST ask nobody.

#### Scenario: The push of the request is refused
- **WHEN** the push of the request is refused
- **THEN** the error says "Cannot request a review on #1078" with its reason
- **AND** nobody is asked

### Requirement: Request Review: Its hover names whom it asks

Its hover MUST name whom it asks: "Ask Ilya Korennoy to review #1078".

#### Scenario: The hover of Request Review
- **WHEN** the pointer hovers "Request Review…"
- **THEN** it says "Ask Ilya Korennoy to review #1078"

### Requirement: Request Review: The popover lists the code owners to ask

It MUST open `RequestReviewPopover`, mounted by the surface around its own trigger (Base UI `render`), with the branch,
its pull request and its review readiness: "Request a review on #1078" over the title of the pull request; "Code
owners", a checkbox per suggested code owner with a GitHub login of the groups whose approval is missing (the full
name, the login beside it, the groups below).

#### Scenario: The popover opens
- **WHEN** the user chooses "Request Review…"
- **THEN** the popover says "Request a review on #1078" over the title of the pull request
- **AND** "Code owners" has a checkbox per suggested code owner with a GitHub login of the groups whose approval is
  missing, the full name, the login beside it, the groups below
  [@test] crates/domain-policy/src/review_test.rs (`are_the_suggested_ones_of_the_missing_groups_one_per_group_nobody_is_asked_for_chosen_each_once`)

### Requirement: Request Review: One rule decides the candidates and the checked ones

One rule MUST decide the code owners for the popover and `brd request-review` alike (`domain_policy::review::candidates`,
`ReviewCandidate.preselected`), which the view of the session sends with the open pull request: never the author, a
member asked already or one whose approval counts, each once; checked when it opens: one per missing group nobody is
asked for yet, each once, as the Create Pull Request dialog chooses them.

#### Scenario: One per missing group nobody is asked for
- **WHEN** the candidates are decided
- **THEN** they are the suggested ones of the missing groups, each once
- **AND** one per group nobody is asked for yet is checked
  [@test] crates/domain-policy/src/review_test.rs (`are_the_suggested_ones_of_the_missing_groups_one_per_group_nobody_is_asked_for_chosen_each_once`)

#### Scenario: The author, the asked and an approval that counts
- **WHEN** a suggested code owner is the author, is asked already, or has an approval that counts
- **THEN** it is left out
- **AND** a group someone is asked for chooses nobody
  [@test] crates/domain-policy/src/review_test.rs (`leave_out_the_author_whoever_is_asked_already_and_an_approval_that_counts_a_group_someone_is_asked_for_chooses_nobody`)

#### Scenario: No approval missing or an unknown readiness
- **WHEN** no approval is missing, or the readiness is unknown
- **THEN** there are no candidates
  [@test] crates/domain-policy/src/review_test.rs (`are_none_when_no_approval_is_missing_or_the_readiness_is_unknown`)

### Requirement: Request Review: A readiness that comes after the opening checks them too

A readiness that comes after the opening MUST check the code owners too, until the user checks or unchecks one.

#### Scenario: The readiness of the whole stack comes after the opening
- **WHEN** the readiness comes after Request Review… opened
- **THEN** the code owner it names is checked
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should choose the code owner of a readiness of the whole stack that comes after its Request Review… opened`)

### Requirement: Request Review: While Patronus names none the popover says why

While Patronus names none the popover MUST say why, never nothing as if none were needed (`describeCodeOwnersFact`).

#### Scenario: Patronus checks, failed, or doesn't tell
- **WHEN** Patronus has not answered, its check failed, or its check doesn't judge the code owners
- **THEN** the popover says "Asking Patronus which code owners must approve…", "Patronus didn't answer which code
  owners must approve (…): ask the reviewers yourself.", or "Patronus doesn't tell yet which code owners must approve:
  ask the reviewers yourself."
  [@test] src/presenters/review.test.ts (`says while Patronus checks them, when its check failed or doesn't tell, never nothing as if none were needed`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should say in Request Review… while Patronus names no code owners, when it fails to, and when the rules of the branch can't be read`)

### Requirement: Request Review: Others searches reviewers and Request Review asks them all

"Others" (or "Reviewers" without code owners) MUST be the reviewer search of the dialog (`ReviewerPicker`,
`useReviewerSearch`). "Request Review" MUST ask the checked ones and the chosen ones.

#### Scenario: The checked and the chosen reviewers
- **WHEN** the user chooses reviewers in "Others" and presses "Request Review"
- **THEN** the checked code owners and the chosen reviewers are asked

### Requirement: Request Review: The row's slot mounts the popover

On the surfaces ([Next Step](../next-step/spec.md), row 9) the row's slot MUST mount the popover around its step, or,
for another step, around an anchor at its end, which "Request Review…" of the branch menu opens
(`request-review-popover`).

#### Scenario: Request Review at the row's step
- **WHEN** the user asks for a review from the row's step
- **THEN** the popover opens around the step, and the row waits for the code owner from the click on
  [@test] tests/e2e/next-step/next-step.spec.ts (`should ask the code owner nobody asked yet from Request Review…, then wait for him from the click on`)

#### Scenario: Request Review from the branch menu
- **WHEN** the user chooses "Request Review…" in the branch menu
- **THEN** the popover opens at the row's next step
  [@test] tests/e2e/next-step/next-step.spec.ts (`should open Request Review… from the branch menu, at the row's next step`)

### Requirement: Request Review: The Safe Push popover has no Request Review

The Safe Push popover MUST have no Request Review…: a start whose approvals are missing is refused, and the row's step
asks for them.

#### Scenario: A start whose approvals are missing
- **WHEN** the Safe Push popover opens for a branch whose approvals are missing
- **THEN** it has no Request Review…, and the row's step asks for them

### Requirement: Request Review: The pull request of the whole stack has its own

The pull request of the whole stack MUST have its own Request Review…, in the actions of the whole stack
([Whole Stack](../whole-stack/spec.md#the-whole-stack-in-the-app)): its popover at the header of the island, else at an
anchor of the root's slot (`WholeStackRequestReviewAnchor`).

#### Scenario: Request Review of the whole stack
- **WHEN** the user asks for a review of the pull request of the whole stack
- **THEN** its popover opens at the header of the island, else at an anchor of the root's slot

### Requirement: Request Review: Without Patronus it still opens from every menu

Request Review… asks GitHub, not Patronus: in a repository without Patronus, where a row and the header of an island
have no next step, it MUST still open from every menu that offers it (the branch menu, "⋯" of the row and of the
preview, "Whole Stack ›" of the root, "⋯" of the island), never a dead item; no code owners without Patronus, the
search finds whom to ask.

#### Scenario: The branch menu, the row's and the preview's ⋯
- **GIVEN** a repository without Patronus
- **WHEN** the user chooses Request Review… in the branch menu, "⋯" of the row or "⋯" of the preview
- **THEN** the popover opens at the row
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should ask for a review from the branch menu, the row's ⋯ and the preview, the popover at the row`)

#### Scenario: Whole Stack of the root and the island's ⋯
- **GIVEN** a repository without Patronus
- **WHEN** the user chooses Request Review… in "Whole Stack ›" of the root's menu, or "⋯" of the island
- **THEN** the popover opens at the root's row, or at the header
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask for a review of the pull request of a small stack without Patronus from Whole Stack › of the root's menu, the popover at the root's row`, `should ask for a review of the pull request of the whole stack without Patronus from the island's ⋯, the popover at the header`)

### Requirement: Request Review: Its anchors take no room in the step column

Its popover MUST open at anchors that take no room in the step column of the Actions cell, so "⋯" of the row stands
where it stands in every other row (`RequestReviewAnchors`, `GaplessAnchors`).

#### Scenario: A row without a next step
- **GIVEN** a repository without Patronus
- **WHEN** a row shows its Actions cell
- **THEN** "⋯" of the row stands where it stands in every other row

### Requirement: Request Review: The request asks each login once, split only on a 422

The request MUST be the last effect of `RequestReview` (`RequestReviewers`, after its push:
`POST pulls/<n>/requested_reviewers`, users only, no team), each login once: one request, and one per reviewer only
when GitHub refuses the list with a 422 (it names nobody), to tell whom it refuses (the author, a user without access);
a login it accepts but doesn't list MUST be refused too ("GitHub left the reviewer out").

#### Scenario: The reviewers of an open pull request
- **WHEN** the request runs
- **THEN** one `POST pulls/<n>/requested_reviewers` asks each login once
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`asks_the_reviewers_of_an_open_pull_request_once_each`)

#### Scenario: GitHub refuses the list with a 422
- **WHEN** GitHub refuses the list with a 422
- **THEN** each reviewer is asked alone, to name whom GitHub refuses
- **AND** a login GitHub accepts but doesn't list is refused ("GitHub left the reviewer out")
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_refused_list_is_asked_one_by_one_to_name_whom_github_refuses`)

### Requirement: Request Review: Another refusal fails the request

Another refusal (no right to write, no such pull request), also of one reviewer after the split, MUST fail the
request; the token chain tries the next token after a 401. The creation of a pull request asks its reviewers the same
way.

#### Scenario: GitHub refuses the request as a whole
- **WHEN** GitHub refuses the request otherwise
- **THEN** the request fails
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_request_github_refuses_as_a_whole_fails_and_no_login_asks_nothing`)

#### Scenario: A rejected token after the split
- **WHEN** GitHub rejects the token after the split
- **THEN** the request fails
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_rejected_token_after_the_split_fails_the_request`)

### Requirement: Request Review: The request holds the reviewers of its pull request

The request MUST hold the reviewers of its pull request: a retarget of it runs beside, unless the push of the request
retargets the pull request too (it goes into a virtual branch), which holds its base: then it waits for the retarget and
is planned again over the base it left, the PATCH of the base before the request.

#### Scenario: A retarget during a review request
- **WHEN** a retarget of the pull request runs while a review is requested in it
- **THEN** both reach GitHub, neither refuses the other
  [@test] tests/e2e/next-step/next-step.spec.ts (`should retarget a pull request while a review is requested in it: both reach GitHub, neither refuses the other`)
  [@test] crates/operations/src/pull_request_test.rs (`a_review_request_waits_for_a_retarget_of_its_pull_request_only_when_its_push_retargets_it`)

#### Scenario: The push of the request retargets the pull request
- **WHEN** the push of the request retargets the pull request into the target
- **THEN** the request waits for the retarget and is planned again over the base it left
- **AND** the PATCH of the base comes before the request, then the request asks whom its plan names
  [@test] crates/operations/src/pull_request_test.rs (`a_review_request_retargets_and_pushes_first_then_asks_whom_its_plan_names`, `a_review_request_waits_for_a_retarget_of_its_pull_request_only_when_its_push_retargets_it`)

### Requirement: Request Review: The result is a toast with Open Pull Request

The result MUST be a toast with "Open Pull Request": "Requested a review from Ilya Korennoy on #1078", a `warning`
whose description names whom GitHub refused ("GitHub refused Bob: …").

#### Scenario: GitHub asked the reviewers
- **WHEN** GitHub asked the reviewers, and refused some
- **THEN** the toast says "Requested a review from Ilya Korennoy on #1078"
- **AND** it is a `warning` whose description says "GitHub refused Bob: …"
  [@test] src/presenters/review.test.ts (`names whom GitHub asked, and whom it refused as a warning`)

### Requirement: Request Review: A request that asked nobody keeps the popover

A request GitHub asked nobody in, or one that failed, MUST be "Cannot request a review on #1078" with the reason, and
the popover MUST stay.

#### Scenario: GitHub asked nobody
- **WHEN** GitHub asked nobody, or the request failed
- **THEN** the error says "Cannot request a review on #1078" with the reason, and the popover stays
  [@test] src/presenters/review.test.ts (`is none when GitHub asked nobody: the error says why`)

### Requirement: Request Review: Whom GitHub asked is requested in the row at once

Whom GitHub asked MUST be requested in the pull request of the row at once, and a refresh of the pull requests MUST
follow: the review readiness reads the new request.

#### Scenario: After the request
- **WHEN** GitHub asked the reviewers
- **THEN** they are requested in the pull request of the row at once
- **AND** a refresh of the pull requests follows, and the review readiness reads the new request

### Requirement: Request Review: brd request-review runs the same intent

`brd request-review <branch>` MUST run the same intent from the agent CLI: the same code owners, `--reviewer <login>`
others; work the remote lacks refuses it (`needs_push`) until `--push` pushes it first
([Agent CLI](../../cli/agent-cli/spec.md#brd-request-review)).

#### Scenario: Request a review from the agent CLI
- **WHEN** an agent runs `brd request-review <branch>`
- **THEN** it asks the same code owners, and `--reviewer <login>` others
- **AND** work the remote lacks refuses it with `needs_push` until `--push` pushes it first

### Requirement: Branch preview: The preview shows the pull request and opens it with O

The meta line of the [branch preview](../branch-preview/spec.md) MUST end with the chip; the body MUST start with a
pull request section (the details of the chip popover) above the commits; `O` and the footer button "Open Pull Request #7"
(the item of the menu) open it. A branch without a pull request has neither.

#### Scenario: The preview of a branch with a pull request
- **WHEN** the preview of a branch with a pull request opens
- **THEN** its meta line ends with the chip and its body starts with the pull request section above the commits
- **AND** `O` and the footer button "Open Pull Request #7" open the pull request
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request in the branch preview and open it with O`)

#### Scenario: The preview of a branch without a pull request
- **WHEN** the preview of a branch without a pull request opens
- **THEN** it has neither the chip nor the section

### Requirement: Branch preview: The footer offers Create Pull Request where the menu does

Where the menu offers "Create Pull Request…", the footer MUST have it instead of "Open Pull Request": the preview closes
and the dialog opens; `O` keeps opening a closed pull request.

#### Scenario: Create Pull Request from the preview
- **WHEN** the user chooses "Create Pull Request…" in the footer of the preview
- **THEN** the preview closes and the dialog opens
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)

#### Scenario: A branch with a closed pull request
- **WHEN** the branch has only a closed pull request and the user presses `O` in the preview
- **THEN** the closed pull request opens

### Requirement: Landing overview: Each row shows its chip after its reason

The [landing overview](../../repository/landing-overview/spec.md#overview) MUST show the chip of each row after its
reason, before its next step.

#### Scenario: The rows of the overview
- **WHEN** the overview shows branches with pull requests
- **THEN** each row shows its chip after its reason, before its next step
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request chips of the rows, and a pull request that waits for its author as a fix`)

### Requirement: Landing overview: Off Space a branch lands through its pull request

Off Space, a branch MUST land through its pull request: an open one with changes requested or failing checks is a fix,
a move of the user, and the row says why (`landing-node.ts`). A pushed branch MUST stay on the way to master until it
lands.

#### Scenario: Changes requested or failing checks
- **GIVEN** a repository not on Space
- **WHEN** the open pull request of a branch has changes requested or failing checks
- **THEN** the branch is a fix, a move of the user
- **AND** the row says "bob requested changes on #761", "Changes requested on #800", or "Checks fail on #761"
  [@test] crates/domain-policy/src/landing_node_test.rs (`off_space_a_branch_lands_through_its_pull_request`)
  [@test] src/presenters/landing-node.test.ts (`lands by its pull request in a repository that is not on Space`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request chips of the rows, and a pull request that waits for its author as a fix`)

#### Scenario: A pushed branch whose pull request waits for its author
- **WHEN** a pushed branch's pull request waits for its author
- **THEN** the branch stays on the way to master until it lands
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should keep a pushed branch on the way to master while its pull request waits for its author`)

### Requirement: Landing overview: The footer says when the pull requests were checked

The footer of the overview MUST have a line "Pull requests checked just now" (it ages while shown, as every relative
time: [Update Project](../../repository/update-project/spec.md)) with a Refresh button, named "Refresh pull requests"
(the Patronus line has a Refresh button too).

#### Scenario: The footer after a listing
- **WHEN** the pull requests were listed
- **THEN** the footer says "Pull requests checked just now", ageing while shown
- **AND** its Refresh button is named "Refresh pull requests"
  [@test] src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`)

### Requirement: Landing overview: When GitHub can't be asked the line is a quiet hint

When GitHub can't be asked, the line MUST be a quiet hint instead, never a toast. A repository that is not on GitHub
MUST have no line.

#### Scenario: No token, or a rejected one
- **WHEN** there is no token, or GitHub rejected the last token tried
- **THEN** the line says "No GitHub token to see pull requests", or "GitHub rejected the token from gh" (the source of
  the last token tried: `GH_TOKEN`, `gh`, the Branch Deck sign-in, the git credentials)
- **AND** both have "Sign In…" ("GitHub Settings…" in a build without the sign-in), which opens Settings → Accounts at
  GitHub ([GitHub Sign-In](../../repository/github-sign-in/spec.md#where-it-is-offered))
  [@test] src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should hint quietly when GitHub rejects the token`)

#### Scenario: SAML SSO
- **WHEN** the organization requires SAML SSO
- **THEN** the line says "GitHub: the organization requires SAML SSO, authorize the token for it", with "Authorize…"
  when GitHub gave the URL
  [@test] src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`)

#### Scenario: No access, or another error
- **WHEN** the token has no access, or GitHub answers another error
- **THEN** the line says "GitHub: <message> (the token from gh)", or "GitHub: <the error>"
- **AND** for the token of the Branch Deck sign-in it adds that an organization that restricts OAuth apps has to
  approve Branch Deck, with "Request Access…" ([GitHub Sign-In](../../repository/github-sign-in/spec.md#where-it-is-offered))
  [@test] src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`)

#### Scenario: A repository not on GitHub
- **WHEN** the repository is not on GitHub
- **THEN** the footer has no line of the pull requests
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say nothing for a repository that is not on GitHub`)

## User Experience

- The chip is text-sized (`text-xs`, `size-3.5` icon in the hue, the number `text-muted-foreground` in tabular
  figures), a plain button that underlines on hover, with the `rounded-2xl` focus outline of buttons and badges. No chip has an outline: a draft shows by its icon and its muted
  hue. The dots are `size-1.5 rounded-full shrink-0`, as the status dot of the row.
- The notice of a changed branch is `text-xs text-warning` after the warning triangle; the line of a rebuilt one is
  `text-xs text-muted-foreground`, without an icon (`ReviewCommitNotice`, shared with the Patronus details).
- `info` is the merged colour; push states never use it ([Outgoing Commits](../../repository/outgoing/spec.md)).
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
  ([Remote services](../../../../docs/design/e2e-testing.md#remote-services)); `POST /repositories/{id}/pull-requests` sets
  what it holds (an empty `headRefOid` means the local tip; `availablePullRequests` answers for `acme/monorepo`), and
  `GET` returns how often the app asked and every request GitHub got. The opener plugin is mocked and records the
  opened URLs in `window.__OPENED_URLS__`. A push goes to the remote of the test repository, a bare
  `.git/upstream.git` ([The remote of a repository](../../../../docs/design/e2e-testing.md#the-remote-of-a-repository)): a
  branch pushed before is a push there behind the app's back (`pushInTerminal`, an earlier build of its commits for a
  pull request that shows one), and a test asserts the pushes the remote took (`remotePushes`).
  No test answers a GitHub write in the page (`routeCommand`, a fulfilling `page.route`): the registry test fails it,
  and the page answers of the reads are pinned and only go down (`ROUTED_IN_PAGE`).
  [@test] crates/test-server/src/command_registry_test.rs (`no_e2e_test_answers_a_write_in_the_page`)
  [@test] crates/test-server/src/command_registry_test.rs (`the_page_answers_of_the_e2e_tests_are_pinned`)
  [@test] crates/test-server/src/github_host_test.rs (`pull_requests_are_listed_created_retargeted_and_closed_through_the_real_service`)
  [@test] crates/test-server/src/github_host_test.rs (`a_fixture_pull_request_reads_back_through_the_real_parser`)

## Open Questions / Risks

- `author:@me` uses the GitHub search, which can lag a few seconds behind a newly opened pull request.
- A token of git may be a fine-grained token without access to pull requests: GitHub answers 403, and the hint names
  the git credentials as the source.
- Only the 50 most recent pull requests of the user are read; an old pull request of a long-lived branch may be missed.
- A branch pushed to a fork as `origin`, with the pull request on the upstream repository, is not matched.
- The Space settings file is a snapshot committed by the repository's admins: a mirror configured only in Space, not in
  the file, needs `branchdeck.githubRepository` (Settings → Landing).

## References

- [Branch Row](../branch-row/spec.md): the Status cell and the anatomy of the chip details
- [Outgoing Commits](../../repository/outgoing/spec.md)
- [Branch Preview](../branch-preview/spec.md)
- [Patronus](../patronus/spec.md): the other review status, shown next to the chip
- [Review Requests](../../repository/review-requests/spec.md): the pull requests of others that wait for the user
- [Remote Status for Virtual Branches](../../../../docs/design/remote-status.md)
