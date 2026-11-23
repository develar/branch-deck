## ADDED Requirements

### Requirement: Commands: Every command of the registry is a root command

Every command of `for_each_command!` but the channel commands MUST be the root command `x-branch-deck/<name>`, its name
of the registry verbatim, with `{channel: "ahp-root://", window?, args}`, `args` its arguments as the page's `invoke`
sends them; a command of a window SHALL be `-32602 badParams` without `window`.

#### Scenario: A read
- **WHEN** a client asks `x-branch-deck/get_commit_files` with the arguments of a commit
- **THEN** the answer is `{value}` with the files of the commit, as the Tauri command answers them
  [@test] crates/backend-host/src/call_test.rs (`a_read_answers_its_value`)

#### Scenario: Arguments that do not parse
- **WHEN** a client asks `x-branch-deck/get_commit_files` with `args: {}`
- **THEN** the answer is `-32602 badParams` with the `ApiError` of the parse in `data.apiError`, and the command does not run
  [@test] crates/backend-host/src/corpus_test.rs (`the_errors_corpus_runs`)

### Requirement: Commands: A command answers its value, its bytes or its error

A command MUST answer `{value}` with the JSON of its output, `{bytes}` with the base64 of the output of a raw command,
or `{error}` with its `ApiError`: a failed command is an answer, never a JSON-RPC error.

#### Scenario: A refused sync
- **WHEN** a client asks `x-branch-deck/sync` of a repository with a rebase in progress
- **THEN** the result is `{error}` whose `code` is `operation_in_progress`
  [@test] crates/backend-host/src/call_test.rs (`a_refused_sync_answers_its_error`)

#### Scenario: The highlighting of a blob
- **WHEN** a client asks `x-branch-deck/get_blob_syntax`
- **THEN** the result is `{bytes}`, the bytes the Tauri command answers
  [@test] crates/backend-host/src/corpus_test.rs (`the_command_stream_corpus_runs`)

### Requirement: Commands: The stream of a command precedes its answer

Each event of the channel of a command MUST be `x-branch-deck/callEvent {requestId, event}`, the id of its request,
in the order the command sends them, every one before its answer; `x-branch-deck/cancel {requestId}` SHALL end the
call where the command can stop, and the call gets exactly one answer.

#### Scenario: The diffs of a commit
- **WHEN** a client asks `x-branch-deck/get_file_diffs`
- **THEN** it gets the `files` event, then a `content` event per file, then the answer
  [@test] crates/backend-host/src/corpus_test.rs (`the_command_stream_corpus_runs`)

#### Scenario: A cancel of a stream
- **WHEN** a client cancels a `get_file_diffs` that runs
- **THEN** no event of it follows the answer, and it gets one answer
  [@test] crates/backend-host/src/corpus_test.rs (`the_command_stream_corpus_runs`)

### Requirement: Commands: An answer that names a version follows that version

An answer or a stream event that names a version V of a repository (`SyncAnswer.version`, the answer of
`refresh_facts`, `Outcome.versionAfter`) MUST reach a connection after every action of that channel up to V to each of
its subscribed windows, a due resync included; it SHALL not wait when no window of the connection subscribes.

#### Scenario: A rewrite
- **WHEN** a subscribed client runs `x-branch-deck/execute` of a rewrite
- **THEN** the `viewPatched` actions up to the version its `done` event names come before that event, and those up to
  the version its answer names before the answer
  [@test] crates/backend-host/src/corpus_test.rs (`the_command_stream_corpus_runs`)
  [@test] crates/backend-host/src/fence_test.rs (`an_answer_waits_for_the_actions_up_to_its_version`)
  [@test] crates/backend-host/src/fence_test.rs (`an_answer_waits_behind_a_due_resync`)

#### Scenario: An undo without a subscription
- **WHEN** a client that subscribes to no channel runs `execute` of the undo of a rewrite
- **THEN** the undo runs and answers its `Outcome`
  [@test] crates/backend-host/src/corpus_test.rs (`the_command_stream_corpus_runs`)
  [@test] crates/backend-host/src/fence_test.rs (`an_answer_without_a_subscription_goes_at_once`)

### Requirement: Commands: A channel command is the client's

`subscribe_view`, `unsubscribe_view` and `set_demand` MUST be sent by the client as `subscribe`, `unsubscribe` and
`x-branch-deck/setDemand` of the window on the repository channel; at the root they SHALL be `-32601 channelCommand`.
`get_snapshot` stays a root command for the window that asks.

#### Scenario: subscribe_view at the root
- **WHEN** a client asks `x-branch-deck/subscribe_view`
- **THEN** the answer is `-32601` with `data.reason` `channelCommand`
  [@test] crates/backend-host/src/corpus_test.rs (`the_errors_corpus_runs`)

### Requirement: Windows: A window is a connection and a label

A `WindowId` MUST be the connection and the label of the window in it, and the end of a connection SHALL forget each of
its windows as a close does (`forget_window`), and no window of another connection.

#### Scenario: Two main windows
- **WHEN** the app and the IDE each show their main window and the IDE closes
- **THEN** the main window of the app keeps its subscription, its reads and its review fetches
  [@test] crates/backend-host/src/channel_test.rs (`the_end_of_a_connection_forgets_only_its_windows`)

### Requirement: Windows: The client tells the life of its windows

A client MUST tell `x-branch-deck/windowShown`, `windowHidden`, `windowClosed` and `pageLoaded` of its windows: a close
forgets the window, a page that loads again forgets what its page held (`forget_page`), and the host SHALL answer
`WindowHost::is_open` from them.

#### Scenario: A review tab closes
- **WHEN** the bridge tells `windowClosed` of `review:3`
- **THEN** its read of a review plan is superseded, and the window is forgotten as its close forgets it
  [@test] crates/backend-host/src/channel_test.rs (`a_review_tab_that_closes_is_forgotten`)

### Requirement: Windows: A read that is one at a time is one per window

A read that app-api keeps one of at a time (the request for branch names, the search of a conflict fix) MUST be one per
window, so a window of one connection never supersedes the read of a window of another.

#### Scenario: Branch names asked in the app and in the IDE
- **WHEN** the IDE asks branch names while the app's request runs
- **THEN** both get their answer
  [@test] crates/app-api/src/session/windows_test.rs (`the_main_windows_of_two_connections_never_supersede_each_other`)

### Requirement: Desktop: The Desktop of a connection is its own

app-api MUST take the `Desktop` of the connection of a call: the native one of the Tauri shell for the connection in
the process; for a socket client the host requests it declares (`openWindow`, `showWindow`, `notify`) and the parts of
its capabilities (`folderDialog`, `updater`, `restart`, `agentCli`, `nativeMenu`), each part it lacks answered as a
desktop without it.

#### Scenario: A review opened in the IDE
- **WHEN** the main view of the IDE asks `open_sub_window` of a review
- **THEN** the bridge gets `x-branch-deck/openWindow`, and no window of the app opens
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)
  [@test] crates/backend-host/src/channel_test.rs (`a_review_opened_in_the_ide_opens_no_window_of_the_app`)

#### Scenario: Check for updates in the IDE
- **WHEN** a window of the bridge asks `check_for_updates`
- **THEN** the answer is the `ApiError` of a desktop without an updater, and the updater of the app does not run
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)
