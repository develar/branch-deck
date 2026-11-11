## Why

Branch Deck runs as a Tauri window, and its users work in IntelliJ IDEA. The same page and the same Rust backend must
run inside the IDE, with the windows kept as windows and no fork of the page. Air hosts such an app over its protocol
`x-air-app` (the decision record of idea-1 of 2026-11-11, the capability `air-apps-host`).

## What Changes

- A new capability `repository/ide-embedding`: what `brd mcp` serves, the tools `bd_invoke` and `bd_bootstrap`, the
  mirror of the view, the events, the windows as views of the IDE, the roots, the page of the Air build and the copy
  of the contract.
- The design doc `docs/design/ide-embedding.md` and the copy of the contract of Air in
  `docs/design/ide-embedding/protocol/`, with the corpus file of the app link of Branch Deck.
- This change lands the contract ahead of the code, so the spec is a Draft and its scenarios have no test links yet.
  The commits of the code add them.

## Capabilities

### New Capabilities

- `repository/ide-embedding`

### Modified Capabilities

- None. The settings of two processes stay in `repository/settings`, which the change of their code amends.

## Impact

- Later: `crates/app-api` (`dispatch`), a new crate `crates/mcp-host`, `crates/test-server`, `crates/app-settings`,
  `crates/bd-cli` (`brd mcp`, the install of the IDE), `src/platform/air/`, `src/air-main.ts`, `vite.config.ts`.
- Out of scope: one process for every project of a user (P2), a tool for an agent, a project file of the app.
