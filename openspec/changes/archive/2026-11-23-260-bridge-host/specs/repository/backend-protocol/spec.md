## ADDED Requirements

### Requirement: Channel: A window follows the prefix of now without a new subscription

`x-branch-deck/followPrefix {channel, window}` of a subscribed window MUST make the channel take the session of the
prefix the git config names now: under the prefix it follows the answer is `{}` at once, else every window of the
channel MUST move to the new session, each with one resync, before the answer; a window that subscribes not SHALL get
`-32008 notSubscribed`.

#### Scenario: A prefix set in a terminal while the IDE shows the repository
- **WHEN** the prefix changes and the bridge asks `followPrefix` of its mirror window
- **THEN** its window and every window of the app on the channel get one resync of the new prefix, of another epoch,
  before the answer `{}`
  [@test] crates/backend-host/src/channel_test.rs (`a_follow_of_the_prefix_moves_every_window_of_the_channel_before_its_answer`)
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

### Requirement: Socket: The backend counts its clients

The host MUST count the clients connected now that initialized, the app's own connection left out, and tell each
change of the count (`Host::clients`): the life of an app without a window reads it.

#### Scenario: A bridge connects and ends
- **WHEN** a bridge initializes, and then its connection ends
- **THEN** the count is 1, and then 0; the connection of the app's own windows counts not
  [@test] crates/backend-host/src/channel_test.rs (`the_count_of_the_clients_leaves_out_the_connection_of_the_app`)

## MODIFIED Requirements

### Requirement: Desktop: The Desktop of a connection is its own

app-api MUST take the `Desktop` of the connection of a call: the native one of the Tauri shell for the connection in
the process; for a socket client the host requests it declares (`openWindow`, `showWindow`) and the parts of
its capabilities (`folderDialog`, `updater`, `restart`, `agentCli`, `nativeMenu`), each part it lacks answered as a
desktop without it: a client without `showWindow` (the bridge, which shows its views itself) gets none.

#### Scenario: A review opened in the IDE
- **WHEN** the main view of the IDE asks `open_sub_window` of a review
- **THEN** the bridge gets `x-branch-deck/openWindow`, and no window of the app opens
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)
  [@test] crates/backend-host/src/channel_test.rs (`a_review_opened_in_the_ide_opens_no_window_of_the_app`)

#### Scenario: Check for updates in the IDE
- **WHEN** a window of the bridge asks `check_for_updates`
- **THEN** the answer is the `ApiError` of a desktop without an updater, and the updater of the app does not run
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)

#### Scenario: A window of the IDE due to show
- **WHEN** a window of a client without `showWindow` is due to show (its page settled, a deadline of a window built hidden)
- **THEN** the client gets no `showWindow`; a client that declares it does
  [@test] crates/backend-host/src/channel_test.rs (`a_client_that_shows_its_windows_itself_gets_no_show_window`)

### Requirement: Events: A news is told once by the backend

A client MUST claim a news before it tells it (`x-branch-deck/claimNews {repositoryPath, news}`), and the host SHALL
answer `first: true` to the first claim of that news of that repository and `false` to every later one. The bridge
claims each news it tells; the desktop page claims nothing until it goes through the host, so until then the app and
the IDE may both tell a news.

#### Scenario: The app and the IDE on one repository
- **WHEN** a robot of a repository fails while the app and the IDE show it, neither with the focus
- **THEN** the first claim of the news is `first: true` and every later one `first: false`, so one of them tells the failure
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)
  [@test] crates/ide-host/src/server_test.rs (`a_news_another_client_claimed_first_is_no_notification_of_the_bridge`)
