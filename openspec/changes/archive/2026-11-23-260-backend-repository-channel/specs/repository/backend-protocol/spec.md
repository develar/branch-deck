## ADDED Requirements

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
builds from `have`, when it moved since; a `have` the session cannot patch from SHALL get the whole state.

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
client SHALL apply it with `repository_model::patch::apply_patch` only to the view at its `base`.

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
