## MODIFIED Requirements

### Requirement: Pages: Tools has `brd` and the CLIs

Tools SHALL show one list, its rows apart by a divider: first `brd`, whose skill coding agents read: its path with a
copy button and the skill directories of the agents ("Agent skill branch-deck · ~/.claude/skills, ~/.agents/skills",
the files in its tooltip), paths under the home directory as `~` (`agent_cli_status`, asked on each open and focus;
the window has `core:path`), or "The agent skill is not installed" with Install Agent CLI, and a link to Agents, whose
rules the agents ask brd for; then IntelliJ IDEA ([IDE](../ide-embedding/spec.md)), "Not installed" while it is not;
then the `teamcity` and `youtrack` CLIs, their version, sign-ins and the action each needs
([Command-Line Tools](../cli-tools/spec.md)).

#### Scenario: The agent skill is not installed
- **GIVEN** the agent skill is not installed (`agent_cli_status`)
- **WHEN** the user clicks Install Agent CLI on Tools
- **THEN** it runs the install of Help → Install Agent CLI…, whose toast has no action here
- **AND** Tools then names `brd`: its path with a copy button and the skill directories of the agents, paths under the home directory as `~`
- **AND** its link Agents shows the page of Agents
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should install the agent skill from the page, which Tools then names`)

#### Scenario: The status of the tools is asked again
- **WHEN** the window opens again or gets the focus
- **THEN** `agent_cli_status` is asked again

#### Scenario: The CLIs
- **WHEN** the user opens Tools
- **THEN** after `brd` and IntelliJ IDEA it shows the `teamcity` and `youtrack` CLIs, their version, sign-ins and the action each needs
- **AND** the sign-ins of the CLIs are their own, not those of Accounts
