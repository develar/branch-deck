# Tasks

- [x] The messages of `bd-remote`, `bd-update` and `bd-lease` that name a command with its arguments build it from
  `RepoContext::bd` or `in_worktree`: `approvals_missing`, `target_conflict`, `push_dismisses_approvals`, the
  `push_refused` and `lease_rejected` of a stack branch, `alone`, a `--robot` of another branch, `no_pull_request`,
  `unsupported_conflict`, `no_home_branch`
- [x] The exact message of `approvals_missing` without a GitHub repository; the tests of the others assert the command
  with its `-C`
