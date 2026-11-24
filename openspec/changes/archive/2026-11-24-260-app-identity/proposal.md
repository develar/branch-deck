# Proposal

## Why

The bridge `branch-deck ide` and `brd` named the data directory `org.develar.branch-deck` as a constant, while the app
takes the identifier it was built with. The socket key of the backend is a hash of the data directory and the
executable, so a bridge of a build with another identifier (a check copy) computed another key: it did not see the
backend of its own app, started it, did not find it, and opened its fallback on the data directory of the user's
installed app.

## What Changes

- The identifier of the app is compiled into the binary (`app_settings::identity`): the `identifier` of
  `src-tauri/tauri.conf.json` under the overlay of `tauri build --config` (`TAURI_CONFIG`), as tauri-build reads it.
- The app's backend keys its socket, the bridge serves and `brd` reads the data directory of that one function.

Capabilities: `repository/ide-embedding`.
