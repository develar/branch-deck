## ADDED Requirements

### Requirement: Pages: Tools in the IDE installs nothing

Where the host installs nothing (`installable` of `agent_cli_status` false: the page in the IDE, whose desktop app
installs), Tools SHALL show neither `brd` nor IntelliJ IDEA, and Agents and the hint of the agents offer no install.

#### Scenario: Tools in the IDE
- **WHEN** the user opens Settings → Tools in the IDE
- **THEN** it shows the `teamcity` and `youtrack` CLIs, and no row `brd` or IntelliJ IDEA
  [@test] tests/e2e/air/air-page.spec.ts (`Settings → Tools in the IDE shows the command-line tools but no install of brd or of IntelliJ IDEA, which the desktop app installs`)
