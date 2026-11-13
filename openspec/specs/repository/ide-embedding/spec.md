---
name: Branch Deck in the IDE
description: Requirements for running the page and the backend of Branch Deck as an app of IntelliJ IDEA over x-air-app, with brd mcp as its process and its windows as views of the IDE.
style: plain-1
targets:
  - docs/design/ide-embedding/protocol/x-air-app.schema.json
  - docs/design/ide-embedding/protocol/air-app.schema.json
  - docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson
  - docs/design/ide-embedding/protocol/examples/branch-deck.yaml
  - src/air-main.ts
  - src/platform/tauri-page.ts
  - src/platform/air/start.ts
  - src/platform/air/shim.ts
  - src/platform/air/view-mirror.ts
  - src/platform/air/theme.ts
  - src/platform/air/bootstrap.ts
  - src/platform/air/frame-link.ts
  - crates/app-api/src/dispatch.rs
  - crates/mcp-host/src/server.rs
  - crates/mcp-host/src/mirror.rs
  - crates/mcp-host/src/hosts.rs
  - crates/mcp-host/src/resources.rs
  - crates/mcp-host/src/notices.rs
  - crates/mcp-host/src/process.rs
  - crates/mcp-host/src/projects.rs
  - crates/app-settings/src/ide_roots.rs
  - crates/test-server/src/mcp_socket.rs
  - crates/bd-cli/src/install.rs
  - src/components/settings/ToolsPage.tsx
  - src/components/settings/AgentsPage.tsx
  - src/components/repository/AgentsHint.tsx
  - src/hooks/useRevealBranchEvent.ts
---

# Branch Deck in the IDE

Status: Draft
Date: 2026-11-13

## Purpose

Branch Deck runs inside IntelliJ IDEA as an app of the IDE: the page of the desktop app in a frame of the IDE, the
backend of the desktop app as the process `brd mcp` of the project. The main window is a tool window, a review an
editor tab, the settings a frame and the dialogs dialogs of the IDE. The IDE reads `~/.air/apps/branch-deck.yaml`, and
it speaks `x-air-app` of Air with the page and with `brd mcp` ([IDE Embedding](../../../../docs/design/ide-embedding.md)).

## Goals

- The same page and the same backend as the desktop app, with no fork and no rule of the domain in the page.
- Every window of the desktop app at a place of the IDE, the review tabs kept over a restart of the IDE.
- The desktop app and the IDE open at once, on the same data.

## Non-goals

- One process for all the projects of a user (P2): each project of the IDE runs its own `brd mcp`.
- Tools for an agent: an agent keeps the `brd` command line.
- A project file of the app, or a project that is not on this machine.
- Remote development: in a split IDE the frame runs in the client and `air-ide` on the backend, so Air serves no app
  there.

## Concepts

- The contract is Air's: `plugins/air/native/air-apps/protocol/` of idea-1. `docs/design/ide-embedding/protocol/` is its
  copy, with `corpus/branch-deck-app.ndjson`, the app link as Branch Deck writes it.
- The IDE hosts an app only with the registry key `air.apps.enabled` of Air, which is off by default, and it reads the
  executable of the command of an app only after the user answered the consent of that command.
- An instance is one open view of the IDE, such as `review:3`. It is the window of the desktop app: the window of a
  command, of an event to one window and of `forget_window`.
- The host page of a view comes from `air-ide` on `http://127.0.0.1:<port>`, a port that changes with each run of the
  IDE. The frame of Branch Deck stays on `https://branch-deck.air-genui-app.local`, so its `localStorage` survives a
  restart of the IDE.
- The page of Branch Deck speaks only the frame link, MCP Apps with `x-air-app`, and `brd mcp` only the app link. The
  page link between the host page and `air-ide` is Air's. A mirror is one channel of it, `x-air-ide:/appView/<n>`,
  which every view of the mirror subscribes to, so one change is one action with one `serverSeq` for all of them; a
  `viewOpen` with the params of the view that its connection holds answers that subscription, `held` and without a
  snapshot. The events, the call events and the progress of `brd mcp` are notices of the channel of its process,
  `x-air-ide:/appLink/<n>`. The echo of an intent holds its result in `_meta["x-air-ide"].result`, and a cancel is a
  notification of the root channel whose call gets exactly one answer. A change of the page link changes neither the
  page nor brd.

## Requirements

### Requirement: `brd mcp`: branch-deck cli mcp serves MCP with x-air-app over stdio

`branch-deck cli mcp` (`brd mcp`) MUST serve MCP over its stdin and stdout, and answer `initialize` with
`capabilities.experimental["x-air-app"]` of version `1.0.0`. Its stdout carries only the protocol.

#### Scenario: initialize
- **WHEN** air-ide sends `initialize` with `x-air-app` 1.0.0
- **THEN** brd answers the server `branch-deck` with the tools, the resources and `x-air-app` 1.0.0
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

#### Scenario: The app link corpus
- **WHEN** a test runs `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson` against `brd mcp` on the fixtures
- **THEN** each line of the app matches the corpus
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: `brd mcp`: The data of brd mcp is the data of the desktop app

`brd mcp` MUST keep its settings, its caches and its journals in the directories of `org.develar.branch-deck`, the
same as the desktop app and `brd`.

#### Scenario: A setting of the desktop app
- **WHEN** the desktop app saved a setting, and `brd mcp` starts
- **THEN** `bd_bootstrap` gives that setting
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: `brd mcp`: The page of each view is a resource built into the binary

`resources/read` of `ui://branch-deck/<view>` MUST answer the page of the view as `text/html;profile=mcp-app`. Each
file of the Air build is `ui://branch-deck/<path>`, and another URI gets `-32602 badParams`.

#### Scenario: The main page and a missing file
- **WHEN** air-ide reads `ui://branch-deck/main` and then `ui://branch-deck/nothing.js`
- **THEN** it gets the HTML of the page, and then `-32602 {reason: "badParams"}`
  [@test] crates/mcp-host/src/resources_test.rs (`a_view_is_the_page_and_a_missing_file_is_none`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: `brd mcp`: The end of stdin ends brd mcp

At the end of its stdin, `brd mcp` MUST close its sessions and exit with 0.

#### Scenario: The IDE closes the project
- **WHEN** the stdin of `brd mcp` ends
- **THEN** the process exits with 0
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: Tools: Every tool of brd mcp is only for the app

`tools/list` MUST list `bd_invoke`, `bd_bootstrap`, the tool of each action of the app file, `sync` and
`open_command_palette`, and `reveal_branch`, the action of a notification of brd. Each has
`_meta.ui.visibility: ["app"]`, so no agent sees it.

#### Scenario: The list
- **WHEN** air-ide sends `tools/list`
- **THEN** it gets the five tools, each only for the app
  [@test] crates/mcp-host/src/server_test.rs (`every_tool_is_only_for_the_app_and_each_action_of_the_app_file_has_its_tool`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: bd_invoke runs one command of app-api by its name

`bd_invoke {command, args}` MUST run the command of `for_each_command!` named `command` with `args`. The output is
`structuredContent.value`, and an `ApiError` is `isError` with `structuredContent.error`.

#### Scenario: A JSON command
- **WHEN** the page calls `bd_invoke` with `get_settings_changes`
- **THEN** the result holds the output of the command in `structuredContent.value`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

#### Scenario: An unknown command
- **WHEN** the page calls `bd_invoke` with a name that no command has
- **THEN** the result is `isError` with an `ApiError`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: A raw command answers its bytes in base64

`bd_invoke` of a raw command, such as `get_blob_syntax`, MUST answer its bytes in base64 in
`structuredContent.bytes`.

#### Scenario: The syntax of a blob
- **WHEN** the page calls `bd_invoke` with `get_blob_syntax`
- **THEN** `structuredContent.bytes` decodes to the bytes that the command gives in the desktop app

### Requirement: Tools: The channel of a command is the call events of its call

Each event of the channel of a command MUST be `x-air-app/callEvent {callId, event}`, with the id of its `tools/call`.
Every call event comes before the answer of its call.

#### Scenario: The diffs of a commit
- **WHEN** the page calls `bd_invoke` with `get_file_diffs`
- **THEN** each file diff is one call event of that call, and the answer comes after the last one
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The window of a call is the instance of its view

A command MUST run for the window of `_meta["x-air-app"].instance`. A command for its window, such as
`sub_window_loaded`, takes the instance as the window.

#### Scenario: Two editor tabs
- **WHEN** two review tabs call the same command for their window
- **THEN** each gets the answer for its own instance
  [@test] crates/test-server/src/mcp_corpus_test.rs (`two_review_tabs_each_get_the_answer_of_their_own_instance`)

### Requirement: Tools: A call of an agent is refused

A `tools/call` with the origin `agent` MUST get `-32009 {reason: "agentOrigin"}`, whatever its tool. The main path of
an agent stays the `brd` command line.

#### Scenario: The origin agent
- **WHEN** air-ide sends `bd_invoke` with the origin `agent`
- **THEN** brd answers `-32009 {reason: "agentOrigin"}` and runs no command
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: A cancel stops a read and lets a write end

On `notifications/cancelled`, `brd mcp` MUST stop a read command and give no answer. A command that writes runs to its
end, and its events go to the view and the journal.

#### Scenario: A cancelled diff
- **WHEN** air-ide cancels a running `get_file_diffs`
- **THEN** the command stops, and no answer and no call event of it follow
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_cancelled_read_gives_no_answer_and_no_call_event`)

### Requirement: Tools: bd_bootstrap gives what the page needs before its main

`bd_bootstrap {view}` MUST answer the settings, the warm frame, the init data of the view and the view of the title
bar. The page takes them where the desktop app takes its init script.

#### Scenario: The main view
- **WHEN** the page of `main` calls `bd_bootstrap`
- **THEN** the result has `settings`, `warmFrame`, `initData` and `titleBar`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The action Sync Virtual Branches is the tool sync

A `tools/call` of `sync` with the origin `action` or `click` MUST sync the repository of the main view, as Sync does in
the desktop app. A call of an agent is refused, as for every tool.

#### Scenario: The shortcut of Sync
- **WHEN** the user presses Sync Virtual Branches in the IDE, or its button in the title of the tool window
- **THEN** brd syncs the repository that the main view shows
  [@test] tests/e2e/air/air-page.spec.ts (`the actions in the title of the tool window: Branch Deck Command Palette opens the palette of the main view, Sync Virtual Branches syncs its repository`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The action Command Palette opens the palette of the main view

A `tools/call` of `open_command_palette` MUST send the main view the event `open_command_palette`, as the menu of the
desktop app sends it to the main window.

#### Scenario: The button in the title
- **WHEN** the user clicks Branch Deck Command Palette in the title of the tool window
- **THEN** brd sends `x-air-app/event` of `open_command_palette` with the instance of the main view, and the palette
  opens
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`the actions in the title of the tool window: Branch Deck Command Palette opens the palette of the main view, Sync Virtual Branches syncs its repository`)

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar `native`, without updates and without
a folder dialog (`folderDialog: false`).

#### Scenario: The settings of the desktop app
- **WHEN** the settings window bootstraps after the desktop app saved a setting
- **THEN** `settings.values` holds the setting
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: View: subscribe_view is the mirror of the repository

`x-air-app/subscribe {instance, params}` MUST answer the mirror `repositoryPath` and the view of the session with its
`version`, which the page compares with the versions the commands answer. The views of one repository share that
mirror.

#### Scenario: The main view of a repository
- **WHEN** air-ide subscribes `main:1` to a repository
- **THEN** the answer names the path of the repository and holds the view
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/mcp-host/src/mirror_test.rs (`the_state_is_the_view_with_its_version`)

### Requirement: View: Each patch of the session is one change of the mirror

Each `ViewPatch` MUST become one `x-air-app/viewChanged`. A keyed part sets each changed key and removes each removed
key, every other part is replaced whole, and the part `version` is set last to the version after the patch.

#### Scenario: A renamed branch
- **WHEN** a sync renames the branch `a` to `b`
- **THEN** one change sets `branches/b`, removes `branches/a`, replaces `order` and sets `version`
  [@test] crates/mcp-host/src/mirror_test.rs (`a_renamed_branch_sets_the_new_key_removes_the_old_and_replaces_the_order`)

#### Scenario: The news
- **WHEN** a patch carries news
- **THEN** the change replaces the part `news` with the last news of the session, so a resync tells none twice

### Requirement: View: A patch and the events of a call come before its answer

`brd mcp` MUST write the changes and the events that a command caused before the answer of the command.

#### Scenario: A rewrite
- **WHEN** the page runs a rewrite with `execute`
- **THEN** its `viewChanged` and its call events come before the answer of `bd_invoke`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_rewrite_answers_after_its_changes_and_its_call_events`)

### Requirement: View: The mirror takes no intent

`x-air-app/dispatchAction` MUST get `-32602 {reason: "unknownAction"}`. The page sends its intents with `bd_invoke`,
as in the desktop app.

#### Scenario: An intent
- **WHEN** air-ide dispatches an action to the mirror of a repository
- **THEN** brd answers `unknownAction`, and the view does not change
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: View: The end of a session ends its mirror

When the session of a repository closes, `brd mcp` MUST send `x-air-app/viewEnded {mirror, reason}` with the reason
`sessionClosed`. brd cannot tell a repository that is gone from a session that closed, so it has no other reason.

#### Scenario: A removed repository
- **WHEN** the repository of a mirror is gone, and its session closes
- **THEN** brd sends `viewEnded` for it with the reason `sessionClosed`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_end_of_a_session_ends_its_mirror_with_the_reason_session_closed`)

### Requirement: View: The demand of a view is the demand of its mirror

`set_demand` of a view MUST name the subscription of the mirror of its instance, which the page does not know.

#### Scenario: A review that demands the details of a robot
- **WHEN** a review tab sets its demand
- **THEN** the session of its repository reads what the demand asks

### Requirement: View: A change of the prefix moves the mirror to the session of the new prefix

A subscription again to a mirror brd holds, by `bd_invoke` of `subscribe_view` or by another view, MUST take the session
of the prefix the git config names now. A mirror of another prefix moves to it in one change that replaces every part,
its `version` of another epoch, before the answer, and no patch of the old session reaches it after.

#### Scenario: A prefix set in a terminal
- **WHEN** the prefix of the git config changes, and the main view subscribes again after its sync answered
  `prefix_changed`
- **THEN** one change of every part moves the mirror to the session of the new prefix before the answer, and the main
  view shows its branches without an end of the mirror
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_the_prefix_moves_the_mirror_to_the_session_of_the_new_prefix`)
  [@test] crates/mcp-host/src/mirror_test.rs (`every_part_replaces_the_state_with_the_view_and_its_version_last`)
  [@test] tests/e2e/air/air-page.spec.ts (`a prefix set in a terminal that a sync finds moves the main view to the session of the new prefix: the frame keeps its one view, which brd moves before it answers the subscription again`)

### Requirement: Events: An event of the app is an event of x-air-app

`AppEvents::emit` MUST send `x-air-app/event {event, payload}`, and `emit_to` sends it with the instance of its window.

#### Scenario: A change of the settings
- **WHEN** a setting changes
- **THEN** brd sends one `x-air-app/event` without an instance
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_a_setting_is_an_event_and_the_badge_marks_the_tool_window`)

### Requirement: Events: An emit of the page is an event of the views

`bd_invoke` of `plugin:event|emit {event, payload, target}` MUST send `x-air-app/event` of the event to every view, or
with the instance of the window `target` names.

#### Scenario: An event for the main window
- **WHEN** a review tab emits an event to the main window
- **THEN** brd sends `x-air-app/event` with the instance of the main view
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_emit_of_the_page_is_an_event_of_the_views`)

### Requirement: Windows: A window of the desktop app is a view of the IDE

Each open of a sub-window MUST be `x-air-app/openView` of its view. A review is an editor tab keyed by its target,
the settings a frame, and the conflicting files and the missing commits a dialog.

#### Scenario: A review
- **WHEN** the user opens the review of a branch twice
- **THEN** brd asks `openView` of `review` with the key of the branch, and the IDE shows one tab
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_open_of_a_review_is_an_open_view_keyed_by_its_target`)

#### Scenario: A new title
- **WHEN** what a review shows changes its title
- **THEN** brd asks `setViewTitle` of its instance
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_open_of_a_review_is_an_open_view_keyed_by_its_target`)

### Requirement: Windows: The life of a place reaches the window registry

`x-air-app/viewClosed` MUST forget the page and the window of the instance. `x-air-app/viewShown` is the show of the
window, and the end of a WebSocket forgets nothing.

#### Scenario: A closed review tab
- **WHEN** air-ide sends `viewClosed` of a review tab
- **THEN** brd forgets its page and its window, as when a review window closes

### Requirement: Windows: A notification of the OS is a notification of the IDE

A notification of the desktop app MUST be `x-air-app/notify`. The failure of a robot, a news of the session, brd MUST
tell itself while the main view is not shown (its last `viewHidden` after its `viewShown`) and "Safe Push failures" of
Settings is on, with the action Show in Branch Deck: `run` of the tool `reveal_branch {repositoryPath, branchName}` of
the branch the robot tested (the root of a whole stack). While the main view shows, its toast tells it.

#### Scenario: A failed Safe Push
- **WHEN** a Safe Push fails while the IDE shows another tab
- **THEN** the IDE shows a notification whose action opens the branch
  [@test] tests/e2e/air/air-page.spec.ts (`a Safe Push that fails while the IDE shows another tab is a notification of the IDE, whose action shows the branch in the tool window`)
  [@test] crates/mcp-host/src/server_test.rs (`a_failed_robot_of_a_patch_is_a_notification_of_the_ide_after_the_change_of_the_mirror`)
  [@test] crates/mcp-host/src/notices_test.rs (`a_failed_robot_is_a_notification_whose_action_reveals_its_branch`)

### Requirement: Windows: The notification of the page names its news

The page MUST name the news its own notification tells (`extra: {repositoryPath, news}`). brd leaves it out once it
told that news, and else gives it the kind and the action of the news, so the IDE shows one notification of a failure,
and it opens the branch.

#### Scenario: The notification of the page
- **WHEN** the page notifies the failure as its frame loses the focus
- **THEN** the IDE shows no second notification of a failure brd told, and the notification of one brd did not tell
  opens the branch too; with "Safe Push failures" off brd tells none
  [@test] crates/mcp-host/src/notices_test.rs (`brd_tells_a_failure_while_the_main_view_is_hidden_and_the_notification_of_the_page_takes_its_action_while_shown`)
  [@test] src/platform/air/shim.test.ts (`The plugins: a link, the clipboard and the log go to the host, the badge, the progress, a notification and the close of the window are bd_invoke of their plugin calls, the size does nothing`)

### Requirement: Windows: Show in Branch Deck reveals the branch in the main view

The tool `reveal_branch` MUST ask `openView` of `main` and tell the instance the IDE answers the event `reveal-branch`
with the repository and the branch, which the main view reveals in the repository it shows.

#### Scenario: Show in Branch Deck
- **WHEN** the IDE runs the action of the notification
- **THEN** brd asks `openView` of `main`, and tells the instance the IDE answered `reveal-branch`: the main view reveals
  the branch as Go to of the command palette does, in the repository it shows
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`a Safe Push that fails while the IDE shows another tab is a notification of the IDE, whose action shows the branch in the tool window`)

### Requirement: Windows: The badge and the progress of the Dock mark the tool window

The Dock badge and the Dock progress MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of the view `main`.

#### Scenario: A robot that needs the user
- **WHEN** the attention count becomes 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Windows: A plugin call of the page is the desktop of the IDE

`bd_invoke` of `plugin:window|set_badge_count`, `plugin:window|set_progress_bar` and `plugin:notification|notify`
MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of `main` and `x-air-app/notify`. The page calls no tool of
the IDE for them: brd is their one path.

#### Scenario: The badge
- **WHEN** the page sets the badge of the Dock to 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_a_setting_is_an_event_and_the_badge_marks_the_tool_window`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Windows: A second open of a shown view hands it its data

When the IDE answers an open with an instance the window showed already, brd MUST send it the data of the open
(`init-data-updated`) and its title (`setViewTitle`).

#### Scenario: The review of the same branch
- **WHEN** a review tab shows a branch and the user opens its review again
- **THEN** the tab gets `init-data-updated` with the data and the title of the new open
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_open_of_a_review_is_an_open_view_keyed_by_its_target`)

### Requirement: Windows: The close of a window of the page closes its place

`bd_invoke` of `plugin:window|close` MUST send `x-air-app/closeView {instance}` with the instance of the window of the
call. So Escape closes a review tab and a dialog, as their hint says.

#### Scenario: Escape in a review tab
- **WHEN** the user presses Escape in a review tab
- **THEN** brd asks `closeView` of its instance, and the IDE closes the tab
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`Escape in a review tab closes its place: brd asks the IDE to close the view of its instance`)

### Requirement: Repositories: The roots of the project are the repositories of brd mcp

`brd mcp` MUST read the repositories from `roots/list`, at its start and on `notifications/roots/list_changed`. The
views see the Git roots as their recent projects (`recentProjects`), the root the main view shows first, and
`bd_bootstrap` names that root as `initData.repositoryPath`: the page selects it and its selector lists the roots.

#### Scenario: A project with two roots
- **WHEN** the project has two Git roots
- **THEN** the main view shows the first one until the user picks the other, and its selector lists both
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] tests/e2e/air/air-page.spec.ts (`the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project`)
  [@test] crates/mcp-host/src/projects_test.rs (`the_main_view_shows_the_root_picked_last_else_the_one_it_shows_else_the_first`)

### Requirement: Repositories: brd remembers the root picked last for the roots of a project

A pick of the page (its write of `recentProjects` with another root first) MUST be remembered by the set of the roots,
in `ideRootPicks` of `repository-state.json`; the next start of a project with the same roots, in any order, shows it.
Rust alone reads and writes it, never the page.

#### Scenario: The next start of the project
- **GIVEN** the user picked the second root of a project
- **WHEN** the project opens again
- **THEN** the main view shows the second root
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] tests/e2e/air/air-page.spec.ts (`the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project`)
  [@test] crates/app-settings/src/ide_roots_test.rs (`the_root_picked_last_is_kept_by_the_set_of_the_roots_in_the_repository_state`)

### Requirement: Repositories: A pick in the IDE keeps the recent projects of the desktop app

What the page writes of a root (a collapsed card, its issue links) MUST go into the recent projects of the settings in
the place of that root, a root they lack at their end; their order, which selects the repository of the desktop app,
stays.

#### Scenario: A pick in the IDE
- **WHEN** the user picks another root in the IDE while the desktop app shows a repository
- **THEN** the desktop app keeps the repository it shows
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] crates/mcp-host/src/projects_test.rs (`the_views_see_the_roots_as_the_recent_projects_and_a_write_keeps_the_order_of_the_desktop_app`)

### Requirement: Repositories: A change of the roots tells the main view

When `roots/list` answers other Git roots than before, brd MUST send the main view the event `roots-changed` with them,
and the views that took the settings the recent projects of the roots, as a change of `recentProjects`.

#### Scenario: A root added to the project
- **WHEN** the project gets a second Git root
- **THEN** the main view hears `roots-changed` with both roots, and its selector lists both
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_root_added_to_the_project_tells_the_main_view_and_the_recent_projects_of_the_views`)

### Requirement: Hosts: The hosts without a place in the IDE do nothing

In `brd mcp`, the menu, the updater, the restart and the install of the agent CLI MUST do nothing. A folder dialog is
not offered, because the roots name the repositories.

#### Scenario: A check for updates
- **WHEN** the page asks for an update in the IDE
- **THEN** brd does nothing, because the app updates with the desktop app
  [@test] crates/mcp-host/src/hosts_test.rs (`the_hosts_without_a_place_in_the_ide_do_nothing`)

### Requirement: Hosts: The page in the IDE offers no folder dialog

In `brd mcp`, the dialog host MUST answer that it has no folder dialog (`has_folder_dialog`), which `bd_bootstrap` gives
the page as `titleBar.folderDialog: false`. The page then offers no Open Repository… beside the selector of the
repositories or in the command palette, and no Add Companion… on Settings → Landing; it only words the fact.

#### Scenario: The main view in the IDE
- **WHEN** the IDE shows the main view
- **THEN** its selector lists the roots without the folder button beside it, and the command palette offers no Open
  Repository…
  [@test] crates/mcp-host/src/hosts_test.rs (`the_hosts_without_a_place_in_the_ide_do_nothing`)
  [@test] tests/e2e/air/air-page.spec.ts (`the main view in the IDE offers no Open Repository, as the IDE has no folder dialog: the roots of the project are the repositories of its selector`)
  [@test] src/lib/command-palette.test.ts (`offers no Open Repository… where the host has no folder dialog`)

### Requirement: Hosts: The page in the IDE offers no install

In `brd mcp`, `agent_cli_status` MUST answer `installable: false`: the desktop app installs `brd` and the app file of
the IDE. The page then shows no row `brd` and no row IntelliJ IDEA on Settings → Tools, and no install of the Agent CLI
on Agents or in the hint of the agents; it only words the status.

#### Scenario: Settings → Tools in the IDE
- **WHEN** the user opens Settings → Tools in the IDE
- **THEN** it shows the command-line tools without `brd` and IntelliJ IDEA
  [@test] crates/mcp-host/src/hosts_test.rs (`the_hosts_without_a_place_in_the_ide_do_nothing`)
  [@test] tests/e2e/air/air-page.spec.ts (`Settings → Tools in the IDE shows the command-line tools but no install of brd or of IntelliJ IDEA, which the desktop app installs`)

### Requirement: Page: The page in the IDE is the page of the desktop app

The Air build (`vite build --mode air`, entry `src/air-main.ts`, into `dist-air/`) MUST run the same page as the desktop
app. It awaits `bd_bootstrap`, installs the globals of the shim, and then runs the usual main on the page of its view
(`main` is `/`, another view `/<view>`). Its chunks and its worker stay files of their own, and `index.html` is the page
of every view.

#### Scenario: The main window
- **WHEN** the IDE opens the tool window of Branch Deck on a repository
- **THEN** the page shows the branches of the repository as the desktop app does
  [@test] tests/e2e/air/air-page.spec.ts (`the tool window shows the branches of the repository after a sync of the page, each patch of the mirror a change of its rows`)

### Requirement: Page: The shim maps the Tauri calls onto x-air-app

The shim of `src/platform/air/` MUST map `invoke` to `bd_invoke`, a `Channel` to the call events and `subscribe_view`
to the mirror. `listen` and `emit` map to the events of x-air-app, and each plugin of Tauri to what the IDE offers in
its place. A failure of the frame link is the `ApiError` that brd put in the data of the error, else `internal`. The
page mock of the E2E tests installs the same internals of Tauri (`src/platform/tauri-page.ts`).

#### Scenario: A command with a channel
- **WHEN** the page invokes `get_file_diffs` with a `Channel`
- **THEN** the channel gets each call event of the call, in order, before the promise resolves
  [@test] src/platform/air/shim.test.ts (`A command with a channel: the channel gets each call event of the call, in order, before the promise resolves`)

#### Scenario: The answers of a command
- **WHEN** a command answers a value, raw bytes, an `ApiError`, or the link is lost
- **THEN** the page gets the value, an `ArrayBuffer`, the `ApiError`, or an `internal` error with the reason
  [@test] src/platform/air/shim.test.ts (`A JSON command answers its value, a raw one its bytes, a failed one its ApiError`)

#### Scenario: A subscription that brd refused
- **WHEN** brd refuses the subscription of the main view with `no_branch_prefix`, its `ApiError` in `error.data.error`
  of the answer that Air passes on
- **THEN** the page gets that `ApiError` and shows the welcome card with the prefix that git suggests, and Save shows
  the repository
  [@test] src/platform/air/shim.test.ts (`A subscription that brd refused fails with the ApiError brd answered, which Air passes on in the data of its error`)
  [@test] tests/e2e/air/air-page.spec.ts (`a repository without a branch prefix shows the welcome card in the main view, as brd refused its subscription with no_branch_prefix, and Save shows the repository`)

#### Scenario: A resync
- **WHEN** the subscription of the page gets `x-air-app/resync`
- **THEN** the page takes the state as a new snapshot of the view, without a reload
  [@test] src/platform/air/shim.test.ts (`subscribe_view is the mirror of the repository: its state the view, each change a patch, a resync a snapshot without a reload`)

#### Scenario: The end of a mirror
- **WHEN** the subscription of the page gets `x-air-app/subscriptionEnded`, with the reason `sessionClosed` of brd,
  `serverExited` of an exit of brd, `closed` of Air, or a reason the page does not know, while brd may still hold the
  session of the repository
- **THEN** the snapshot of the repository is none, so the page subscribes again and takes the changes of the new
  subscription, and a late change of the one that ended reaches nothing
  [@test] src/platform/air/shim.test.ts (`The end of a mirror, for each reason alike (sessionClosed, serverExited, closed, a reason the page does not know): the snapshot of its repository is none, so the page subscribes again, and a late change of the mirror reaches nothing`)
  [@test] tests/e2e/air/air-page.spec.ts (`a mirror that Air ends while brd keeps the session of its repository is subscribed to again, so the main view shows what a sync after the end changes`)

#### Scenario: An event
- **WHEN** `brd mcp` tells an event, and the page emits one to the main window
- **THEN** the listeners of its name hear it, and the emit is `bd_invoke` of `plugin:event|emit` with its target
  [@test] src/platform/air/shim.test.ts (`listen hears the events of x-air-app, and emit goes to the views of the app through brd mcp`)

#### Scenario: The plugins
- **WHEN** the page opens a link, copies, sets the badge or the progress of the Dock, notifies, logs, or sets the size
  of its window
- **THEN** the host gets `ui/open-link`, the tool `air_copy` and `notifications/message`; the badge, the progress and
  the notification are `bd_invoke` of their plugin calls; the size does nothing
  [@test] src/platform/air/shim.test.ts (`The plugins: a link, the clipboard and the log go to the host, the badge, the progress, a notification and the close of the window are bd_invoke of their plugin calls, the size does nothing`)

### Requirement: Page: The subscriptions of the page share the one view of its frame

Air holds one view for each frame, so the shim MUST keep at most one `x-air-app/subscribe` of a frame. A subscription of
the repository the frame holds joins its view and asks brd to subscribe again (`bd_invoke` of `subscribe_view`); one of
another repository subscribes once the view before it answered and was unsubscribed or ended.

#### Scenario: A switch of the repository before the first answer
- **WHEN** the main view picks another root before its first subscription answered
- **THEN** the picked root subscribes once that subscription answered and the page unsubscribed it, and Air refuses
  nothing
  [@test] src/platform/air/shim.test.ts (`A switch of the repository before the first answer: the next repository subscribes once the view of the one before answered and was unsubscribed, so the frame holds one view`)
  [@test] tests/e2e/air/air-page.spec.ts (`a pick of another root before the first subscription of the main view answered subscribes to it once that subscription answered and ended: the frame holds one view`)

#### Scenario: A subscription again
- **WHEN** the main view subscribes again to its repository after `prefix_changed`, holding the subscription it replaces
- **THEN** both share the view of the frame, the new one answers after brd moved the mirror, and the one it replaces
  leaves without an unsubscribe of the frame
  [@test] src/platform/air/shim.test.ts (`A subscription again of the repository the frame holds shares its view: brd subscribes again, moving the mirror to the session of another prefix before it answers, and the subscription it replaces leaves without an unsubscribe of the frame`)

### Requirement: Page: The theme of the IDE maps onto the tokens of the page

The page MUST map the theme and the styles of `hostContext` onto its semantic tokens, and follow each
`host-context-changed`. The canvas is the first background of the IDE, the background of the place, so the page is one
surface with its tool window; the cards, the popups and the fields are the second, the raised surface, so the canvas is
darker than the cards in both themes.

#### Scenario: Light to Dark
- **WHEN** the IDE switches from Light to Dark
- **THEN** the page shows the dark tokens without a reload
  [@test] tests/e2e/air/air-page.spec.ts (`the page shows the theme of the IDE and follows its switch from Light to Dark without a reload`)

### Requirement: Page: The mirror is the view, and each change of it a patch of the session

The state of the mirror MUST be the `RepositoryView` of the session with its part `version`. The page builds the
`ViewPatch` of each `x-air-app/viewChanged` from its ops and the state before them, so its view after the patch is the
view of the session.

#### Scenario: A renamed branch
- **WHEN** a change sets `branches/b`, removes `branches/a` and replaces `order`
- **THEN** the patch of the page has the branch `b`, removes `a`, and its view is the view of the session
  [@test] src/platform/air/view-mirror.test.ts (`A renamed branch: one change sets branches/b, removes branches/a and replaces order`)

#### Scenario: The news and the nested parts
- **WHEN** a change replaces the news, the sections, the Patronus view or the facts
- **THEN** the view of the page after the patch is the view of the session
  [@test] src/platform/air/view-mirror.test.ts (`The news: the part news of the kept list, of which the page tells those it has not told`, `A change of the parts that hold keyed entries of their own: the sections, the Patronus view and the facts`)

### Requirement: Contract: The copy of the contract is valid against its schema

Each line of `docs/design/ide-embedding/protocol/corpus/*.ndjson` MUST be valid against `x-air-app.schema.json`, and
each result and action that the line schema takes as any value against the def of its method, as the README of the copy
names them. The example app file and each app file the corpus writes are valid against `air-app.schema.json`; a file whose entry
names a problem of the file (`invalidFile`, `unsupportedFormat`) is not.

#### Scenario: The corpus
- **WHEN** a test reads each corpus file of the copy
- **THEN** each line is valid, and the example app file too
  [@test] src/platform/air/contract.test.ts (`each line of the corpus is valid, each result and action by the def of its method, and each app file the corpus writes`, `the example app file is valid against air-app.schema.json`)

### Requirement: Install: Settings → Tools installs Branch Deck in IntelliJ IDEA

Settings → Tools MUST show IntelliJ IDEA after `brd`. Install MUST write `~/.air/apps/branch-deck.yaml` and its icons
in `~/.air/apps/icons/`, replacing what is there; Remove MUST delete the file and the icons of Branch Deck and leave
those of other apps. The row MUST name the file while it exists, and say so when another Branch Deck wrote it, with
Install to write it for this one.

#### Scenario: Install and Remove
- **WHEN** the user clicks Install on the row IntelliJ IDEA of Settings → Tools
- **THEN** the backend writes the app file, and the row names it with Remove
- **AND** Remove deletes it, and the row offers Install again
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should install Branch Deck in IntelliJ IDEA beside brd, which then names its app file, and remove it`)
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_ide_app_for_this_binary_and_removes_it`)

#### Scenario: Another Branch Deck wrote the file
- **GIVEN** the app file starts another binary of Branch Deck
- **WHEN** the user opens Settings → Tools
- **THEN** the row says that another Branch Deck wrote it, with Install and Remove
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_ide_app_for_this_binary_and_removes_it`)

### Requirement: Install: The app file is the example of the contract with this binary

The app file MUST be the example app file of the contract with the command of this binary: the app binary with
`cli mcp`; `brd`, on Windows `brd.exe` beside the app, with `mcp`. A binary that macOS runs translocated is refused, as
the install of the agent CLI refuses it.

#### Scenario: The app file
- **WHEN** the install writes the app file
- **THEN** it reads as `docs/design/ide-embedding/protocol/examples/branch-deck.yaml` with the command of this binary
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_app_file_is_the_example_of_the_contract_with_the_command_of_this_binary`)

#### Scenario: The command of each binary
- **WHEN** the binary is the app binary, or `brd`
- **THEN** the arguments are `cli mcp` (`mcp` on Windows), or `mcp`
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_runs_brd_mcp_with_the_app_binary_and_mcp_with_brd`)

## User Experience

- The tool window **Branch Deck** at the bottom, with the tab **Activity**; the action **Sync Virtual Branches** in the
  VCS menu (⌥⌘Y, Ctrl+Alt+Y).
- The title of the tool window: **Sync Virtual Branches**, **Branch Deck Command Palette** and **Branch Deck Settings**,
  the actions of the app file, in place of the menu of the desktop app.
- Before the first start, the empty tool window shows the consent banner of the IDE: **Allow Once**, **Always Allow**,
  **Deny**.

## Open Questions / Risks

- The chain JCEF → WebSocket → `air-ide` → stdio on a big diff: stage 0 of the plan measures it.
- The page entry of Air is measured on macOS only: a loopback document that cannot open its socket on Windows, on
  Linux or behind a proxy of the IDE would change the transport.
- The desktop app and `brd mcp` both poll GitHub and Patronus in P1.
- Air lets one view wait for at most 24 calls and the views of one process for 36 (`-32011 tooManyCalls` over that).
  The views of the E2E suite of the Air build wait for at most 9 at once, and its fake IDE refuses a call over either
  share as Air does.

## References

- [IDE Embedding](../../../../docs/design/ide-embedding.md)
- [Settings](../settings/spec.md)
