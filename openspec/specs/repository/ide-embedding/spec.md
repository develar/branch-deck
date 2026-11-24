---
name: Branch Deck in the IDE
description: Requirements for running the page of Branch Deck as an app of IntelliJ IDEA over x-air-app, with the bridge branch-deck ide as its process, a client of the one backend of the user, and its windows as views of the IDE.
style: plain-1
targets:
  - crates/app-api/src/app/life.rs
  - src-tauri/src/life.rs
  - src-tauri/src/lib.rs
  - src-tauri/src/menu.rs
  - src-tauri/src/auto_update.rs
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
  - crates/domain-types/src/title_bar.rs
  - src/platform/air/title-bar.ts
  - crates/app-api/src/dispatch.rs
  - crates/ide-host/src/server.rs
  - crates/ide-host/src/mirror.rs
  - crates/ide-host/src/hosts.rs
  - crates/ide-host/src/resources.rs
  - crates/ide-host/src/notices.rs
  - crates/ide-host/src/process.rs
  - crates/ide-host/src/backend.rs
  - crates/ide-host/src/start.rs
  - crates/ide-host/src/fallback.rs
  - crates/ide-host/src/settings.rs
  - src-tauri/src/main.rs
  - crates/ide-host/src/projects.rs
  - crates/app-settings/src/ide_roots.rs
  - crates/test-server/src/mcp_socket.rs
  - crates/bd-cli/src/install.rs
  - src/components/settings/ToolsPage.tsx
  - src/components/settings/AgentsPage.tsx
  - src/components/repository/AgentsHint.tsx
  - src/hooks/useRevealBranchEvent.ts
  - src/components/repository/RepositorySelector.tsx
  - src/lib/page-start.ts
  - src/lib/dock.ts
---

# Branch Deck in the IDE

Status: Draft
Date: 2026-11-13

## Purpose

Branch Deck runs inside IntelliJ IDEA as an app of the IDE: the page of the desktop app in a frame of the IDE, the
process of the project the bridge `branch-deck ide`, a client of the one backend of the user, the desktop app, which it
starts without a window when it does not run. The main window is a tool window, a review an
editor tab, the settings a frame and the dialogs dialogs of the IDE. The IDE reads `~/.air/apps/branch-deck.yaml`, and
it speaks `x-air-app` of Air with the page and with the bridge ([IDE Embedding](../../../../docs/design/ide-embedding.md),
[Backend Protocol](../../../../docs/design/backend-protocol.md)).

## Goals

- The same page and the same backend as the desktop app, with no fork and no rule of the domain in the page.
- Every window of the desktop app at a place of the IDE, the review tabs kept over a restart of the IDE.
- The desktop app and the IDE open at once, on the same data.

## Non-goals

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
- The page of Branch Deck speaks only the frame link, MCP Apps with `x-air-app`, and the bridge only the app link. The
  page link between the host page and `air-ide` is Air's. A mirror is one channel of it, `x-air-ide:/appView/<n>`,
  which every view of the mirror subscribes to, so one change is one action with one `serverSeq` for all of them; a
  `viewOpen` with the params of the view that its connection holds answers that subscription, `held` and without a
  snapshot. The events, the call events and the progress of the bridge are notices of the channel of its process,
  `x-air-ide:/appLink/<n>`. The echo of an intent holds its result in `_meta["x-air-ide"].result`, and a cancel is a
  notification of the root channel whose call gets exactly one answer. A change of the page link changes neither the
  page nor the bridge.

## Requirements

### Requirement: Bridge: branch-deck ide serves MCP with x-air-app over stdio

`branch-deck ide` MUST serve MCP over its stdin and stdout, and answer `initialize` with
`capabilities.experimental["x-air-app"]` of version `1.0.0`, before its backend is ready too; `branch-deck cli mcp` and
`brd mcp` are its alias. Its stdout carries only the protocol.

#### Scenario: initialize
- **WHEN** air-ide sends `initialize` with `x-air-app` 1.0.0
- **THEN** the bridge answers the server `branch-deck` with the tools, the resources and `x-air-app` 1.0.0
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/bd-cli/tests/mcp.rs (`brd_mcp_answers_initialize_on_its_stdout_and_ends_with_its_stdin`)

#### Scenario: The app link corpus
- **WHEN** a test runs `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson` against the bridge on the fixtures
- **THEN** each line of the app matches the corpus, through the socket of a backend and in the fallback alike
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs_through_the_socket_of_the_backend`)

### Requirement: Bridge: The data of the bridge is the data of the desktop app

The bridge MUST serve the settings, the caches and the journals of the directories of `org.develar.branch-deck`, the
same as the desktop app and `brd`: those of the backend it joins, or, in its fallback, its own writer of them beside
the app's.

#### Scenario: A setting of the desktop app
- **WHEN** the desktop app saved a setting, and the bridge starts
- **THEN** `bd_bootstrap` gives that setting
  [@test] crates/ide-host/src/process_test.rs (`the_bridge_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: Bridge: The page of each view is a resource built into the binary

`resources/read` of `ui://branch-deck/<view>` MUST answer the page of the view as `text/html;profile=mcp-app`. Each
file of the Air build is `ui://branch-deck/<path>`, and another URI gets `-32602 badParams`.

#### Scenario: The main page and a missing file
- **WHEN** air-ide reads `ui://branch-deck/main` and then `ui://branch-deck/nothing.js`
- **THEN** it gets the HTML of the page, and then `-32602 {reason: "badParams"}`
  [@test] crates/ide-host/src/resources_test.rs (`a_view_is_the_page_and_a_missing_file_is_none`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Bridge: The end of stdin ends the bridge with 0, an end of its backend with 1

At the end of its stdin, the bridge MUST end its connection to the backend and exit with 0; when the backend ends the
connection first, the bridge MUST exit with 1, so the IDE starts it again.

#### Scenario: The IDE closes the project
- **WHEN** the stdin of the bridge ends
- **THEN** the process exits with 0
  [@test] crates/ide-host/src/process_test.rs (`the_bridge_serves_the_settings_of_the_app_until_its_input_ends`)
  [@test] crates/bd-cli/tests/mcp.rs (`brd_mcp_answers_initialize_on_its_stdout_and_ends_with_its_stdin`)
  [@test] crates/ide-host/src/fallback_test.rs (`an_end_of_the_backend_is_unexpected_and_an_end_of_the_input_is_asked_for`)

#### Scenario: The app exits
- **WHEN** the backend ends the connection of the bridge
- **THEN** the bridge exits with 1
  [@test] crates/ide-host/src/fallback_test.rs (`an_end_of_the_backend_is_unexpected_and_an_end_of_the_input_is_asked_for`)

### Requirement: Bridge: The bridge joins the backend of the desktop app

`branch-deck ide` MUST be a client of the one backend of the user: when a backend of its build answers on the socket of
the key of the app (the data directory and the app binary), the bridge MUST serve the IDE with it, its mirrors the
subscriptions of the repository channels under its window `mirror:<path>`, its places the windows of its connection.

#### Scenario: The app and the IDE on one repository
- **WHEN** the desktop app runs and the IDE opens a project of the same repository
- **THEN** the bridge serves the IDE with the backend of the app, through its socket, as the corpus of the app link runs
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs_through_the_socket_of_the_backend`)
  [@test] crates/ide-host/src/start_test.rs (`the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds`)

### Requirement: Bridge: The bridge starts the desktop app without a window

With no backend on the socket and `backend.lock` of the key free, the bridge MUST start the app with `--background`
under `backend.launch.lock`, so that no process of the bridge is its parent (macOS `/usr/bin/open -g -j`, a double
fork on Linux, `DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB` on Windows), with no variable of Air or the IDE, and wait
up to 15 s for its socket; a held lock MUST be waited for the same way.

#### Scenario: The IDE opens a project while the app does not run
- **WHEN** the bridge finds no backend and the lock free
- **THEN** it starts the app without a window and joins it once its socket answers
  [@test] crates/ide-host/src/start_test.rs (`the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds`)
  [@test] crates/ide-host/src/start_test.rs (`the_app_the_bridge_starts_gets_no_variable_of_air_or_of_the_ide`)

#### Scenario: Another build that exits
- **WHEN** the backend of the key is of another build, stale and without a window, and answers `otherBuild` with `exits`
- **THEN** the bridge waits for its lock to go free and starts the app of its own build
  [@test] crates/ide-host/src/start_test.rs (`the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds`)

### Requirement: Bridge: The fallback serves the IDE where the app cannot run

The bridge MUST serve the IDE with a backend in its own process over the in-process transport when the app cannot run:
Linux without `DISPLAY` and `WAYLAND_DISPLAY`, a backend of another build with a window open, Windows while the job of
the IDE refuses the breakaway, a start that fails or does not listen within 15 s, `BRANCH_DECK_IDE_BACKEND=in-process`.
Then it MUST end with 1 once a backend of its build answers on the socket.

#### Scenario: No display
- **WHEN** the bridge starts on Linux without a display
- **THEN** it serves the IDE alone at once, and the corpus of the app link runs in it
  [@test] crates/ide-host/src/start_test.rs (`the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

#### Scenario: The way back
- **GIVEN** the bridge serves alone while a backend of another build keeps a window open
- **WHEN** that backend goes and the app of the bridge's build starts
- **THEN** the bridge ends with 1, and the IDE starts it again, which joins that backend
  [@test] crates/ide-host/src/fallback_test.rs (`the_fallback_ends_once_a_backend_of_its_build_listens`)

### Requirement: Tools: Every tool of brd mcp is only for the app

`tools/list` MUST list `bd_invoke`, `bd_bootstrap`, the tool of each action of the app file, `sync` and
`open_command_palette`, and `reveal_branch`, the action of a notification of brd. Each has
`_meta.ui.visibility: ["app"]`, so no agent sees it.

#### Scenario: The list
- **WHEN** air-ide sends `tools/list`
- **THEN** it gets the five tools, each only for the app
  [@test] crates/ide-host/src/server_test.rs (`every_tool_is_only_for_the_app_and_each_action_of_the_app_file_has_its_tool`)
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

### Requirement: Tools: A tool brd has not is refused with the code of the contract

A `tools/call` of a name that is none of the tools of `tools/list` MUST get `-32601 {reason: "unknownTool"}` and run
nothing. Every refusal of brd MUST have the code that `ErrorObject` of the contract gives its reason, and a reason it
gives no code the code of JSON-RPC (`badParams` and `unknownAction` `-32602`, `parse` `-32700`).

#### Scenario: A tool brd has not
- **WHEN** air-ide calls a tool that brd has not
- **THEN** brd answers `-32601 {reason: "unknownTool"}` and runs no command
- **AND** an unknown method, a call of an agent, an intent of the mirror, a missing resource, a subscription without a
  repository and a line that is no JSON get the code of their reason by the contract
  [@test] crates/ide-host/src/server_test.rs (`every_refusal_of_the_bridge_has_the_code_the_contract_gives_its_reason`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: A cancel stops a read and lets a write end

On `notifications/cancelled`, `brd mcp` MUST stop a read command and give no answer. A command that writes runs to its
end, and its events go to the view and the journal.

#### Scenario: A cancelled diff
- **WHEN** air-ide cancels a running `get_file_diffs`
- **THEN** the command stops, and no answer and no call event of it follow
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_cancelled_read_gives_no_answer_and_no_call_event`)

### Requirement: Tools: bd_bootstrap gives what the page needs before its main

`bd_bootstrap {view}` MUST answer the settings, the warm frame, the init data of the view, the view of the title bar
and the home directory of `brd mcp` (`homeDir`). The page takes them where the desktop app takes its init script.

#### Scenario: The main view
- **WHEN** the page of `main` calls `bd_bootstrap`
- **THEN** the result has `settings`, `warmFrame`, `initData`, `titleBar` and `homeDir`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The action Sync Virtual Branches is the tool sync

A `tools/call` of `sync` with the origin `action` or `click` MUST sync the repository of the main view, as Sync does in
the desktop app. A call of an agent is refused, as for every tool.

#### Scenario: The shortcut of Sync
- **WHEN** the user presses Sync Virtual Branches in the IDE, or picks it in the VCS menu
- **THEN** brd syncs the repository that the main view shows
  [@test] tests/e2e/air/air-page.spec.ts (`the actions of the app file: Branch Deck Command Palette opens the palette of the main view, Sync Virtual Branches syncs its repository`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The action Command Palette opens the palette of the main view

A `tools/call` of `open_command_palette` MUST send the main view the event `open_command_palette`, as the menu of the
desktop app sends it to the main window.

#### Scenario: The action of the IDE
- **WHEN** the user runs the action Branch Deck Command Palette of the IDE
- **THEN** brd sends `x-air-app/event` of `open_command_palette` with the instance of the main view, and the palette
  opens
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`the actions of the app file: Branch Deck Command Palette opens the palette of the main view, Sync Virtual Branches syncs its repository`)

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar without updates, without a folder
dialog (`folderDialog: false`) and with a text of the progress in the title of the tool window
(`progressText: true`). It is `overlay` when the page passes an overlay title bar of its host context, else `native`.

#### Scenario: The settings of the desktop app
- **WHEN** the settings window bootstraps after the desktop app saved a setting
- **THEN** `settings.values` holds the setting
  [@test] crates/ide-host/src/process_test.rs (`the_bridge_serves_the_settings_of_the_app_until_its_input_ends`)

#### Scenario: A window whose row is in the title bar of the IDE
- **GIVEN** the host context of the Settings window names `{kind: "overlay", height: 40, insetStart: 80}`
- **WHEN** its page calls `bd_bootstrap` with that title bar
- **THEN** the title bar is `{kind: "overlay", doubleClick: "host", height: 40}` with the same flags
- **AND** a place whose host context names no title bar gets `native`
  [@test] crates/ide-host/src/process_test.rs (`the_bridge_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: View: subscribe_view is the mirror of the repository

`x-air-app/subscribe {instance, params}` MUST answer the mirror `repositoryPath` and the view of the session with its
`version`, which the page compares with the versions the commands answer. The views of one repository share that
mirror.

#### Scenario: The main view of a repository
- **WHEN** air-ide subscribes `main:1` to a repository
- **THEN** the answer names the path of the repository and holds the view
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/ide-host/src/mirror_test.rs (`the_state_is_the_view_with_its_version`)

### Requirement: View: Each patch of the session is one change of the mirror

Each `ViewPatch` MUST become one `x-air-app/viewChanged`. A keyed part sets each changed key and removes each removed
key, every other part is replaced whole, and the part `version` is set last to the version after the patch.

#### Scenario: A renamed branch
- **WHEN** a sync renames the branch `a` to `b`
- **THEN** one change sets `branches/b`, removes `branches/a`, replaces `order` and sets `version`
  [@test] crates/ide-host/src/mirror_test.rs (`a_renamed_branch_sets_the_new_key_removes_the_old_and_replaces_the_order`)

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

`set_demand` of a view MUST name the subscription of the mirror of its instance, which the page does not know. The
views of a mirror share its subscription, so brd MUST keep the demand of each view and send the session the union of
them (`Demand::union`, as the session joins the demands of its subscriptions); the view that leaves the mirror takes
only its own demand with it.

#### Scenario: A review that demands the details of a robot
- **WHEN** a review tab sets its demand
- **THEN** the session of its repository reads what the demand asks

#### Scenario: The main view and a review
- **WHEN** a review sets its demand and the main view then sends its demand of the branches
- **THEN** the session reads what both ask; once the review leaves, only what the main view asks
  [@test] crates/ide-host/src/server_test.rs (`the_session_of_a_mirror_gets_the_union_of_the_demands_of_its_views`)

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
  [@test] crates/ide-host/src/mirror_test.rs (`every_part_replaces_the_state_with_the_view_and_its_version_last`)
  [@test] tests/e2e/air/air-page.spec.ts (`a prefix set in a terminal that a sync finds moves the main view to the session of the new prefix: the frame keeps its one view, which brd moves before it answers the subscription again`)

### Requirement: View: A patch into another epoch moves the mirror to the session of now

A patch of the session of a mirror into another epoch (the session opened anew: a slot of a lease pool leased again)
applies to no view of the epoch before. The bridge MUST then read the view of the session anew (`get_snapshot` of the
window of the mirror) and take it in one change that replaces every part, the patches that came meanwhile after it; a
session that answers no view MUST end the mirror with `sessionClosed`.

#### Scenario: A slot leased again
- **GIVEN** the main view shows a slot of a lease pool
- **WHEN** the slot goes back and an agent leases it again, and the stamp of that lease opens the session anew
- **THEN** one change of every part moves the mirror to the view of the new lease, none of the rows of the lease before,
  the main view shows the new lease with no row of the lease before selected, and the mirror holds the view of the
  session, without an end of the mirror
  [@test] crates/ide-host/src/server_test.rs (`a_patch_into_another_epoch_reads_the_view_anew`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_slot_leased_again_moves_the_mirror_to_the_view_of_the_new_lease`)
  [@test] tests/e2e/air/air-page.spec.ts (`a slot of a lease pool the main view shows that an agent leased again moves the mirror to the view of the new lease: none of its rows is selected as the row of the same name of the lease before was, and the view subscribed once`)

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
- **WHEN** a Safe Push fails while the tool window is hidden
- **THEN** the IDE shows a notification whose action opens the branch
  [@test] tests/e2e/air/air-page.spec.ts (`a Safe Push that fails while the tool window is hidden is a notification of the IDE, whose action shows the branch in the tool window`)
  [@test] crates/ide-host/src/server_test.rs (`a_failed_robot_of_a_patch_is_a_notification_of_the_ide_after_the_change_of_the_mirror`)
  [@test] crates/ide-host/src/notices_test.rs (`a_failed_robot_is_a_notification_whose_action_reveals_its_branch`)

### Requirement: Windows: The notification of the page names its news

The page MUST name the news its own notification tells (`extra: {repositoryPath, news}`). The bridge leaves it out once
it told that news, and else gives it the kind and the action of the news; each tell of a news MUST claim it first
(`x-branch-deck/claimNews`), and only the first claim of the backend tells it, so one notification of a failure shows,
and it opens the branch.

#### Scenario: The notification of the page
- **WHEN** the page notifies the failure as its frame loses the focus
- **THEN** the IDE shows no second notification of a failure the bridge told, and the notification of one it did not
  tell opens the branch too; with "Safe Push failures" off the bridge tells none
  [@test] crates/ide-host/src/notices_test.rs (`the_bridge_tells_a_failure_while_the_main_view_is_hidden_and_the_notification_of_the_page_takes_its_action_while_shown`)
  [@test] src/platform/air/shim.test.ts (`The plugins: a link, the clipboard and the log go to the host, the badge, the progress, a notification and the close of the window are bd_invoke of their plugin calls, the size does nothing`)

#### Scenario: A news another client told
- **WHEN** another client of the backend (the desktop app, a second IDE) claimed a news first
- **THEN** the bridge tells it not
  [@test] crates/ide-host/src/server_test.rs (`a_news_another_client_claimed_first_is_no_notification_of_the_bridge`)

### Requirement: Windows: Show in Branch Deck reveals the branch in the main view

The tool `reveal_branch` MUST ask `openView` of `main` and tell the instance the IDE answers the event `reveal-branch`
with the repository and the branch, which the main view reveals in the repository it shows.

#### Scenario: Show in Branch Deck
- **WHEN** the IDE runs the action of the notification
- **THEN** brd asks `openView` of `main`, and tells the instance the IDE answered `reveal-branch`: the main view reveals
  the branch as Go to of the command palette does, in the repository it shows
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`a Safe Push that fails while the tool window is hidden is a notification of the IDE, whose action shows the branch in the tool window`)

### Requirement: Windows: The focus of the main window shows the tool window

`bd_invoke` of `plugin:window|set_focus` of the instance of the main view MUST ask `openView {view: "main"}`, as the
focus brings the main window of the desktop app to the front: the IDE shows the tool window. A focus of another view
does nothing: the IDE shows its place.

#### Scenario: A reveal that Activity asks
- **WHEN** the Activity window or a review asks the main view to reveal a branch
- **THEN** the main view takes the focus of its window, and brd asks `openView {view: "main"}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`a branch that the Activity window reveals shows in the main view: the IDE shows the tool window with the row open`)

### Requirement: Windows: The badge and the progress of the Dock mark the tool window

The Dock badge and the Dock progress MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of the view `main`. In
the IDE the progress MUST name what runs before its percent (`text`): Safe Push or Dry Run of the robot the Dock
follows, Update or Fetch of a running Update Project. A progress without an estimate MUST be the text without
`fraction`, a progress without a percent; `fraction: null` stays the clear.

#### Scenario: A robot that needs the user
- **WHEN** the attention count becomes 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

#### Scenario: A running Safe Push
- **WHEN** the Safe Push the Dock follows is at 37 %
- **THEN** brd asks `setProgress {view: "main", fraction: 0.37, text: "Safe Push"}`, and the title of the tool window
  reads "Branch Deck: Safe Push 37%"
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/ide-host/src/server_test.rs (`the_progress_of_the_dock_is_set_progress_of_the_main_view_with_the_text_of_what_runs`)
  [@test] src/lib/dock-text.test.ts (`names what runs before the percent, a progress without an estimate by its text alone, and clears without a text`)

#### Scenario: A Dry Run without an estimate
- **WHEN** the Dock follows a Dry Run that has no estimate left
- **THEN** brd asks `setProgress {view: "main", text: "Dry Run"}` without `fraction`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/ide-host/src/server_test.rs (`the_progress_of_the_dock_is_set_progress_of_the_main_view_with_the_text_of_what_runs`)
  [@test] src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing with its kind, indeterminate without an estimate, nothing while none runs`)

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
`bd_bootstrap` names that root as `initData.repositoryPath`: the page selects it and its selector lists the roots. The
path of a root MUST have no trailing separator but the root of a file system, as the desktop app spells it.

#### Scenario: A project with two roots
- **WHEN** the project has two Git roots
- **THEN** the main view shows the first one until the user picks the other, and its selector lists both
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] tests/e2e/air/air-page.spec.ts (`the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project`)
  [@test] crates/ide-host/src/projects_test.rs (`the_main_view_shows_the_root_picked_last_else_the_one_it_shows_else_the_first`)

#### Scenario: The URI of a directory
- **WHEN** `roots/list` names `file:///x/repo/`, as the IDE writes the URI of a directory
- **THEN** the repository is `/x/repo`, the path whose session the desktop app opens; `file:///` stays `/`
  [@test] crates/ide-host/src/server_test.rs (`the_path_of_a_root_has_no_trailing_separator`)

#### Scenario: A project with one root
- **WHEN** the project has one Git root
- **THEN** the main view shows its path as text, without the menu of a selector and without Open Repository
  [@test] tests/e2e/air/air-page.spec.ts (`the main view in the IDE offers no Open Repository, as the IDE has no folder dialog: the one root of the project is the text of the repository, without the menu of a selector`)

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
  [@test] crates/ide-host/src/projects_test.rs (`the_views_see_the_roots_as_the_recent_projects_and_a_write_keeps_the_order_of_the_desktop_app`)

### Requirement: Repositories: A change of the roots tells the main view

When `roots/list` answers other Git roots than before, brd MUST send the main view the event `roots-changed` with them,
and the views that took the settings the recent projects of the roots, as a change of `recentProjects`.

#### Scenario: A root added to the project
- **WHEN** the project gets a second Git root
- **THEN** the main view hears `roots-changed` with both roots, and its selector lists both
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_root_added_to_the_project_tells_the_main_view_and_the_recent_projects_of_the_views`)

### Requirement: Hosts: The hosts without a place in the IDE do nothing

The connection of the bridge MUST declare no part of the desktop but the host request `openWindow`: the
menu, the restart and the install of the agent CLI do nothing, the updater answers as a desktop without one, and no
folder dialog is offered, because the roots name the repositories.

#### Scenario: A check for updates
- **WHEN** a window of the bridge asks `check_for_updates`
- **THEN** the answer is the `ApiError` of a desktop without an updater, because the app updates with the desktop app
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)

### Requirement: Hosts: The page in the IDE offers no folder dialog

In the bridge, the dialog host MUST answer that it has no folder dialog (`has_folder_dialog`), which `bd_bootstrap` gives
the page as `titleBar.folderDialog: false`. The page then offers no Open Repository… beside the selector of the
repositories or in the command palette, and no Add Companion… on Settings → Landing; it only words the fact.

#### Scenario: The main view in the IDE
- **WHEN** the IDE shows the main view
- **THEN** its repository shows without the folder button beside it, and the command palette offers no Open
  Repository…
  [@test] crates/backend-host/src/channel_test.rs (`the_desktop_of_the_bridge_has_no_folder_dialog_and_installs_nothing`)
  [@test] tests/e2e/air/air-page.spec.ts (`the main view in the IDE offers no Open Repository, as the IDE has no folder dialog: the one root of the project is the text of the repository, without the menu of a selector`)
  [@test] src/lib/command-palette.test.ts (`offers no Open Repository… where the host has no folder dialog`)

### Requirement: Hosts: The page in the IDE offers no install

In the bridge, `agent_cli_status` MUST answer `installable: false`: the desktop app installs `brd` and the app file of
the IDE. The page then shows no row `brd` and no row IntelliJ IDEA on Settings → Tools, and no install of the Agent CLI
on Agents or in the hint of the agents; it only words the status.

#### Scenario: Settings → Tools in the IDE
- **WHEN** the user opens Settings → Tools in the IDE
- **THEN** it shows the command-line tools without `brd` and IntelliJ IDEA
  [@test] crates/backend-host/src/channel_test.rs (`the_desktop_of_the_bridge_has_no_folder_dialog_and_installs_nothing`)
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

### Requirement: Page: The page sends initialized once it listens

The page MUST send `ui/notifications/initialized` only once the first render of its page ran its effects, after the
listens of its events. The host delivers an event to a view only after its page sent `initialized`, and holds it
until then, so brd tells an event at once, also to the page of a view that the IDE builds for it.

#### Scenario: A main view never shown
- **WHEN** Show in Branch Deck asks the main view before the IDE built its page
- **THEN** the event `reveal-branch` reaches the page once it listens, and the main view reveals the branch
  [@test] src/lib/page-start.test.ts (`runs each waiter once the page started, and a later one at once`)

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

### Requirement: Page: The home directory of brd mcp is the home directory of the page

The shim MUST answer `homeDir` of Tauri (`plugin:path|resolve_directory` of the home) with `homeDir` of `bd_bootstrap`,
so the page shows a path under it with `~`, as the desktop app does. Another directory, or a process without a home,
is an error, and the page shows the whole path.

#### Scenario: The repository under the home directory
- **WHEN** `bd_bootstrap` names the home `/Users/ann`, and the main view shows `/Users/ann/projects/idea-1`
- **THEN** the page shows `~/projects/idea-1`
  [@test] src/platform/air/shim.test.ts (`homeDir of Tauri is the home directory of brd mcp that bd_bootstrap gave, so the page shows a path under it with ~; the IDE resolves no other directory`)

### Requirement: Page: The subscriptions of the page share the one view of its frame

Air holds one view for each frame, so the shim MUST keep at most one `x-air-app/subscribe` of a frame. A subscription of
the repository the frame holds joins its view and asks brd to subscribe again (`bd_invoke` of `subscribe_view`), or,
when brd refused the `subscribe` of that view, subscribes the frame anew: every subscription gets an answer of brd asked
after it. One of another repository subscribes once the view before it answered and was unsubscribed or ended.

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

#### Scenario: A subscription while the view of the frame opens
- **GIVEN** the main view subscribes again to a repository without a prefix (a focus after the refusal), and the user
  saves the prefix of the welcome card while that view opens
- **WHEN** brd refuses the view, as it read the config before the prefix
- **THEN** the subscription after Save subscribes the frame anew, and brd answers it with the view of the repository
  [@test] src/platform/air/shim.test.ts (`A subscription asked while the view of the frame opens gets an answer of brd asked after it: the refusal of that view, asked before a prefix was saved, makes it subscribe anew`)

#### Scenario: The link of the view breaks while a subscription joined it
- **GIVEN** a subscription joined the view of the frame while it opened
- **WHEN** the `subscribe` of that view fails without an answer of brd (the link broke)
- **THEN** the joined subscription fails with it and subscribes nothing anew
  [@test] src/platform/air/shim.test.ts (`A subscription that joined a view whose link broke fails with it: only a refusal of brd is an answer of the config as it was`)

### Requirement: Page: The theme of the IDE maps onto the tokens of the page

The page MUST map the theme and the styles of `hostContext` onto its semantic tokens, and follow each
`host-context-changed`. The canvas is the background of the place; the cards, the popups and the fields are the raised
surface of the IDE; the separator is the separator of the IDE. A token whose meaning the IDE lacks stays the page's:
`--info`, the violet of merged, and `--primary`. The `styles.variables` of a context MUST be the whole set: a variable
that a later set lacks leaves the page.

#### Scenario: Light to Dark
- **WHEN** the IDE switches from Light to Dark
- **THEN** the page shows the dark tokens without a reload
  [@test] tests/e2e/air/air-page.spec.ts (`the page shows the theme of the IDE and follows its switch from Light to Dark without a reload`)

#### Scenario: The styles all
- **GIVEN** the app file has `styles: all`, so Air sends every variable
- **WHEN** the page shows the theme of the IDE
- **THEN** `--info` is the violet of the page, and `--border` is the separator of the IDE
  [@test] tests/e2e/air/air-page.spec.ts (`with the styles all of the app file, the violet of merged stays the page's, and the lines of the rows are the separator of the IDE`)

#### Scenario: The styles neutral, then none
- **GIVEN** the app file has `styles: neutral`, so Air sends no status tone
- **WHEN** the page shows the theme of the IDE
- **THEN** `--success` and `--destructive` are the page's, and `--canvas` and `--card` the IDE's
- **AND** after a change of the field to `none` without a reload, every token is the page's
  [@test] tests/e2e/air/air-page.spec.ts (`with the styles neutral of the app file, the status tones are the page's and the surfaces the IDE's, and a change to none without a reload makes every token the page's`)

### Requirement: Page: The IDE moves a window whose row is in its title bar

Where the host context names an overlay title bar, a left press on the empty parts of the row MUST send
`x-air-app/windowDrag {clickCount}` at its mouse down with `event.detail`, and the IDE moves the window.
`is_fullscreen` MUST be true when the IDE names the inset 0. A place with a native title bar MUST send no `windowDrag`.

#### Scenario: A drag and a double click of the row of Settings
- **WHEN** the user presses the empty part of the row of the Settings window, then double-clicks it
- **THEN** the IDE gets `windowDrag` with the click count 1, then 1 and 2
  [@test] tests/e2e/air/air-page.spec.ts (`the row of the Settings window is in the title bar of the IDE: it starts after the traffic lights, a press on its empty parts moves the window and a double click is the double click of the system, full screen moves it to the edge, and the row of the tool window is no drag region`)
  [@test] src/platform/air/shim.test.ts (`A window whose row is in the title bar of the IDE: a drag and a double click of the row are windowDrag, its full screen is what the IDE says, and a change of the title bar resizes the window for the page`)

#### Scenario: Full screen
- **WHEN** the IDE enters full screen and names the inset 0 in `host-context-changed`
- **THEN** the window resizes for the page, and the row starts at the edge
- **AND** once the IDE names the inset of the traffic lights again, the row starts after them
  [@test] tests/e2e/air/air-page.spec.ts (`the row of the Settings window is in the title bar of the IDE: it starts after the traffic lights, a press on its empty parts moves the window and a double click is the double click of the system, full screen moves it to the edge, and the row of the tool window is no drag region`)
  [@test] src/platform/air/shim.test.ts (`A window whose row is in the title bar of the IDE: a drag and a double click of the row are windowDrag, its full screen is what the IDE says, and a change of the title bar resizes the window for the page`)

#### Scenario: A tool window
- **GIVEN** the host context of the main view names no title bar
- **WHEN** the page asks to drag or to zoom its window
- **THEN** the IDE gets no `windowDrag`, and the row of the tool window is no drag region
  [@test] src/platform/air/shim.test.ts (`A tool window, an editor tab or a dialog draws its own title: the page moves no window of the IDE`)
  [@test] tests/e2e/air/air-page.spec.ts (`the row of the Settings window is in the title bar of the IDE: it starts after the traffic lights, a press on its empty parts moves the window and a double click is the double click of the system, full screen moves it to the edge, and the row of the tool window is no drag region`)

#### Scenario: The app file
- **WHEN** the install writes the app file
- **THEN** the Settings window names `titleBar: overlay`, and the other places name no title bar
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_app_file_is_the_example_of_the_contract_with_the_command_of_this_binary`)

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

The app file MUST be the example app file of the contract with the command of this binary: the app binary with `ide`
on every platform (`branch-deck.exe ide` on Windows), `brd` with `mcp` only where no app binary is beside it. A binary
that macOS runs translocated is refused. The file MUST have `styles: neutral`, but an install MUST keep the value `all`,
`neutral` or `none` that the file has, and the status MUST compare the file with the text of that value.

#### Scenario: The app file
- **WHEN** the install writes the app file
- **THEN** it reads as `docs/design/ide-embedding/protocol/examples/branch-deck.yaml` with the command of this binary
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_app_file_is_the_example_of_the_contract_with_the_command_of_this_binary`)

#### Scenario: The command of each binary
- **WHEN** the binary is the app binary, or `brd`
- **THEN** the arguments are `ide` on every platform, or `mcp`; a file of an earlier Branch Deck (`cli mcp`) is not
  current, and nothing writes it but Install
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_runs_the_bridge_with_the_app_binary_on_every_platform_and_mcp_with_brd_alone`)
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_ide_app_for_this_binary_and_removes_it`)

#### Scenario: The styles of the user
- **GIVEN** the user set `styles: all` in the app file of this binary
- **WHEN** Settings → Tools reads the file, and the user clicks Install
- **THEN** the row says the file is current, and the install keeps `styles: all`
- **AND** a value that Air does not know is not current, and the install writes `neutral`
  [@test] crates/bd-cli/src/install_test.rs (`an_install_keeps_the_styles_the_user_set_in_the_ide_app_file`)

### Requirement: Backend: A start with --background builds no window

The app started with `--background` (the start of the bridge of an IDE) MUST build no window and, on macOS, show no
Dock icon (`ActivationPolicy::Accessory`). With no bridge connected, it MUST exit 30 s after its start.

#### Scenario: The bridge starts the app
- **WHEN** the app starts with `--background`
- **THEN** it builds no window, and the Dock shows no icon of it
- **AND** with no bridge connected it exits 30 s after its start
  [@test] crates/app-api/src/app/life_test.rs (`without_a_window_and_a_connection_the_process_exits_after_the_grace_of_the_later_of_its_windowless_start_and_its_last_connection`)
  [@test] crates/app-api/src/app/life_test.rs (`only_the_background_argument_starts_without_a_window_and_a_second_start_with_it_shows_no_main_window`)

### Requirement: Backend: The desktop app stays while an IDE is connected

When the last window closes or the user chooses Quit of the app menu (⌘Q, an item of the app, not `terminate:`) while
a bridge is connected, the windows MUST close and the process MUST stay without a window (no Dock icon on macOS). With
no window and no connection it MUST exit 30 s after the later of the moment it became windowless and the end of its
last connection. Quit of the Dock MUST still end the process.

#### Scenario: The last window closes while an IDE is connected
- **GIVEN** a bridge is connected
- **WHEN** the last window closes, or the user chooses Quit (⌘Q)
- **THEN** the windows close, the process stays, and the Dock shows no icon of it
  [@test] crates/app-api/src/app/life_test.rs (`the_windows_close_and_the_process_stays_only_while_a_bridge_is_connected`)

#### Scenario: Quit with no IDE connected
- **GIVEN** no bridge is connected
- **WHEN** the last window closes, or the user chooses Quit (⌘Q)
- **THEN** the app quits, as an app does
  [@test] crates/app-api/src/app/life_test.rs (`the_windows_close_and_the_process_stays_only_while_a_bridge_is_connected`)

#### Scenario: The last connection ends
- **GIVEN** the app runs without a window for an IDE
- **WHEN** the last bridge disconnects
- **THEN** the app exits 30 s later, unless a bridge connects or a window opens meanwhile
  [@test] crates/app-api/src/app/life_test.rs (`without_a_window_and_a_connection_the_process_exits_after_the_grace_of_the_later_of_its_windowless_start_and_its_last_connection`)

#### Scenario: Quit of the Dock
- **WHEN** the user chooses Quit of the menu of the Dock icon
- **THEN** the process ends, as AppKit's `terminate:` ends it with no event to keep it; the bridge of a connected IDE starts the app again or runs its fallback

### Requirement: Backend: Reopen and showMain show the main window

Reopen of macOS with no window shown (a click on the Dock icon, an open of the running app) and
`x-branch-deck/showMain` of a second start MUST build the main window, or bring it to the front, and put the app back
in the Dock. A second start with `--background` MUST show no window.

#### Scenario: The app is opened again
- **GIVEN** the app runs without a window
- **WHEN** the user opens it again, or a second start sends `x-branch-deck/showMain`
- **THEN** the main window opens and the Dock shows the app
  [@test] crates/app-api/src/app/life_test.rs (`only_the_background_argument_starts_without_a_window_and_a_second_start_with_it_shows_no_main_window`)

### Requirement: Backend: A restart keeps the windows as they are

A restart of the app (Restart of a setting, Install and Restart of an update) MUST start the new process without
`--background` when a window is open and with it when none is, whatever the arguments of the first start were.

#### Scenario: A start for an IDE shows its window, then restarts
- **GIVEN** the app started with `--background` and its main window opened since
- **WHEN** it restarts to install an update
- **THEN** the new process starts with its main window
  [@test] crates/app-api/src/app/life_test.rs (`a_restart_starts_with_a_window_when_one_is_open_and_without_one_when_none_is`)

### Requirement: Backend: The app without a window checks for updates

While no main window is open, the app MUST check for an update itself, as the page of the main window does: 3 s
after its start, then every six hours, never in a development build. The update it finds is downloaded, and Setup of
the next main window offers it.

#### Scenario: An update while the app has no window
- **GIVEN** the app runs without a window for an IDE
- **WHEN** six hours pass with an update published
- **THEN** the app downloads it, and the main window that opens next offers it in Setup

## User Experience

- The tool window **Branch Deck** at the bottom, one frame: the main window as in the desktop app, with its switcher
  of **Branches**, **Reviews** and **Landing**, its **Activity** button and side panel, its Sync and its Settings.
- **Activity** in a window of the IDE where the tool window has no room for the side panel.
- **Sync Virtual Branches** (the VCS menu, ⌥⌘Y, Ctrl+Alt+Y), **Branch Deck Command Palette** and **Branch Deck
  Settings**, the actions of the app file, in place of the menu of the desktop app; the title of the tool window has no
  action of its own.
- With one root, the repository is its path as text, `~` for the home directory, without the menu of a selector.
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
