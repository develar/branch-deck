## 1. The refusal

- [x] 1.1 `git_ops::commit_list::UnrelatedHistory` at the first root of `walk_commits_ahead` and of `History::read_ahead_of`, with its shallow case
- [x] 1.2 `SyncDeferred::UnrelatedHistory` and `ErrorCode::UnrelatedHistory` with its details; the walk before every event and the update check
- [x] 1.3 The classifiers of the app (`api_error`), of `brd` (`CliError`, `regroup_error`, `RegroupError::code`) and the deferral of the session
- [x] 1.4 The page: the toast and the alert of `unrelated_history`, `warning`, above the table too
- [x] 1.5 Tests: git-ops, the sync, the process budget, the session, `brd sync` and `brd reword`, the store, the e2e of a forced update to an unrelated history
