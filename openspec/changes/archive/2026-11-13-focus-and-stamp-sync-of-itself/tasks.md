# Tasks

- [x] 1.1 The mark of a sync in `crates/sync-core/src/synced_mark.rs`, written by `sync_branches`, its test in
  `synced_mark_test.rs`; read by `SessionHost::synced_before` of the app and of the E2E server.
- [x] 1.2 One predicate in the actor (`synced`, `syncs_itself`), the read again of the host for a focus or a stamp, the
  argument of `SyncScheduler::request`; the table tests, the session test, the stamp test of `registry_watch`.
- [x] 1.3 The E2E tests of a focus before the first sync and after the user's Sync in
  `tests/e2e/onboarding/onboarding.spec.ts`.
- [x] 1.4 The specs, the docs of `SyncReason` and `SyncOutcome`, `docs/design/git-operations.md` and
  `docs/design/rust-domain-authority.md`.
