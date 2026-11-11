## MODIFIED Requirements

### Requirement: Page: The page in the IDE is the page of the desktop app

The Air build (`vite build --mode air`, entry `src/air-main.ts`, into `dist-air/`) MUST run the same page as the desktop
app. It answers `ui/initialize` with `x-air-app`, awaits `bd_bootstrap`, installs the globals of the init script of the
window and the internals of the shim, and then runs the usual main on the page of its view (`main` is `/`, another view
`/<view>`). Its files stay separate, the chunks and the worker each a resource `ui://branch-deck/<path>`, and
`index.html` is the page of every view.

#### Scenario: The main window
- **WHEN** the IDE opens the tool window of Branch Deck on a repository
- **THEN** the page shows the branches of the repository as the desktop app does

### Requirement: Page: The shim maps the Tauri calls onto x-air-app

The shim of `src/platform/air/` MUST map `invoke` to `bd_invoke` with the arguments as the page sent them, a `Channel`
to the call events of its call (the argument the table of app-api names, `crates/app-api/commands.json`) and
`subscribe_view` to the mirror. A raw answer is its bytes, an `isError` answer rejects with its `ApiError`, and a failure
of the link rejects with an `internal` error that names its reason. `listen` hears the events of `x-air-app`, and `emit`
goes to the views of the app through `brd mcp`. The plugins of Tauri are what the IDE offers: a link opens by
`ui/open-link`, the clipboard, an OS notification, the badge and the progress of the Dock are tools of the host, the log
is `notifications/message`, and the size, the zoom and the frame of a window are the IDE's. The page of the IDE and the
page mock of the E2E tests install the same internals of Tauri (`src/platform/tauri-page.ts`).

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

## ADDED Requirements

### Requirement: Page: The mirror is the view, and each change of it a patch of the session

The state of the mirror MUST be the `RepositoryView` of the session, its parts the fields of the view and its version
the part `version`. The page MUST build the `ViewPatch` of each `x-air-app/viewChanged` from the ops and the state
before them: a keyed part sets and removes by key, another part is replaced whole, a part with keyed entries of its own
(the sections, the Patronus view, the facts) changes by the entries that differ, and the news are the part `news`
whole. The view the page has after the patch is the view of the session.

#### Scenario: A renamed branch
- **WHEN** a change sets `branches/b`, removes `branches/a` and replaces `order`
- **THEN** the patch of the page has the branch `b`, removes `a`, and its view is the view of the session
  [@test] src/platform/air/view-mirror.test.ts (`A renamed branch: one change sets branches/b, removes branches/a and replaces order`)

#### Scenario: The news and the nested parts
- **WHEN** a change replaces the news, the sections, the Patronus view or the facts
- **THEN** the view of the page after the patch is the view of the session
  [@test] src/platform/air/view-mirror.test.ts (`The news: the part news of the kept list, of which the page tells those it has not told`, `A change of the parts that hold keyed entries of their own: the sections, the Patronus view and the facts`)
