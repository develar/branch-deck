## MODIFIED Requirements

### Requirement: Review window: A failed fetch says why, with Retry

A failed fetch MUST be a `destructive` callout "Cannot fetch pull request #101" with the reason, Retry and Open Pull
Request, which stays until Retry or another request. A fetch killed for a newer one ("canceled") MUST be no failure.

#### Scenario: GitHub asks for credentials
- **WHEN** the fetch fails as GitHub asks for credentials
- **THEN** the reason says "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)"
  [@test] crates/sync-core/src/review_fetch_test.rs (`refused_github_credentials_are_told_how_to_fix`)

#### Scenario: Another failure, and Retry
- **WHEN** the fetch fails otherwise
- **THEN** the callout says "<remote>: <git's last line>", and Retry fetches again
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say why a fetch failed and fetch again on Retry`)

#### Scenario: The window shows the same request anew
- **GIVEN** the fetch of the request, or of the stack (Review Stack), the window shows failed
- **WHEN** the window shows the same request or the same layers anew: the session patched the review requests, a kept
  view shows again
- **THEN** nothing is fetched and the failure stays with its Retry, which fetches again; a pushed request, a pushed or
  rebased layer, is fetched at once, and the failure of the one before goes
  [@test] src/stores/review-fetch.test.ts (`keeps a failure on screen when the window asks for the same request again, and fetches it again only on Retry`, `fetches a pushed request at once, also after a failure of the one before, whose entry goes`, `keeps a failure on screen when the window asks for the same stack again, and fetches it again only on Retry`, `fetches the stack at once when a layer was pushed or rebased, also after a failure, whose entry goes`)

#### Scenario: A fetch killed for a newer one
- **WHEN** a fetch is killed for a newer one ("canceled")
- **THEN** no failure is shown

### Requirement: Review window: A withdrawn request stays with a quiet line

A request withdrawn while the window shows it MUST stay, as the list had it last, with the quiet line "No longer waiting
for your review" ("No longer in Reviews" of one opened from Reviews).

#### Scenario: The request is withdrawn meanwhile
- **GIVEN** the window shows a request
- **WHEN** the request is withdrawn
- **THEN** the review and its refs stay, with the quiet line "No longer waiting for your review"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)

#### Scenario: A request pushed while its review is open, then withdrawn
- **GIVEN** the window shows a request, which someone pushed meanwhile
- **WHEN** the window fetched the pushed request, and the request then leaves the list
- **THEN** the window shows the pushed request it fetched, not the one it opened on, and fetches nothing again
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should fetch a request pushed while its review is open once, and keep the pushed one once it is withdrawn`)
