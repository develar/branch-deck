# Tasks

- [ ] The root commands of `crates/backend-host` over `app_api::dispatch`: `{value}`, `{bytes}`, `{error}`,
  `callEvent`, `x-branch-deck/cancel`, `channelCommand`
- [ ] The version fence (`crates/backend-host/src/fence_test.rs`): the answer after V, behind a resync, at once without
  a subscription
- [ ] `WindowId` of app-api carries the connection; the end of a connection forgets only its windows; the reads one per
  window (`remote/mod.rs` branch names, `ConflictFixSearch`)
- [ ] The Desktop by the connection, the host requests and the capabilities
- [ ] The corpus `command-stream.ndjson`, `host-requests.ndjson` and `errors.ndjson` of
  `docs/design/backend-protocol/corpus/` run on the host (`crates/backend-host/src/corpus_test.rs`)
- [ ] The test links of every scenario, `pnpm lint:specs`
