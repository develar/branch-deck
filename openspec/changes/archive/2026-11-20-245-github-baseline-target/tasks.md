## 1. The target

- [x] 1.1 `target_branch_of` of the upstream, the fetch URL of its remote and the GitHub repository of the branches; `Remotes::fetch_urls`; `no_target_message`
- [x] 1.2 `SyncReport::target_branch`, `BranchTable::target_branch` (of the frame's header on a warm start); the app's start takes it from the table
- [x] 1.3 `brd request-review`, `safe-push`, `land` and `robot-stats` take the target of their sync or their baseline
- [x] 1.4 Tests: the rule, the read of an action, the sync of a repository on Space with the remote of its GitHub repository, the start of the app, the review readiness
