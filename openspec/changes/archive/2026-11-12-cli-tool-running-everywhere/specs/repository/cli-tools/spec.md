## MODIFIED Requirements

### Requirement: Agent skill: The row shows the running action with Cancel

The row MUST show the last line of the running action and Cancel, in every window: Rust keeps the actions that run
(`cli_tools_running`, the event `cli-tools-running`, whose `version` grows with each change), so a window shows one
that another window started (the Setup dot of the main window) or that ran before its page loaded, and keeps the
newest of the read and the events. The sign-in of `youtrack` with the app's YouTrack, another command, shows only in
the window that started it; `brd mcp` in an IDE keeps its own actions, which the app does not see.

#### Scenario: An action runs
- **WHEN** an action runs
- **THEN** the row shows each line as it comes, the last one standing, and Cancel
  [@test] crates/sync-utils/src/child_process_test.rs (`hands_over_each_line_as_it_comes_and_returns_the_output`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should stop a running install on Cancel, without an error`)

#### Scenario: An action another window started
- **GIVEN** an update started from the Setup dot of the main window
- **WHEN** Settings → Tools opens while it runs
- **THEN** its row shows the update with its last line and Cancel, not Update
- **AND** Cancel ends it, and neither window toasts
  [@test] crates/app-api/src/app/cli_tools_test.rs (`every_window_hears_the_action_of_a_tool_start_write_its_lines_and_end_and_a_second_one_is_refused_meanwhile`)
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should show the update an item started as running, in the dot and in Settings opened meanwhile, which offers no second one`)

### Requirement: Agent skill: One action per tool, at most 10 minutes

An action MUST run at most 10 minutes; one action per tool at a time; the other buttons of the row wait for it. A
second action of the tool while one runs (two windows that start one at once) is refused with `cli_tool_busy`, which
toasts nothing: the row shows the action that runs. An action that panics frees its tool in every window.

#### Scenario: An action runs
- **WHEN** an action of a tool runs
- **THEN** no other action of the tool starts and the other buttons of the row wait for it
- **AND** it ends after 10 minutes at most

#### Scenario: A second action while one runs
- **GIVEN** an action of `youtrack` runs
- **WHEN** another window starts an action of `youtrack`
- **THEN** it is refused with `cli_tool_busy`, and no toast shows
  [@test] crates/app-api/src/app/cli_tools_test.rs (`every_window_hears_the_action_of_a_tool_start_write_its_lines_and_end_and_a_second_one_is_refused_meanwhile`)

#### Scenario: An action panics
- **WHEN** an action panics
- **THEN** its tool is free again, and every window hears that it ended
  [@test] crates/app-api/src/app/cli_tools_test.rs (`an_action_that_panics_frees_its_tool_in_every_window`)

### Requirement: Status: The servers of teamcity are read from its auth status

A server of `teamcity auth status --json` MUST be read as signed in (`authenticated`), a guest, or failed with the
CLI's error; the entry of no server is no server; `token_expiry` is its expiry. The first `http://` or `https://`
word of the error is its page (`error_url`), without the punctuation after it.

#### Scenario: The servers of teamcity with their user and expiry
- **WHEN** `teamcity auth status --json` is read
- **THEN** each server is signed in (`authenticated`), a guest, or failed with the CLI's error, with `token_expiry` as its expiry
- **AND** the entry of no server is no server
  [@test] crates/cli-tools/src/parse_test.rs (`reads_the_servers_of_teamcity_with_their_user_and_expiry`)

#### Scenario: An error that names a page
- **WHEN** the error of a server is "login gateway detected - https://jb.gg/warp may be required"
- **THEN** its page is `https://jb.gg/warp`
  [@test] crates/cli-tools/src/parse_test.rs (`a_failed_server_names_the_page_of_its_reason`)

### Requirement: Sign-in: The server is the one an open of Settings names

The server MUST be the one an open of Settings names (`SettingsTarget.server`: the server of a robot's build, from the
toast of a copied prompt), else the first one the CLI knows, else typed. A server the CLI has a line of signs in from
the head of the row, Sign In, with Other Server… for the form; the form shows by itself only for a server the CLI does
not know, so a server has one sign-in.

#### Scenario: An open of Settings names the server
- **WHEN** Settings opens with `SettingsTarget.server`, from the toast of a copied prompt
- **THEN** the sign-in is to that server
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign teamcity in through its own browser sign-in, to the server an open names`)

#### Scenario: No server named
- **WHEN** no open names a server
- **THEN** the server is the first one the CLI knows
- **AND** Sign In at the head of the row signs in to it, with neither its form nor Sign In Again on its line
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign in to a server whose sign-in failed from the head of its row, its reason a link, and to another server from its form`)

#### Scenario: Another server
- **GIVEN** the CLI knows the server to sign in to
- **WHEN** the user clicks Other Server…
- **THEN** the form shows, empty, and signs in to the server typed
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign in to a server whose sign-in failed from the head of its row, its reason a link, and to another server from its form`)

#### Scenario: A typed server
- **WHEN** neither names a server
- **THEN** the user types it
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line`)

### Requirement: Sign-in: Each server line has Sign In Again

Each server line MUST have Sign In Again (a token that expires or was refused), but the line of the server that Sign
In at the head of the row signs in to. A failed server says why in the CLI's words, in `text-warning`, the page they
name a link.

#### Scenario: Sign In Again from a server's line
- **GIVEN** a server line of `teamcity`
- **WHEN** the user clicks its Sign In Again
- **THEN** the browser sign-in of the CLI runs on that server
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line`)

#### Scenario: A failed server
- **GIVEN** the sign-in of a server failed with "login gateway detected - https://jb.gg/warp may be required"
- **WHEN** the row shows
- **THEN** its line says "ci.example · login gateway detected - https://jb.gg/warp may be required", the page a link that opens it
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign in to a server whose sign-in failed from the head of its row, its reason a link, and to another server from its form`)
