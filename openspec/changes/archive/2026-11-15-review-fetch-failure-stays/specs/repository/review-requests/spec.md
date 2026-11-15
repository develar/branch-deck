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
- **GIVEN** the fetch of the request the window shows failed
- **WHEN** the window shows the same request anew: the session patched the review requests, a kept view shows again
- **THEN** nothing is fetched and the failure stays with its Retry, which fetches again; a pushed request is fetched
  at once
  [@test] src/stores/review-fetch.test.ts (`keeps a failure on screen when the window asks for the same request again, and fetches it again only on Retry`, `fetches a pushed request at once, also after a failure of the one before`)

#### Scenario: A fetch killed for a newer one
- **WHEN** a fetch is killed for a newer one ("canceled")
- **THEN** no failure is shown
