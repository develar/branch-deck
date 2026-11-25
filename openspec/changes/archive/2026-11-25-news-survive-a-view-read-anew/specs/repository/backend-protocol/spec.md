## MODIFIED Requirements

### Requirement: Events: A news is told once by the backend

A client MUST claim a news before it tells it (`x-branch-deck/claimNews {repositoryPath, news}`; the command
`claim_news` of a page of the app), the news named by what it tells (`NewsClaim`), never by its id, which a session
counts from 1 under its own numbering (`RepositoryView::news_line`; a session opened anew in place numbers on); the host SHALL answer `first: true` to the first claim of that news of that repository by any client and
`false` to every later one. A page of the app claims a news before its OS notification, and tells it only when its
claim is the first.

#### Scenario: The app and the IDE on one repository
- **WHEN** a robot of a repository fails while the app and the IDE show it, neither with the focus
- **THEN** the first claim of the news is `first: true` and every later one `first: false`, so one of them tells the failure
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)
  [@test] crates/ide-host/src/server_test.rs (`a_news_another_client_claimed_first_is_no_notification_of_the_bridge`)
  [@test] src/lib/os-notification.test.ts (`tells a news by the OS only when its claim is the first of the backend, so the app and the IDE tell it once`)

#### Scenario: A session opened anew
- **WHEN** the session of a repository that opened as another tells the failure of another robot under the id of a news
  claimed in the session before
- **THEN** its claim is `first: true`
  [@test] crates/app-api/src/connections_test.rs (`a_news_is_claimed_once_by_what_it_tells`)
