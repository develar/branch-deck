# Proposal

## Why

Branch Deck in an IDE (`brd mcp`, docs/design/ide-embedding.md) runs a backend of its own beside the desktop app, on
the same app data directory (`org.develar.branch-deck`), and so a second `SettingsService`. Each writer saved its whole
copy in memory over `settings.json` 500 ms after a change: the save of one dropped what the other had saved since it
loaded, and neither showed a change of the other until a restart.

## What Changes

- A save takes the lock of the directory (`settings.lock`), reads the file as it is, puts on top only the keys and the
  fields this process changed and has not saved, and renames that over the file (`app_settings::merge`).
- What the file has beside those is the other process's change: the save takes it into memory, and so does a read the
  watch of the directory asks for (`app_settings::watch`, `notify`). Each such key is a change of its own (the next
  version, origin `other-process`), which `changes_since` answers and `on_external_change` tells; the app sends it to
  the windows as `settings_changed` (`src-tauri/src/settings.rs`).
- `SettingsService` stays the one writer of a process.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `repository/settings`: Writes through the backend: two writers on one directory keep each other's changes, and a
  change another process saved reaches every window.

## Impact

- `crates/app-settings/` (`service.rs`, `file.rs`, the new `merge.rs` and `watch.rs`, the dependency on `notify`),
  `src-tauri/src/settings.rs`, `src-tauri/src/lib.rs`, `docs/design/distributed-settings.md`.
- A host of app-api other than the app (`mcp-host`, the E2E server) sends the changes of another process to its pages
  by `SettingsService::on_external_change` the same way.
