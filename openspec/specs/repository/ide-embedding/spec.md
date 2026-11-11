---
name: Branch Deck in the IDE
description: Requirements for running the page and the backend of Branch Deck as an app of IntelliJ IDEA over x-air-app, with brd mcp as its process and its windows as views of the IDE.
style: plain-1
targets:
  - docs/design/ide-embedding/protocol/x-air-app.schema.json
  - docs/design/ide-embedding/protocol/air-app.schema.json
  - docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson
  - docs/design/ide-embedding/protocol/examples/branch-deck.yaml
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

#### Scenario: The app link corpus
- **WHEN** a test runs `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson` against `brd mcp` on the fixtures
- **THEN** each line of the app matches the corpus

### Requirement: `brd mcp`: The data of brd mcp is the data of the desktop app

`brd mcp` MUST keep its settings, its caches and its journals in the directories of `org.develar.branch-deck`, the
same as the desktop app and `brd`.

#### Scenario: A setting of the desktop app
- **WHEN** the desktop app saved a setting, and `brd mcp` starts
- **THEN** `bd_bootstrap` gives that setting

### Requirement: `brd mcp`: The page of each view is a resource built into the binary

`resources/read` of `ui://branch-deck/<view>` MUST answer the page of the view as `text/html;profile=mcp-app`. Each
file of the Air build is `ui://branch-deck/<path>`, and another URI gets `-32602 badParams`.

#### Scenario: The main page and a missing file
- **WHEN** air-ide reads `ui://branch-deck/main` and then `ui://branch-deck/nothing.js`
- **THEN** it gets the HTML of the page, and then `-32602 {reason: "badParams"}`

### Requirement: `brd mcp`: The end of stdin ends brd mcp

At the end of its stdin, `brd mcp` MUST close its sessions and exit with 0.

#### Scenario: The IDE closes the project
- **WHEN** the stdin of `brd mcp` ends
- **THEN** the process exits with 0

### Requirement: Tools: The tools are bd_invoke and bd_bootstrap, both only for the app

`tools/list` MUST list `bd_invoke` and `bd_bootstrap`, each with `_meta.ui.visibility: ["app"]`, and no other tool.

#### Scenario: The list
- **WHEN** air-ide sends `tools/list`
- **THEN** it gets the two tools, and no agent sees either

### Requirement: Tools: bd_invoke runs one command of app-api by its name

`bd_invoke {command, args}` MUST run the command of `for_each_command!` named `command` with `args`. The output is
`structuredContent.value`, and an `ApiError` is `isError` with `structuredContent.error`.

#### Scenario: A JSON command
- **WHEN** the page calls `bd_invoke` with `get_settings_changes`
- **THEN** the result holds the output of the command in `structuredContent.value`

#### Scenario: An unknown command
- **WHEN** the page calls `bd_invoke` with a name that no command has
- **THEN** the result is `isError` with an `ApiError`

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

### Requirement: View: subscribe_view is the mirror of the repository

`x-air-app/subscribe {instance, params}` MUST answer the mirror `repositoryPath` and the view of the session without
its `version`. The views of one repository share that mirror.

#### Scenario: The main view of a repository
- **WHEN** air-ide subscribes `main:1` to a repository
- **THEN** the answer names the path of the repository and holds the view

### Requirement: View: Each patch of the session is one change of the mirror

Each `ViewPatch` MUST become one `x-air-app/viewChanged`. A keyed part sets each changed key and removes each removed
key, and every other part is replaced whole.

#### Scenario: A renamed branch
- **WHEN** a sync renames the branch `a` to `b`
- **THEN** one change sets `branches/b`, removes `branches/a` and replaces `order`

#### Scenario: The news
- **WHEN** a patch carries news
- **THEN** the change replaces the part `news` with the last news of the session, so a resync tells none twice

### Requirement: View: A patch and the events of a call come before its answer

`brd mcp` MUST write the changes and the events that a command caused before the answer of the command.

#### Scenario: A rewrite
- **WHEN** the page runs a rewrite with `execute`
- **THEN** its `viewChanged` and its call events come before the answer of `bd_invoke`

### Requirement: View: The mirror takes no intent

`x-air-app/dispatchAction` MUST get `-32602 {reason: "unknownAction"}`. The page sends its intents with `bd_invoke`,
as in the desktop app.

#### Scenario: An intent
- **WHEN** air-ide dispatches an action to the mirror of a repository
- **THEN** brd answers `unknownAction`, and the view does not change

### Requirement: View: The end of a session ends its mirror

When the session of a repository closes, `brd mcp` MUST send `x-air-app/viewEnded {mirror, reason}`.

#### Scenario: A removed repository
- **WHEN** the repository of a mirror is gone
- **THEN** brd sends `viewEnded` for it

### Requirement: Events: An event of the app is an event of x-air-app

`AppEvents::emit` MUST send `x-air-app/event {event, payload}`, and `emit_to` sends it with the instance of its window.

#### Scenario: A change of the settings
- **WHEN** a setting changes
- **THEN** brd sends one `x-air-app/event` without an instance

### Requirement: Windows: A window of the desktop app is a view of the IDE

Each open of a sub-window MUST be `x-air-app/openView` of its view. A review is an editor tab keyed by its target,
the settings a frame, and the conflicting files and the missing commits a dialog.

#### Scenario: A review
- **WHEN** the user opens the review of a branch twice
- **THEN** brd asks `openView` of `review` with the key of the branch, and the IDE shows one tab

#### Scenario: A new title
- **WHEN** what a review shows changes its title
- **THEN** brd asks `setViewTitle` of its instance

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

### Requirement: Repositories: The roots of the project are the repositories of brd mcp

`brd mcp` MUST read the repositories from `roots/list`, at its start and on `notifications/roots/list_changed`. The
main view shows the root that the user picked last in the project, else the first Git root.

#### Scenario: A project with two roots
- **WHEN** the project has two Git roots
- **THEN** the main view shows the first one until the user picks the other

### Requirement: Hosts: The hosts without a place in the IDE do nothing

In `brd mcp`, the menu, the updater, the restart and the install of the agent CLI MUST do nothing. A folder dialog is
not offered, because the roots name the repositories.

#### Scenario: A check for updates
- **WHEN** the page asks for an update in the IDE
- **THEN** brd does nothing, because the app updates with the desktop app

### Requirement: Page: The page in the IDE is the page of the desktop app

The Air build (`vite build --mode air`, entry `src/air-main.ts`) MUST run the same page as the desktop app. It awaits
`bd_bootstrap`, installs the globals of the shim, and then runs the usual main.

#### Scenario: The main window
- **WHEN** the IDE opens the tool window of Branch Deck on a repository
- **THEN** the page shows the branches of the repository as the desktop app does

### Requirement: Page: The shim maps the Tauri calls onto x-air-app

The shim of `src/platform/air/` MUST map `invoke` to `bd_invoke`, a `Channel` to the call events and `subscribe_view`
to the mirror. `listen` and `emit` map to the events of x-air-app.

#### Scenario: A command with a channel
- **WHEN** the page invokes `get_file_diffs` with a `Channel`
- **THEN** the channel gets each call event of the call, in order, before the promise resolves

#### Scenario: A resync
- **WHEN** the subscription of the page gets `x-air-app/resync`
- **THEN** the page takes the state as a new snapshot of the view, without a reload

### Requirement: Page: The theme of the IDE maps onto the tokens of the page

The page MUST map the theme and the styles of `hostContext` onto its semantic tokens, and follow each
`host-context-changed`.

#### Scenario: Light to Dark
- **WHEN** the IDE switches from Light to Dark
- **THEN** the page shows the dark tokens without a reload

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
