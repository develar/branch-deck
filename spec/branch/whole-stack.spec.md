---
name: Whole Stack
description: Requirements for the whole stack of stacked branches that fork, as one more virtual branch `{prefix}/stack/<root>` that the sync builds of all their commits in the order of the history, without notes or branch events, remembered by its commits, its state (built, blocked, conflict, stale) and remote status reported per sync, deleted once the stack no longer forks unless it is on the remote, never pushed unless built, and landed in one robot by `bd safe-push --whole-stack` from its own pull request, the robots of the stack and of its branches waiting for each other.
style: plain-1
targets:
  - ../../crates/sync-core/src/stack_branch.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/branch_processor.rs
  - ../../crates/sync-types/src/stack.rs
  - ../../crates/sync-types/src/lib.rs
  - ../../crates/git-ops/src/branch_names.rs
  - ../../crates/git-ops/src/tree_id_cache.rs
  - ../../crates/object-cache/src/stack_branch.rs
  - ../../crates/branch-integration/src/common.rs
  - ../../crates/bd-cli/src/stack_summary.rs
  - ../../crates/bd-cli/src/status.rs
  - ../../crates/bd-cli/src/sync_command.rs
  - ../../crates/bd-cli/src/safe_push_command.rs
  - ../../crates/bd-cli/src/push_command.rs
  - ../../crates/bd-cli/src/push_hold.rs
  - ../../crates/bd-cli/src/robot_command.rs
  - ../../crates/patronus/src/service.rs
  - ../../crates/sync-core/src/patronus_start.rs
  - ../../crates/sync-core/src/pull_requests.rs
  - ../../crates/sync-core/src/pull_request_create.rs
  - ../../crates/sync-core/src/review_readiness.rs
  - ../../crates/sync-core/src/stack_pull_request.rs
  - ../../crates/sync-types/src/patronus.rs
  - ../../crates/sync-types/src/pull_request.rs
  - ../../src-tauri/src/commands/patronus.rs
  - ../../src/stores/branch-sync.ts
  - ../../src/stores/repository.ts
  - ../../src/lib/patronus.ts
---

# Whole Stack

Status: In progress (the stack branch is built and `bd` lands it in one robot from its own pull request; the UI follows)
Date: 2026-10-18

## Summary

A stack lands into master in pieces: the Safe Push of a branch lands it with the branches it is built on, in one robot
([Landing Work into Master](../../docs/design/landing.md)). Side branches never land with the main line that way. So a
stack that forks is also one more virtual branch, `{prefix}/stack/<root>`: every commit of its branches, in the order
of the local history, on the base of the sync. Its own pull request into master and one robot land the whole stack;
the sync keeps it up to date like any virtual branch, and says when it can't.

## Goals

- One branch with everything a stack holds, so the whole stack lands in one robot, its approvals in one pull request.
- Built as a flat virtual branch of those commits: no conflict that the local history doesn't have, no rewrite of all of
  it when a side branch gets a commit.
- No cost while nothing changed, and never a stale one pushed.

## Non-goals

- A stack branch of a straight chain: its leaf is the whole stack.
- Mapping notes of the stack branch's copies, or a row of it among the branches; so no commit map of a failed robot
  of the whole stack yet.
- The app's UI of the whole stack, its robot, pull request and Safe Push form: [Landing](../../docs/design/landing.md),
  steps 8–9. The app reads the facts below already.

## Requirements

### The stack branch

- With stacked branches on, every stack that forks (a branch of it has two children or more in it) must also be built
  as the virtual branch `{prefix}/stack/<root>` (`to_stack_branch_name`), stacks inside an issue family included; a
  straight chain has none. The stack is the root and the branches built on it (`effective_base`, the parent that
  blocks a blocked branch, the declared parent of a branch with a stack problem); a branch whose parents lead into a
  cycle is in no stack.
  [@test] ../../crates/sync-core/src/stack_branch_test.rs (`a_stack_forks_where_a_branch_has_two_children_and_takes_its_commits_in_history_order`, `a_fork_below_the_root_and_two_stacks`, `a_straight_chain_and_a_lone_branch_have_none`, `a_member_with_a_problem_blocks_the_stack_and_a_cycle_is_in_none`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_straight_stack_has_none`)
  [@test] ../../crates/git-ops/src/branch_names_test.rs (`the_stack_branch_of_a_root_is_in_its_own_namespace`)
- It must hold the original commits of all its branches in the order of the checked out branch's history, on the base
  of the sync, copied as a flat virtual branch of those commits is (the prefix stripped, `branch-deck` the committer,
  the deterministic dates), so a copy on the same parent is the same commit as the flat build's.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_forking_stack_builds_its_commits_in_history_order_as_one_branch`)
- The build must write no mapping note (the note of an original names the copy of its own branch) and send no event
  of a branch row; one `StackBranchUpdate` per stack branch reports it, after the `BranchesGrouped` that names its root
  (`stack_branches`).
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`the_stack_branch_writes_no_mapping_notes_and_sends_no_branch_events`)
- A stack with the same commits on the same base must cost no git process: the outcome of a build is remembered by a
  hash of the base and the original commit ids, in the memory of the process and in the object cache
  (`stack_branch`). Without either it is built again, to the same commit, and its ref doesn't move.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`an_unchanged_stack_reuses_its_stack_branch_without_a_git_process`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_first_sync_of_a_forking_stack`, `an_unchanged_sync_of_a_forking_stack`)
  [@test] ../../crates/object-cache/src/cache_test.rs (`stack_builds_are_read_by_their_version`)
- Its state must be one of: `Built`, the ref is the build of the stack's commits now; `Conflict`, a commit conflicts
  with the commits of the stack before it, named with its files; `Blocked`, a member has a stack problem, is blocked,
  conflicts or failed, named with why (the first in stack order, a failure before a block), while the stack branch's
  own build doesn't conflict; `Stale`, not rebuilt for another reason, said in words. In every state but `Built` the ref
  keeps its tip, and nothing may push it.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_conflict_keeps_the_old_ref_stale_and_says_why`, `a_member_that_fails_blocks_the_stack_branch`)
  [@test] ../../crates/sync-core/src/stack_branch_test.rs (`a_conflicting_build_names_the_commit_and_keeps_the_ref`)
- The ref must move with compare-and-swap on the tip the sync listed; another process's move keeps its tip, `Stale`.
  [@test] ../../crates/sync-core/src/stack_branch_test.rs (`another_process_moving_the_stack_branch_keeps_its_tip_stale`)
- Its remote status must be by content, as a virtual branch's, against its remote branch on `origin`.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_stack_that_no_longer_forks_loses_its_stack_branch_unless_it_is_on_the_remote`)
- A stack branch whose stack no longer forks (or whose root is gone, or with stacked branches off) must be deleted in
  one transaction with compare-and-swap, but only while it has no remote branch and no worktree has it checked out;
  one with a remote branch stays, `Stale` without members, for the user to decide on.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_stack_that_no_longer_forks_loses_its_stack_branch_unless_it_is_on_the_remote`)
- The archive step and the integration detection must leave the stack branches out: never archived, and a stack tip
  never names the detection note of an archived branch.
  [@test] ../../crates/integration-detection-tests/src/archive_step_tests.rs (`branches_are_classified_by_their_namespace`)
- `bd sync` and `bd status` must list them as `stacks`: `root`, `branch`, `state` (`built`, `blocked`, `conflict`,
  `stale`), `tip`, `member` and `reason` of a block, `conflict`, `error`, `commits`, `members`, and with `bd sync` the
  `remote` summary. `bd status` reads the state without a sync and without a git process: blocked by the placement,
  else the remembered build of the current commits when the ref is at it, else stale.
  [@test] ../../crates/bd-cli/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_status_of_a_forking_stack`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)
- The app must keep the stack branches of the last sync by root (`useStackBranch`, `getStackBranch`; the full name by
  `getFullStackBranchName`): a grouping keeps the ones it names as they were until their update and drops the others.
  [@test] ../../src/stores/branch-sync.test.ts (`keeps the stack branch of each stack the sync reports, with its state and remote status`, `keeps what the last sync reported until the next one updates it, and drops a stack branch the grouping no longer announces`, `forgets the stack branches with the repository`)

### The Safe Push of the whole stack

- `bd safe-push <branch> --whole-stack` must land the whole stack of the branch in one robot from its stack branch,
  the only branch it pushes; a straight chain from its last branch; refused unless the stack branch is `built`
  (`conflict`, `stack_not_built`, exit 1), and both ways while a robot of the stack or of a branch of it runs
  ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)). `bd push <root> --with-children` must take the stack branch
  along only when the sync built it; `bd robot --whole-stack` must print its robot.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`, `the_whole_stack_of_a_straight_chain_lands_from_its_last_branch`, `a_whole_stack_that_conflicts_is_refused_with_the_conflict`)
  [@test] ../../crates/bd-cli/src/push_test.rs (`with_children_pushes_the_stack_branch_the_sync_built`)
  [@test] ../../crates/bd-cli/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)
- A robot started on a stack branch must name the root of its stack (`PatronusRobot::stack_root`) and no branch, and
  must belong to no branch, not even by its revision, in `bd` (`find_branch_robot`) as in the app
  (`findBranchRobot`); `find_stack_robot` finds the robot of the whole stack.
  [@test] ../../crates/patronus/src/service_test.rs (`a_robot_of_the_stack_branch_names_the_root_of_its_stack_and_no_branch`)
  [@test] ../../crates/sync-core/src/patronus_start_test.rs (`the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch`, `the_whole_stack_and_its_branches_wait_for_each_other`)
  [@test] ../../src/lib/patronus.test.ts (`never takes the robot of the whole stack, which runs on its stack branch, for a branch's`)
- The pull request of a stack branch must be the stack's, by its root (`PullRequestsResult::stack_pull_requests`),
  never a branch's; one that GitHub says exists must be found by its head.
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`the_pull_request_of_a_stack_branch_is_the_stacks_by_its_root`)
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`the_pull_request_of_the_whole_stack_is_its_stack_branchs_and_the_start_comments_on_it`)
- Its text must be the stack branch's own description, else the root's title with the count of the branches built on
  it and a section per branch, its description or the subjects of its commits, from what the sync read, cut to the
  65,536 characters of a body and the 256 of a title GitHub takes, naming the branches left out.
  [@test] ../../crates/sync-core/src/stack_pull_request_test.rs (`the_pull_request_of_the_whole_stack_names_each_member_by_its_description_else_its_commits`, `a_text_too_long_for_github_is_cut_and_names_the_members_left_out`)
- The readiness of the whole stack must ask Patronus about its stack branch (`ReviewReadinessBranch::source`, a stack
  branch of the prefix, else refused), the answer under the key the caller gives; the start of the app takes `stack`
  (`StartPatronusRobotParams::stack`) to start the robot from the stack branch.
  [@test] ../../crates/sync-core/src/review_readiness_test.rs (`the_whole_stack_is_asked_as_its_stack_branch_under_the_key_of_the_caller`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/stack_sync_test.rs`, `stack_branch_test.rs`, `process_budget_test.rs`,
  `patronus_start_test.rs`, `stack_pull_request_test.rs`; `crates/bd-cli/src/status_test.rs`, `safe_push_test.rs`,
  `push_test.rs`, `robot_test.rs`.
- `pnpm test:unit`: `src/stores/branch-sync.test.ts`, `src/lib/patronus.test.ts`.
- `pnpm perf:monorepo`: the full and the unchanged `bd sync` of the pinned monorepo clone, one stack that forks.

## References

- [Landing Work into Master](../../docs/design/landing.md): why the whole stack is one more virtual branch
- [Git Operations](../../docs/design/git-operations.md): "Stack branch", the build and the ref rules
- [Stacked Branches](stacked-branches.spec.md): how branches stack
- [Stack Island](stack-island.spec.md): the header the whole stack's Safe Push goes to
