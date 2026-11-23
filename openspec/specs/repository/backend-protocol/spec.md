---
name: Backend Protocol
description: Requirements for the one backend of a user, an AHP-shaped host of app-api in the process of the desktop app for the Tauri shell, the bridge of the IDE and later brd, over the in-process transport and a local socket.
style: plain-1
targets:
  - docs/design/backend-protocol/backend.schema.json
  - docs/design/backend-protocol/corpus/connection.ndjson
  - docs/design/backend-protocol/corpus/repository-channel.ndjson
  - docs/design/backend-protocol/corpus/command-stream.ndjson
  - docs/design/backend-protocol/corpus/host-requests.ndjson
  - docs/design/backend-protocol/corpus/errors.ndjson
  - crates/backend-host/src/host.rs
  - crates/backend-host/src/connection.rs
  - crates/backend-host/src/channel.rs
  - crates/backend-host/src/call.rs
  - crates/backend-host/src/client.rs
  - crates/backend-host/src/outbox.rs
  - crates/backend-host/src/wire.rs
  - crates/backend-host/src/identity.rs
  - crates/backend-host/build.rs
  - scripts/install-app.sh
  - crates/backend-host/src/in_process.rs
  - crates/backend-host/src/stream.rs
  - crates/backend-host/src/socket.rs
  - crates/app-api/src/connections.rs
  - crates/app-api/src/transport.rs
  - crates/app-api/src/context.rs
  - crates/app-api/src/session/windows.rs
  - crates/app-api/src/reads/conflict_fix.rs
  - crates/app-api/src/remote/suggest.rs
  - src/platform/backend/contract.test.ts
---

# Backend Protocol

Status: Draft
Date: 2026-11-23

## Purpose

The process of the desktop app is the one backend of a user (#260): every client of it, the Tauri shell in the process,
the bridge of the IDE over a local socket and later brd, speaks one AHP-shaped protocol with it, so two windows on one
repository show one session, its last sync, its facts and its estimates, and each remote service gets one poll per
repository. The design and its decisions: [Backend Protocol](../../../../docs/design/backend-protocol.md).

## Requirements

### Requirement: Wire: Every message is one JSON-RPC 2.0 object on one line

A connection MUST carry one JSON-RPC 2.0 object per line, of 16 MiB at most, and every `params` MUST name its
`channel`: `ahp-root://` for a connection command and a root command, `x-branch-deck:/repository/<n>` for a channel
command. A method keeps its AHP name where its meaning is AHP's, and is `x-branch-deck/<name>` else.

#### Scenario: A batch array
- **WHEN** a client sends a JSON array of two requests on one line
- **THEN** the host answers `-32600` with `data.reason` `invalidRequest` and the id `null`, and serves neither
  [@test] crates/backend-host/src/corpus_test.rs (`the_connection_corpus_runs`)
  [@test] crates/backend-host/src/wire_test.rs (`a_line_is_a_request_a_notification_or_an_answer`)

#### Scenario: A method without a channel
- **WHEN** a client sends `ping` without `channel`
- **THEN** the host answers `-32602 badParams`
  [@test] crates/backend-host/src/corpus_test.rs (`the_connection_corpus_runs`)
  [@test] crates/backend-host/src/wire_test.rs (`a_line_is_a_request_a_notification_or_an_answer`)

### Requirement: Wire: Initialize names the versions, the builds and the capabilities

The first request of a connection MUST be `initialize` with the offered AHP versions, `clientId`, `clientInfo.build`
and `_meta["x-branch-deck"]` with the vocabulary version and the capabilities of the client. The host SHALL answer the
highest version it serves in their caret ranges, `serverSeq`, `serverInfo.build` and its vocabulary version.

#### Scenario: An accepted offer
- **WHEN** a client offers `1.0.0` with the build of the host
- **THEN** the answer has `protocolVersion` `1.0.0`, `serverSeq`, `snapshots: []` and `serverInfo` with `name`
  `branch-deck` and the build
  [@test] crates/backend-host/src/corpus_test.rs (`the_connection_corpus_runs`)
  [@test] crates/backend-host/src/wire_test.rs (`the_host_takes_the_highest_offered_version_of_its_major`)

#### Scenario: A request before initialize
- **WHEN** a client sends `subscribe` before `initialize`
- **THEN** the host answers `-32600 notInitialized`, and a second `initialize` after the first is
  `-32600 alreadyInitialized`
  [@test] crates/backend-host/src/corpus_test.rs (`the_connection_corpus_runs`)

#### Scenario: No common version
- **WHEN** a client offers only `2.0.0`
- **THEN** the host answers `-32005 ahpVersion`
  [@test] crates/backend-host/src/corpus_test.rs (`the_connection_corpus_runs`)
  [@test] crates/backend-host/src/wire_test.rs (`the_host_takes_the_highest_offered_version_of_its_major`)

### Requirement: Wire: The build id names the version and the commit

The build id of a binary MUST be `<version>+<commit>`, the commit as its build names it (`BRANCH_DECK_COMMIT`, with
`-dirty` for a tree with changes, else `GITHUB_SHA` of the job of CI, else `dev`) and never read by a git process of
the app, so two releases of one version from different commits are two builds.

#### Scenario: The build of a binary
- **WHEN** a client reads `serverInfo.build` of a backend
- **THEN** it is the version of the binary, `+`, and the commit its build named
  [@test] crates/backend-host/src/identity_test.rs (`the_build_id_names_the_version_and_the_commit`)

### Requirement: Errors: A failure of the protocol is a code with data.reason

A failure of the protocol MUST be a JSON-RPC error whose code is a JSON-RPC or an AHP code and whose `data.reason`
names its kind in camelCase; an app-api refusal of a method without an answer of its own SHALL be `-32603 apiError`
with the `ApiError` in `data.apiError`. A receiver MUST ignore a field of `data` it does not know.

#### Scenario: An unknown method
- **WHEN** a client asks `x-branch-deck/nothing`
- **THEN** the answer is `-32601 unknownMethod` with the id of the request
  [@test] crates/backend-host/src/corpus_test.rs (`the_connection_corpus_runs`)

#### Scenario: A line that is not JSON
- **WHEN** a client sends a line that does not parse
- **THEN** the answer is `-32700 parse` with the id `null`, and the connection stays open
  [@test] crates/backend-host/src/corpus_test.rs (`the_connection_corpus_runs`)

### Requirement: Socket: The socket of a build is owner-only

The socket MUST be `<base>/branch-deck-<key>/backend.sock`, the key 16 hex digits of a hash of the data directory and
the path of the executable, in a directory of its owner with mode 0700, and both sides MUST refuse a directory that is
not; on Windows the pipe `\\.\pipe\branch-deck-<key>` with a DACL of its owner and remote clients refused.

#### Scenario: A dev build beside a release
- **WHEN** a dev build and a release of the app run as one user
- **THEN** each has its own socket and its own backend
  [@test] crates/backend-host/src/socket_test.rs (`a_dev_build_beside_a_release_has_its_own_key`)

#### Scenario: A directory of another mode
- **WHEN** the directory of the key exists with mode 0755
- **THEN** the backend opens no socket and logs why, and the bridge does not connect to it
  [@test] crates/backend-host/src/socket_test.rs (`a_directory_of_another_mode_is_refused_by_both_sides`)

### Requirement: Socket: A peer of another user is refused

The host MUST close a connection whose peer runs as another user (its uid on Unix, its SID on Windows), and a client
MUST close a connection to a server that runs as another user, before either reads a message.

#### Scenario: The uid of the peer
- **WHEN** a process of another uid connects to the socket
- **THEN** the host closes the connection without an answer
  [@test] crates/backend-host/src/socket_test.rs (`a_peer_of_another_user_is_refused`)

### Requirement: Socket: One backend per key holds backend.lock

A backend MUST hold `backend.lock` in the directory of its key for the life of its process and release it at its exit;
a starting backend SHALL retry the lock for a few seconds, and its holder removes a stale socket before it listens.
A second start of the app MUST connect and send `x-branch-deck/showMain`, which shows the main window.

#### Scenario: A second start of the app
- **WHEN** the app starts while its backend runs without a window
- **THEN** the backend builds and shows the main window, and the second process exits
  [@test] crates/backend-host/src/socket_test.rs (`a_second_start_of_the_app_shows_the_main_window_of_the_backend`)
  [@test] crates/backend-host/src/socket_test.rs (`a_second_backend_cannot_take_the_lock_while_the_first_holds_it`)

#### Scenario: A stale socket
- **WHEN** a backend starts and finds a socket file that no process listens on
- **THEN** it removes the file under the lock and listens
  [@test] crates/backend-host/src/socket_test.rs (`a_stale_socket_is_removed_under_the_lock_and_the_backend_listens`)

### Requirement: Socket: The stale side yields to a client of another build

An `initialize` whose `clientInfo.build` is not the host's MUST be `-32005 otherBuild` with the host's build and
`exits`. The side whose executable on disk is no longer its build yields: a stale backend without a window exits after
the answer, a stale backend with a window stays and offers Restart to Update, and a stale bridge exits with 1.

#### Scenario: The app updated in place, no window
- **WHEN** a bridge of the new build connects to a backend of the old one that has no window
- **THEN** the answer is `otherBuild` with `exits: true`, and the backend exits after it
  [@test] crates/backend-host/src/connection_test.rs (`a_stale_backend_without_a_window_exits_after_its_answer`)
  [@test] crates/backend-host/src/identity_test.rs (`an_executable_replaced_on_disk_is_stale`)

#### Scenario: The app updated in place, a window open
- **WHEN** a bridge of the new build connects to a backend of the old one that shows a window
- **THEN** the answer is `otherBuild` with `exits: false`, and the backend stays and tells Restart to Update
  [@test] crates/backend-host/src/connection_test.rs (`a_stale_backend_with_a_window_stays_and_tells_restart_to_update`)

### Requirement: Events: The events of app-api go to every connection

An event of app-api MUST be `x-branch-deck/event {event, payload, window?}` to every connection, without `window` to
each of its windows, and one to the main window to the main window of each connection; an event a page emits SHALL stay
inside its connection.

#### Scenario: A setting changed in the IDE
- **WHEN** a window of the IDE sets a setting
- **THEN** every window of the app gets `settings_changed` with it
  [@test] crates/backend-host/src/call_test.rs (`a_setting_changed_in_the_ide_reaches_every_window_of_the_app`)
  [@test] crates/app-api/src/connections_test.rs (`an_event_of_the_app_goes_to_every_connection_and_one_to_a_window_to_its_connection`)

#### Scenario: An event of a page
- **WHEN** a review window of the IDE emits `reveal-branch` to its main window
- **THEN** the main view of the IDE gets it, and no window of the app does
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_emit_of_the_page_is_an_event_of_the_views`)

### Requirement: Events: A news is told once by the backend

A client MUST claim a news before it tells it (`x-branch-deck/claimNews {repositoryPath, news}`), and the host SHALL
answer `first: true` to the first claim of that news of that repository and `false` to every later one. The desktop
page claims nothing until it goes through the host, so until then the app and the IDE may both tell a news.

#### Scenario: The app and the IDE on one repository
- **WHEN** a robot of a repository fails while the app and the IDE show it, neither with the focus
- **THEN** the first claim of the news is `first: true` and every later one `first: false`, so one of them tells the failure
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)

### Requirement: Channel: One channel per repository path for the life of the backend

`x-branch-deck/repositoryChannel {repositoryPath}` MUST answer the URI of the channel of that path,
`x-branch-deck:/repository/<n>`, the same URI for the same path for the life of the backend; the path is the one the
window names, without a trailing separator.

#### Scenario: Two clients on one repository
- **WHEN** the Tauri shell and the bridge ask the channel of `/x/repo`
- **THEN** both get the same URI, and their windows subscribe to one session
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)
  [@test] crates/backend-host/src/channel_test.rs (`two_connections_on_one_repository_get_the_same_actions_and_the_end_of_one_leaves_the_other`)

#### Scenario: An open after the last unsubscribe
- **WHEN** the last subscriber of a channel unsubscribed and a client asks the channel of its path again
- **THEN** the answer is the same URI, and a subscribe to it opens the session again
  [@test] crates/backend-host/src/channel_test.rs (`an_open_after_the_last_unsubscribe_is_the_same_channel_and_opens_the_session_again`)

### Requirement: Channel: A subscription is a window's

`subscribe` MUST name the window of the connection it is for (`_meta["x-branch-deck"].window`) and subscribe that
window to the session of the prefix the git config names now, as `subscribe_view` does; a second subscribe of the same
window of the connection SHALL be `-32011 alreadySubscribed`, and `unsubscribe` of the window ends its subscription.

#### Scenario: Two windows of one connection
- **WHEN** the main window and a review window of the Tauri shell subscribe to one channel
- **THEN** both are subscribed, each with its own snapshot, and a second subscribe of the main window is
  `alreadySubscribed`
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

#### Scenario: A subscribe without a window
- **WHEN** a client subscribes to a repository channel without `window`
- **THEN** the answer is `-32602 badParams`
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

### Requirement: Channel: A subscribe answers the view with its Version

A subscribe MUST answer `{snapshot: {resource, state, fromSeq}}`, the state the `RepositoryView` of the session with
its `Version`, and every later action of that window SHALL have a higher `serverSeq`; a subscription the session refuses
is `-32603 apiError` with its `ApiError`.

#### Scenario: A subscribe without have
- **WHEN** a window subscribes to a channel without `have`
- **THEN** the snapshot holds the view of the session at its `version`, and the next `viewPatched` of the window has
  `serverSeq` above `fromSeq` and `base` equal to that version
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

#### Scenario: A repository without a prefix
- **WHEN** a window subscribes to the channel of a repository with no branch prefix
- **THEN** the answer is `-32603 apiError` with `data.apiError.code` `no_branch_prefix`
  [@test] crates/backend-host/src/corpus_test.rs (`the_errors_corpus_runs`)

### Requirement: Channel: A subscribe with have answers held and the catch-up first

A subscribe with `_meta["x-branch-deck"].have`, the version the window holds, MUST answer a snapshot without state and
with `_meta["x-branch-deck"].held` that version, then as the first action of the window the `viewPatched` the session
builds from `have`, when it moved since: the session patches only from the seed of its warm start, so any other `have`
but the version it holds SHALL get the whole state.

#### Scenario: The seed of the warm start
- **WHEN** a window subscribes with the version of the seed and the session synced once since
- **THEN** the snapshot is `held`, and the first action is one `viewPatched` from the seed to the version after the sync
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

#### Scenario: The view the client holds
- **WHEN** a window subscribes with the version the session holds now
- **THEN** the snapshot is `held`, and no catch-up action follows
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

#### Scenario: A seed of another epoch
- **WHEN** a window subscribes with a version of another epoch
- **THEN** the snapshot holds the whole view
  [@test] crates/backend-host/src/channel_test.rs (`a_seed_of_another_epoch_gets_the_whole_view`)

### Requirement: Channel: Each ViewPatch is one viewPatched action to each window

Each patch of the session MUST be one action `x-branch-deck/viewPatched {patch}` of its channel, in the order of the
session, in one frame per subscribed window that names the window, every frame with the `serverSeq` of the action; a
client SHALL apply it with `repository_model::patch::apply_patch` only to the view at its `base`. A sync or an operation
is several patches, and the session sends patches of its own (a fact read again): no client SHALL count on one per
command.

#### Scenario: Two connections
- **WHEN** a sync of a repository that a window of each of two connections subscribes to ends
- **THEN** both get the same `viewPatched` with the same `serverSeq`, and their views have one version
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)
  [@test] crates/backend-host/src/channel_test.rs (`two_connections_on_one_repository_get_the_same_actions_and_the_end_of_one_leaves_the_other`)

#### Scenario: Two windows of one connection
- **WHEN** a sync of a repository that two windows of one connection subscribe to ends
- **THEN** the connection gets two frames of the action, one naming each window
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

### Requirement: Channel: Another epoch is a resync on the same channel

A move of the channel to another session (a new prefix, a session opened anew) MUST be `x-branch-deck/resync
{channel, window, snapshot}` to every subscribed window, the state the view of that session with the `Version` of its
epoch; no patch of the old session SHALL follow it.

#### Scenario: A prefix set in a terminal
- **WHEN** the prefix of the repository changes and a window subscribes again
- **THEN** every other subscribed window of the channel gets one resync with the view of the new prefix, of another
  epoch, and the window that subscribed gets it as its snapshot
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

### Requirement: Channel: A slow subscriber is resynced

A window whose waiting actions of a channel pass 8 MiB MUST get them dropped and one `x-branch-deck/resync` with the
snapshot once the queue of its connection drains, never a backlog past the cap and never a silent gap.

#### Scenario: A connection that reads nothing for a while
- **WHEN** a connection holds 9 MiB of patches of a channel unread
- **THEN** it gets one resync whose `fromSeq` is above the last action it got, and the patches between are not sent
  [@test] crates/backend-host/src/outbox_test.rs (`a_window_past_the_cap_is_resynced_once_the_queue_drains`)

### Requirement: Channel: The end of a session ends the subscription

The host MUST tell `x-branch-deck/subscriptionEnded {channel, window, reason}` when the session of a channel goes away
(`sessionClosed`) or the channel closes (`closed`); a client SHALL read a reason it does not know as `closed` and may
subscribe again.

#### Scenario: The repository is deleted
- **WHEN** the session of a subscribed repository closes because its repository is gone
- **THEN** each subscribed window gets `subscriptionEnded` with `sessionClosed`, and no action of the channel after it
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

### Requirement: Channel: The channel takes no intent

A `dispatchAction` on a repository channel MUST get its echo with `rejectionReason` `unknownAction` and change
nothing: an intent of the page is the root command `execute`.

#### Scenario: An intent on the channel
- **WHEN** a client dispatches an action on a repository channel
- **THEN** it gets one echo with `rejectionReason` `unknownAction`, and the view keeps its version
  [@test] crates/backend-host/src/corpus_test.rs (`the_repository_channel_corpus_runs`)

### Requirement: Channel: The demand is set by window

`x-branch-deck/setDemand {channel, window, demand}` MUST replace the demand of the subscription of that window, and
the session SHALL ask for the facts of the union of its subscriptions; the subscription of a closed window and its
demand end with it.

#### Scenario: A review window and the main window of one connection
- **WHEN** the main window demands the branch `a` and a review window of the same connection the branch `b`
- **THEN** the session reads the facts of both, and after the review window closes only those of `a`
  [@test] crates/backend-host/src/channel_test.rs (`the_demand_of_a_window_ends_with_the_window`)
  [@test] crates/repository-model/src/session_test.rs (`the_demand_of_a_session_is_the_union_of_its_windows_and_goes_with_each`)

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

An answer or a stream event that names a version V of a repository (`SyncAnswer.version`, `refresh_facts`,
`Outcome.versionAfter`, the `done` of a rewrite the rewrite's, its answer the one after its sync) MUST reach a
connection after every action of that channel up to V to each of its subscribed windows, a due resync included; it
SHALL not wait when no window of the connection subscribes. Other stream events have no order against the actions.

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
