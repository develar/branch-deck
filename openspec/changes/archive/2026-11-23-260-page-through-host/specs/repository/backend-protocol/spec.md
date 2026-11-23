## MODIFIED Requirements

### Requirement: Events: A news is told once by the backend

A client MUST claim a news before it tells it (`x-branch-deck/claimNews {repositoryPath, news}`; the command
`claim_news` of a page of the app), the news named by what it tells (`NewsClaim`), never by its id, which each session
counts from 1; the host SHALL answer `first: true` to the first claim of that news of that repository by any client and
`false` to every later one. A page of the app claims a news before its OS notification, and tells it only when its
claim is the first.

#### Scenario: The app and the IDE on one repository
- **WHEN** a robot of a repository fails while the app and the IDE show it, neither with the focus
- **THEN** the first claim of the news is `first: true` and every later one `first: false`, so one of them tells the failure
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)
  [@test] src/lib/os-notification.test.ts (`tells a news by the OS only when its claim is the first of the backend, so the app and the IDE tell it once`)

#### Scenario: A session opened anew
- **WHEN** the session of a repository opened anew tells the failure of another robot under the id of a news claimed
  in the session before
- **THEN** its claim is `first: true`
  [@test] crates/app-api/src/connections_test.rs (`a_news_is_claimed_once_by_what_it_tells`)

## ADDED Requirements

### Requirement: Pages: Every invoke of a page is a root command of its window

The Tauri shell and the page mock of the e2e server MUST answer every `invoke` of a page of the app through one
in-process connection of the host (`ConnectionId::APP`), as the root command of the window of the page with the
payload of the invoke as its `args`: its `{value}` the answer of the invoke as the text of its JSON, its `{error}` the
rejection with its `ApiError`, the bytes of a raw command as they are, each call event to the channel the page gave,
before the answer.

#### Scenario: A read of a page
- **WHEN** the main page invokes `get_commit_files`, and `sync` of a repository with a rebase in progress
- **THEN** the first answers the files of the commit, the second rejects with the `ApiError` of code
  `operation_in_progress`
  [@test] crates/backend-host/src/page_test.rs (`a_command_of_a_page_answers_its_value_or_its_error`)

#### Scenario: The highlighting of a blob
- **WHEN** a page invokes `get_blob_syntax`
- **THEN** the answer is its bytes
  [@test] crates/backend-host/src/page_test.rs (`a_raw_command_of_a_page_answers_its_bytes`)

#### Scenario: The diffs of a commit
- **WHEN** a review page invokes `get_file_diffs` with its channel
- **THEN** the `files` event is on the channel before the answer
  [@test] crates/backend-host/src/page_test.rs (`the_events_of_a_command_reach_its_channel_before_its_answer`)

### Requirement: Pages: The subscription of a page is the subscription of its window

`subscribe_view` of a page MUST be `subscribe` of its window on the channel of its path with its `have`, answered as
the `ViewSubscription` of an id the pages keep, each `viewPatched` of the window a patch on the channel of the page;
`unsubscribe_view` and `set_demand` of the id SHALL be `unsubscribe` and `x-branch-deck/setDemand` of the window, and
of an id that ended nothing.

#### Scenario: A subscription and its patches
- **WHEN** the main page subscribes, a sync runs, and the page ends its subscription
- **THEN** the answer holds the view, the patches up to the version of the sync are on the channel before the answer of
  the sync, and no patch of a sync after the end reaches the page
  [@test] crates/backend-host/src/page_test.rs (`a_subscription_of_a_page_answers_its_view_and_its_patches_reach_its_channel`)

#### Scenario: The seed of the warm start
- **WHEN** the main page subscribes with the version of the seed and the session synced since
- **THEN** the answer has no view, and the first patch on its channel goes from the seed
  [@test] crates/backend-host/src/page_test.rs (`the_seed_of_a_page_gets_its_catch_up_on_the_channel`)

### Requirement: Pages: A window holds one subscription of a channel

A subscribe of a window that holds a subscription of the channel MUST end that one first, also while it still joins,
so the later end of the replaced id ends nothing.

#### Scenario: A subscription again of the window
- **WHEN** the page subscribes again with the version it holds before it ends the subscription it replaces
- **THEN** the new subscription answers no view, its patches go from that version, and the end of the replaced id ends
  nothing
  [@test] crates/backend-host/src/page_test.rs (`a_subscription_again_of_a_window_replaces_the_one_it_holds`)

#### Scenario: A subscription again while the first joins
- **WHEN** the page subscribes its window again before its first subscribe answered
- **THEN** both answer, and the patches of a sync reach the second subscription alone
  [@test] crates/backend-host/src/page_test.rs (`a_subscription_again_while_the_first_joins_holds_the_window`)

### Requirement: Pages: A resync is a patch no view applies

A resync of a window MUST reach its page as a patch from and to the version V of its snapshot, `{base: V, next: V,
changes: {}}`, which no view applies, so the page reads the snapshot of the session as it does after a gap.

#### Scenario: A prefix set in a terminal
- **WHEN** the prefix of the repository changes and the IDE subscribes to its channel
- **THEN** the page gets the patch `{base: V, next: V, changes: {}}` of the version V of the view of the new prefix
  [@test] crates/backend-host/src/page_test.rs (`a_resync_reaches_the_page_as_a_patch_no_view_applies`)

### Requirement: Pages: The desktop window and the IDE share one backend

The desktop window and the IDE view of one repository MUST show the one session of the backend, its check time
included, and an event of a page of one connection (the Command Palette of the IDE's action) SHALL reach no window of
another.

#### Scenario: A check of the desktop window
- **WHEN** the desktop window and the IDE view show one repository, and a focus of the desktop window a minute after
  the sync finds nothing changed
- **THEN** both say "Checked just now", and the Command Palette action of the IDE opens the palette of its view and none
  of the desktop window
  [@test] tests/e2e/air/one-backend.spec.ts (`the desktop window and the IDE view of one repository say the same check time, and the Command Palette of the IDE opens no palette of the desktop window`)
