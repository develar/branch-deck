# Proposal

## Why

Issue #245: a repository with `origin` on Space and a second remote `github` of the GitHub repository of the branches
(the one the Space settings name, where the pull requests go), whose checked out branch tracks `github/master`, could
not create a pull request: "The baseline github/master is not a branch of origin". The target of a start (the branch
the robots push into and a pull request goes into) was only a branch of `origin`, so every start that needs it failed
on that baseline: Create Pull Request, Request Review, a Safe Push and a Dry Run, the restart of a robot, the review
readiness, the merge profiles, `brd robot-stats`. The GitHub repository holds the branches of origin under their names
and becomes the home of the repository: its `master` is the `master` of origin.

## What Changes

- The target is the branch the baseline tracks on `origin` or on the remote that fetches from the GitHub repository of
  the branches (`sync_core::patronus::target_branch_of`, of the fetch URLs of `remote -v` and the GitHub repository the
  sync resolves: no process more). A fork stays without one.
- The sync finds it once (`SyncReport::target_branch`, `BranchesGrouped.target_branch`, `BranchTable::target_branch`);
  a start of the app takes it from the table of its session and runs no git for it, `brd` from its sync.
- One refusal for a baseline without a target, which names the GitHub repository the host knows
  (`no_target_message`).

Capabilities: `branch/patronus`, `branch/pull-requests`, `repository/reviews`, `repository/review-requests`,
`repository/safe-push-statistics`.
