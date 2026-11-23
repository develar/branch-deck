# Tasks

- [x] `SyncScheduler::after_writes` and `Sessions::after_writes`: the run that serves the last write not yet ended,
      in flight, behind a focus check or as a follow-up; `Skipped` at once without one
- [x] `SyncWait::Writes` for the start; `SyncWait::Running` waits for the sync of the writes too
- [x] The start fetches its target once, beside its wait (`StartFetch`), and its checks replay onto it
- [x] `NotRun::BranchesMoved` of the witness (local branches alone), `run_frame::catch_up`, one plan decided again per
      operation (`execute_land`, `run_planned_in` of a push, a pull request intent, a restart)
- [x] The start waits for the facts the click had answered and a sync made the session read again
      (`SessionStart::answered_before`), and the session asks the readiness of the subject of a start that runs
      (`derive::wanted_readiness`)
- [x] Tests: the scheduler table, the prologue, a start during the sync of a write and one whose branch another
      process rebuilt, a push of such a branch, the witness signals, the E2E of a start pressed during Update Project
- [x] The headers of the modules, docs/design/git-operations.md, docs/design/operations.md,
      docs/design/rust-domain-authority.md
