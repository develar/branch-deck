## MODIFIED Requirements

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
