# Tasks

- [ ] The repository channels of `crates/backend-host` over `repository_model::session::Sessions`: the URI per path,
  the snapshot, `have` and `held`, the catch-up, `viewPatched`, the resync, `subscriptionEnded`
- [ ] The corpus `docs/design/backend-protocol/corpus/repository-channel.ndjson` runs on the host
  (`crates/backend-host/src/corpus_test.rs`)
- [ ] Two connections on one repository get the same actions, and the end of one leaves the other
  (`crates/backend-host/src/channel_test.rs`)
- [ ] The subscription per window, its demand, the frame per window, the stale cap
- [ ] The test links of every scenario, `pnpm lint:specs`
