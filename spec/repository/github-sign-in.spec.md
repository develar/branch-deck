---
name: GitHub Sign-In
description: Requirements for the GitHub token without the gh CLI - the app's own sign-in through the OAuth device flow of the Branch Deck OAuth app (a code to enter on GitHub, then a token in a 0600 file shared with bd), shown in Settings next to the token the app takes from GH_TOKEN, gh or git, and offered by the hint of a repository without a token.
style: plain-1
targets:
  - ../../crates/github/src/device_flow.rs
  - ../../crates/github/src/sign_in_session.rs
  - ../../crates/remote-facts/src/sign_in.rs
  - ../../crates/remote-facts/src/scheduler_auth.rs
  - ../../crates/domain-types/src/sign_in.rs
  - ../../crates/domain-policy/src/refresh.rs
  - ../../crates/app-api/src/api_error.rs
  - ../../crates/github/src/token_store.rs
  - ../../crates/sync-utils/src/private_file.rs
  - ../../crates/sync-utils/src/secret_store.rs
  - ../../crates/github/src/token.rs
  - ../../crates/domain-types/src/pull_request.rs
  - ../../src-tauri/src/services.rs
  - ../../crates/app-api/src/remote/accounts.rs
  - ../../crates/app-api/src/app/sign_in.rs
  - ../../crates/app-api/src/events.rs
  - ../../src-tauri/src/lib.rs
  - ../../crates/bd-core/src/context.rs
  - ../../crates/test-server/src/tauri_command_bridge.rs
  - ../../crates/test-server/src/host.rs
  - ../../crates/test-server/src/remote_host.rs
  - ../../src/stores/auth.ts
  - ../../src/lib/sign-in-run.ts
  - ../../src/components/settings/GitHubSettings.tsx
  - ../../src/components/settings/GitHubPage.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/components/repository/PullRequestStatusLine.tsx
  - ../../src/components/repository/YourTurnCard.tsx
  - ../../src/lib/pull-request.ts
  - ../../src/hooks/useAvailabilityAction.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
---

# GitHub Sign-In

Status: Implemented
Date: 2026-10-16

## Summary

The pull requests, Your Turn and the writes to GitHub need a token ([Pull Requests](../branch/pull-requests.spec.md#the-token)).
Most users have one already: `gh`, `GH_TOKEN`, or the HTTPS credentials git keeps for github.com. A user with none
(SSH remotes, no `gh`) signs in in the app: Settings shows a code, the user enters it on GitHub, and the app keeps the
token. This is the OAuth device flow of the Branch Deck OAuth app, the way `gh auth login` works, without a browser
redirect and without a client secret.

## Goals

- Pull requests and review requests without installing `gh`.
- One place that says where the token comes from, and how to get one.
- `bd` uses the same sign-in.

## Non-goals

- A GitHub App: it works only where it is installed, which needs an admin of every organization, and its user
  tokens expire after 8 hours, so the app and `bd` would race to refresh one token.
- Revoking the token: that needs the client secret. Sign Out removes it here; GitHub revokes it on its settings page.
- GitHub Enterprise, several accounts.
- A keychain item of the app itself: unsigned local builds would make macOS ask again for the item after every
  update, and `bd` in the background could not answer. The item is written by `/usr/bin/security` instead
  ([The token file](#the-token-file)).

## Requirements

### Sign-in

- The client id of the Branch Deck OAuth app is given at build time (`BRANCH_DECK_GITHUB_CLIENT_ID`,
  `GITHUB_OAUTH_CLIENT_ID`), by the release workflow (`.github/workflows/build-tauri.yml`); a build without it offers no sign-in of its own (Settings says what gives a token, and
  the hint's button is "GitHub Settings…": [Where it is offered](#where-it-is-offered)).
  The device flow must be enabled in the settings of the OAuth app.
- "Sign In to GitHub" asks GitHub for a device code (`POST github.com/login/device/code`, `scope=repo read:org`,
  `Accept: application/json`): `repo` for private repositories and the writes to pull requests, `read:org` for the
  slug of a team asked to review.
  [@test] ../../crates/github/src/device_flow_test.rs (`the_device_code_is_asked_for_the_scopes_of_the_app`)
- The app polls `login/oauth/access_token` (`grant_type=urn:ietf:params:oauth:grant-type:device_code`) every
  `interval` seconds: `authorization_pending` waits, `slow_down` takes the new interval (else 5 s more), a token ends
  it; `expired_token`, `access_denied` and `device_flow_disabled` end it with an error, as does the code's
  `expires_in`. A network failure is asked again at the next interval.
  [@test] ../../crates/github/src/device_flow_test.rs (`the_wait_polls_while_pending_slows_down_when_told_and_takes_the_token`, `a_denied_or_expired_code_and_a_cancel_end_the_wait`)
- A sign-in is a command of the app, not an intent of a repository: the token is one for the app and Settings opens
  without a repository too ([Rust as the Domain Authority](../../docs/design/rust-domain-authority.md), "Решение
  2026-10-28"). `sign_in({ provider: "gitHub" }, channel)` runs it in a task of its own (`remote_facts::sign_in::SignIns`
  over `GitHubSignInSession`) and tells its events on the channel of the call, `done` last: `waiting` while GitHub is
  asked for the code, `deviceCode` (`code`, `url`, the seconds `expiresIn` it has left), `done` with the login; the call
  answers how it ended. The lock of the sign-ins only registers a run, never spans a request of GitHub.
  [@test] ../../crates/remote-facts/src/sign_in_test.rs (`a_second_sign_in_joins_the_one_that_runs_hears_its_code_and_ends_with_it`)
  [@test] ../../crates/test-server/src/github_host_test.rs (`the_sign_in_runs_the_device_flow_of_the_fake_and_ends_with_its_repository`)
- One sign-in at a time: a second call while one runs (another window, a second click) joins it: it hears the
  current event (the code, with the seconds left since GitHub gave it), then the rest, and answers the same end; GitHub
  is asked for one code. A window runs one sign-in of GitHub itself (`runSignIn` of `src/lib/sign-in-run.ts`), its
  button disabled while it does.
  [@test] ../../crates/remote-facts/src/sign_in_test.rs (`a_second_sign_in_joins_the_one_that_runs_hears_its_code_and_ends_with_it`)
  [@test] ../../src/stores/auth.test.ts (`shows the code while GitHub waits for it, then tells who signed in`)
- `cancel_sign_in("gitHub")` and a sign-out end the sign-in that runs at once with the error code `canceled` (the
  waiting request dropped, the code forgotten: nothing pending); a sign-in after it starts anew, with a code of its own,
  never joining the canceled one. `canceled` is no error: the page tells nothing of it, by the code, never by the words
  of the error.
  [@test] ../../crates/remote-facts/src/sign_in_test.rs (`a_cancel_ends_the_sign_in_at_once_as_canceled_and_the_next_one_starts_anew`, `a_sign_out_cancels_the_sign_in_that_runs_then_removes_the_token`)
  [@test] ../../crates/github/src/sign_in_session_test.rs (`a_cancel_ends_the_wait_and_a_build_without_the_oauth_app_signs_in_nowhere`)
  [@test] ../../crates/app-api/src/api_error_test.rs (`api_error_of_each_contract_case_is_its_json_in_the_fixture`)
  [@test] ../../src/stores/auth.test.ts (`says nothing when canceled, and why when GitHub refused, by the code of the error and not its words`)
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)
- With the token, the app asks `GET /user` for the login, keeps the token and forgets what GitHub accepted and denied
  (`TokenChain::forget`). A sign-in or a sign-out that changed the account tells the host once, however many calls
  joined it (`SignInHost::changed`): every session reads its fact `auth` again (`AfterWrite`) and lists the pull requests
  and the review requests again (`Manual`), and every window hears `auth-changed`, on which a window without a session
  reads `get_auth_status` again.
  [@test] ../../crates/github/src/sign_in_session_test.rs (`a_sign_in_polls_until_the_code_is_entered_then_keeps_the_token_of_its_user`)
  [@test] ../../crates/remote-facts/src/sign_in_test.rs (`a_second_sign_in_joins_the_one_that_runs_hears_its_code_and_ends_with_it`)
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)
- The status of the sign-ins (`AuthStatus`: GitHub's below, YouTrack's of [YouTrack](youtrack.spec.md)) is the fact
  `auth` of every session (`FactsView.auth`), read by the rule `domain_policy::refresh::auth_due`: when a window opens
  the session, after a sign-in or a sign-out of any window and by the refresh button, once more after a read in flight
  for those; on a focus by the rule of the lists (at least a minute after the last read and when it is two minutes
  old); never by a sync, a push or a poll. A window of a repository takes it from its session (`applyAuthFacts` of
  `src/stores/auth.ts`), which keeps the last status until a new session answered and says why a read failed.
  [@test] ../../crates/domain-policy/src/refresh_test.rs (`the_sign_ins_are_read_when_the_session_opens_after_a_sign_in_and_on_a_focus_by_the_rule_of_the_lists`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`the_sign_ins_are_read_when_a_window_opens_the_session_after_a_sign_in_and_on_a_late_focus`)
  [@test] ../../src/stores/auth.test.ts (`comes from the fact of the session, which keeps the last status until it answered and says why a read failed`)

### The token file

- `{ token, login, scopes, created_at }`, which `bd` reads too, in a secret of `sync_utils::secret_store`
  ([GitHub credentials](../../docs/design/git-operations.md#github-credentials)): on macOS the login keychain item
  `org.develar.branch-deck` / `github-token`, written and read by `/usr/bin/security` with the secret on stdin, as `gh`
  does, so an update of the unsigned app asks nothing; elsewhere, and while the keychain can't be used,
  `github-token.json` in the app data directory, written as a temporary file created 0600 in the same directory and
  renamed over the old one; a file others may read is not used, and a warning says why. A file of an earlier version
  is copied into the keychain on the first read and removed by the next sign-in or sign-out.
  [@test] ../../crates/github/src/token_store_test.rs (`a_token_is_written_and_read_back`)
  [@test] ../../crates/sync-utils/src/private_file_test.rs (`a_file_is_written_for_the_user_only_and_read_back`, `a_file_others_may_read_is_not_used`)
  [@test] ../../crates/sync-utils/src/secret_store_test.rs (`without_the_keychain_a_secret_is_the_0600_file`, `a_secret_lives_in_the_keychain_through_stdin_and_hex_without_a_file`, `the_file_of_an_earlier_version_is_copied_into_the_keychain_and_removed_by_the_next_write`, `a_locked_keychain_leaves_the_secret_in_the_file`)
- The token of the file comes after `GH_TOKEN` and `gh`, before git ([Pull Requests](../branch/pull-requests.spec.md#the-token)):
  an organization that restricts OAuth apps has usually approved `gh`, and the Branch Deck app only once an admin did.
  [@test] ../../crates/github/src/token_test.rs (`the_sign_in_of_the_app_comes_after_gh_and_before_git`)
- Sign Out (`sign_out("gitHub")`) removes the file and tells every window. "Manage on GitHub" opens
  `github.com/settings/connections/applications/<client id>`: the organizations the app may see, and revoking it.
  [@test] ../../crates/github/src/sign_in_session_test.rs (`a_sign_in_polls_until_the_code_is_entered_then_keeps_the_token_of_its_user`)
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

### Settings

- The GitHub page of the Settings window ([Settings](settings.spec.md)) says, asked again on every open of the window
  and on every sign-in or sign-out of any window (`get_auth_status`, `auth-changed`):
  - signed in: "Signed in as @octocat", with Sign Out and Manage on GitHub; while `gh` or `GH_TOKEN` comes first,
    "; the token of gh comes first while GitHub accepts it";
  - else the source the app takes first, without git (whose credentials are per repository): "Uses the token of gh";
  - else "No GitHub token: run gh auth login, set GH_TOKEN, store HTTPS credentials of github.com in git, or sign in
    here", with "Sign In to GitHub"; a build without the sign-in says "No GitHub token: run gh auth login, set
    GH_TOKEN, or store HTTPS credentials of github.com in git".
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`, `should cancel the sign-in, and sign out`)
- Before the sign-in, and while its token is the one the app takes, a note (`text-xs text-muted-foreground`) says
  what the sign-in can't do by itself: "An organization that restricts OAuth apps shows its private repositories only
  after an admin approves Branch Deck", then "; gh is often approved there already" before the sign-in, ": request it
  in Manage on GitHub" after it: the reason `gh` comes first in the chain, told before the user picks a token. No
  note while the token of `gh` or `GH_TOKEN` is the one taken, nor in a build without the sign-in.
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)
- While GitHub waits: "Enter this code on GitHub to sign in Branch Deck", the code (`font-mono text-lg`), "Copy Code
  and Open GitHub" (copies it and opens the verification page), Cancel, and "Waiting for GitHub · expires in 14:32"
  with a spinner, renewed every second. The toast of a sign-in is "Signed in to GitHub as @octocat"; a failure is
  "Cannot sign in to GitHub" with the reason; a sign-out "Signed out of GitHub", which says that GitHub keeps the
  authorization until it is revoked there.
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`, `should cancel the sign-in, and sign out`)
  [@test] ../../src/stores/auth.test.ts (`shows the code while GitHub waits for it, then tells who signed in`)

### Where it is offered

- The status line of the pull requests ([Pull Requests](../branch/pull-requests.spec.md#landing-overview)) and of Your
  Turn, after the hint "No GitHub token to see pull requests" or "GitHub rejected the token from gh", has "Sign In…",
  which opens Settings at GitHub; a build without the sign-in has "GitHub Settings…" there, which opens it too: it never names a
  sign-in the build has not. The build decides which, so the button waits for the status, the fact `auth` the session
  of the window reads when it opens. After "SAML SSO" with a URL, "Authorize…", which opens it.
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`, `should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)
  [@test] ../../src/lib/pull-request.test.ts (`offers the sign-in for a missing or rejected token, GitHub Settings in a build without it, and nothing while unknown`)
  [@test] ../../src/stores/auth.test.ts (`comes from the fact of the session, which keeps the last status until it answered and says why a read failed`)
- The token of the sign-in that can't see the repository (no access, after every other source of the chain) is
  most likely not approved by the organization: the user has a clone of it. The hint says so, "GitHub: no repository
  acme/monorepo (the token from the Branch Deck sign-in). An organization that restricts OAuth apps shows its private
  repositories only after an admin approves Branch Deck: request it, or run gh auth login", with "Request Access…",
  which opens Manage on GitHub (`github.com/settings/connections/applications/<client id>`), where the user requests
  the approval of the organization. No such button for another token: the page of the Branch Deck app does nothing for it.
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should ask an organization to approve the token of the sign-in that can't see the repository`)
  [@test] ../../src/lib/pull-request.test.ts (`hints at what to do, and says nothing when there is nothing to do`, `asks an organization to approve the token of the app's sign-in, on the page of the app on GitHub`)

## User Experience

- The section is the GitHub page of Settings: `text-sm` lines, `xs` buttons ("Sign In to GitHub" and "Sign Out" `outline`, "Manage
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
  ([E2E Testing](../../docs/design/e2e-testing.md)).
- In the app: build with `BRANCH_DECK_GITHUB_CLIENT_ID=<the client id of build-tauri.yml> pnpm install-app`, unset `GH_TOKEN`, move `gh`
  out of `PATH`, sign in from Settings, and check that the pull requests and Your Turn load and `bd safe-push --check`
  sees the pull request.

## Open Questions / Risks

- Only the release build has the client id: a local build (`pnpm install-app`, `pnpm tauri dev`) signs in only with
  `BRANCH_DECK_GITHUB_CLIENT_ID` set, else it takes the token of `gh`, `GH_TOKEN` or git.
- An organization with OAuth app access restrictions shows its private repositories to the app's token only once an
  admin approved Branch Deck: Settings says so before the sign-in, and the hint of a repository the token can't see
  has "Request Access…" ([Where it is offered](#where-it-is-offered)).

## References

- [Pull Requests](../branch/pull-requests.spec.md#the-token): the token chain.
- [Review Requests](review-requests.spec.md): Your Turn and its hint.
- [Settings](settings.spec.md): the window of the GitHub page.
- [Agent CLI](../cli/agent-cli.spec.md): `bd safe-push --create-pull-request` with the same token.
