## ADDED Requirements

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

## MODIFIED Requirements

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

### Requirement: Hosts: The hosts without a place in the IDE do nothing

The connection of the bridge MUST declare no part of the desktop but the host request `openWindow`: the
menu, the restart and the install of the agent CLI do nothing, the updater answers as a desktop without one, and no
folder dialog is offered, because the roots name the repositories.

#### Scenario: A check for updates
- **WHEN** a window of the bridge asks `check_for_updates`
- **THEN** the answer is the `ApiError` of a desktop without an updater, because the app updates with the desktop app
  [@test] crates/backend-host/src/corpus_test.rs (`the_host_requests_corpus_runs`)

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
