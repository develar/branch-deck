## ADDED Requirements

### Requirement: Hosts: The page in the IDE offers no install

In `brd mcp`, `agent_cli_status` MUST answer `installable: false`: the desktop app installs `brd` and the app file of
the IDE. The page then shows no row `brd` and no row IntelliJ IDEA on Settings → Tools, and no install of the Agent CLI
on Agents or in the hint of the agents; it only words the status.

#### Scenario: Settings → Tools in the IDE
- **WHEN** the user opens Settings → Tools in the IDE
- **THEN** it shows the command-line tools without `brd` and IntelliJ IDEA
  [@test] crates/mcp-host/src/hosts_test.rs (`the_hosts_without_a_place_in_the_ide_do_nothing`)
  [@test] tests/e2e/air/air-page.spec.ts (`Settings → Tools in the IDE shows the command-line tools but no install of brd or of IntelliJ IDEA, which the desktop app installs`)
