---
name: YouTrack
description: Requirements for YouTrack - the issue popover on every issue key of the YouTrack signed in to, Mark Fixed with Undo, and the sign-in through the Hub of the YouTrack (OAuth with PKCE by Hub's public client localhost-auth, a refresh token kept in a 0600 file), or a pasted token, or YOUTRACK_TOKEN; offered for the YouTrack the issue links of .idea/vcs.xml name, and sent to no other host.
style: plain-1
targets:
  - ../../crates/youtrack/src/url.rs
  - ../../crates/youtrack/src/client.rs
  - ../../crates/youtrack/src/sign_in.rs
  - ../../crates/youtrack/src/issue.rs
  - ../../crates/youtrack/src/service.rs
  - ../../crates/youtrack/src/token_store.rs
  - ../../crates/sync-utils/src/secret_store.rs
  - ../../crates/sync-types/src/youtrack.rs
  - ../../src-tauri/src/commands/youtrack.rs
  - ../../src-tauri/src/lib.rs
  - ../../crates/test-server/src/tauri_command_bridge.rs
  - ../../src/lib/youtrack.ts
  - ../../src/stores/youtrack.ts
  - ../../src/components/settings/YouTrackSettings.tsx
  - ../../src/components/settings/YouTrackPage.tsx
  - ../../src/components/commit/LinkedText.tsx
  - ../../src/components/issue/IssueHoverHost.tsx
  - ../../src/components/issue/IssueDetails.tsx
  - ../../src/components/ui/popover.tsx
  - ../../src/lib/issue-actions.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/App.tsx
---

# YouTrack

Status: Implemented
Date: 2026-10-20

## Summary

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
  `lastAccessTime`); the refresh token is the time-limited form.
- Reading the token the `youtrack` CLI saved: it is in a keychain item of another program, which macOS guards with a
  prompt, and the CLI prints no saved token.
- Several YouTracks, revoking a token (that is Account Security of the YouTrack profile), `bd`: a refresh by two
  processes would need a lock on the secret, so `bd` does not read it yet.

## Requirements

### Which YouTrack

- The URL field of Settings offers the YouTracks the issue links of the recent repositories point to: a link template
  `<base>/issue/$0` (any group) of `.idea/vcs.xml` names `<base>`, most recent repository first, each once, with
  "From .idea/vcs.xml of <repository>" under the field; several are buttons beside it. Nothing names a YouTrack in
  the code, and no sign-in starts by itself: a repository's `vcs.xml` only offers a URL.
  [@test] ../../src/lib/youtrack.test.ts (`offers the YouTrack of each issue page link, most recent repository first, each once`)
  [@test] ../../tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`)
- The backend cuts a pasted URL to the YouTrack: no query, no fragment, no page of YouTrack (`/issue/…`, `/issues`,
  `/dashboard`, `/agiles`, `/articles`, `/projects`, `/users`, `/admin`, `/api`), no trailing slash; a host in lower
  case. It takes https only, http only to a loopback host (the fakes of the tests), and no user name or password.
  [@test] ../../crates/youtrack/src/url_test.rs (`a_pasted_url_is_cut_to_the_youtrack_it_names`, `a_token_goes_only_over_https_and_never_in_the_url`)

### Sign-in in the browser

- "Sign In to YouTrack" asks the YouTrack for its Hub (`GET <url>/api/config?fields=ring(url,serviceId)`, open to
  every visitor; a built-in Hub is a path of the YouTrack) and listens on `127.0.0.1:<free port>`; the Hub page is
  `<hub>/api/rest/oauth2/auth` with `client_id=localhost-auth`, `redirect_uri=http://127.0.0.1:<port>` (no path, the
  literal address: RFC 8252), `scope=<service id>`, `access_type=offline`, a `state` and an S256 PKCE challenge of 32
  random bytes each. The app opens it in the browser; "Open the Page Again" opens it again.
  [@test] ../../crates/youtrack/src/sign_in_test.rs (`the_hub_is_the_one_the_youtrack_names`, `the_browser_answer_is_traded_for_tokens_with_the_verifier_of_the_challenge`)
  [@test] ../../src/stores/youtrack.test.ts (`opens the Hub page, waits for the browser, then tells who signed in where`)
- Before the browser opens, a request of the page without following redirects tells whether the Hub knows the
  client: a redirect to `/auth/oauth/error` is "This YouTrack does not allow the browser sign-in of apps: use a token
  instead".
  [@test] ../../crates/youtrack/src/sign_in_test.rs (`a_hub_that_does_not_know_the_client_is_told_before_the_browser_opens`)
- The listener answers only `GET /?code=…&state=…` of this sign-in (a favicon is a 404, another state an error, an
  `error` of the Hub "The sign-in was denied: <description>"), for five minutes; the page says "Signed in to YouTrack.
  You can close this tab and return to Branch Deck." The code is traded at `<hub>/api/rest/oauth2/token`
  (`grant_type=authorization_code`, the verifier, `client_id`), then `GET <url>/api/users/me?fields=login,fullName`
  names the user.
  [@test] ../../crates/youtrack/src/sign_in_test.rs (`only_the_answer_of_this_sign_in_is_taken`, `the_browser_answer_is_traded_for_tokens_with_the_verifier_of_the_challenge`)
- One sign-in at a time (`start_youtrack_sign_in`, then `wait_youtrack_sign_in`); a newer one, Cancel and a sign-out
  end the wait of an older one with "canceled", which is no error (a generation counter, as for GitHub).
  [@test] ../../crates/youtrack/src/sign_in_test.rs (`a_canceled_sign_in_stops_waiting`)
  [@test] ../../src/stores/youtrack.test.ts (`says nothing when canceled, and why when the Hub refused`)
  [@test] ../../tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)

### A pasted token, the environment

- "Use a Token Instead" shows a password field for a token created in Account Security of the YouTrack profile,
  "Create Token…" (opens `<url>/users/me?tab=account-security`) and "Save Token", which keeps it once
  `GET /api/users/me` accepted it; a refused token is "Cannot sign in to YouTrack" with the reason, and the field stays.
  The token lives only in the field until then: never in a persisted store, a log or an error text.
  [@test] ../../tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should cancel the browser sign-in, and paste a token instead`)
  [@test] ../../src/stores/youtrack.test.ts (`is false and says why when YouTrack rejects the token`)
- `YOUTRACK_TOKEN` with `YOUTRACK_BASE_URL`, the variables of the `youtrack` CLI, come before the token file:
  "Uses the token of YOUTRACK_TOKEN for <host>", without Sign Out.
  [@test] ../../crates/youtrack/src/service_test.rs (`the_environment_comes_before_the_token_file`)

### The token file and the renewal

- `{ url, login, full_name, access_token, access_expires_at, refresh: { hub_url, service_id, refresh_token },
  created_at }` in a secret of `sync_utils::secret_store`, as the GitHub token ([GitHub
  Sign-In](github-sign-in.spec.md#the-token-file)): on macOS the login keychain item `org.develar.branch-deck` /
  `youtrack-token` by `/usr/bin/security`, elsewhere `youtrack-token.json` in the app data directory, 0600 and renamed
  into place; a pasted token has no expiry and no refresh. The debug form of the token names neither token.
- An access token that expires within a minute is renewed before it is sent (`grant_type=refresh_token`, `client_id`,
  `scope`), once for the lookups that need it at the same time; a new refresh token the Hub hands out replaces the old
  one in the file. A 401 renews once and asks again.
  [@test] ../../crates/youtrack/src/service_test.rs (`an_expiring_access_token_is_renewed_once_for_parallel_lookups_and_kept`)
- `bd` (the issue summaries of the pull request texts it writes) only reads the token: it sends a token that doesn't
  expire within a minute and never renews it (`YouTrackService::without_renewal`). A renewal rotates the refresh token,
  and the app, which keeps the old one in memory, would be signed out by its next renewal; an expiring token gives `bd`
  no summary until the app renews it.
  [@test] ../../crates/youtrack/src/service_test.rs (`a_service_without_renewal_sends_a_valid_token_and_leaves_an_expiring_one_to_the_app`)
- `invalid_grant` is an expired sign-in; a 401 to a token that can't be renewed is a rejected one. Either is not sent
  again until the next sign-in (`forget`): Settings says "Your YouTrack sign-in expired: sign in again" or "YouTrack
  rejected the token: sign in again" above the sign-in, with the URL of the token.
  [@test] ../../crates/youtrack/src/service_test.rs (`a_refused_refresh_token_is_an_expired_sign_in`, `a_rejected_pasted_token_is_not_sent_again`)
  [@test] ../../tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should ask to sign in again once the sign-in expired`)

### Requests

- A token goes only to the YouTrack it is for and its Hub: redirects are not followed, every path is built from
  segments, and an issue key is checked first (`^[A-Za-z][A-Za-z0-9_]*-\d+$`), so a key a repository's `vcs.xml` put
  into a link never names another path. A read is asked again once after a transient failure, a write never.
  [@test] ../../crates/youtrack/src/url_test.rs (`only_an_issue_key_is_a_path_segment_of_a_lookup`, `a_segment_is_escaped_and_never_leaves_the_base`)
  [@test] ../../crates/youtrack/src/issue_test.rs (`a_redirect_is_not_followed`)
  [@test] ../../crates/youtrack/src/service_test.rs (`a_lookup_of_something_else_than_a_key_is_never_sent`)

### Settings

- The YouTrack page of Settings ([Settings](settings.spec.md)), asked again on each open of the window and on every
  sign-in or sign-out of any window (`get_youtrack_auth_status`, `youtrack-auth-changed`), says what the sign-in is
  for: "Hover an issue key to see the issue, and mark it fixed once its work lands".
  - Signed out: the URL with its source ("From .idea/vcs.xml of <repository>"), or, empty, "The address you open
    YouTrack at, e.g. https://youtrack.example.com"; "Sign In to YouTrack" (disabled without a URL) with "Opens
    YouTrack in the browser. The sign-in renews itself until YouTrack ends it." under it; below, as a link, "Use a
    Token Instead", the fallback.
  - The token form in the order of its steps: "For a YouTrack that does not sign in apps in the browser: create a
    token there, then paste it here", "Create Token…" (Account Security of the profile), the field "Token", "Save
    Token" and Cancel.
  - While the browser signs in: "Sign in to YouTrack in the browser tab that opened, then return here", "Open the
    Page Again", Cancel and "Waiting for the browser…" with a spinner; the URL field stays as typed.
  - Signed in: "Signed in to youtrack.example.com as Jane Doe (jane)", and how: "Through the browser since yesterday
    3:00 PM. It renews itself until YouTrack ends the sign-in, then Branch Deck asks you to sign in again.", "With a
    pasted token since …. It works until you revoke it in YouTrack.", or for `YOUTRACK_TOKEN` "Set in the
    environment of Branch Deck: unset it to sign in here."; Sign Out (not for the environment), and "Manage
    Sign-In" (a browser sign-in) or "Manage Tokens" (a token), which opens the page that lists the token
    (`manageUrl`): Account Security of the YouTrack profile, or the profile of the Hub when the Hub is not built into
    the YouTrack (youtrack.jetbrains.com signs in at hub.jetbrains.com), where the refresh token of the sign-in is.
  [@test] ../../tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`, `should cancel the browser sign-in, and paste a token instead`)
  [@test] ../../crates/youtrack/src/service_test.rs (`a_refused_refresh_token_is_an_expired_sign_in`, `the_tokens_of_a_sign_in_through_an_external_hub_are_managed_in_the_hub`)
- The toasts: "Signed in to youtrack.example.com as Jane Doe"; "Cannot sign in to YouTrack" with the reason; "Signed
  out of YouTrack", which says that YouTrack keeps the token until it is revoked in Account Security.

### The issue popover

- Signed in, a link `LinkedText` renders to `<url>/issue/<key>` of the YouTrack of the token (same origin, under its
  path, a valid key) carries `data-issue-key`; nothing else changes, and signed out no link carries it, so the links
  of a user without YouTrack stay as they are and no request is sent.
  [@test] ../../src/lib/youtrack.test.ts (`takes the key of a link to the issue page of the YouTrack`, `refuses a link of another host, path or scheme, and what is not a key`)
  [@test] ../../tests/e2e/youtrack/issue-popover.spec.ts (`should leave the links of a signed-out user as they are`)
- One popover per window (`IssueHoverHost`, in `App`): a hover of 500 ms on such a link, or its keyboard focus
  (`:focus-visible`), opens it under the link; the pointer may cross to it (200 ms), and it stays while the pointer is
  over it. One delegated listener, not a popover per link of every commit list. It opens without taking the focus;
  Escape closes it only, so a dialog under it stays; it closes once its link leaves the page (a sync, a collapsed
  branch, a closed toast).
  [@test] ../../tests/e2e/youtrack/issue-popover.spec.ts (`should show the issue of a key on hover, mark it fixed, and undo it from the toast`, `should open on the keyboard focus, and mark the issue of a branch fixed from its menu`, `should keep the branch preview open under the popover of its title`)
- It lies in the layer of its link: the popup of another popover, a dialog (the branch preview) or the toasts. The
  layer counts it as its own (a click in it is no outside press, the toast's timer waits while the pointer is over it),
  and it is above the layer.
  [@test] ../../tests/e2e/youtrack/issue-popover.spec.ts (`should keep the branch preview open under the popover of its title`, `should show the issue of a key on hover, mark it fixed, and undo it from the toast`)
- It shows the key, the summary, the state ("Open"; resolved: "Fixed · resolved 1 hour ago" in `text-success`) and the
  single-value fields with a value (enum, user, version, owned, build) by the names YouTrack gives them, in its order:
  a field is told by its `$type`, never by its name. It asks when it opens (`get_youtrack_issue`, the backend keeps an
  answer a minute); "Not found, or no access" (YouTrack answers 404 for both), "Cannot load ABC-1: <reason>" with Try
  Again, and for a rejected or expired sign-in the reason with "YouTrack Settings…", which opens Settings at YouTrack.
  [@test] ../../crates/youtrack/src/issue_test.rs (`an_issue_shows_its_state_and_its_single_value_fields_by_type`, `a_resolved_issue_does_not_ask_for_the_values_of_its_state`)
  [@test] ../../tests/e2e/youtrack/issue-popover.spec.ts (`should show a resolved issue without Mark Fixed, and send an expired sign-in to Settings`)
- Its actions come from one builder (`getIssueActions`): Mark Fixed (an open issue with a state field) and Open in
  YouTrack.

### Mark Fixed

- Mark Fixed sets the state field of an open issue to Fixed, the resolved state of YouTrack's default workflow, by
  the command `<field> Fixed` (`POST /api/commands`; a field or a value with spaces stays as it is: YouTrack takes no
  braces, `State {Fixed}` is "State expected", checked with `/api/commands/assist`). The field
  is the issue's state field (`StateIssueCustomField`, `StateMachineIssueCustomField`); the issue's field is asked for
  its values only for an open issue (the bundles of a project are large), and without a value Fixed the action is
  disabled with "State of ABC has no Fixed state". The backend refuses an issue resolved already ("ABC-1 is already
  Duplicate"), so a menu built before cannot turn a Duplicate into Fixed.
  [@test] ../../crates/youtrack/src/issue_test.rs (`a_command_names_the_field_and_the_value_without_braces`)
  [@test] ../../crates/youtrack/src/service_test.rs (`a_state_change_returns_the_previous_state_and_an_undo_checks_the_state_first`, `a_resolved_issue_is_not_marked_again`)
- It is one operation (`mark-fixed` of `issue:<key>`, remote): a second click joins it. "Marked ABC-1 Fixed", "Was
  Open", with Undo, which sets the previous state only while the issue is still Fixed ("ABC-1 is now Verified"
  otherwise), then "ABC-1 is Open again"; YouTrack clears `resolved` itself. A failure is "Cannot mark ABC-1 fixed" or
  "Cannot undo the Fixed state of ABC-1" with YouTrack's reason (`notifyError`, so in Activity too).
  [@test] ../../tests/e2e/youtrack/issue-popover.spec.ts (`should show the issue of a key on hover, mark it fixed, and undo it from the toast`, `should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)
- Surfaces: the popover of every key (the rows, the commits, the preview, the toasts, the merge toast of a robot
  among them), and "Mark ABC-1 Fixed" in the menu of a branch whose issue key is of the YouTrack: the keyboard path,
  where the work landed. Patronus marks an issue fixed itself only for a commit that says so ("fixed ABC-1", also
  "ABC-1 fix the …"): for those the popover shows it resolved already.
  [@test] ../../tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)

## User Experience

- The popover is `w-96 gap-3 p-3`: the key `text-xs text-muted-foreground`, the summary `font-medium`, the fields a
  `text-xs` grid of names (muted) and values, the actions `xs` buttons (Mark Fixed `outline`, Open in YouTrack `ghost`).
- The page is one section: `text-sm` lines, the hints `text-xs text-muted-foreground`, `xs` buttons ("Sign In to
  YouTrack", "Sign Out" and "Create Token…" `outline`, "Save Token" default, "Use a Token Instead" a muted `link` at
  the edge of the text, the others `ghost`), the token form and the wait in a `rounded-lg` bordered panel, a problem
  in `text-warning`.

## Testing / Local Run

- `cargo nextest run -p youtrack`: the URL policy, the sign-in against a fake YouTrack with a built-in Hub (the fake of
  `github::fake_github`), the issue answers, the renewal and the rejection (`service_test.rs`).
- `pnpm test:unit`: `src/lib/youtrack.test.ts`, `src/stores/youtrack.test.ts`.
- `pnpm e2e`: `tests/e2e/youtrack/` (the sign-in, the popover). The test server answers `get_youtrack_auth_status` as signed out; a test answers
  the sign-in commands in the page (`routeCommand`, `tests/e2e/helpers/youtrack-helpers.ts`) and links the keys of the
  template to `https://youtrack.example.com/issue/$0` for one sync (`linkIssuesToYouTrack`).
- By hand: `pnpm tauri dev`, Settings → YouTrack → Sign In to YouTrack with `https://youtrack.jetbrains.com`.
