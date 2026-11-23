## ADDED Requirements

### Requirement: Commands: Every command of the registry is a root command

Every command of `for_each_command!` but the channel commands MUST be the root command `x-branch-deck/<name>`, its name
of the registry verbatim, with `{channel: "ahp-root://", window?, args}`, `args` its arguments as the page's `invoke`
sends them; a command of a window SHALL be `-32602 badParams` without `window`.

#### Scenario: A read
- **WHEN** a client asks `x-branch-deck/get_commit_files` with the arguments of a commit
- **THEN** the answer is `{value}` with the files of the commit, as the Tauri command answers them

#### Scenario: Arguments that do not parse
- **WHEN** a client asks `x-branch-deck/get_commit_files` with `args: {}`
- **THEN** the answer is `-32602 badParams` with the `ApiError` of the parse in `data.apiError`, and the command does not run

### Requirement: Commands: A command answers its value, its bytes or its error

A command MUST answer `{value}` with the JSON of its output, `{bytes}` with the base64 of the output of a raw command,
or `{error}` with its `ApiError`: a failed command is an answer, never a JSON-RPC error.

#### Scenario: A refused sync
- **WHEN** a client asks `x-branch-deck/sync` of a repository with a rebase in progress
- **THEN** the result is `{error}` whose `code` is `operation_in_progress`

#### Scenario: The highlighting of a blob
- **WHEN** a client asks `x-branch-deck/get_blob_syntax`
- **THEN** the result is `{bytes}`, the bytes the Tauri command answers

### Requirement: Commands: The stream of a command precedes its answer

Each event of the channel of a command MUST be `x-branch-deck/callEvent {requestId, event}`, the id of its request,
in the order the command sends them, every one before its answer; `x-branch-deck/cancel {requestId}` SHALL end the
call where the command can stop, and the call gets exactly one answer.

#### Scenario: The diffs of a commit
- **WHEN** a client asks `x-branch-deck/get_file_diffs`
- **THEN** it gets the `files` event, then a `content` event per file, then the answer

#### Scenario: A cancel of a stream
- **WHEN** a client cancels a `get_file_diffs` that runs
- **THEN** no event of it follows the answer, and it gets one answer

### Requirement: Commands: An answer that names a version follows that version

An answer or a stream event that names a version V of a repository (`SyncAnswer.version`, the answer of
`refresh_facts`, `Outcome.versionAfter`) MUST reach a connection after every action of that channel up to V to each of
its subscribed windows, a due resync included; it SHALL not wait when no window of the connection subscribes.

#### Scenario: A rewrite
- **WHEN** a subscribed client runs `x-branch-deck/execute` of a rewrite
- **THEN** it gets the `viewPatched` to V, then the `done` event, then the answer, both naming V

#### Scenario: An undo without a subscription
- **WHEN** a client that subscribes to no channel runs `execute` of the undo of a rewrite
- **THEN** the undo runs and answers its `Outcome`

### Requirement: Commands: A channel command is the client's

`subscribe_view`, `unsubscribe_view` and `set_demand` MUST be sent by the client as `subscribe`, `unsubscribe` and
`x-branch-deck/setDemand` of the window on the repository channel; at the root they SHALL be `-32601 channelCommand`.
`get_snapshot` stays a root command for the window that asks.

#### Scenario: subscribe_view at the root
- **WHEN** a client asks `x-branch-deck/subscribe_view`
- **THEN** the answer is `-32601` with `data.reason` `channelCommand`

### Requirement: Windows: A window is a connection and a label

A `WindowId` MUST be the connection and the label of the window in it, and the end of a connection SHALL forget each of
its windows as a close does (`forget_window`), and no window of another connection.

#### Scenario: Two main windows
- **WHEN** the app and the IDE each show their main window and the IDE closes
- **THEN** the main window of the app keeps its subscription, its reads and its review fetches

### Requirement: Windows: The client tells the life of its windows

A client MUST tell `x-branch-deck/windowShown`, `windowHidden`, `windowClosed` and `pageLoaded` of its windows: a close
forgets the window, a page that loads again forgets what its page held (`forget_page`), and the host SHALL answer
`WindowHost::is_open` from them.

#### Scenario: A review tab closes
- **WHEN** the bridge tells `windowClosed` of `review:3`
- **THEN** the review fetches of that window stop, and its read of a review plan is superseded

### Requirement: Windows: A read that is one at a time is one per window

A read that app-api keeps one of at a time (the request for branch names, the search of a conflict fix) MUST be one per
window, so a window of one connection never supersedes the read of a window of another.

#### Scenario: Branch names asked in the app and in the IDE
- **WHEN** the IDE asks branch names while the app's request runs
- **THEN** both get their answer

### Requirement: Desktop: The Desktop of a connection is its own

app-api MUST take the `Desktop` of the connection of a call: the native one of the Tauri shell for the connection in
the process; for a socket client the host requests it declares (`openWindow`, `showWindow`, `notify`) and the parts of
its capabilities (`folderDialog`, `updater`, `restart`, `agentCli`, `nativeMenu`), each part it lacks answered as a
desktop without it.

#### Scenario: A review opened in the IDE
- **WHEN** the main view of the IDE asks `open_sub_window` of a review
- **THEN** the bridge gets `x-branch-deck/openWindow`, and no window of the app opens

#### Scenario: Check for updates in the IDE
- **WHEN** a window of the bridge asks `check_for_updates`
- **THEN** the answer is the `ApiError` of a desktop without an updater, and the updater of the app does not run
