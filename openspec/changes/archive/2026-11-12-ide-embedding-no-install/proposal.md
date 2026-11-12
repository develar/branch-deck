# Proposal

## Why

`brd mcp` installs nothing: the desktop app installs `brd` and the app file of the IDE. Settings → Tools of the page in
the IDE still showed both rows with an Install that fails, and Agents and the hint of the agents offered the install of
the Agent CLI.

## What Changes

- `agent_cli_status` answers `installable`, false in `brd mcp`, true in the app and the e2e server.
- The page shows no row `brd` and no row IntelliJ IDEA on Tools where the host installs nothing, and no install of the
  Agent CLI on Agents or in the hint of the agents.
