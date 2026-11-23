# Proposal

## Why

The IDE names the root of a project by `Path.toUri`, which ends the URI of a directory with `/`. `brd mcp` kept that
separator, so the IDE spelled the repository `/x/repo/` and the desktop app `/x/repo`: the two opened two sessions of
one repository. The shim of the page refused the home directory, so the page showed the whole path, not `~`. With one
root, the selector offered a menu with nothing to pick.

## What Changes

- `brd mcp` takes the path of a root without a trailing separator, but the root of a file system.
- `bd_bootstrap` gives the home directory of `brd mcp` (`homeDir`), and the shim answers `homeDir` of Tauri with it.
- In the IDE with one root, the repository selector shows the path as text, without its menu.

Capabilities: `repository/ide-embedding`.
