---
name: GitHub Sign-In
description: Requirements for the GitHub token without the gh CLI - the app's own sign-in through the OAuth device flow of the Branch Deck OAuth app (a code to enter on GitHub, then a token in a 0600 file shared with brd), shown in Settings next to the token the app takes from GH_TOKEN, gh or git, and offered by the hint of a repository without a token.
style: plain-1
targets:
  - crates/github/src/device_flow.rs
  - crates/github/src/sign_in_session.rs
  - crates/remote-facts/src/sign_in.rs
  - crates/remote-facts/src/scheduler_auth.rs
  - crates/domain-types/src/sign_in.rs
  - crates/domain-policy/src/refresh.rs
  - crates/app-api/src/api_error.rs
  - crates/github/src/token_store.rs
  - crates/sync-utils/src/private_file.rs
  - crates/sync-utils/src/secret_store.rs
  - crates/github/src/token.rs
  - crates/domain-types/src/pull_request.rs
  - src-tauri/src/services.rs
  - crates/app-api/src/remote/accounts.rs
  - crates/app-api/src/app/sign_in.rs
  - crates/app-api/src/events.rs
  - src-tauri/src/lib.rs
  - crates/bd-core/src/context.rs
  - crates/test-server/src/session_routes.rs
  - crates/test-server/src/host.rs
  - crates/test-server/src/remote_host.rs
  - src/stores/auth.ts
  - src/model/store.ts
  - src/lib/sign-in-run.ts
  - src/components/settings/GitHubSettings.tsx
  - src/components/settings/AccountsPage.tsx
  - src/components/repository/ReviewStatusLine.tsx
  - src/components/repository/PullRequestStatusLine.tsx
  - src/components/repository/YourTurnCard.tsx
  - src/lib/pull-request.ts
  - src/hooks/useAvailabilityAction.ts
  - src/hooks/useReviewStatusRefresh.ts
---

# GitHub Sign-In

Status: Implemented
Date: 2026-10-16

## Purpose

The pull requests, Your Turn and the writes to GitHub need a token ([Pull Requests](../../branch/pull-requests/spec.md#the-token)).
Most users have one already: `gh`, `GH_TOKEN`, or the HTTPS credentials git keeps for github.com. A user with none
(SSH remotes, no `gh`) signs in in the app: Settings shows a code, the user enters it on GitHub, and the app keeps the
token. This is the OAuth device flow of the Branch Deck OAuth app, the way `gh auth login` works, without a browser
redirect and without a client secret.

## Goals

- Pull requests and review requests without installing `gh`.
- One place that says where the token comes from, and how to get one.
- `brd` uses the same sign-in.

## Non-goals

- A GitHub App: it works only where it is installed, which needs an admin of every organization, and its user
  tokens expire after 8 hours, so the app and `brd` would race to refresh one token.
- Revoking the token: that needs the client secret. Sign Out removes it here; GitHub revokes it on its settings page.
- GitHub Enterprise, several accounts.
- A keychain item of the app itself: unsigned local builds would make macOS ask again for the item after every
  update, and `brd` in the background could not answer. The item is written by `/usr/bin/security` instead
  ([The token file](#the-token-file)).

## Requirements

### Requirement: Sign-in: The client id of the OAuth app is given at build time

The client id of the Branch Deck OAuth app SHALL be given at build time (`BRANCH_DECK_GITHUB_CLIENT_ID`,
`GITHUB_OAUTH_CLIENT_ID`), by the release workflow (`.github/workflows/build-tauri.yml`). The device flow MUST be
enabled in the settings of the OAuth app.

#### Scenario: A release build
- **WHEN** the release workflow builds the app
- **THEN** it gives the client id of the Branch Deck OAuth app at build time
- **AND** the device flow is enabled in the settings of the OAuth app

### Requirement: Sign-in: A build without the client id offers no sign-in of its own

A build without the client id MUST offer no sign-in of its own: Settings says what gives a token, and the hint's
button is "GitHub Settings…" ([Where it is offered](#where-it-is-offered)).

#### Scenario: A build without the OAuth app
- **GIVEN** a build without the client id
- **WHEN** the user looks for a sign-in
- **THEN** the build signs in nowhere and Settings says what gives a token
- **AND** the hint's button is "GitHub Settings…"
  [@test] crates/github/src/sign_in_session_test.rs (`a_cancel_ends_the_wait_and_a_build_without_the_oauth_app_signs_in_nowhere`)
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)

### Requirement: Sign-in: Sign In to GitHub asks for a device code of the app's scopes

"Sign In to GitHub" MUST ask GitHub for a device code (`POST github.com/login/device/code`, `scope=repo read:org`,
`Accept: application/json`): `repo` for private repositories and the writes to pull requests, `read:org` for the
slug of a team asked to review.

#### Scenario: The device code is asked for the scopes of the app
- **WHEN** the user clicks "Sign In to GitHub"
- **THEN** the app asks `POST github.com/login/device/code` with `scope=repo read:org` and `Accept: application/json`
  [@test] crates/github/src/device_flow_test.rs (`the_device_code_is_asked_for_the_scopes_of_the_app`)

### Requirement: Sign-in: The app polls for the token every interval

The app MUST poll `login/oauth/access_token` (`grant_type=urn:ietf:params:oauth:grant-type:device_code`) every
`interval` seconds: `authorization_pending` waits, `slow_down` takes the new interval (else 5 s more), a token ends
it; `expired_token`, `access_denied` and `device_flow_disabled` end it with an error, as does the code's
`expires_in`. A network failure MUST be asked again at the next interval.

#### Scenario: Pending, then slow down, then the token
- **WHEN** GitHub answers `authorization_pending`, then `slow_down`, then a token
- **THEN** the app waits, takes the new interval of `slow_down` (else 5 s more), and the token ends the wait
  [@test] crates/github/src/device_flow_test.rs (`the_wait_polls_while_pending_slows_down_when_told_and_takes_the_token`)

#### Scenario: A denied or expired code
- **WHEN** GitHub answers `expired_token` or `access_denied`, or the code's `expires_in` has passed
- **THEN** the wait ends with an error
  [@test] crates/github/src/device_flow_test.rs (`a_denied_or_expired_code_and_a_cancel_end_the_wait`)

#### Scenario: The device flow is disabled
- **WHEN** GitHub answers `device_flow_disabled`
- **THEN** the wait ends with an error

#### Scenario: A network failure
- **WHEN** a poll fails on the network
- **THEN** it is asked again at the next interval

### Requirement: Sign-in: A sign-in is a command of the app

A sign-in MUST be a command of the app, not an intent of a repository: the token is one for the app and Settings
opens without a repository too ([Rust as the Domain Authority](../../../../docs/design/rust-domain-authority.md),
"Решение 2026-10-28").

#### Scenario: Settings without a repository
- **GIVEN** Settings opened without a repository
- **WHEN** the user signs in
- **THEN** the sign-in runs as a command of the app, and the token is one for the app

### Requirement: Sign-in: The sign-in runs in a task of its own and tells its events

`sign_in({ provider: "gitHub" }, channel)` MUST run it in a task of its own (`remote_facts::sign_in::SignIns` over
`GitHubSignInSession`) and tell its events on the channel of the call, `done` last: `waiting` while GitHub is asked
for the code, `deviceCode` (`code`, `url`, the seconds `expiresIn` it has left), `done` with the login; the call
MUST answer how it ended.

#### Scenario: The events of a sign-in
- **WHEN** a sign-in runs to its end
- **THEN** its channel hears `waiting`, then `deviceCode` (`code`, `url`, `expiresIn`), then `done` with the login
- **AND** the call answers how it ended
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_sign_in_joins_the_one_that_runs_hears_its_code_and_ends_with_it`)
  [@test] crates/test-server/src/github_host_test.rs (`the_sign_in_runs_the_device_flow_of_the_fake_and_ends_with_its_repository`)

### Requirement: Sign-in: The lock of the sign-ins only registers a run

The lock of the sign-ins MUST only register a run, and MUST NOT span a request of GitHub.

#### Scenario: GitHub is asked while a sign-in runs
- **WHEN** a sign-in asks GitHub for the code or the token
- **THEN** the lock of the sign-ins is not held across the request

### Requirement: Sign-in: A second call while one runs joins it

One sign-in at a time: a second call while one runs (another window, a second click) MUST join it: it hears the
current event (the code, with the seconds left since GitHub gave it), then the rest, and answers the same end;
GitHub is asked for one code.

#### Scenario: A second window signs in while the code is shown
- **GIVEN** a sign-in whose code GitHub waits for
- **WHEN** a second call signs in
- **THEN** it hears the code, with the seconds left since GitHub gave it, then the rest, and answers the same end
- **AND** GitHub is asked for one code
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_sign_in_joins_the_one_that_runs_hears_its_code_and_ends_with_it`)

### Requirement: Sign-in: A window runs one sign-in of GitHub itself

A window MUST run one sign-in of GitHub itself (`runSignIn` of `src/lib/sign-in-run.ts`), its button disabled while
it does.

#### Scenario: A second click in the same window
- **GIVEN** a sign-in the window runs
- **WHEN** the user clicks again
- **THEN** the window starts nothing, its button disabled while the sign-in runs
  [@test] src/stores/auth.test.ts (`shows the code while GitHub waits for it, then tells who signed in`)

### Requirement: Sign-in: A cancel or a sign-out ends the sign-in at once as canceled

`cancel_sign_in("gitHub")` and a sign-out MUST end the sign-in that runs at once with the error code `canceled` (the
waiting request dropped, the code forgotten: nothing pending); a sign-in after it MUST start anew, with a code of its
own, never joining the canceled one.

#### Scenario: A cancel while GitHub waits
- **GIVEN** a sign-in whose code GitHub waits for
- **WHEN** `cancel_sign_in("gitHub")` runs
- **THEN** the sign-in ends at once with the error code `canceled`, the waiting request dropped and nothing pending
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_cancel_ends_the_sign_in_at_once_as_canceled_and_the_next_one_starts_anew`)
  [@test] crates/github/src/sign_in_session_test.rs (`a_cancel_ends_the_wait_and_a_build_without_the_oauth_app_signs_in_nowhere`)
  [@test] crates/app-api/src/api_error_test.rs (`api_error_of_each_contract_case_is_its_json_in_the_fixture`)
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

#### Scenario: A sign-out while a sign-in runs
- **GIVEN** a sign-in whose code GitHub waits for
- **WHEN** the user signs out
- **THEN** the sign-in ends at once with the error code `canceled`, then the token is removed
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_sign_out_cancels_the_sign_in_that_runs_then_removes_the_token`)

#### Scenario: The next sign-in starts anew
- **GIVEN** a canceled sign-in
- **WHEN** the user signs in again
- **THEN** the sign-in starts anew with a code of its own, never joining the canceled one
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_cancel_ends_the_sign_in_at_once_as_canceled_and_the_next_one_starts_anew`)

### Requirement: Sign-in: A canceled sign-in is no error

`canceled` is no error: the page MUST tell nothing of it, by the code, never by the words of the error.

#### Scenario: The page hears a canceled sign-in
- **WHEN** a sign-in ends with the error code `canceled`
- **THEN** the page tells nothing of it
- **AND** an error whose words say canceled but whose code does not is told as an error
  [@test] src/stores/auth.test.ts (`says nothing when canceled, and why when GitHub refused, by the code of the error and not its words`)
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

### Requirement: Sign-in: With the token the app keeps it with the login of its user

With the token, the app MUST ask `GET /user` for the login, keep the token and forget what GitHub accepted and
denied (`TokenChain::forget`).

#### Scenario: The user entered the code
- **WHEN** GitHub answers the poll with a token
- **THEN** the app asks `GET /user` for the login and keeps the token
- **AND** it forgets what GitHub accepted and denied, so GitHub is asked again
  [@test] crates/github/src/sign_in_session_test.rs (`a_sign_in_polls_until_the_code_is_entered_then_keeps_the_token_of_its_user`)
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)

### Requirement: Sign-in: A change of the account tells the host once

A sign-in or a sign-out that changed the account MUST tell the host once, however many calls joined it
(`SignInHost::changed`).

#### Scenario: Two calls of one sign-in
- **WHEN** a sign-in that two calls joined changes the account
- **THEN** the host hears it once
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_sign_in_joins_the_one_that_runs_hears_its_code_and_ends_with_it`)

#### Scenario: A sign-in, then a sign-out
- **WHEN** a sign-in and then a sign-out change the account, a canceled sign-in between them
- **THEN** the host hears the sign-in and the sign-out once each, never the canceled one
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_sign_out_cancels_the_sign_in_that_runs_then_removes_the_token`)

### Requirement: Sign-in: Both hosts hand a change of the account to one rule

Both hosts, the app and the e2e server, MUST hand a change of the account to one rule
(`app_api::app::sign_in::account_changed`): every session reads its fact `auth` again (`AfterWrite`) and lists the
pull requests and the review requests again (`Manual`), and every window hears `auth-changed`, on which a window
without a session reads `get_auth_status` again.

#### Scenario: A change of the GitHub account
- **WHEN** a sign-in or a sign-out changed the GitHub account
- **THEN** every session reads its fact `auth` again (`AfterWrite`) and lists the pull requests and the review requests again (`Manual`)
- **AND** every window hears `auth-changed`
  [@test] crates/app-api/src/app/sign_in_test.rs (`a_change_of_an_account_reads_the_sign_ins_again_and_after_github_its_lists_too`)
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)

#### Scenario: A window without a session
- **WHEN** a window without a session hears `auth-changed`
- **THEN** it reads `get_auth_status` again

### Requirement: Sign-in: The status of the sign-ins is the fact auth of every session

The status of the sign-ins (`AuthStatus`: GitHub's below, YouTrack's of [YouTrack](../youtrack/spec.md)) MUST be the
fact `auth` of every session (`FactsView.auth`), read by the rule `domain_policy::refresh::auth_due`.

#### Scenario: A session holds the status of the sign-ins
- **WHEN** a session reads the status of the sign-ins
- **THEN** it is the fact `auth` of the session, read when the rule `domain_policy::refresh::auth_due` says so
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_sign_ins_are_read_when_a_window_opens_the_session_after_a_sign_in_and_on_a_late_focus`)

### Requirement: Sign-in: The fact auth is read on an open, after a sign-in and on a focus

The fact `auth` MUST be read when a window opens the session, after a sign-in or a sign-out of any window and by the
refresh button, once more after a read in flight for those; on a focus by the rule of the lists (at least a minute
after the last read and when it is two minutes old); it MUST NOT be read by a sync, a push or a poll.

#### Scenario: An open, a sign-in or the refresh button
- **WHEN** a window opens the session, a window signs in or out, or the user clicks the refresh button
- **THEN** the session reads the fact `auth`, once more after a read in flight for those
  [@test] crates/domain-policy/src/refresh_test.rs (`the_sign_ins_are_read_when_the_session_opens_after_a_sign_in_and_on_a_focus_by_the_rule_of_the_lists`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_sign_ins_are_read_when_a_window_opens_the_session_after_a_sign_in_and_on_a_late_focus`)

#### Scenario: A focus
- **WHEN** a window takes the focus
- **THEN** the fact `auth` is read only at least a minute after the last read and when it is two minutes old
  [@test] crates/domain-policy/src/refresh_test.rs (`the_sign_ins_are_read_when_the_session_opens_after_a_sign_in_and_on_a_focus_by_the_rule_of_the_lists`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_sign_ins_are_read_when_a_window_opens_the_session_after_a_sign_in_and_on_a_late_focus`)

#### Scenario: A sync, a push or a poll
- **WHEN** a sync, a push or a poll runs
- **THEN** the fact `auth` is not read
  [@test] crates/domain-policy/src/refresh_test.rs (`the_sign_ins_are_read_when_the_session_opens_after_a_sign_in_and_on_a_focus_by_the_rule_of_the_lists`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_sign_ins_are_read_when_a_window_opens_the_session_after_a_sign_in_and_on_a_late_focus`)

### Requirement: Sign-in: A window of a repository takes the status from its session

A window of a repository MUST take the status from its session: the store of the session puts the fact into the
status of the window with every view (`applyAuthFacts` of `src/stores/auth.ts`, called by `src/model/store.ts`),
which keeps the last status until a new session answered and says why a read failed.

#### Scenario: Every view of the session
- **WHEN** the session sends its first view or a patch of it
- **THEN** the store of the session puts the fact `auth` into the status of the sign-ins of the window
  [@test] src/model/store.test.ts (`puts the fact auth of its session into the status of the sign-ins of the window, from its first view and from each patch of it`)

#### Scenario: A new session, a failed read
- **WHEN** a new session has not answered yet, or a read of the fact failed
- **THEN** the window keeps the last status, and says why a read failed
  [@test] src/stores/auth.test.ts (`comes from the fact of the session, which keeps the last status until it answered and says why a read failed`)

### Requirement: Sign-in: The root of every window loads only the store of the status

The root of every window MUST load the store of the status alone, never the store of a session: a window without a
session reads the status itself.

#### Scenario: A window without a session
- **WHEN** a window without a session opens
- **THEN** its root loads the store of the status, not the store of a session, and reads the status itself

### Requirement: The token file: The token is a secret of the secret store that brd reads too

The token MUST be kept as `{ token, login, scopes, created_at }`, which `brd` reads too, in a secret of
`sync_utils::secret_store` ([GitHub credentials](../../../../docs/design/git-operations.md#github-credentials)).

#### Scenario: A token is written and read back
- **WHEN** a sign-in keeps its token
- **THEN** `{ token, login, scopes, created_at }` is written into the secret, and read back as it was
  [@test] crates/github/src/token_store_test.rs (`a_token_is_written_and_read_back`)

### Requirement: The token file: On macOS the secret is a login keychain item

On macOS the secret MUST be the login keychain item `org.develar.branch-deck` / `github-token`, written and read by
`/usr/bin/security` with the secret on stdin, as `gh` does, so an update of the unsigned app asks nothing.

#### Scenario: The secret in the keychain
- **GIVEN** macOS with a usable keychain
- **WHEN** the token is written and read
- **THEN** `/usr/bin/security` writes and reads the item `org.develar.branch-deck` / `github-token` with the secret on stdin, and no file is written
  [@test] crates/sync-utils/src/secret_store_test.rs (`a_secret_lives_in_the_keychain_through_stdin_and_hex_without_a_file`)

### Requirement: The token file: Elsewhere the secret is a 0600 file of the app data directory

Elsewhere, and while the keychain can't be used, the secret MUST be `github-token.json` in the app data directory,
written as a temporary file created 0600 in the same directory and renamed over the old one.

#### Scenario: Without the keychain
- **GIVEN** no keychain
- **WHEN** the token is written
- **THEN** it is `github-token.json`, written as a temporary file created 0600 in the same directory and renamed over the old one
  [@test] crates/sync-utils/src/private_file_test.rs (`a_file_is_written_for_the_user_only_and_read_back`)
  [@test] crates/sync-utils/src/secret_store_test.rs (`without_the_keychain_a_secret_is_the_0600_file`)

#### Scenario: A locked keychain
- **GIVEN** a keychain that can't be used
- **WHEN** the token is written
- **THEN** it stays in the file, and is read back from it
  [@test] crates/sync-utils/src/secret_store_test.rs (`a_locked_keychain_leaves_the_secret_in_the_file`)

### Requirement: The token file: A file others may read is not used

A token file others may read MUST NOT be used, and a warning MUST say why.

#### Scenario: A file others may read
- **GIVEN** a token file others may read
- **WHEN** the token is read
- **THEN** the file is not used, and a warning says why
  [@test] crates/sync-utils/src/private_file_test.rs (`a_file_others_may_read_is_not_used`)

### Requirement: The token file: A file of an earlier version moves into the keychain

A file of an earlier version MUST be copied into the keychain on the first read and removed by the next sign-in or
sign-out.

#### Scenario: The first read after an update
- **GIVEN** the token file of an earlier version
- **WHEN** the token is read the first time, then a sign-in or a sign-out writes it
- **THEN** the first read copies it into the keychain, and the next sign-in or sign-out removes the file
  [@test] crates/sync-utils/src/secret_store_test.rs (`the_file_of_an_earlier_version_is_copied_into_the_keychain_and_removed_by_the_next_write`)

### Requirement: The token file: The token of the file comes after GH_TOKEN and gh, before git

The token of the file MUST come after `GH_TOKEN` and `gh`, before git ([Pull Requests](../../branch/pull-requests/spec.md#the-token)):
an organization that restricts OAuth apps has usually approved `gh`, and the Branch Deck app only once an admin did.

#### Scenario: The chain with every source
- **GIVEN** a token of `gh`, the token of the sign-in and credentials of git
- **WHEN** the app takes a token, and GitHub rejects each in turn
- **THEN** it takes the token of `gh` first, then the token of the sign-in, then the credentials of git
  [@test] crates/github/src/token_test.rs (`the_sign_in_of_the_app_comes_after_gh_and_before_git`)

### Requirement: The token file: Sign Out removes the file and tells every window

Sign Out (`sign_out("gitHub")`) MUST remove the file and tell every window.

#### Scenario: Sign Out
- **GIVEN** the app signed in
- **WHEN** the user clicks Sign Out
- **THEN** the token file is removed and every window hears of it
  [@test] crates/github/src/sign_in_session_test.rs (`a_sign_in_polls_until_the_code_is_entered_then_keeps_the_token_of_its_user`)
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

### Requirement: The token file: Manage on GitHub opens the page of the app on GitHub

"Manage on GitHub" MUST open `github.com/settings/connections/applications/<client id>`: the organizations the app
may see, and revoking it.

#### Scenario: Manage on GitHub
- **WHEN** the user clicks "Manage on GitHub"
- **THEN** `github.com/settings/connections/applications/<client id>` opens, with the organizations the app may see and revoking it
  [@test] crates/github/src/sign_in_session_test.rs (`a_sign_in_polls_until_the_code_is_entered_then_keeps_the_token_of_its_user`)

### Requirement: Settings: The GitHub section asks again on every open and every sign-in

The GitHub section of Settings → Accounts ([Settings](../settings/spec.md)) MUST ask for what it says again on every
open of the window and on every sign-in or sign-out of any window (`get_auth_status`, `auth-changed`).

#### Scenario: The window opens again
- **GIVEN** a sign-in elsewhere since the window was open
- **WHEN** the window opens again
- **THEN** the section asks `get_auth_status` again and says who signed in
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

#### Scenario: A sign-in or a sign-out of any window
- **WHEN** any window hears `auth-changed`
- **THEN** the section asks again and says what it answers
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)

### Requirement: Settings: Signed in, the section names the login

Signed in, the section MUST say "Signed in as @octocat", with Sign Out and Manage on GitHub; while `gh` or `GH_TOKEN`
comes first, it MUST add "; the token of gh comes first while GitHub accepts it".

#### Scenario: Signed in
- **GIVEN** the app signed in as `octocat`
- **WHEN** the section opens
- **THEN** it says "Signed in as @octocat", with Sign Out and Manage on GitHub
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

#### Scenario: Signed in while gh comes first
- **GIVEN** the app signed in, and `gh` or `GH_TOKEN` comes first
- **WHEN** the section opens
- **THEN** it adds "; the token of gh comes first while GitHub accepts it"

### Requirement: Settings: Not signed in, the section names the source taken first

Else the section MUST name the source the app takes first, without git (whose credentials are per repository):
"Uses the token of gh".

#### Scenario: The token of gh
- **GIVEN** no sign-in, and a token of `gh`
- **WHEN** the section opens
- **THEN** it says "Uses the token of gh"
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)

#### Scenario: Only the credentials of git
- **GIVEN** no sign-in, and only credentials of git
- **WHEN** the app names the source it takes first
- **THEN** it names none, as the credentials of git are per repository
  [@test] crates/github/src/token_test.rs (`the_sign_in_of_the_app_comes_after_gh_and_before_git`)

### Requirement: Settings: Without a token, the section says what gives one

Else the section MUST say "No GitHub token: run gh auth login, set GH_TOKEN, store HTTPS credentials of github.com in git, or sign in here",
with "Sign In to GitHub"; a build without the sign-in MUST say "No GitHub token: run gh auth login, set GH_TOKEN, or store HTTPS credentials of github.com in git".

#### Scenario: No token in a build with the sign-in
- **GIVEN** no token, and a build with the sign-in
- **WHEN** the section opens
- **THEN** it says "No GitHub token: run gh auth login, set GH_TOKEN, store HTTPS credentials of github.com in git, or sign in here", with "Sign In to GitHub"
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

#### Scenario: No token in a build without the sign-in
- **GIVEN** no token, and a build without the sign-in
- **WHEN** the section opens
- **THEN** it says "No GitHub token: run gh auth login, set GH_TOKEN, or store HTTPS credentials of github.com in git", and no "Sign In to GitHub"
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)

### Requirement: Settings: A note says what the sign-in can't do by itself

Before the sign-in, and while its token is the one the app takes, a note (`text-xs text-muted-foreground`) MUST say
what the sign-in can't do by itself: "An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck",
then "; gh is often approved there already" before the sign-in, ": request it in Manage on GitHub" after it: the
reason `gh` comes first in the chain, told before the user picks a token.

#### Scenario: Before the sign-in
- **GIVEN** a build with the sign-in and no token
- **WHEN** the section opens
- **THEN** the note says "An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck" and "; gh is often approved there already"
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)

#### Scenario: After the sign-in
- **GIVEN** the token of the sign-in is the one the app takes
- **WHEN** the section shows it
- **THEN** the note ends with ": request it in Manage on GitHub"
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)

### Requirement: Settings: No note for the token of gh or in a build without the sign-in

There MUST be no note while the token of `gh` or `GH_TOKEN` is the one taken, nor in a build without the sign-in.

#### Scenario: The token of gh is taken
- **GIVEN** the token of `gh` or `GH_TOKEN` is the one taken, or a build without the sign-in
- **WHEN** the section opens
- **THEN** it shows no note

### Requirement: Settings: While GitHub waits, the section shows the code

While GitHub waits, the section MUST show "Enter this code on GitHub to sign in Branch Deck", the code
(`font-mono text-lg`), "Copy Code and Open GitHub" (copies it and opens the verification page), Cancel, and
"Waiting for GitHub · expires in 14:32" with a spinner, renewed every second.

#### Scenario: The code and its copy
- **GIVEN** a sign-in whose code GitHub waits for
- **WHEN** the user clicks "Copy Code and Open GitHub"
- **THEN** the code is shown (`font-mono text-lg`) with "Waiting for GitHub · expires in 14:32" and a spinner, renewed every second
- **AND** the code is copied and the verification page opens
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)
  [@test] src/stores/auth.test.ts (`shows the code while GitHub waits for it, then tells who signed in`)

#### Scenario: Cancel
- **GIVEN** a sign-in whose code GitHub waits for
- **WHEN** the user clicks Cancel
- **THEN** the sign-in ends and "Sign In to GitHub" is shown again
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

### Requirement: Settings: The toast of a sign-in names the login

The toast of a sign-in MUST be "Signed in to GitHub as @octocat".

#### Scenario: A sign-in ends
- **WHEN** a sign-in ends with the login `octocat`
- **THEN** the toast says "Signed in to GitHub as @octocat"
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)
  [@test] src/stores/auth.test.ts (`shows the code while GitHub waits for it, then tells who signed in`)

### Requirement: Settings: A failed sign-in tells the reason

A failure of a sign-in MUST be "Cannot sign in to GitHub" with the reason.

#### Scenario: GitHub refused
- **WHEN** a sign-in fails with an error that is not `canceled`
- **THEN** the page tells "Cannot sign in to GitHub" with the reason
  [@test] src/stores/auth.test.ts (`says nothing when canceled, and why when GitHub refused, by the code of the error and not its words`)

### Requirement: Settings: The toast of a sign-out says GitHub keeps the authorization

The toast of a sign-out MUST be "Signed out of GitHub", which says that GitHub keeps the authorization until it is
revoked there.

#### Scenario: Sign Out
- **WHEN** the user signs out
- **THEN** the toast says "Signed out of GitHub", and that GitHub keeps the authorization until it is revoked there
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

### Requirement: Where it is offered: The hint of a missing or rejected token has Sign In

The status line of the pull requests ([Pull Requests](../../branch/pull-requests/spec.md#landing-overview)) and of
Your Turn, after the hint "No GitHub token to see pull requests" or "GitHub rejected the token from gh", MUST have
"Sign In…", which opens Settings → Accounts at GitHub.

#### Scenario: No GitHub token in a build with the sign-in
- **GIVEN** a build with the sign-in
- **WHEN** the status line says "No GitHub token to see pull requests" or "GitHub rejected the token from gh"
- **THEN** it has "Sign In…", which opens Settings → Accounts at GitHub
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)
  [@test] src/lib/pull-request.test.ts (`offers the sign-in for a missing or rejected token, GitHub Settings in a build without it, and nothing while unknown`)

### Requirement: Where it is offered: A build without the sign-in has GitHub Settings there

A build without the sign-in MUST have "GitHub Settings…" there, which opens Settings → Accounts at GitHub too: it
MUST NOT name a sign-in the build has not.

#### Scenario: No GitHub token in a build without the sign-in
- **GIVEN** a build without the sign-in
- **WHEN** the status line says "No GitHub token to see pull requests" or "GitHub rejected the token from gh"
- **THEN** it has "GitHub Settings…", which opens Settings → Accounts at GitHub
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)
  [@test] src/lib/pull-request.test.ts (`offers the sign-in for a missing or rejected token, GitHub Settings in a build without it, and nothing while unknown`)

### Requirement: Where it is offered: The button waits for the status of the sign-ins

The build decides which button, so the button MUST wait for the status, the fact `auth` the session of the window
reads when it opens.

#### Scenario: The status is unknown yet
- **GIVEN** a window whose session has not read the fact `auth` yet
- **WHEN** the status line shows the hint of a missing token
- **THEN** it has no button until the status answers
  [@test] src/lib/pull-request.test.ts (`offers the sign-in for a missing or rejected token, GitHub Settings in a build without it, and nothing while unknown`)
  [@test] src/stores/auth.test.ts (`comes from the fact of the session, which keeps the last status until it answered and says why a read failed`)

### Requirement: Where it is offered: SAML SSO has Authorize

After "SAML SSO" with a URL, the status line MUST have "Authorize…", which opens it.

#### Scenario: SAML SSO with a URL
- **WHEN** the status line says "SAML SSO" with a URL
- **THEN** it has "Authorize…", which opens the URL
  [@test] src/lib/pull-request.test.ts (`offers the sign-in for a missing or rejected token, GitHub Settings in a build without it, and nothing while unknown`)

### Requirement: Where it is offered: The token of the sign-in that can't see a repository asks for approval

The token of the sign-in that can't see the repository (no access, after every other source of the chain) is most
likely not approved by the organization: the user has a clone of it. The hint MUST say so, with "Request Access…",
which opens Manage on GitHub (`github.com/settings/connections/applications/<client id>`), where the user requests the
approval of the organization.

#### Scenario: No access with the token of the sign-in
- **GIVEN** the token of the Branch Deck sign-in can't see the repository `acme/monorepo`
- **WHEN** the status line shows the hint
- **THEN** it says "GitHub: no repository acme/monorepo (the token from the Branch Deck sign-in). An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck: request it, or run gh auth login"
- **AND** it has "Request Access…", which opens `github.com/settings/connections/applications/<client id>`
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should ask an organization to approve the token of the sign-in that can't see the repository`)
  [@test] src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`, `asks an organization to approve the token of the app's sign-in, on the page of the app on GitHub`)

### Requirement: Where it is offered: No Request Access for another token

There MUST be no "Request Access…" for another token: the page of the Branch Deck app does nothing for it.

#### Scenario: No access with the token of gh
- **WHEN** the token of `gh` can't see the repository
- **THEN** the hint has no "Request Access…"
  [@test] src/lib/pull-request.test.ts (`asks an organization to approve the token of the app's sign-in, on the page of the app on GitHub`)

## User Experience

- The section is GitHub's on the Accounts page of Settings: `text-sm` lines, `xs` buttons ("Sign In to GitHub" and "Sign Out" `outline`, "Manage
  on GitHub" `ghost`), the code in a nested `rounded-lg border` panel. The hint's button is `outline xs`, before
  Refresh.
- No sign-in is ever started by itself, and no window opens: the user starts it and opens GitHub.

## Testing / Local Run

- `cargo nextest run -p github`: the device flow against the fake GitHub (`device_flow_test.rs`), the token file
  (`token_store_test.rs`), the order of the sources (`token_test.rs`).
- `cargo nextest run -p remote-facts -E 'test(/sign_in|sign_ins/)'`: the sign-ins of the app, the join, the cancel and the
  sign-out against the fake GitHub (`sign_in_test.rs`), and when a session reads the fact `auth` (`scheduler_test.rs`).
- `pnpm test:unit`: `src/stores/auth.test.ts`.
- `cargo nextest run -p github`: the session from the code to the token file, a cancel and a sign-out against the fake
  GitHub (`sign_in_session_test.rs`); `-p test-server`: the same through the routes of the test server
  (`github_host_test.rs`).
- `pnpm e2e`: `tests/e2e/github-sign-in/`. The test server runs the sign-in commands through the real session against
  the device flow of a fake GitHub per test repository, a build without the OAuth app until a fixture names one
  (`setGitHubSignIn`: `clientId`, `authorize` for the user entering the code, `signedIn`, `ghToken`); a sign-in or a
  sign-out that changed an account has the sessions of the repository read their fact `auth`, and the test server
  tells every page of the repository `auth-changed` on its event stream, as the backend tells its windows
  ([E2E Testing](../../../../docs/design/e2e-testing.md)).
- In the app: build with `BRANCH_DECK_GITHUB_CLIENT_ID=<the client id of build-tauri.yml> pnpm install-app`, unset `GH_TOKEN`, move `gh`
  out of `PATH`, sign in from Settings, and check that the pull requests and Your Turn load and `brd safe-push --dry-run`
  sees the pull request.

## Open Questions / Risks

- Only the release build has the client id: a local build (`pnpm install-app`, `pnpm tauri dev`) signs in only with
  `BRANCH_DECK_GITHUB_CLIENT_ID` set, else it takes the token of `gh`, `GH_TOKEN` or git.
- An organization with OAuth app access restrictions shows its private repositories to the app's token only once an
  admin approved Branch Deck: Settings says so before the sign-in, and the hint of a repository the token can't see
  has "Request Access…" ([Where it is offered](#where-it-is-offered)).

## References

- [Pull Requests](../../branch/pull-requests/spec.md#the-token): the token chain.
- [Review Requests](../review-requests/spec.md): Your Turn and its hint.
- [Settings](../settings/spec.md): the window of the Accounts page.
- [Agent CLI](../../cli/agent-cli/spec.md): `brd safe-push --create-pull-request` with the same token.
