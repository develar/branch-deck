## ADDED Requirements

### Requirement: Wire: The build id names the version and the commit

The build id of a binary MUST be `<version>+<commit>`, the commit as its build names it (`BRANCH_DECK_COMMIT`, with
`-dirty` for a tree with changes, else `GITHUB_SHA` of the job of CI, else `dev`) and never read by a git process of
the app, so two releases of one version from different commits are two builds.

#### Scenario: The build of a binary
- **WHEN** a client reads `serverInfo.build` of a backend
- **THEN** it is the version of the binary, `+`, and the commit its build named
  [@test] crates/backend-host/src/identity_test.rs (`the_build_id_names_the_version_and_the_commit`)

## MODIFIED Requirements

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

### Requirement: Commands: An answer that names a version follows that version

An answer or a stream event that names a version V of a repository (`SyncAnswer.version`, the answer of
`refresh_facts`, `Outcome.versionAfter`) MUST reach a connection after every action of that channel up to V to each of
its subscribed windows, a due resync included, each to its own version (the `done` of a rewrite names the rewrite,
its answer the version after the sync that follows); it SHALL not wait when no window of the connection subscribes, and
the other events of a stream have no order against the actions.

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
