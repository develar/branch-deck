---
name: YouTrack
description: Requirements for YouTrack - the issue popover on every issue key of the YouTrack signed in to, Mark Fixed with Undo, and the sign-in through the Hub of the YouTrack (OAuth with PKCE by Hub's public client localhost-auth, a refresh token kept in a 0600 file), or a pasted token, or YOUTRACK_TOKEN; offered for the YouTrack the issue links of .idea/vcs.xml name, and sent to no other host.
style: plain-1
targets:
  - crates/youtrack/src/url.rs
  - crates/youtrack/src/client.rs
  - crates/youtrack/src/sign_in.rs
  - crates/app-api/src/api_error.rs
  - crates/youtrack/src/hub.rs
  - crates/remote-facts/src/youtrack_cli.rs
  - crates/cli-tools/src/youtrack_login.rs
  - crates/youtrack/src/sign_in_session.rs
  - crates/youtrack/src/fake_youtrack.rs
  - crates/youtrack/src/issue.rs
  - crates/youtrack/src/service.rs
  - crates/youtrack/src/token_store.rs
  - crates/sync-utils/src/secret_store.rs
  - crates/domain-types/src/youtrack.rs
  - crates/app-api/src/remote/accounts.rs
  - crates/app-api/src/remote/pull_requests.rs
  - src-tauri/src/services.rs
  - crates/operations/src/issue.rs
  - src/lib/issue-actions.ts
  - src-tauri/src/lib.rs
  - crates/test-server/src/session_routes.rs
  - crates/test-server/src/host.rs
  - crates/test-server/src/youtrack_fixtures.rs
  - src/lib/youtrack.ts
  - src/stores/youtrack-issues.ts
  - src/stores/auth.ts
  - src/model/auth-facts.ts
  - src/lib/sign-in-run.ts
  - crates/remote-facts/src/sign_in.rs
  - crates/domain-types/src/sign_in.rs
  - crates/app-api/src/app/sign_in.rs
  - crates/test-server/src/remote_host.rs
  - src/components/settings/YouTrackSettings.tsx
  - src/components/settings/AccountsPage.tsx
  - src/components/commit/LinkedText.tsx
  - src/components/issue/IssueHoverHost.tsx
  - src/components/issue/IssueDetails.tsx
  - src/components/ui/popover.tsx
  - src/hooks/useBranchContextActions.ts
  - src/App.tsx
  - src/model/intents.ts
---

# YouTrack

Status: Implemented
Date: 2026-10-20

## Purpose

An issue key (`ABC-1`) in a branch name or a commit subject is a link to the tracker. Signed in to the YouTrack the
links point to, Branch Deck can tell what the issue is and mark it fixed once the work landed: Patronus marks an
issue fixed only for a commit that says so ("fixed ABC-1"), and most subjects only name the key. The sign-in is the one
of the `youtrack` CLI (`youtrack instance login`): the browser signs in at the Hub of the YouTrack, by Hub's public
client for apps on the user's machine, so no administrator registers anything. Unlike the CLI, the app asks for a
refresh token (`access_type=offline`): the access token lasts an hour and is renewed by itself, the sign-in as long
as the Hub keeps the refresh token (90 days on hub.jetbrains.com, 2026-10). A YouTrack whose Hub does not know the
client takes a pasted token.

## Goals

- The YouTrack of the repository's issue links, signed in with one click and a browser page.
- A sign-in that lasts without a token that never expires.
- A token sent only to the YouTrack it is for, never named by a repository.

## Non-goals

- A token "for a month": Hub's permanent tokens have no expiry (`permanentToken` has only `creationTime` and
  `lastAccessTime`); the refresh token is the time-limited form. The one permanent token the app makes is the one it
  hands to the `youtrack` CLI, which keeps no other ([The youtrack CLI](#the-youtrack-cli)).
- Reading the token the `youtrack` CLI saved: it is in a keychain item of another program, which macOS guards with a
  prompt, and the CLI prints no saved token. The app gives the CLI a token; it never takes one.
- A Hub scope on the app's own sign-in: its refresh token is in a file `brd` reads, and with Hub's scope it would make
  permanent tokens. Only the one-shot sign-in for the CLI asks Hub, and keeps nothing.
- Several YouTracks, revoking a token the user made (that is Account Security of the YouTrack profile), `brd`: a refresh
  by two processes would need a lock on the secret, so `brd` does not read it yet.

## Requirements

### Requirement: Which YouTrack: Settings offers the YouTracks of the repositories' issue links

The URL field of Settings SHALL offer the YouTracks the issue links of the recent repositories point to: a link
template `<base>/issue/$0` (any group) of `.idea/vcs.xml` names `<base>`, most recent repository first, each once,
with "From .idea/vcs.xml of <repository>" under the field; several are buttons beside it.

#### Scenario: The YouTracks of several repositories
- **GIVEN** recent repositories whose `.idea/vcs.xml` has link templates `<base>/issue/$0` (any group)
- **WHEN** the URL field of Settings offers the YouTracks
- **THEN** each `<base>` is offered once, most recent repository first
- **AND** several are buttons beside the field
  [@test] src/lib/youtrack.test.ts (`offers the YouTrack of each issue page link, most recent repository first, each once`)

#### Scenario: The YouTrack of the repository's issue links, with its source
- **GIVEN** a repository whose issue links point to a YouTrack
- **WHEN** the user opens Settings signed out
- **THEN** the URL field holds that YouTrack, with "From .idea/vcs.xml of <repository>" under it
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)

### Requirement: Which YouTrack: Nothing names a YouTrack and no sign-in starts by itself

The code MUST NOT name a YouTrack, and a sign-in MUST NOT start by itself: a repository's `vcs.xml` only offers a
URL.

#### Scenario: A repository's vcs.xml names a YouTrack
- **GIVEN** a repository whose `vcs.xml` names a YouTrack
- **WHEN** Settings opens
- **THEN** the URL is only offered, and no sign-in starts until the user starts it
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)

### Requirement: Which YouTrack: A pasted URL is cut to the YouTrack it names

The backend MUST cut a pasted URL to the YouTrack: no query, no fragment, no page of YouTrack (`/issue/…`, `/issues`,
`/dashboard`, `/agiles`, `/articles`, `/projects`, `/users`, `/admin`, `/api`), no trailing slash; a host in lower
case.

#### Scenario: A pasted URL of a page of YouTrack
- **WHEN** the user pastes a URL with a query, a fragment, a page of YouTrack, a trailing slash or a host in upper case
- **THEN** the backend cuts it to the YouTrack, its host in lower case
  [@test] crates/youtrack/src/url_test.rs (`a_pasted_url_is_cut_to_the_youtrack_it_names`)

### Requirement: Which YouTrack: Only https, http only to a loopback host, no credentials

The backend MUST take https only, http only to a loopback host (the fakes of the tests), and no user name or password.

#### Scenario: http to another host, another scheme or a user name and password
- **WHEN** the URL is http to a host that is not a loopback one, of another scheme, or holds a user name and password
- **THEN** the backend refuses it
  [@test] crates/youtrack/src/url_test.rs (`a_token_goes_only_over_https_and_never_in_the_url`)

#### Scenario: http to a loopback host
- **WHEN** the URL is http to a loopback host (the fakes of the tests)
- **THEN** the backend takes it
  [@test] crates/youtrack/src/url_test.rs (`a_pasted_url_is_cut_to_the_youtrack_it_names`)

### Requirement: Sign-in in the browser: The sign-in asks the YouTrack for its Hub

"Sign In to YouTrack" MUST ask the YouTrack for its Hub (`GET <url>/api/config?fields=ring(url,serviceId)`, open to
every visitor; a built-in Hub is a path of the YouTrack).

#### Scenario: The Hub the YouTrack names
- **WHEN** the user presses "Sign In to YouTrack"
- **THEN** the app asks `GET <url>/api/config?fields=ring(url,serviceId)` and takes the Hub it names
- **AND** a built-in Hub is a path of the YouTrack
  [@test] crates/youtrack/src/sign_in_test.rs (`the_hub_is_the_one_the_youtrack_names`)

### Requirement: Sign-in in the browser: The Hub page of the public client back to a loopback listener

The sign-in MUST listen on `127.0.0.1:<free port>`; the Hub page MUST be `<hub>/api/rest/oauth2/auth` with
`client_id=localhost-auth`, `redirect_uri=http://127.0.0.1:<port>` (no path, the literal address: RFC 8252),
`scope=<service id>`, `access_type=offline`, a `state` and an S256 PKCE challenge of 32 random bytes each.

#### Scenario: The Hub page of a sign-in
- **WHEN** a sign-in starts
- **THEN** it listens on `127.0.0.1:<free port>`
- **AND** the Hub page is `<hub>/api/rest/oauth2/auth` with `client_id=localhost-auth`, `redirect_uri=http://127.0.0.1:<port>`, `scope=<service id>`, `access_type=offline`, a `state` and an S256 PKCE challenge
  [@test] crates/youtrack/src/sign_in_test.rs (`the_browser_answer_is_traded_for_tokens_with_the_verifier_of_the_challenge`)

### Requirement: Sign-in in the browser: The app opens the Hub page, again on request

The app SHALL open the Hub page in the browser; "Open the Page Again" SHALL open it again.

#### Scenario: The page opens, and opens again
- **WHEN** the sign-in waits for the browser
- **THEN** the app opens the Hub page in the browser
- **AND** "Open the Page Again" opens it again
  [@test] src/stores/auth.test.ts (`opens the Hub page once the sign-in waits for the browser, again on request, then tells who signed in where`)

### Requirement: Sign-in in the browser: A Hub that does not know the client is told first

Before the browser opens, a request of the page without following redirects MUST tell whether the Hub knows the
client: a redirect to `/auth/oauth/error` is "This YouTrack does not allow the browser sign-in of apps: use a token
instead".

#### Scenario: The Hub redirects the page to its error
- **WHEN** the request of the Hub page, without following redirects, is redirected to `/auth/oauth/error`
- **THEN** the sign-in ends before the browser opens with "This YouTrack does not allow the browser sign-in of apps: use a token instead"
  [@test] crates/youtrack/src/sign_in_test.rs (`a_hub_that_does_not_know_the_client_is_told_before_the_browser_opens`)

### Requirement: Sign-in in the browser: The listener answers only the answer of this sign-in

The listener MUST answer only `GET /?code=…&state=…` of this sign-in, for five minutes: a favicon is a 404, another
state an error, an `error` of the Hub "The sign-in was denied: <description>". The page MUST say "Signed in to
YouTrack. You can close this tab and return to Branch Deck."

#### Scenario: A favicon, another state, an error of the Hub
- **WHEN** the listener gets a favicon request, an answer of another state, or an `error` of the Hub
- **THEN** the favicon is a 404, another state an error, and the `error` "The sign-in was denied: <description>"
  [@test] crates/youtrack/src/sign_in_test.rs (`only_the_answer_of_this_sign_in_is_taken`, `the_browser_answer_is_traded_for_tokens_with_the_verifier_of_the_challenge`)

#### Scenario: The answer of this sign-in
- **WHEN** the browser comes back with `GET /?code=…&state=…` of this sign-in within five minutes
- **THEN** the page says "Signed in to YouTrack. You can close this tab and return to Branch Deck."
  [@test] crates/youtrack/src/sign_in_test.rs (`the_browser_answer_is_traded_for_tokens_with_the_verifier_of_the_challenge`)

### Requirement: Sign-in in the browser: The code is traded for tokens and the user is named

The code MUST be traded at `<hub>/api/rest/oauth2/token` (`grant_type=authorization_code`, the verifier,
`client_id`), then `GET <url>/api/users/me?fields=login,fullName` names the user.

#### Scenario: The trade of the code
- **WHEN** the listener took the code
- **THEN** the app trades it at `<hub>/api/rest/oauth2/token` with `grant_type=authorization_code`, the verifier of the challenge and `client_id`
  [@test] crates/youtrack/src/sign_in_test.rs (`the_browser_answer_is_traded_for_tokens_with_the_verifier_of_the_challenge`)

#### Scenario: The user of the tokens
- **WHEN** the Hub answered the tokens
- **THEN** `GET <url>/api/users/me?fields=login,fullName` names the user, and the tokens are kept
  [@test] crates/youtrack/src/sign_in_session_test.rs (`the_browser_sign_in_keeps_the_tokens_of_the_hub_and_marks_an_issue_fixed_with_them`)

### Requirement: Sign-in in the browser: A sign-in is a command of the app

A sign-in MUST be a command of the app, not an intent of a repository: the token is one for the app and Settings
opens without a repository too ([Rust as the Domain Authority](../../../../docs/design/rust-domain-authority.md),
"Решение 2026-10-28").

#### Scenario: A sign-in without a repository
- **WHEN** Settings starts a sign-in
- **THEN** it runs as a command of the app, which needs no repository, and its token is one for the app
  [@test] crates/test-server/src/youtrack_host_test.rs (`the_youtrack_commands_run_the_real_sign_ins_and_service_against_the_fake`)

### Requirement: Sign-in in the browser: The sign-in runs in its task and tells its events on the channel

`sign_in({ provider: "youTrack", url }, channel)` MUST run it in a task of its own (`remote_facts::sign_in::SignIns`
over `youtrack::sign_in_session`) and tell its events on the channel of the call: `waiting`, `browser` with the Hub
page, and `done` with the YouTrack and its user; the call answers how it ended.

#### Scenario: The events of a browser sign-in
- **WHEN** `sign_in({ provider: "youTrack", url }, channel)` runs
- **THEN** the channel hears `waiting` while the YouTrack is asked for its Hub
- **AND** `browser` with the Hub page, which the page opens in the browser ("Open the Page Again" opens it again)
- **AND** `done` with the YouTrack and its user, and the call answers how it ended
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)
  [@test] crates/test-server/src/youtrack_host_test.rs (`the_youtrack_commands_run_the_real_sign_ins_and_service_against_the_fake`)
  [@test] src/stores/auth.test.ts (`opens the Hub page once the sign-in waits for the browser, again on request, then tells who signed in where`)

### Requirement: Sign-in in the browser: A second call of the same request joins the one that runs

One sign-in MUST run at a time: a second call of the same request while one runs (another window, a second click)
MUST join it: it hears the Hub page of the one that runs, then its end; the YouTrack is asked for one Hub.

#### Scenario: A second click or another window
- **GIVEN** a browser sign-in that runs
- **WHEN** another window or a second click calls the same request
- **THEN** the second call hears the Hub page of the one that runs, then its end
- **AND** the YouTrack is asked for one Hub
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

### Requirement: Sign-in in the browser: Another request ends the one that runs first

Another request (a token pasted while the browser signs in, another YouTrack) MUST NOT join the sign-in that runs:
the one that runs ends first, as canceled, and the new one runs. A sign-in of the `youtrack` CLI shares the session
([The youtrack CLI](#the-youtrack-cli)): one of either MUST end the other first, as canceled.

#### Scenario: A token pasted while the browser signs in
- **GIVEN** a browser sign-in that waits for the Hub
- **WHEN** a pasted token is signed in
- **THEN** the browser sign-in ends first, as canceled, and the pasted token runs and is kept
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_pasted_token_ends_the_browser_sign_in_that_runs_and_is_kept`)

#### Scenario: The sign-in of the youtrack CLI while the app's runs
- **GIVEN** the app's browser sign-in that runs
- **WHEN** a sign-in of the `youtrack` CLI starts
- **THEN** the app's ends first, as canceled
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

### Requirement: Sign-in in the browser: Cancel and a sign-out end the sign-in with the code canceled

Cancel and a sign-out MUST end the sign-in that runs at once with the error code `canceled` (`SignInError::Canceled`
through `app_api::api_error`; the listener closed, nothing pending), which is no error: the page tells nothing of it,
by the code, never by the words of the error.

#### Scenario: A canceled sign-in stops waiting
- **GIVEN** a sign-in that waits for the browser
- **WHEN** it is canceled
- **THEN** it ends at once with `SignInError::Canceled`, the listener closed and nothing pending
  [@test] crates/youtrack/src/sign_in_test.rs (`a_canceled_sign_in_stops_waiting`)
  [@test] crates/youtrack/src/sign_in_session_test.rs (`a_canceled_sign_in_ends_its_wait_and_a_pasted_token_is_kept_once_youtrack_accepts_it`)

#### Scenario: The code canceled reaches the page
- **WHEN** a canceled sign-in answers through `app_api::api_error`
- **THEN** its error code is `canceled`
  [@test] crates/app-api/src/api_error_test.rs (`api_error_of_each_contract_case_is_its_json_in_the_fixture`)

#### Scenario: The page tells nothing of a cancel
- **WHEN** the user presses Cancel while the browser signs in
- **THEN** the sign-in ends, nothing waits, and the page shows no toast, by the code `canceled`
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

### Requirement: Sign-in in the browser: A sign-in that changed the account tells the host once

A sign-in that changed the account MUST tell the host once, however many calls joined it: every session reads its
fact `auth` again and every window hears `auth-changed`, as for GitHub
([GitHub Sign-In](../github-sign-in/spec.md#sign-in)).

#### Scenario: A sign-in that several calls joined
- **GIVEN** a sign-in that several calls joined
- **WHEN** it changes the account
- **THEN** the host is told once: every session reads its fact `auth` again and every window hears `auth-changed`
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

### Requirement: A pasted token, the environment: Use a Token Instead shows the token form

"Use a Token Instead" SHALL show a password field for a token created in Account Security of the YouTrack profile,
"Create Token…" (opens `<url>/users/me?tab=account-security`) and "Save Token".

#### Scenario: The token form
- **WHEN** the user presses "Use a Token Instead"
- **THEN** a password field for the token shows, with "Create Token…" and "Save Token"
- **AND** "Create Token…" opens `<url>/users/me?tab=account-security`, Account Security of the YouTrack profile
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

### Requirement: A pasted token, the environment: Save Token keeps a token YouTrack accepted

"Save Token" MUST keep the token only once `GET /api/users/me` accepted it; a refused token is "Cannot sign in to
YouTrack" with the reason, and the field stays.

#### Scenario: A refused token
- **WHEN** the user saves a token that `GET /api/users/me` refuses
- **THEN** the toast is "Cannot sign in to YouTrack" with the reason, nothing is kept, and the field stays with the token
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)
  [@test] src/stores/auth.test.ts (`says why YouTrack refused a pasted token, and answers null, so the field keeps it`)

#### Scenario: An accepted token
- **WHEN** the user saves a token that `GET /api/users/me` accepts
- **THEN** it is kept, and the sign-in is one with a pasted token
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

### Requirement: A pasted token, the environment: The token lives only in the field until it is kept

Until it is kept, the token MUST live only in the field: never in a persisted store, a log or an error text.

#### Scenario: A token typed and not kept yet
- **WHEN** the user typed a token that is not kept yet
- **THEN** it is in the field only, never in a persisted store, a log or an error text

### Requirement: A pasted token, the environment: Save Token is the sign-in with the token, one at a time

Save Token MUST be the sign-in of YouTrack with the token (`sign_in({ provider: "youTrack", url, token })`): one at a
time with the browser sign-in, and the same tells of a changed account.

#### Scenario: Save Token while the browser sign-in asks for the Hub
- **GIVEN** the browser sign-in asks the YouTrack for its Hub
- **WHEN** the user typed a token
- **THEN** Save Token is disabled, its hover saying why
- **AND** "Cancel Sign-In", beside the spinner of that sign-in, ends it at once, and the token typed stays
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

#### Scenario: The form while the browser signs in
- **WHEN** the browser sign-in waits for the browser
- **THEN** the form hides, the typed token kept for after a cancel
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

#### Scenario: A saved token tells of a changed account
- **WHEN** Save Token signs in with a token YouTrack accepted
- **THEN** the changed account is told as the browser sign-in tells it
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

### Requirement: A pasted token, the environment: YOUTRACK_TOKEN comes before the token file

`YOUTRACK_TOKEN` with `YOUTRACK_BASE_URL`, the variables of the `youtrack` CLI, MUST come before the token file:
"Uses the token of YOUTRACK_TOKEN for <host>", without Sign Out.

#### Scenario: The environment and a token file
- **GIVEN** `YOUTRACK_TOKEN` with `YOUTRACK_BASE_URL` set, and a token file
- **WHEN** a lookup is sent
- **THEN** it sends the token of `YOUTRACK_TOKEN`, and the status names the environment
  [@test] crates/youtrack/src/service_test.rs (`the_environment_comes_before_the_token_file`)

#### Scenario: Settings of a token of the environment
- **GIVEN** the token of `YOUTRACK_TOKEN`
- **WHEN** Settings shows the sign-in
- **THEN** it says "Uses the token of YOUTRACK_TOKEN for <host>", without Sign Out

### Requirement: The token file and the renewal: The sign-in is kept in a secret of the secret store

The sign-in MUST be kept as `{ url, login, full_name, access_token, access_expires_at, refresh: { hub_url, service_id, refresh_token }, created_at }`
in a secret of `sync_utils::secret_store`, as the GitHub token
([GitHub Sign-In](../github-sign-in/spec.md#the-token-file)); a pasted token has no expiry and no refresh.

#### Scenario: The secret on macOS
- **WHEN** the app keeps a sign-in on macOS
- **THEN** it is the login keychain item `org.develar.branch-deck` / `youtrack-token`, written by `/usr/bin/security`

#### Scenario: The secret elsewhere
- **WHEN** the app keeps a sign-in on another platform
- **THEN** it is `youtrack-token.json` in the app data directory, 0600 and renamed into place

#### Scenario: A pasted token
- **WHEN** the app keeps a pasted token
- **THEN** it has no expiry and no refresh

### Requirement: The token file and the renewal: The debug form of the token names neither token

The debug form of the token MUST name neither the access token nor the refresh token.

#### Scenario: A kept token in a debug form
- **WHEN** the kept token is written in its debug form
- **THEN** it names neither token

### Requirement: The token file and the renewal: An expiring access token is renewed before it is sent

An access token that expires within a minute MUST be renewed before it is sent (`grant_type=refresh_token`,
`client_id`, `scope`), once for the lookups that need it at the same time; a new refresh token the Hub hands out MUST
replace the old one in the file.

#### Scenario: Parallel lookups with an expiring token
- **GIVEN** an access token that expires within a minute
- **WHEN** two lookups need it at the same time
- **THEN** the Hub is asked once with `grant_type=refresh_token`, `client_id` and `scope`, and both send the renewed token
- **AND** the new refresh token of the Hub replaces the old one in the file
  [@test] crates/youtrack/src/service_test.rs (`an_expiring_access_token_is_renewed_once_for_parallel_lookups_and_kept`)

### Requirement: The token file and the renewal: A 401 renews once and asks again

A 401 MUST renew the token once and ask again.

#### Scenario: A request answered 401
- **GIVEN** a sign-in with a refresh token
- **WHEN** YouTrack answers a request 401
- **THEN** the token is renewed once and the request asked again

### Requirement: The token file and the renewal: A renewal holds nothing but itself across the Hub

A renewal MUST hold nothing but itself across the request to the Hub: the status (Settings, the popovers), a sign-in,
a sign-out and an issue the cache has answer while a slow Hub holds it; the lookups that need it wait for the one
renewal and send its token.

#### Scenario: A slow Hub holds the renewal
- **GIVEN** a renewal that a slow Hub holds
- **WHEN** the status, a sign-in, a sign-out or an issue the cache has is asked
- **THEN** it answers meanwhile
- **AND** the lookups that need the renewal wait for the one renewal and send its token
  [@test] crates/youtrack/src/service_test.rs (`a_renewal_held_at_the_hub_leaves_the_status_free_and_is_shared_by_the_lookups`)

### Requirement: The token file and the renewal: A refused or failed renewal is shared as its token is

A renewal that the Hub refuses or fails MUST be shared as its token is: the lookups that waited for it answer the
expired sign-in, or the same failure, at once and never ask the Hub again one after another.

#### Scenario: The Hub refuses the renewal
- **GIVEN** lookups that wait for a renewal
- **WHEN** the Hub refuses it
- **THEN** they answer the expired sign-in at once, without asking the Hub again one after another
  [@test] crates/youtrack/src/service_test.rs (`the_lookups_that_wait_for_a_refused_renewal_answer_the_expired_sign_in_without_asking_again`)

#### Scenario: The renewal fails
- **GIVEN** lookups that wait for a renewal
- **WHEN** it fails
- **THEN** they answer the same failure at once, without asking the Hub again one after another
  [@test] crates/youtrack/src/service_test.rs (`the_lookups_that_wait_for_a_failed_renewal_share_its_failure_and_a_later_lookup_renews_again`)

### Requirement: The token file and the renewal: A later lookup, or one whose renewal went away, renews

A lookup after a failed renewal MUST renew again, and one that waited for a renewal whose lookup went away (its
popover closed) MUST renew in its place.

#### Scenario: A lookup after a failed renewal
- **GIVEN** a renewal that failed
- **WHEN** a later lookup needs the token
- **THEN** it renews again
  [@test] crates/youtrack/src/service_test.rs (`the_lookups_that_wait_for_a_failed_renewal_share_its_failure_and_a_later_lookup_renews_again`)

#### Scenario: The lookup of the renewal went away
- **GIVEN** a lookup that waits for a renewal
- **WHEN** the lookup of that renewal goes away (its popover closed)
- **THEN** the waiting lookup renews in its place and sends the renewed token
  [@test] crates/youtrack/src/service_test.rs (`a_lookup_that_waited_for_a_renewal_whose_lookup_went_away_renews_in_its_place`)

### Requirement: The token file and the renewal: A sign-in or a sign-out makes a late renewal one that is gone

A sign-in or a sign-out during a renewal MUST make the late answer of the Hub one of a sign-in that is gone: it is
neither kept nor written to the token file.

#### Scenario: A sign-out while the Hub holds a renewal
- **GIVEN** a renewal that the Hub holds
- **WHEN** the user signs out, and then the Hub answers
- **THEN** the answer is neither kept nor written to the token file
  [@test] crates/youtrack/src/service_test.rs (`a_sign_out_during_a_renewal_keeps_nothing_of_the_answer`)

### Requirement: The token file and the renewal: A sign-in or a sign-out changes what a lookup sends first

A sign-in or a sign-out MUST change what a lookup sends before it writes the token file: a lookup while the file is
written or removed sends the new token, or nothing, and never reads the token being replaced back from the file to
renew it; a renewal writes the file before or after that write, never over it or after it.

#### Scenario: A lookup during the removal of a sign-out
- **WHEN** a lookup runs while a sign-out removes the token file
- **THEN** it sends nothing and renews nothing, and writes no token after the removal
  [@test] crates/youtrack/src/service_test.rs (`a_lookup_during_the_removal_of_a_sign_out_renews_nothing`)

#### Scenario: A lookup during the write of a sign-in
- **WHEN** a lookup runs while a sign-in writes its token over an expiring one
- **THEN** it sends the new token, never renews the old one from the file, and the file ends with the new token
  [@test] crates/youtrack/src/service_test.rs (`a_lookup_during_the_write_of_a_sign_in_sends_the_new_token`)

### Requirement: The token file and the renewal: `brd` only reads the token and never renews it

`brd` (the issue summaries of the pull request texts it writes) MUST only read the token: it sends a token that doesn't
expire within a minute and never renews it (`YouTrackService::without_renewal`). A renewal rotates the refresh token,
and the app, which keeps the old one in memory, would be signed out by its next renewal.

#### Scenario: A valid token
- **GIVEN** a kept token that doesn't expire within a minute
- **WHEN** `brd` looks up an issue
- **THEN** it sends that token
  [@test] crates/youtrack/src/service_test.rs (`a_service_without_renewal_sends_a_valid_token_and_leaves_an_expiring_one_to_the_app`)

#### Scenario: An expiring token
- **GIVEN** a kept token that expires within a minute
- **WHEN** `brd` looks up an issue
- **THEN** it renews nothing, the token file stays as it was, and `brd` gets no summary until the app renews it
  [@test] crates/youtrack/src/service_test.rs (`a_service_without_renewal_sends_a_valid_token_and_leaves_an_expiring_one_to_the_app`)

### Requirement: The token file and the renewal: An expired or a rejected sign-in is not sent again

`invalid_grant` MUST be an expired sign-in; a 401 to a token that can't be renewed MUST be a rejected one. Either is
not sent again until the next sign-in (`forget`).

#### Scenario: The Hub refuses the refresh token
- **WHEN** the Hub answers a renewal `invalid_grant`
- **THEN** the sign-in is expired
- **AND** the next sign-in (`forget`) forgets the problem
  [@test] crates/youtrack/src/service_test.rs (`a_refused_refresh_token_is_an_expired_sign_in`)
  [@test] crates/youtrack/src/sign_in_session_test.rs (`a_sign_in_whose_refresh_token_the_hub_does_not_know_is_expired`)

#### Scenario: A 401 to a pasted token
- **WHEN** YouTrack answers 401 to a token that can't be renewed
- **THEN** the sign-in is rejected and the token is not sent again
  [@test] crates/youtrack/src/service_test.rs (`a_rejected_pasted_token_is_not_sent_again`)

### Requirement: The token file and the renewal: Settings asks to sign in again

For an expired or a rejected sign-in, Settings MUST say "Your YouTrack sign-in expired: sign in again" or "YouTrack
rejected the token: sign in again" above the sign-in, with the URL of the token.

#### Scenario: Settings of an expired sign-in
- **GIVEN** a sign-in the Hub no longer renews
- **WHEN** the user opens Settings
- **THEN** it says "Your YouTrack sign-in expired: sign in again" above the sign-in, with the URL of the token
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should ask to sign in again once the sign-in expired`)

#### Scenario: Settings of a rejected token
- **GIVEN** a token YouTrack rejected
- **WHEN** the user opens Settings
- **THEN** it says "YouTrack rejected the token: sign in again" above the sign-in, with the URL of the token

### Requirement: Requests: A token goes only to the YouTrack it is for and its Hub

A token MUST go only to the YouTrack it is for and its Hub: redirects are not followed, every path is built from
segments, and an issue key is checked first (`^[A-Za-z][A-Za-z0-9_]*-\d+$`), so a key a repository's `vcs.xml` put
into a link never names another path.

#### Scenario: A redirect
- **WHEN** YouTrack answers a request with a redirect
- **THEN** the redirect is not followed, and the request fails with it
  [@test] crates/youtrack/src/issue_test.rs (`a_redirect_is_not_followed`)

#### Scenario: A path of segments
- **WHEN** a request is built from segments that hold `/` or `?`
- **THEN** each segment is escaped and the path never leaves the base
  [@test] crates/youtrack/src/url_test.rs (`a_segment_is_escaped_and_never_leaves_the_base`)

#### Scenario: A key that is not an issue key
- **WHEN** a lookup names something that does not match `^[A-Za-z][A-Za-z0-9_]*-\d+$`
- **THEN** it is not a path segment of a lookup, and nothing is sent
  [@test] crates/youtrack/src/url_test.rs (`only_an_issue_key_is_a_path_segment_of_a_lookup`)
  [@test] crates/youtrack/src/service_test.rs (`a_lookup_of_something_else_than_a_key_is_never_sent`)

### Requirement: Requests: A read is asked again once after a failure, never after a timeout

A read MUST be asked again once after a failure of the server or the connection, never after a timeout, which
already waited its full time; a write MUST NOT be asked again.

#### Scenario: A read that timed out
- **WHEN** a read times out
- **THEN** it is not asked again
  [@test] crates/youtrack/src/issue_test.rs (`an_issue_read_that_timed_out_is_not_asked_again`)

#### Scenario: A read that failed at the server or the connection
- **WHEN** a read fails at the server or the connection
- **THEN** it is asked again once

#### Scenario: A write that failed
- **WHEN** a write fails
- **THEN** it is never asked again

### Requirement: Settings: The status of the sign-in is read as GitHub's is

The status of the sign-in MUST be the `youtrack` part of the status of both accounts (`AuthStatus`), read as GitHub's
is ([GitHub Sign-In](../github-sign-in/spec.md#sign-in)): a window of a repository from the fact `auth` of its
session, any other window by `get_auth_status`, Settings again on each open of its window (`useAuthSync`,
`useAuthStatusOnOpen` of `src/stores/auth.ts`).

#### Scenario: A window of a repository
- **GIVEN** a window of a repository (the main window, the review window of a branch)
- **WHEN** the sign-in changes
- **THEN** the window takes the status from the fact `auth` of its session (`applyAuthFacts`, which the store of the session calls with every view)
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should leave the links of a signed-out user as they are`)

#### Scenario: Any other window
- **GIVEN** a window without a repository
- **WHEN** it starts, or hears `auth-changed`
- **THEN** it reads `get_auth_status`

#### Scenario: Settings opens
- **WHEN** the window of Settings opens
- **THEN** it reads the status again (`useAuthSync`, `useAuthStatusOnOpen` of `src/stores/auth.ts`)

### Requirement: Settings: The answers of the popovers go once the sign-in changed

The answers of the popovers MUST go once the sign-in changed (another YouTrack, user, token or sign-in time), not for
a problem the YouTrack found with the same token; a popover that shows asks again (`src/stores/youtrack-issues.ts`).

#### Scenario: Another sign-in, or a problem of the same one
- **GIVEN** answers of the popovers
- **WHEN** the sign-in changes (another YouTrack, user, token or sign-in time)
- **THEN** the answers go, and a popover that shows asks again
- **AND** a problem the YouTrack found with the same token keeps them
  [@test] src/stores/youtrack-issues.test.ts (`forgets the answers once the YouTrack sign-in changed, not for a problem of the same sign-in`)

### Requirement: Settings: The YouTrack section says what the sign-in is for

The YouTrack section of Settings → Accounts ([Settings](../settings/spec.md)) SHALL say what the sign-in is for:
"Hover an issue key to see the issue, and mark it fixed once its work lands".

#### Scenario: The section of YouTrack
- **WHEN** the user opens Settings → Accounts
- **THEN** the YouTrack section says "Hover an issue key to see the issue, and mark it fixed once its work lands"

### Requirement: Settings: Signed out, the URL with its source or a hint

Signed out, the section MUST show the URL with its source ("From .idea/vcs.xml of <repository>"), or, empty, "The
address you open YouTrack at, e.g. https://youtrack.example.com".

#### Scenario: A URL the issue links offer
- **GIVEN** signed out, a URL the issue links of a repository offer
- **WHEN** the section shows
- **THEN** the field holds the URL with "From .idea/vcs.xml of <repository>"
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)

#### Scenario: No URL
- **GIVEN** signed out, no URL offered
- **WHEN** the section shows
- **THEN** the field is empty, with "The address you open YouTrack at, e.g. https://youtrack.example.com"
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

### Requirement: Settings: Signed out, Sign In to YouTrack and the token as the fallback

Signed out, the section MUST show "Sign In to YouTrack" (disabled without a URL) with "Opens YouTrack in the browser.
The sign-in renews itself until YouTrack ends it." under it, a spinner on it and "Cancel Sign-In" beside it while it
asks the YouTrack for its Hub; below, as a link, "Use a Token Instead", the fallback.

#### Scenario: Without a URL
- **WHEN** the URL field is empty
- **THEN** "Sign In to YouTrack" is disabled
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

#### Scenario: With a URL
- **WHEN** the URL field holds a URL
- **THEN** "Sign In to YouTrack" shows with "Opens YouTrack in the browser. The sign-in renews itself until YouTrack ends it." under it
- **AND** below, as a link, "Use a Token Instead"

#### Scenario: While it asks the YouTrack for its Hub
- **WHEN** the sign-in asks the YouTrack for its Hub
- **THEN** "Sign In to YouTrack" has a spinner on it and "Cancel Sign-In" beside it
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

### Requirement: Settings: The token form in the order of its steps

The token form MUST show, in the order of its steps: "For a YouTrack that does not sign in apps in the browser:
create a token there, then paste it here", "Create Token…" (Account Security of the profile), the field "Token",
"Save Token" and Cancel.

#### Scenario: The token form
- **WHEN** the user opens the token form
- **THEN** it shows "For a YouTrack that does not sign in apps in the browser: create a token there, then paste it here"
- **AND** then "Create Token…" (Account Security of the profile), the field "Token", "Save Token" and Cancel
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

### Requirement: Settings: While the browser signs in

While the browser signs in, the section MUST show "Sign in to YouTrack in the browser tab that opened, then return
here", "Open the Page Again", Cancel and "Waiting for the browser…" with a spinner; the URL field stays as typed.

#### Scenario: The wait for the browser
- **WHEN** the sign-in waits for the browser
- **THEN** the section shows "Sign in to YouTrack in the browser tab that opened, then return here", "Open the Page Again", Cancel and "Waiting for the browser…" with a spinner
- **AND** the URL field stays as typed
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`, `should cancel the browser sign-in, and paste a token instead`)

### Requirement: Settings: Signed in, the section says who and how

Signed in, the section MUST say who, "Signed in to youtrack.example.com as Jane Doe (jane)", and how: through the
browser, with a pasted token, or for `YOUTRACK_TOKEN` set in the environment.

#### Scenario: Through the browser
- **GIVEN** a browser sign-in
- **WHEN** the section shows
- **THEN** it says "Signed in to youtrack.example.com as Jane Doe (jane)"
- **AND** "Through the browser since yesterday 3:00 PM. It renews itself until YouTrack ends the sign-in, then Branch Deck asks you to sign in again."
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)
  [@test] crates/youtrack/src/service_test.rs (`a_refused_refresh_token_is_an_expired_sign_in`)

#### Scenario: With a pasted token
- **GIVEN** a pasted token
- **WHEN** the section shows
- **THEN** it says "With a pasted token since …. It works until you revoke it in YouTrack."
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

#### Scenario: YOUTRACK_TOKEN
- **GIVEN** the token of `YOUTRACK_TOKEN`
- **WHEN** the section shows
- **THEN** it says "Set in the environment of Branch Deck: unset it to sign in here."

### Requirement: Settings: Sign Out and the page that lists the token

Signed in, the section MUST show Sign Out (not for the environment), and "Manage Sign-In" (a browser sign-in) or
"Manage Tokens" (a token), which opens the page that lists the token (`manageUrl`): Account Security of the YouTrack
profile, or the profile of the Hub when the Hub is not built into the YouTrack.

#### Scenario: A browser sign-in through a built-in Hub
- **GIVEN** a browser sign-in whose Hub is built into the YouTrack
- **WHEN** the user presses "Manage Sign-In"
- **THEN** Account Security of the YouTrack profile opens, where the refresh token of the sign-in is
- **AND** Sign Out signs out
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)
  [@test] crates/youtrack/src/service_test.rs (`a_refused_refresh_token_is_an_expired_sign_in`)

#### Scenario: A pasted token
- **GIVEN** a pasted token
- **WHEN** the user presses "Manage Tokens"
- **THEN** Account Security of the YouTrack profile opens
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

#### Scenario: A Hub not built into the YouTrack
- **GIVEN** a browser sign-in at a Hub that is not built into the YouTrack (youtrack.jetbrains.com signs in at hub.jetbrains.com)
- **WHEN** the section shows the sign-in
- **THEN** `manageUrl` is the profile of the Hub, where the refresh token of the sign-in is
  [@test] crates/youtrack/src/service_test.rs (`the_tokens_of_a_sign_in_through_an_external_hub_are_managed_in_the_hub`)

#### Scenario: The token of the environment
- **GIVEN** the token of `YOUTRACK_TOKEN`
- **WHEN** the section shows
- **THEN** it has no Sign Out

### Requirement: Settings: The toasts of the sign-in and the sign-out

The sign-in and the sign-out SHALL tell their toasts: "Signed in to youtrack.example.com as Jane Doe"; "Cannot sign in
to YouTrack" with the reason; "Signed out of YouTrack", which says that YouTrack keeps the token until it is revoked in
Account Security.

#### Scenario: A sign-in
- **WHEN** a sign-in ends signed in
- **THEN** the toast is "Signed in to youtrack.example.com as Jane Doe"
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)

#### Scenario: A sign-in that fails
- **WHEN** a sign-in fails
- **THEN** the toast is "Cannot sign in to YouTrack" with the reason
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

#### Scenario: A sign-out
- **WHEN** the user signs out
- **THEN** the toast is "Signed out of YouTrack", which says that YouTrack keeps the token until it is revoked in Account Security
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)

### Requirement: Settings: A read of the status that fails is no sign-out

A read of the status that fails MUST be no sign-out: the last status stays (its links keep their popovers), and the
page says why with Retry ([Settings](../settings/spec.md#pages)).

#### Scenario: A later read fails
- **GIVEN** a status a read answered
- **WHEN** a later read of the status fails
- **THEN** the last status stays, and its links keep their popovers
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should say a failed read of the sign-in with Retry, never signed out for the session, and show the issues once a read answers`)
  [@test] src/model/auth-facts.test.ts (`tells a failed read once, as an error with Retry, and keeps the last status until a read answers`)

#### Scenario: The page of Settings
- **WHEN** a read of the status fails
- **THEN** the page of Settings says why with Retry

### Requirement: Settings: The main window tells a failed read once, with Retry

The main window MUST tell a failed read of the status once per failure as the error "Cannot read the GitHub and
YouTrack sign-ins" with Retry, which has its session read the fact `auth` again (`src/model/auth-facts.ts`; a window
without a session reads `get_auth_status`).

#### Scenario: A failed read
- **WHEN** a read of the status fails
- **THEN** the main window tells it once, as the error "Cannot read the GitHub and YouTrack sign-ins" with Retry
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should say a failed read of the sign-in with Retry, never signed out for the session, and show the issues once a read answers`)
  [@test] src/model/auth-facts.test.ts (`tells a failed read once, as an error with Retry, and keeps the last status until a read answers`)

#### Scenario: Retry
- **WHEN** the user presses Retry
- **THEN** the session of the window reads the fact `auth` again, and a window without a session reads `get_auth_status`
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should say a failed read of the sign-in with Retry, never signed out for the session, and show the issues once a read answers`)
  [@test] src/model/auth-facts.test.ts (`has the session of the window read its fact on Retry, and a window without one read it itself`)

### Requirement: Settings: The session reads the status again by its rule

The session MUST read the status again by its rule too: on a focus at most every minute, when it is two minutes old.

#### Scenario: A focus two minutes after the last answer
- **GIVEN** a status two minutes old
- **WHEN** the window gets the focus
- **THEN** the session reads the status again, and a failure is told again with no sign-out
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should say a failed read of the sign-in with Retry, never signed out for the session, and show the issues once a read answers`)

### Requirement: Settings: Without any status no popover and no Mark Fixed

Without any status, the links MUST show no popover and the menus no Mark Fixed until a read answers.

#### Scenario: No read answered yet
- **GIVEN** no read of the status answered yet
- **WHEN** the user hovers an issue link or opens the menu of its branch
- **THEN** the link shows no popover and the menu has no Mark Fixed
- **AND** once a read answers, both show
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should say a failed read of the sign-in with Retry, never signed out for the session, and show the issues once a read answers`)

### Requirement: The issue popover: A link to an issue of the YouTrack signed in to carries its key

Signed in, a link `LinkedText` renders to `<url>/issue/<key>` of the YouTrack of the token (same origin, under its
path, a valid key) MUST carry `data-issue-key`; nothing else of the link changes.

#### Scenario: A link to the issue page of the YouTrack
- **GIVEN** signed in to a YouTrack
- **WHEN** `LinkedText` renders a link to `<url>/issue/<key>` of that YouTrack, same origin, under its path, a valid key
- **THEN** the link carries `data-issue-key`, and nothing else changes
  [@test] src/lib/youtrack.test.ts (`takes the key of a link to the issue page of the YouTrack`)

#### Scenario: A link of another host, path or scheme
- **WHEN** a link is of another host, path or scheme, or names what is not a key
- **THEN** it carries no `data-issue-key`
  [@test] src/lib/youtrack.test.ts (`refuses a link of another host, path or scheme, and what is not a key`)

### Requirement: The issue popover: Signed out no link carries a key

Signed out, no link MUST carry `data-issue-key`, so the links of a user without YouTrack stay as they are and no
request is sent.

#### Scenario: A signed-out user hovers an issue link
- **GIVEN** signed out of YouTrack
- **WHEN** the user hovers a link to an issue
- **THEN** the link carries no `data-issue-key`, stays as it is, and no request is sent
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should leave the links of a signed-out user as they are`)

### Requirement: The issue popover: One popover per window, one delegated listener

There MUST be one popover per window (`IssueHoverHost`, in `App`), opened under the link by one delegated listener,
not a popover per link of every commit list.

#### Scenario: The links of every commit list
- **WHEN** the window shows many links of issues
- **THEN** one delegated listener of `IssueHoverHost` serves them all, and one popover opens under the link

### Requirement: The issue popover: A hover of 500 ms or the keyboard focus opens it

A hover of 500 ms on such a link, or its keyboard focus (`:focus-visible`), MUST open the popover under the link.

#### Scenario: A hover
- **WHEN** the pointer rests 500 ms on a link that carries `data-issue-key`
- **THEN** the popover opens under the link
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show the issue of a key on hover, mark it fixed, and undo it from the toast`)

#### Scenario: The keyboard focus
- **WHEN** the link gets the keyboard focus (`:focus-visible`)
- **THEN** the popover opens under the link
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)

### Requirement: The issue popover: A link that came under a still pointer opens nothing

A link that came under a still pointer (the rows moved) MUST open nothing until the pointer comes over it
(`pointerMoved`, [Branch Row](../../branch/branch-row/spec.md#hover-and-click)).

#### Scenario: The rows moved under the pointer
- **WHEN** a link comes under a still pointer, as the rows moved, and the delay of the hover runs out
- **THEN** no popover opens
- **AND** once the pointer comes over the link, the popover opens
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show no issue of a link that came under a still pointer, and show it once the pointer comes over it`)

### Requirement: The issue popover: The pointer may cross to it and it stays while over it

The pointer SHALL be allowed to cross to the popover (200 ms), and the popover MUST stay while the pointer is over it,
also when one move took the pointer from the link into it (no leave of the link is counted then, nor of the focus that
moves into it).

#### Scenario: One move from the link into the popover
- **WHEN** one move takes the pointer from the link onto an action of the popover
- **THEN** no leave of the link is counted, and the popover stays past the delay of a leave
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should keep the branch preview open under the popover of its title`)

#### Scenario: The pointer crosses to the popover
- **WHEN** the pointer leaves the link and comes over the popover within 200 ms
- **THEN** the popover stays while the pointer is over it

### Requirement: The issue popover: It opens without the focus and Escape closes it only

The popover MUST open without taking the focus; Escape MUST close it only, so a dialog under it stays.

#### Scenario: Escape over a dialog
- **GIVEN** the popover of a link in a dialog (the branch preview)
- **WHEN** the user presses Escape
- **THEN** the popover closes and the dialog stays; a second Escape closes the dialog
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should keep the branch preview open under the popover of its title`)

#### Scenario: Escape after the keyboard focus
- **GIVEN** the popover opened on the keyboard focus of its link
- **WHEN** the user presses Escape
- **THEN** the popover closes
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)

### Requirement: The issue popover: It closes once its link leaves the page

The popover MUST close once its link leaves the page (a sync, a collapsed branch, a closed toast).

#### Scenario: The link leaves the page
- **GIVEN** an open popover
- **WHEN** its link leaves the page, by a sync, a collapsed branch or a closed toast
- **THEN** the popover closes

### Requirement: The issue popover: It lies in the layer of its link, above it

The popover MUST lie in the layer of its link: the popup of another popover, a dialog (the branch preview) or the
toasts. The layer MUST count it as its own (a click in it is no outside press, the toast's timer waits while the
pointer is over it), and it is above the layer.

#### Scenario: The popover of a key in a dialog
- **GIVEN** the popover of the title's key in the branch preview
- **WHEN** the user clicks an action of the popover
- **THEN** the click is no outside press, and the preview stays open under it
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should keep the branch preview open under the popover of its title`)

#### Scenario: The popover of a key in a toast
- **WHEN** the user hovers the key in the title of a toast
- **THEN** its popover lies in the toasts, above them, and the toast's timer waits while the pointer is over it
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show the issue of a key on hover, mark it fixed, and undo it from the toast`)

### Requirement: The issue popover: It shows the key, the summary, the state and the fields

The popover MUST show the key, the summary, the state ("Open"; resolved: "Fixed · resolved 1 hour ago" in
`text-success`) and the single-value fields with a value (enum, user, version, owned, build) by the names YouTrack
gives them, in its order: a field MUST be told by its `$type`, never by its name.

#### Scenario: An open issue
- **WHEN** the popover shows an open issue
- **THEN** it shows the key, the summary, the state "Open" and the single-value fields with a value, by their `$type`, in the order of YouTrack
  [@test] crates/youtrack/src/issue_test.rs (`an_issue_shows_its_state_and_its_single_value_fields_by_type`)

#### Scenario: A resolved issue
- **WHEN** the popover shows a resolved issue
- **THEN** the state is "Fixed · resolved 1 hour ago" in `text-success`
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show a resolved issue without Mark Fixed, and send an expired sign-in to Settings`)
  [@test] crates/youtrack/src/issue_test.rs (`a_resolved_issue_does_not_ask_for_the_values_of_its_state`)

### Requirement: The issue popover: It asks when it opens, once while an ask runs

The popover MUST ask for its issue when it opens (`get_youtrack_issue`, the backend keeps an answer a minute); a
second ask while one runs MUST wait for it.

#### Scenario: Two asks of one issue
- **WHEN** an issue is asked while an ask of it runs
- **THEN** the second ask waits for the first, and the issue is asked once
  [@test] src/stores/youtrack-issues.test.ts (`asks an issue once while it is asked, and a sign-in problem has the window read the sign-ins again`)

#### Scenario: An answer of the last minute
- **WHEN** the popover asks an issue the backend answered within a minute
- **THEN** the backend answers what it keeps, without asking YouTrack

### Requirement: The issue popover: It says an issue not found, a failure and a sign-in problem

The popover MUST say "Not found, or no access" (YouTrack answers 404 for both), "Cannot load ABC-1: <reason>" with
Try Again, and for a rejected or expired sign-in the reason with "YouTrack Settings…", which opens Settings → Accounts
at YouTrack; such an answer MUST have the window read the sign-ins again, so every surface of it knows the problem.

#### Scenario: Not found
- **WHEN** YouTrack answers 404
- **THEN** the popover says "Not found, or no access"

#### Scenario: A failure
- **WHEN** the ask of the issue fails
- **THEN** the popover says "Cannot load ABC-1: <reason>" with Try Again

#### Scenario: An expired sign-in
- **WHEN** the answer is a rejected or expired sign-in
- **THEN** the popover says the reason with "YouTrack Settings…", which opens Settings → Accounts at YouTrack
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show a resolved issue without Mark Fixed, and send an expired sign-in to Settings`)

#### Scenario: The window reads the sign-ins again
- **WHEN** the answer is a sign-in problem
- **THEN** the window reads the sign-ins again, so every surface of it knows the problem
  [@test] src/stores/youtrack-issues.test.ts (`asks an issue once while it is asked, and a sign-in problem has the window read the sign-ins again`)

### Requirement: The issue popover: Its actions come from one builder

The actions of the popover MUST come from one builder (`getIssueActions`): Mark Fixed (an open issue with a state
field) and Open in YouTrack.

#### Scenario: An open issue with a state field
- **WHEN** the popover shows an open issue with a state field
- **THEN** its actions are Mark Fixed and Open in YouTrack
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show the issue of a key on hover, mark it fixed, and undo it from the toast`)

#### Scenario: A resolved issue
- **WHEN** the popover shows a resolved issue
- **THEN** its action is Open in YouTrack, without Mark Fixed
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show a resolved issue without Mark Fixed, and send an expired sign-in to Settings`)

### Requirement: Mark Fixed: The command `<field> Fixed` sets the state field of an open issue

Mark Fixed MUST set the state field of an open issue to Fixed, the resolved state of YouTrack's default workflow, by
the command `<field> Fixed` (`POST /api/commands`; a field or a value with spaces stays as it is: YouTrack takes no
braces, `State {Fixed}` is "State expected", checked with `/api/commands/assist`). The field is the issue's state
field (`StateIssueCustomField`, `StateMachineIssueCustomField`).

#### Scenario: The command of Mark Fixed
- **WHEN** the user marks an open issue fixed
- **THEN** the app sends `POST /api/commands` with the command `<field> Fixed`, its state field named
  [@test] crates/youtrack/src/service_test.rs (`a_state_change_returns_the_previous_state_and_an_undo_checks_the_state_first`)

#### Scenario: A field or a value with spaces
- **WHEN** the field or the value has spaces
- **THEN** it stays as it is in the command, without braces
  [@test] crates/youtrack/src/issue_test.rs (`a_command_names_the_field_and_the_value_without_braces`)

### Requirement: Mark Fixed: The values of the state field are asked only for an open issue

The issue's field MUST be asked for its values only for an open issue (the bundles of a project are large), and
without a value Fixed the action MUST be disabled with "State of ABC has no Fixed state".

#### Scenario: A resolved issue
- **WHEN** the issue is resolved
- **THEN** its state field is not asked for its values
  [@test] crates/youtrack/src/issue_test.rs (`a_resolved_issue_does_not_ask_for_the_values_of_its_state`)

#### Scenario: A state without Fixed
- **GIVEN** an open issue whose state field has no value Fixed
- **WHEN** its actions show
- **THEN** Mark Fixed is disabled with "State of ABC has no Fixed state"

### Requirement: Mark Fixed: The backend refuses an issue resolved already

The backend MUST refuse to mark an issue resolved already ("ABC-1 is already Duplicate"), so a menu built before
cannot turn a Duplicate into Fixed.

#### Scenario: A resolved issue marked from the menu
- **GIVEN** an issue resolved as Duplicate
- **WHEN** Mark Fixed reaches the backend
- **THEN** it is refused with "ABC-1 is already Duplicate", and no command is sent
  [@test] crates/youtrack/src/service_test.rs (`a_resolved_issue_is_not_marked_again`)
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)

### Requirement: Mark Fixed: It is one operation of the session of the window

Mark Fixed MUST be one operation of the session of the window (`SetIssueState`, `crates/operations/src/issue.rs`,
holding the issue, with the YouTrack of the app): a second click joins it, and its item says "Marking ABC-1 Fixed…"
meanwhile.

#### Scenario: The intent of Mark Fixed
- **WHEN** Mark Fixed runs
- **THEN** it is the intent `SetIssueState` of the session, its one effect the change of the state field
  [@test] crates/operations/src/issue_test.rs (`a_change_of_the_state_of_an_issue_is_its_one_effect_and_its_undo_the_same_intent`)
  [@test] crates/test-server/src/youtrack_host_test.rs (`the_youtrack_commands_run_the_real_sign_ins_and_service_against_the_fake`)

#### Scenario: A second click
- **WHEN** the user clicks Mark Fixed again while it runs
- **THEN** the click joins the operation that runs, and its item says "Marking ABC-1 Fixed…"

### Requirement: Mark Fixed: The toast tells the change with Undo

A change MUST be told by the toast "Marked ABC-1 Fixed", "Was Open", with Undo.

#### Scenario: Marked from the popover
- **WHEN** Mark Fixed succeeds
- **THEN** the toast says "Marked ABC-1 Fixed", "Was Open", with Undo
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show the issue of a key on hover, mark it fixed, and undo it from the toast`)

### Requirement: Mark Fixed: Undo sets the previous state while the issue is still Fixed

Undo MUST be the same intent back to the previous state, which sets it only while the issue is still Fixed ("ABC-1
is now Verified" otherwise), then "ABC-1 is Open again" in place of the toast it was pressed on (one toast of the
issue, the event of the outcome `NewsKey::Issue`,
[One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event)); YouTrack clears `resolved`
itself.

#### Scenario: Undo of a Fixed issue
- **WHEN** the user presses Undo on the toast and the issue is still Fixed
- **THEN** the same intent sets the previous state
- **AND** the toast it was pressed on says "ABC-1 is Open again", one toast of the issue
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should show the issue of a key on hover, mark it fixed, and undo it from the toast`)
  [@test] crates/operations/src/issue_test.rs (`a_change_of_the_state_of_an_issue_is_its_one_effect_and_its_undo_the_same_intent`)

#### Scenario: The issue is no longer Fixed
- **WHEN** the user presses Undo and the issue is no longer Fixed
- **THEN** nothing is set, "ABC-1 is now Verified"
  [@test] crates/youtrack/src/service_test.rs (`a_state_change_returns_the_previous_state_and_an_undo_checks_the_state_first`)

### Requirement: Mark Fixed: A failure is told with YouTrack's reason

A failure MUST be "Cannot mark ABC-1 fixed" or "Cannot undo the Fixed state of ABC-1" with YouTrack's reason
(`notifyError`, so in Activity too).

#### Scenario: Mark Fixed fails
- **WHEN** Mark Fixed fails
- **THEN** the error is "Cannot mark ABC-1 fixed" with YouTrack's reason, in Activity too
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)

#### Scenario: Undo fails
- **WHEN** the Undo of Mark Fixed fails
- **THEN** the error is "Cannot undo the Fixed state of ABC-1" with YouTrack's reason, in Activity too

### Requirement: Mark Fixed: It waits for no sync

Mark Fixed MUST wait for no sync: pressed while one runs, it is sent and answered at once, and that sync ends once.

#### Scenario: Pressed during a held sync
- **GIVEN** a sync that runs
- **WHEN** the user presses Mark Fixed in the menu of a branch
- **THEN** it is sent and answered at once, and the sync ends once
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should mark the issue of a branch fixed from its menu pressed during a held sync at once, the sync ending once`)

### Requirement: Mark Fixed: The popover of every key and the menu of its branch

Mark Fixed SHALL be offered in the popover of every key (the rows, the commits, the preview, the toasts, the merge
toast of a robot among them), and as "Mark ABC-1 Fixed" in the menu of a branch whose issue key is of the YouTrack:
the keyboard path, where the work landed.

#### Scenario: The popover of a key
- **WHEN** the popover of the key of an open issue shows, in the rows, the commits, the preview or the toasts (the merge toast of a robot among them)
- **THEN** it offers Mark Fixed

#### Scenario: The menu of a branch
- **GIVEN** a branch whose issue key is of the YouTrack
- **WHEN** the user opens its menu
- **THEN** it offers "Mark ABC-1 Fixed"
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)

#### Scenario: A commit that says it fixes the issue
- **GIVEN** a commit that says so ("fixed ABC-1", also "ABC-1 fix the …"), for which Patronus marks the issue fixed itself
- **WHEN** the popover of its issue shows after the work landed
- **THEN** it shows the issue resolved already

### Requirement: The youtrack CLI: Settings → Tools offers Use Branch Deck Sign-In

With the app signed in to a YouTrack and the `youtrack` CLI signed in to none, Settings → Tools SHALL offer Use Branch
Deck Sign-In ([Command-Line Tools](../cli-tools/spec.md#sign-in)).

#### Scenario: The app signed in, the CLI signed in to none
- **GIVEN** the app signed in to a YouTrack and the `youtrack` CLI signed in to none
- **WHEN** the user opens Settings → Tools
- **THEN** the row of the `youtrack` CLI offers Use Branch Deck Sign-In
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign youtrack in with a permanent token of the app's YouTrack sign-in, through one browser sign-in`)

### Requirement: The youtrack CLI: A one-shot browser sign-in that gives no refresh token

Use Branch Deck Sign-In MUST be a one-shot browser sign-in of the public client whose scope is the YouTrack and Hub
itself (`0-0-0-0-0`), without `access_type=offline`, so Hub gives one access token and no refresh token.

#### Scenario: The Hub page of the sign-in for the CLI
- **WHEN** the sign-in for the CLI starts
- **THEN** the Hub page asks the scope of the YouTrack and Hub itself (`0-0-0-0-0`), without `access_type=offline`
- **AND** Hub gives one access token and no refresh token, and the app's own sign-in stays as it was
  [@test] crates/youtrack/src/sign_in_session_test.rs (`the_sign_in_for_the_cli_asks_hub_once_keeps_nothing_and_its_token_makes_and_revokes_a_permanent_token`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign youtrack in with a permanent token of the app's YouTrack sign-in, through one browser sign-in`)

### Requirement: The youtrack CLI: It replaces a sign-in in progress, and Cancel ends it

The sign-in for the CLI MUST replace a sign-in in progress, and Cancel MUST end it as Cancel of the app's sign-in does.

#### Scenario: The app's sign-in in progress
- **GIVEN** a browser sign-in of the app in progress
- **WHEN** the sign-in for the CLI starts
- **THEN** the app's ends, as canceled, and the one for the CLI runs
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

#### Scenario: Cancel
- **WHEN** the user cancels the sign-in for the CLI
- **THEN** it ends at once with the code `canceled`, nothing waiting for the browser
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

### Requirement: The youtrack CLI: A permanent token of the YouTrack alone

With that access token the app MUST make a permanent token named "Branch Deck: youtrack CLI" for the YouTrack alone
(`POST <hub>/api/rest/users/me/permanenttokens`).

#### Scenario: The token made for the CLI
- **WHEN** the sign-in for the CLI ends with its access token
- **THEN** the app makes a permanent token named "Branch Deck: youtrack CLI" whose scope is the YouTrack alone
  [@test] crates/remote-facts/src/youtrack_cli_test.rs (`the_cli_gets_a_permanent_token_of_the_youtrack_alone_and_the_one_before_is_revoked`)
  [@test] crates/youtrack/src/sign_in_session_test.rs (`the_sign_in_for_the_cli_asks_hub_once_keeps_nothing_and_its_token_makes_and_revokes_a_permanent_token`)

### Requirement: The youtrack CLI: The token goes to the CLI on its stdin

The app MUST hand the token to `youtrack auth login --base-url <url> --no-browser --json --quiet` on its stdin, never
as an argument or a line of a log, without the `YOUTRACK_*` variables of the app; the CLI checks it and keeps it in its
own keychain item.

#### Scenario: The login of the CLI
- **WHEN** the app hands the token to the CLI
- **THEN** it runs `youtrack auth login --base-url <url> --no-browser --json --quiet` with the token on its stdin
- **AND** no argument and no line of a log holds the token
  [@test] crates/cli-tools/src/youtrack_login_test.rs (`hands_the_token_on_stdin_and_keeps_the_instance_the_user_had_active`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign youtrack in with a permanent token of the app's YouTrack sign-in, through one browser sign-in`)

#### Scenario: The variables of the app
- **GIVEN** the app runs with `YOUTRACK_*` variables
- **WHEN** it runs the login of the CLI
- **THEN** the CLI runs without them

### Requirement: The youtrack CLI: The instance the CLI had active stays active

The instance the CLI had active MUST stay active (`instance use`).

#### Scenario: The user had an instance active
- **GIVEN** the CLI with an instance of the user active
- **WHEN** the app signs the CLI in
- **THEN** the app makes that instance active again with `instance use`
  [@test] crates/cli-tools/src/youtrack_login_test.rs (`hands_the_token_on_stdin_and_keeps_the_instance_the_user_had_active`)

#### Scenario: The CLI had no instance
- **GIVEN** the CLI without instances
- **WHEN** the app signs the CLI in
- **THEN** no instance is made active again
  [@test] crates/cli-tools/src/youtrack_login_test.rs (`a_cli_without_instances_needs_no_instance_back`)

### Requirement: The youtrack CLI: The app keeps only the id of the token it made

The access token MUST be dropped; the app MUST keep only the id of the token it made (`youtrack-cli-token.json` of
the app data), and revoke that token once the CLI took the next one.

#### Scenario: The CLI takes the next token
- **GIVEN** a token the app made for the CLI, its id in `youtrack-cli-token.json`
- **WHEN** the CLI takes the next one
- **THEN** the app revokes the one before, and keeps only the id of the new one
  [@test] crates/remote-facts/src/youtrack_cli_test.rs (`the_cli_gets_a_permanent_token_of_the_youtrack_alone_and_the_one_before_is_revoked`)

### Requirement: The youtrack CLI: A missing or refusing CLI leaves no token behind

A CLI that is missing MUST get no token made; one that refuses the token MUST get it revoked.

#### Scenario: The CLI is missing
- **GIVEN** the `youtrack` CLI is not installed
- **WHEN** the app would hand it a token
- **THEN** no token is made
  [@test] crates/remote-facts/src/youtrack_cli_test.rs (`a_cli_that_is_missing_leaves_no_token_behind`)

#### Scenario: The CLI refuses the token
- **WHEN** the CLI refuses the token
- **THEN** the app revokes it

### Requirement: The youtrack CLI: The toast names the YouTrack and the user

A CLI signed in MUST be told by the toast "Signed youtrack in to youtrack.example.com as jane".

#### Scenario: The CLI signed in
- **WHEN** the CLI took the token
- **THEN** the toast is "Signed youtrack in to youtrack.example.com as jane"
  [@test] crates/remote-facts/src/youtrack_cli_test.rs (`the_cli_gets_a_permanent_token_of_the_youtrack_alone_and_the_one_before_is_revoked`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign youtrack in with a permanent token of the app's YouTrack sign-in, through one browser sign-in`)

## User Experience

- The popover is `w-96 gap-3 p-3`: the key `text-xs text-muted-foreground`, the summary `font-medium`, the fields a
  `text-xs` grid of names (muted) and values, the actions `xs` buttons (Mark Fixed `outline`, Open in YouTrack `ghost`).
- The page is one section: `text-sm` lines, the hints `text-xs text-muted-foreground`, `xs` buttons ("Sign In to
  YouTrack", "Sign Out" and "Create Token…" `outline`, "Save Token" default, "Use a Token Instead" a muted `link` at
  the edge of the text, the others `ghost`), the token form and the wait in a `rounded-lg` bordered panel, a problem
  in `text-warning`.

## Testing / Local Run

- `cargo nextest run -p youtrack`: the URL policy, the sign-in, the issue answers, the renewal and the rejection against
  scripted answers (the fake of `github::fake_github`), and the session against `FakeYouTrack` (`fake_youtrack.rs`, the
  `fake` feature: a YouTrack with a built-in Hub that redirects its page back with a code, trades it for the verifier
  of its challenge, and keeps issues and the commands that change them; `sign_in_session_test.rs`).
- `pnpm test:unit`: `src/lib/youtrack.test.ts`, `src/stores/auth.test.ts`, `src/model/auth-facts.test.ts`,
  `src/stores/youtrack-issues.test.ts`.
- `pnpm e2e`: `tests/e2e/youtrack/` (the sign-in, the popover). Every YouTrack command runs the real session and service
  of the test server against the `FakeYouTrack` of the test repository ([E2E Testing → Remote
  services](../../../../docs/design/e2e-testing.md#remote-services)); a test sets its issues and the sign-in of the token file
  (`setYouTrack`, the `youtrack` setup option), links the keys of the template to its URL for one sync
  (`linkIssuesToYouTrack(page, url)`), plays the browser of a sign-in (`completeYouTrackSignIn`: the Hub page redirects
  back to the app's listener) and asserts what reached the fake (`getYouTrack`: the commands, the lookups, the tokens
  of `users/me`); no test answers a YouTrack command in the page (`tests/e2e/helpers/youtrack-helpers.ts`).
- By hand: `pnpm tauri dev`, Settings → Accounts → Sign In to YouTrack with `https://youtrack.jetbrains.com`.
