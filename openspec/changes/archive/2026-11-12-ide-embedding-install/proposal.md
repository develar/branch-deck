## Why

The IDE reads `~/.air/apps/branch-deck.yaml`, and nothing wrote it but a hand. The plan puts the install beside the
install of the agent CLI: the binary of the app knows its own path, and a user of the desktop app finds it in Settings.

## What Changes

- Settings → Tools gets the row IntelliJ IDEA after `brd`: Install writes the app file of the IDE and its icons for this
  binary, Remove deletes them; the row names the file, and says when another Branch Deck wrote it.
- The app file is the example of the contract with the command of this binary: the app binary with `cli mcp`, `brd`
  (`brd.exe` on Windows) with `mcp`.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `repository/ide-embedding`: one requirement added, the install.
- `repository/settings`: Tools names IntelliJ IDEA after `brd`.

## Impact

- `crates/bd-cli/src/install.rs` (`install_ide_app`, `uninstall_ide_app`, `ide_app_status`), `crates/bd-core`
  (`cli_subcommand`), `crates/app-api` (`install_ide_app`, `remove_ide_app`, `AgentCliStatus.ide_app`), the hosts of
  `src-tauri`, `mcp-host` and `test-server`, `src/components/settings/ToolsPage.tsx`.
