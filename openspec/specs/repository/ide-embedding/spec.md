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
  - crates/mcp-host/src/process.rs
  - crates/test-server/src/mcp_socket.rs
---

# Branch Deck in the IDE

Status: Draft
Date: 2026-11-12

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

## Concepts

- The contract is Air's: `plugins/air/native/air-apps/protocol/` of idea-1. `docs/design/ide-embedding/protocol/` is its
  copy, with `corpus/branch-deck-app.ndjson`, the app link as Branch Deck writes it.
- An instance is one open view of the IDE, such as `review:3`. It is the window of the desktop app: the window of a
  command, of an event to one window and of `forget_window`.

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

### Requirement: Tools: The tools are bd_invoke and bd_bootstrap, both only for the app

`tools/list` MUST list `bd_invoke` and `bd_bootstrap`, each with `_meta.ui.visibility: ["app"]`, and no other tool.

#### Scenario: The list
- **WHEN** air-ide sends `tools/list`
- **THEN** it gets the two tools, and no agent sees either
  [@test] crates/mcp-host/src/server_test.rs (`both_tools_are_only_for_the_app`)
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

### Requirement: Tools: bd_bootstrap gives what the page needs before its main

`bd_bootstrap {view}` MUST answer the settings, the warm frame, the init data of the view and the view of the title
bar. The page takes them where the desktop app takes its init script.

#### Scenario: The main view
- **WHEN** the page of `main` calls `bd_bootstrap`
- **THEN** the result has `settings`, `warmFrame`, `initData` and `titleBar`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The action Sync Virtual Branches is the tool sync

A `tools/call` of `sync` with the origin `action` MUST sync the repository of the main view, as Sync does in the
desktop app. No list of tools names it, and no other origin calls it.

#### Scenario: The shortcut of Sync
- **WHEN** the user presses Sync Virtual Branches in the IDE
- **THEN** brd syncs the repository that the main view shows

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar `native`.

#### Scenario: The settings of the desktop app
- **WHEN** the settings window bootstraps after the desktop app saved a setting
- **THEN** `settings.values` holds the setting
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: View: subscribe_view is the mirror of the repository

`x-air-app/subscribe {instance, params}` MUST answer the mirror `repositoryPath` and the view of the session without
its `version`. The views of one repository share that mirror.

#### Scenario: The main view of a repository
- **WHEN** air-ide subscribes `main:1` to a repository
- **THEN** the answer names the path of the repository and holds the view
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: View: Each patch of the session is one change of the mirror

Each `ViewPatch` MUST become one `x-air-app/viewChanged`. A keyed part sets each changed key and removes each removed
key, and every other part is replaced whole.

#### Scenario: A renamed branch
- **WHEN** a sync renames the branch `a` to `b`
- **THEN** one change sets `branches/b`, removes `branches/a` and replaces `order`
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

When the session of a repository closes, `brd mcp` MUST send `x-air-app/viewEnded {mirror, reason}`.

#### Scenario: A removed repository
- **WHEN** the repository of a mirror is gone
- **THEN** brd sends `viewEnded` for it

### Requirement: View: The demand of a view is the demand of its mirror

`set_demand` of a view MUST name the subscription of the mirror of its instance, which the page does not know.

#### Scenario: A review that demands the details of a robot
- **WHEN** a review tab sets its demand
- **THEN** the session of its repository reads what the demand asks

### Requirement: Events: An event of the app is an event of x-air-app

`AppEvents::emit` MUST send `x-air-app/event {event, payload}`, and `emit_to` sends it with the instance of its window.

#### Scenario: A change of the settings
- **WHEN** a setting changes
- **THEN** brd sends one `x-air-app/event` without an instance
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_a_setting_is_an_event_and_the_badge_marks_the_tool_window`)

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

A notification of the desktop app MUST be `x-air-app/notify` with an action that opens its object.

#### Scenario: A failed Safe Push
- **WHEN** a Safe Push fails while the IDE shows another tab
- **THEN** the IDE shows a notification whose action opens the branch

### Requirement: Windows: The badge and the progress of the Dock mark the tool window

The Dock badge and the Dock progress MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of the view `main`.

#### Scenario: A robot that needs the user
- **WHEN** the attention count becomes 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`

### Requirement: Windows: A plugin call of the page is the desktop of the IDE

`bd_invoke` of `plugin:window|set_badge_count`, `plugin:window|set_progress_bar` and `plugin:notification|notify`
MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of `main` and `x-air-app/notify`.

#### Scenario: The badge
- **WHEN** the page sets the badge of the Dock to 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_a_setting_is_an_event_and_the_badge_marks_the_tool_window`)

### Requirement: Windows: A second open of a shown view hands it its data

When the IDE answers an open with an instance the window showed already, brd MUST send it the data of the open
(`init-data-updated`) and its title (`setViewTitle`).

#### Scenario: The review of the same branch
- **WHEN** a review tab shows a branch and the user opens its review again
- **THEN** the tab gets `init-data-updated` with the data and the title of the new open
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_open_of_a_review_is_an_open_view_keyed_by_its_target`)

### Requirement: Repositories: The roots of the project are the repositories of brd mcp

`brd mcp` MUST read the repositories from `roots/list`, at its start and on `notifications/roots/list_changed`. The
main view shows the root that the user picked last in the project, else the first Git root.

#### Scenario: A project with two roots
- **WHEN** the project has two Git roots
- **THEN** the main view shows the first one until the user picks the other

### Requirement: Repositories: A change of the roots tells the main view

When `roots/list` answers other Git roots than before, brd MUST send the main view the event `roots-changed` with them.

#### Scenario: A root added to the project
- **WHEN** the project gets a second Git root
- **THEN** the main view hears `roots-changed` with both roots

### Requirement: Hosts: The hosts without a place in the IDE do nothing

In `brd mcp`, the menu, the updater, the restart and the install of the agent CLI MUST do nothing. A folder dialog is
not offered, because the roots name the repositories.

#### Scenario: A check for updates
- **WHEN** the page asks for an update in the IDE
- **THEN** brd does nothing, because the app updates with the desktop app
  [@test] crates/mcp-host/src/hosts_test.rs (`the_hosts_without_a_place_in_the_ide_do_nothing`)

### Requirement: Page: The page in the IDE is the page of the desktop app

The Air build (`vite build --mode air`, entry `src/air-main.ts`, into `dist-air/`) MUST run the same page as the desktop
app. It awaits `bd_bootstrap`, installs the globals of the shim, and then runs the usual main on the page of its view
(`main` is `/`, another view `/<view>`). Its chunks and its worker stay files of their own, and `index.html` is the page
of every view.

#### Scenario: The main window
- **WHEN** the IDE opens the tool window of Branch Deck on a repository
- **THEN** the page shows the branches of the repository as the desktop app does

### Requirement: Page: The shim maps the Tauri calls onto x-air-app

The shim of `src/platform/air/` MUST map `invoke` to `bd_invoke`, a `Channel` to the call events and `subscribe_view`
to the mirror. `listen` and `emit` map to the events of x-air-app, and each plugin of Tauri to what the IDE offers in
its place. The page mock of the E2E tests installs the same internals of Tauri (`src/platform/tauri-page.ts`).

#### Scenario: A command with a channel
- **WHEN** the page invokes `get_file_diffs` with a `Channel`
- **THEN** the channel gets each call event of the call, in order, before the promise resolves
  [@test] src/platform/air/shim.test.ts (`A command with a channel: the channel gets each call event of the call, in order, before the promise resolves`)

#### Scenario: The answers of a command
- **WHEN** a command answers a value, raw bytes, an `ApiError`, or the link is lost
- **THEN** the page gets the value, an `ArrayBuffer`, the `ApiError`, or an `internal` error with the reason
  [@test] src/platform/air/shim.test.ts (`A JSON command answers its value, a raw one its bytes, a failed one its ApiError`)

#### Scenario: A resync
- **WHEN** the subscription of the page gets `x-air-app/resync`
- **THEN** the page takes the state as a new snapshot of the view, without a reload
  [@test] src/platform/air/shim.test.ts (`subscribe_view is the mirror of the repository: its state the view, each change a patch, a resync a snapshot without a reload`)

#### Scenario: An event
- **WHEN** `brd mcp` tells an event, and the page emits one to the main window
- **THEN** the listeners of its name hear it, and the emit is `bd_invoke` of `plugin:event|emit` with its target
  [@test] src/platform/air/shim.test.ts (`listen hears the events of x-air-app, and emit goes to the views of the app through brd mcp`)

#### Scenario: The plugins
- **WHEN** the page opens a link, copies, sets the badge and the progress of the Dock, logs, or sets the size of its window
- **THEN** the host gets `ui/open-link`, its tools, `notifications/message`, and nothing for the size
  [@test] src/platform/air/shim.test.ts (`The plugins of Tauri are what the IDE offers: a link, the clipboard, the badge, the progress, the log`)

### Requirement: Page: The theme of the IDE maps onto the tokens of the page

The page MUST map the theme and the styles of `hostContext` onto its semantic tokens, and follow each
`host-context-changed`.

#### Scenario: Light to Dark
- **WHEN** the IDE switches from Light to Dark
- **THEN** the page shows the dark tokens without a reload

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

Each line of `docs/design/ide-embedding/protocol/corpus/*.ndjson` MUST be valid against `x-air-app.schema.json`. The
example app file is valid against `air-app.schema.json`.

#### Scenario: The corpus
- **WHEN** a test reads each corpus file of the copy
- **THEN** each line is valid, and the example app file too

## User Experience

- The tool window **Branch Deck** at the bottom, with the tab **Activity**; the action **Sync Virtual Branches** in the
  VCS menu (⌥⌘Y, Ctrl+Alt+Y).
- Before the first start, the empty tool window shows the consent banner of the IDE: **Allow Once**, **Always Allow**,
  **Deny**.

## Open Questions / Risks

- The chain JCEF → WebSocket → `air-ide` → stdio on a big diff: stage 0 of the plan measures it.
- The desktop app and `brd mcp` both poll GitHub and Patronus in P1.

## References

- [IDE Embedding](../../../../docs/design/ide-embedding.md)
- [Settings](../settings/spec.md)
