# Tasks

- [ ] The connection of `crates/backend-host`: the frames, `initialize`, `ping`, the errors with `data.reason`
- [ ] The corpus `docs/design/backend-protocol/corpus/connection.ndjson` runs on the host
  (`crates/backend-host/src/corpus_test.rs`) and each of its lines passes `backend.schema.json`
- [ ] The socket: the directory of the key, its mode, the peer uid and SID, `backend.lock`, the stale socket,
  `x-branch-deck/showMain` (`crates/backend-host/src/socket_test.rs`)
- [ ] Another build: `otherBuild`, the exit of a stale backend without a window, the fallback with one; stale is the
  identity of the executable on disk (device, inode, size, mtime) against the one at the start
- [ ] The events of app-api to every connection, a page's within its connection; `x-branch-deck/claimNews`
- [ ] The test links of every scenario, `pnpm lint:specs`
