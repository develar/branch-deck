## ADDED Requirements

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
