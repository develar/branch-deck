## Why

The contract of `repository/ide-embedding` landed ahead of its code. `brd mcp` now hosts app-api for the IDE
(`crates/mcp-host`, `app_api::dispatch`), and its code answers what the contract left open: how the action Sync
Virtual Branches reaches brd, how the page reaches the Dock and the notifications of the IDE, what a second open of a
shown view does, what the demand of a view names, and what `bd_bootstrap` gives.

## What Changes

- The action of the app file runs the tool `sync`, which no list of tools shows.
- The plugin calls of Tauri the page makes for the Dock and the notifications reach the IDE through `bd_invoke`.
- A second open of a shown view hands it its data (`init-data-updated`) and its title.
- `set_demand` of a view names the subscription of its mirror.
- A change of the roots of the project tells the main view.
- `bd_bootstrap` gives the settings snapshot, the init data of the view and the title bar of an IDE.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `repository/ide-embedding`: six requirements added; the scenarios that the tests of `mcp-host` and `test-server` prove
  get their links.

## Impact

- `crates/app-api` (`dispatch`, the services and the warm start shared with the app), `crates/mcp-host`,
  `crates/test-server` (`invoke.rs` on `dispatch`, `mcp_socket.rs`), `crates/bd-cli` (`brd mcp`), `src-tauri`.
