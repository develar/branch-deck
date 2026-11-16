# Add Repository footer

## Why

The adjacent folder icon does not make it clear that opening a repository adds it to the selector. A named action inside the popup makes that path discoverable, including before any repository has been opened.

## What Changes

Add a compact, fixed Add Repository… footer to the repository popup, preserve the existing folder button and empty-state message, and support pointer and keyboard activation through the current browse flow.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: Add a repository from a fixed popup footer and retain the empty-selector explanation above it.

## Impact

The repository selector, shared SelectContent optional footer, E2E coverage and snapshots, and UI theme documentation. No backend command or persisted setting changes.
