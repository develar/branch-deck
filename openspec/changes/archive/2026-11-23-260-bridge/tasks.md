# Tasks

- [x] `crates/ide-host`: the bridge, a client of `backend-host` (`backend.rs`), its start (`start.rs`), its fallback and
  the way back (`fallback.rs`), the mirrors over the repository channels, the windows of its connection, the host
  requests `openWindow` and `notify`, the claim of a news (`notices.rs`), the settings as the backend tells them
- [x] `branch-deck ide` in `src-tauri/src/main.rs`; `brd mcp` its alias
- [x] The app file names the app binary with `ide` on every platform (`crates/bd-cli/src/install.rs`)
- [x] The corpus of the app link through the socket of a backend and in the fallback (`mcp_corpus_test.rs`), the start
  decisions, the ends, the way back
