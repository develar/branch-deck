---
name: Whole Stack
description: Requirements for the whole stack of stacked branches that fork, as one more virtual branch `{prefix}/stack/<root>` that the sync builds of all their commits in the order of the history, without notes or branch events, remembered by its commits, its state (built, blocked, conflict, stale) and remote status reported per sync, deleted once the stack no longer forks unless it is on the remote, never pushed unless built, taken along by Push with Children of the root once it is on the remote or its pull request is open, and landed in one robot by `brd safe-push --whole-stack` from its own pull request, the robots of the stack and of its branches waiting for each other; in the app the whole stack is an object of its own, "the whole air stack": its robot (polled, told, holding every branch, its branches "Landed with the whole air stack" after its merge until the update, its prompt with the commits of every branch), its pull request (created, opened, closed by content after its merge, one that no longer matches it asked before it is closed), its review and one list of its actions in the island's menu and the root's; and its Safe Push and Dry Run in the app, from the island's header (its slot, its "⋯", S) or the root's menu of a small stack, its popover saying what lands, only its stack branch pushed, its readiness and approval news of its own pull request, "In #12" for a branch without one, a stack branch not built disabling it with the way to land it in pieces.
style: plain-1
targets:
  - crates/sync-core/src/stack_branch.rs
  - crates/sync-core/src/sync.rs
  - crates/sync-core/src/branch_processor.rs
  - crates/domain-types/src/stack.rs
  - crates/domain-types/src/lib.rs
  - crates/domain-types/src/branch_names.rs
  - crates/git-ops/src/tree_id_cache.rs
  - crates/object-cache/src/stack_branch.rs
  - crates/branch-integration/src/common.rs
  - crates/bd-core/src/stack_summary.rs
  - crates/domain-policy/src/stack.rs
  - crates/domain-policy/src/push.rs
  - crates/repository-model/src/derive.rs
  - crates/bd-remote/src/status.rs
  - crates/bd-core/src/sync_command.rs
  - crates/bd-remote/src/safe_push_command.rs
  - crates/bd-remote/src/push_command.rs
  - crates/bd-remote/src/push_refusals.rs
  - crates/domain-policy/src/holds.rs
  - crates/domain-policy/src/land.rs
  - crates/domain-policy/src/actions.rs
  - crates/bd-remote/src/robot_command.rs
  - crates/patronus/src/service.rs
  - crates/sync-core/src/patronus_start.rs
  - crates/domain-policy/src/robot_match.rs
  - crates/domain-policy/src/attention.rs
  - crates/remote-facts/src/pull_requests.rs
  - crates/remote-facts/src/pull_request_create.rs
  - crates/remote-facts/src/review_readiness.rs
  - crates/remote-facts/src/stack_pull_request.rs
  - crates/domain-types/src/patronus.rs
  - crates/domain-types/src/pull_request.rs
  - crates/app-api/src/remote/patronus.rs
  - src/stores/branch-sync.ts
  - src/stores/repository.ts
  - src/lib/patronus.ts
  - src/lib/whole-stack.ts
  - src/hooks/useWholeStackActions.ts
  - src/stores/patronus.ts
  - src/stores/pull-requests.ts
  - crates/operations/src/close.rs
  - crates/domain-policy/src/landed.rs
  - src/lib/robot-run.ts
  - src/presenters/actions.ts
  - src/lib/patronus-failing.ts
  - src/lib/robot-overview.ts
  - src/lib/robot-prompt.ts
  - src/lib/robot-prompt-copy.ts
  - src/lib/push-run.ts
  - src/lib/git/push-toast.tsx
  - src/lib/stack.ts
  - src/hooks/useBranchContextActions.ts
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - src/lib/pull-request-dialog.ts
  - src/lib/pull-request-run.ts
  - src/hooks/useCreatePullRequestContext.ts
  - src/components/branch/CreatePullRequestDialog.tsx
  - src/components/branch/InlineCloseStackPullRequest.tsx
  - src/components/branch/OrphanedStackPullRequest.tsx
  - src/components/branch/StackIslandCard.tsx
  - src/lib/stack-state.ts
  - src/components/branch/LandingSpanLabel.tsx
  - src/components/branch/LandedChip.tsx
  - src/components/repository/RobotRows.tsx
  - src/lib/review-window.ts
  - src/hooks/useStackNextStep.ts
  - crates/domain-policy/src/next_step.rs
  - crates/domain-policy/src/landing_node.rs
  - src/presenters/next-step.ts
  - src/hooks/useLandingNode.ts
  - crates/sync-core/src/robot_landing.rs
  - src/hooks/useBranchNextStep.ts
  - src/hooks/useSafePush.ts
  - crates/operations/src/land_execute.rs
  - crates/operations/src/land_session.rs
  - src/presenters/land.ts
  - src/components/branch/SafePushPopover.tsx
  - src/components/branch/SafePushButton.tsx
  - src/components/branch/WholeStackSlot.tsx
  - src/components/branch/NextStepSlot.tsx
  - src/components/branch/RequestReviewForm.tsx
  - src/stores/review-readiness.ts
  - src/lib/approval-news.ts
  - crates/test-utils/src/repo_template.rs
  - crates/sync-core/src/push.rs
  - crates/sync-core/src/robot_commits.rs
  - crates/sync-core/src/review_target.rs
  - crates/app-api/src/remote/pull_requests.rs
  - src/components/branch/BranchPushButton.tsx
  - src/components/branch/BranchActions.tsx
  - crates/operations/src/land.rs
  - crates/operations/src/robot.rs
  - crates/operations/src/push.rs
  - crates/operations/src/pull_request.rs
  - src/hooks/useOrphanedStackPullRequest.ts
---

# Whole Stack

Status: Implemented
Date: 2026-10-18

## Purpose

A stack lands into master in pieces: the Safe Push of a branch lands it with the branches it is built on, in one robot
([Landing Work into Master](../../../../docs/design/landing.md)). Side branches never land with the main line that way. So a
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
- Mapping notes of the stack branch's copies, or a row of it among the branches: the commit map of its robot matches
  the commits by their identity.
- A Safe Push of the whole stack without review: a profile that skips the code-owner review is a choice of the
  popover, but the "Create pull request" dialog of the stack offers no start without review.
- Closing a pull request of the whole stack by itself: one that no longer matches the stack is the user's to close.

## Requirements

### Requirement: The stack branch: A stack that forks is also built as its stack branch

Every stack that forks (a branch of it has two children or more in it) MUST also be built as the virtual branch
`{prefix}/stack/<root>` (`to_stack_branch_name`), stacks inside an issue family included; a straight chain MUST have
none.

#### Scenario: A stack forks where a branch has two children
- **WHEN** a branch of a stack has two children or more in it, at the root or below it
- **THEN** the sync also builds the stack as the virtual branch `{prefix}/stack/<root>`, one per stack that forks
  [@test] crates/sync-core/src/stack_branch_test.rs (`a_stack_forks_where_a_branch_has_two_children_and_takes_its_commits_in_history_order`, `a_fork_below_the_root_and_two_stacks`)

#### Scenario: The name of the stack branch
- **WHEN** the stack branch of a root is named (`to_stack_branch_name`)
- **THEN** it is `{prefix}/stack/<root>`, in a namespace of its own
  [@test] crates/domain-types/src/branch_names_test.rs (`the_stack_branch_of_a_root_is_in_its_own_namespace`)

#### Scenario: A stack inside an issue family
- **WHEN** a stack that forks lies inside an issue family
- **THEN** it is built as its stack branch too

#### Scenario: A straight chain and a lone branch
- **WHEN** a stack is a straight chain, or a branch stands alone
- **THEN** it has no stack branch
  [@test] crates/sync-core/src/stack_branch_test.rs (`a_straight_chain_and_a_lone_branch_have_none`)
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_straight_stack_has_none`)

### Requirement: The stack branch: The stack is the root and the branches built on it

The stack MUST be the root and the branches built on it (`effective_base`, the parent that blocks a blocked branch,
the declared parent of a branch with a stack problem); a branch whose parents lead into a cycle MUST be in no stack.

#### Scenario: A member with a problem and a cycle
- **GIVEN** a branch of the stack has a stack problem, and other branches have parents that lead into a cycle
- **WHEN** the stacks are found
- **THEN** the branch with the problem is in the stack of its declared parent
- **AND** a branch whose parents lead into a cycle is in no stack
  [@test] crates/sync-core/src/stack_branch_test.rs (`a_member_with_a_problem_blocks_the_stack_and_a_cycle_is_in_none`)

#### Scenario: A blocked branch
- **GIVEN** a branch is blocked
- **WHEN** the stacks are found
- **THEN** it is in the stack of the parent that blocks it

### Requirement: The stack branch: It holds every commit of the stack as a flat build does

It MUST hold the original commits of all its branches in the order of the checked out branch's history, on the base of
the sync, copied as a flat virtual branch of those commits is (the prefix stripped, `branch-deck` the committer, the
deterministic dates), so a copy on the same parent is the same commit as the flat build's.

#### Scenario: A forking stack builds its commits in history order
- **WHEN** the sync builds the stack branch of a stack that forks
- **THEN** it holds the original commits of all its branches in the order of the checked out branch's history, on the base of the sync
- **AND** each is copied as a flat build copies it (the prefix stripped, `branch-deck` the committer, the deterministic dates), so a copy on the same parent is the same commit as the flat build's
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_forking_stack_builds_its_commits_in_history_order_as_one_branch`)

### Requirement: The stack branch: The build writes no mapping note and no event of a branch row

The build MUST write no mapping note (the note of an original names the copy of its own branch) and MUST send no event
of a branch row.

#### Scenario: The notes and the events of a sync of a forking stack
- **WHEN** the sync builds a stack branch
- **THEN** the note of each original names the copy in the virtual branch of its own branch
- **AND** every event of a branch row names a grouped branch, none the stack branch
  [@test] crates/sync-core/src/stack_sync_test.rs (`the_stack_branch_writes_no_mapping_notes_and_sends_no_branch_events`)

### Requirement: The stack branch: One StackBranchUpdate reports each stack branch

One `StackBranchUpdate` per stack branch MUST report it, after the `BranchesGrouped` that names its root
(`stack_branches`).

#### Scenario: The report of a stack branch
- **WHEN** the sync builds a stack branch
- **THEN** one `StackBranchUpdate` reports it, after the `BranchesGrouped` that names its root in `stack_branches`
  [@test] crates/sync-core/src/stack_sync_test.rs (`the_stack_branch_writes_no_mapping_notes_and_sends_no_branch_events`)

### Requirement: The stack branch: An unchanged stack costs no git process

A stack with the same commits on the same base MUST cost no git process: the outcome of a build is remembered by a
hash of the base and the original commit ids, in the memory of the process and in the object cache (`stack_branch`).
Without either it MUST be built again, to the same commit, and its ref MUST NOT move.

#### Scenario: The same commits on the same base
- **GIVEN** a stack branch built by an earlier sync, remembered in the memory of the process or in the object cache
- **WHEN** a sync finds the same commits on the same base
- **THEN** it reuses the stack branch without a git process
  [@test] crates/sync-core/src/stack_sync_test.rs (`an_unchanged_stack_reuses_its_stack_branch_without_a_git_process`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_first_sync_of_a_forking_stack`, `an_unchanged_sync_of_a_forking_stack`)

#### Scenario: The object cache keeps the builds by their version
- **WHEN** a build is remembered in the object cache (`stack_branch`)
- **THEN** it is read back by its hash, and a build of another version is never read
  [@test] crates/object-cache/src/cache_test.rs (`stack_builds_are_read_by_their_version`)

#### Scenario: Neither the memory nor the object cache has it
- **GIVEN** a new process without the object cache
- **WHEN** the sync finds the same commits on the same base
- **THEN** it builds the stack branch again, to the same commit, and its ref doesn't move
  [@test] crates/sync-core/src/stack_sync_test.rs (`an_unchanged_stack_reuses_its_stack_branch_without_a_git_process`)

### Requirement: The stack branch: Its state is Built, Conflict, Blocked or Stale

Its state MUST be one of: `Built`, the ref is the build of the stack's commits now; `Conflict`, a commit conflicts with
the commits of the stack before it, named with its files; `Blocked`, a member has a stack problem, is blocked,
conflicts or failed, named with why (the first in stack order, a failure before a block), while the stack branch's own
build doesn't conflict; `Stale`, not rebuilt for another reason, said in words.

#### Scenario: Built
- **WHEN** the ref is the build of the stack's commits now
- **THEN** the state is `Built`

#### Scenario: A commit conflicts with the commits of the stack before it
- **WHEN** a commit of the stack conflicts with the commits of the stack before it
- **THEN** the state is `Conflict`, the commit named with its files
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_conflict_keeps_the_old_ref_stale_and_says_why`)
  [@test] crates/sync-core/src/stack_branch_test.rs (`a_conflicting_build_names_the_commit_and_keeps_the_ref`)

#### Scenario: A member has a problem, is blocked, conflicts or failed
- **GIVEN** the stack branch's own build doesn't conflict
- **WHEN** a member has a stack problem, is blocked, conflicts or failed
- **THEN** the state is `Blocked`, named with why: the first member in stack order, a failure before a block
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_member_that_fails_blocks_the_stack_branch`)

#### Scenario: Not rebuilt for another reason
- **WHEN** the stack branch is not rebuilt for another reason
- **THEN** the state is `Stale`, the reason said in words
  [@test] crates/sync-core/src/stack_branch_test.rs (`another_process_moving_the_stack_branch_keeps_its_tip_stale`)

### Requirement: The stack branch: A stack branch that is not built keeps its tip and is never pushed

In every state but `Built` the ref MUST keep its tip, and nothing MUST push it.

#### Scenario: A conflict keeps the old ref
- **GIVEN** a stack branch an earlier sync built
- **WHEN** a sync finds it in another state than `Built`, as a conflict
- **THEN** the ref keeps its tip
- **AND** nothing pushes it
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_conflict_keeps_the_old_ref_stale_and_says_why`)
  [@test] crates/sync-core/src/stack_branch_test.rs (`a_conflicting_build_names_the_commit_and_keeps_the_ref`)

### Requirement: The stack branch: The ref moves with compare-and-swap

The ref MUST move with compare-and-swap on the tip the sync listed; another process's move MUST keep its tip, `Stale`.

#### Scenario: Another process moves the stack branch
- **WHEN** another process moves the stack branch after the sync listed its tip
- **THEN** the sync keeps that process's tip and the state is `Stale`
  [@test] crates/sync-core/src/stack_branch_test.rs (`another_process_moving_the_stack_branch_keeps_its_tip_stale`)

### Requirement: The stack branch: Its remote status is by content

Its remote status MUST be by content, as a virtual branch's, against its remote branch on `origin`.

#### Scenario: A stack branch on the remote
- **WHEN** the stack branch has a remote branch on `origin`
- **THEN** its remote status compares them by content, as a virtual branch's
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_stack_that_no_longer_forks_loses_its_stack_branch_unless_it_is_on_the_remote`)

### Requirement: The stack branch: Its remote status counts the most frequent author as the user

Its remote status MUST count the commits of the most frequent author of the commits of all its members as the user's,
of a tie the author seen first in history; the status a push of it leaves MUST count the same ones, whoever authored
its root.

#### Scenario: The author of a stack branch
- **WHEN** the remote status of a stack branch is computed
- **THEN** the commits of the most frequent author of all its members count as the user's, of a tie the author seen first in history
  [@test] crates/sync-core/src/stack_branch_test.rs (`the_email_of_a_stack_branch_is_its_most_frequent_author`)

#### Scenario: The status a push leaves
- **WHEN** a push of the stack branch leaves its remote status
- **THEN** it counts the same commits as the user's as the sync does, whoever authored its root
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_stack_branch_counts_the_commits_of_its_members_author_as_the_sync_does`)

### Requirement: The stack branch: A stack that no longer forks loses its stack branch

A stack branch whose stack no longer forks (or whose root is gone) MUST be deleted in one transaction with
compare-and-swap, but only while it has no remote branch and no worktree has it checked out; one with a remote branch
stays, `Stale` without members, for the user to decide on.

#### Scenario: No remote branch
- **GIVEN** the stack branch has no remote branch and no worktree has it checked out
- **WHEN** its stack no longer forks, or its root is gone
- **THEN** the sync deletes it in one transaction with compare-and-swap
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_stack_that_no_longer_forks_loses_its_stack_branch_unless_it_is_on_the_remote`)

#### Scenario: A remote branch
- **GIVEN** the stack branch has a remote branch
- **WHEN** its stack no longer forks
- **THEN** it stays, `Stale` without members, for the user to decide on
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_stack_that_no_longer_forks_loses_its_stack_branch_unless_it_is_on_the_remote`)

#### Scenario: A worktree has it checked out
- **GIVEN** a worktree has the stack branch checked out
- **WHEN** its stack no longer forks
- **THEN** it is not deleted

### Requirement: The stack branch: The archive step and the integration detection leave it out

The archive step and the integration detection MUST leave the stack branches out: never archived, and a stack tip
never names the detection note of an archived branch.

#### Scenario: The branches by their namespace
- **WHEN** the archive step classifies the branches
- **THEN** a stack branch is never archived
- **AND** a stack tip never names the detection note of an archived branch
  [@test] crates/sync-core/src/detection_archive_step_test.rs (`branches_are_classified_by_their_namespace`)

### Requirement: The stack branch: brd sync and brd status list them as stacks

`brd sync` and `brd status` MUST list them as `stacks`: `root`, `branch`, `state` (`built`, `blocked`, `conflict`,
`stale`), `tip`, `member` and `reason` of a block, `conflict`, `error`, `commits`, `members`, and with `brd sync` the
`remote` summary.

#### Scenario: The stacks of a sync and a status
- **WHEN** `brd sync` or `brd status` runs in a repository with a stack that forks
- **THEN** its `stacks` list `root`, `branch`, `state`, `tip`, `commits` and `members`, with `member` and `reason` of a block, `conflict` and `error`
- **AND** `brd sync` adds the `remote` summary
  [@test] crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)

### Requirement: The stack branch: brd status reads the state without a sync and a git process

`brd status` MUST read the state without a sync and without a git process: blocked by the placement, else the
remembered build of the current commits when the ref is at it, else stale.

#### Scenario: The state brd status reads
- **WHEN** `brd status` runs
- **THEN** the state is blocked by the placement, else built by the remembered build of the current commits when the ref is at it, else stale
- **AND** it runs no sync and no git process for it
  [@test] crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_status_of_a_forking_stack`)

### Requirement: The stack branch: The app keeps the stack branches of the last sync by root

The app MUST keep the stack branches of the last sync by root (`useStackBranch`, `getStackBranch`; the full name by
`getFullStackBranchName`): a grouping keeps the ones it names as they were until their update and drops the others
(the session's table in Rust, whose patches the window applies by root).

#### Scenario: A grouping keeps the announced stack branches
- **WHEN** a grouping names some stack branches
- **THEN** the session's table keeps them as they were until their update and drops the others
  [@test] crates/repository-model/src/table_test.rs (`the_grouping_keeps_the_announced_stack_branches_and_the_baseline_status_fills_its_section`)

#### Scenario: The window applies the patches by root
- **WHEN** the patches of the session report, change or remove a stack branch
- **THEN** the window keeps it by root until one changes or removes it
  [@test] src/model/store.test.ts (`keeps the stack branches the patches report until one changes or removes them`)

### Requirement: The Safe Push of the whole stack: The whole stack of a branch is the stack of its declared root

The whole stack of a branch MUST be the stack of its root by declared parent, for the app and `brd` alike
(`domain_policy::stack::whole_stack_of`, the owner's decision: the stack as the user declared it; the parent that
blocks a branch is a detail of its build).

#### Scenario: The root of a branch by its declared parents
- **WHEN** the whole stack of a branch is asked, in the app or in `brd`
- **THEN** it is the stack of the root its declared parents lead to
  [@test] crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_branch_is_the_stack_of_its_declared_root`)

### Requirement: The Safe Push of the whole stack: It is the stack branch while the stack forks, else the chain

The whole stack MUST be the stack branch of the root while the stack forks, else the straight chain from the root,
which its last branch that is not parked lands (a parked one at its end stays local).

#### Scenario: A stack that forks
- **WHEN** the stack of the root forks
- **THEN** the whole stack is the stack branch of the root
  [@test] crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_branch_is_the_stack_of_its_declared_root`)

#### Scenario: A straight chain with a parked branch at its end
- **WHEN** the stack of the root is a straight chain
- **THEN** the whole stack is the chain from the root, which its last branch that is not parked lands
- **AND** a parked branch at its end stays local
  [@test] crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_branch_is_the_stack_of_its_declared_root`)

#### Scenario: The whole stack of a branch in the app
- **WHEN** the app finds the whole stack of a branch
- **THEN** it is the stack that forks and holds the branch, once its stack branch is known, else the straight chain the branch heads
  [@test] src/stores/branch-sync.test.ts (`is the stack that forks and holds the branch, else the straight chain the branch heads`)

### Requirement: The Safe Push of the whole stack: A blocked branch follows its declared parent

A blocked branch whose declared parent has no commits MUST head a whole stack of its own, never the chain of the
branch that blocks it.

#### Scenario: The declared parent has no commits
- **GIVEN** a branch declares a parent that has no commits, and another branch blocks it
- **WHEN** its whole stack is asked
- **THEN** it heads a whole stack of its own, never the chain of the branch that blocks it
- **AND** the Safe Push of the whole stack of the blocking branch lands that branch alone
  [@test] crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_blocked_branch_follows_its_declared_parent_not_what_blocks_it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)

### Requirement: The Safe Push of the whole stack: A fork without a known stack branch waits for the sync

A fork no stack branch is known of yet MUST wait for the sync that builds it (`brd` refuses it as `stack_not_built`).

#### Scenario: No stack branch is known of the fork yet
- **GIVEN** the stack forks, and no stack branch of it is known yet
- **WHEN** its whole stack is asked
- **THEN** it waits for the sync that builds the stack branch
- **AND** `brd` refuses its Safe Push as `stack_not_built`
  [@test] crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_branch_is_the_stack_of_its_declared_root`)

### Requirement: The Safe Push of the whole stack: The view places the whole stack on its root

The view MUST place the whole stack on the structure of its root (`BranchStructure.wholeStack`).

#### Scenario: The root of a stack carries its whole stack
- **WHEN** the session derives the view of a stack
- **THEN** the structure of its root carries the whole stack, the other branches none
  [@test] crates/repository-model/src/derive_test.rs (`the_root_of_a_stack_carries_its_whole_stack`)

### Requirement: The Safe Push of the whole stack: The landing names what its Safe Push lands

The view MUST place what its Safe Push lands on its landing (`StackLanding.rest`, `whole_stack_rest`): the branches as
a sentence names them (the main line first) less those merged Safe Pushes landed whole.

#### Scenario: The members, main line first
- **WHEN** the members of a whole stack are named
- **THEN** the main line comes first, then the side branches
  [@test] crates/domain-policy/src/stack_test.rs (`the_members_of_a_whole_stack_name_its_main_line_first`)

#### Scenario: Merged Safe Pushes landed some branches whole
- **WHEN** merged Safe Pushes landed some branches of the stack whole
- **THEN** the rest of the whole stack leaves them out
  [@test] crates/domain-policy/src/stack_test.rs (`the_rest_of_a_whole_stack_is_what_merged_safe_pushes_left_of_it`)

### Requirement: The Safe Push of the whole stack: brd safe-push --whole-stack lands it in one robot

`brd safe-push <branch> --whole-stack` MUST land the whole stack of the branch in one robot from its stack branch, the
only branch it pushes; a straight chain from its last branch.

#### Scenario: A stack that forks
- **WHEN** `brd safe-push <branch> --whole-stack` runs for a branch of a stack that forks
- **THEN** one robot lands the whole stack from its stack branch, the only branch it pushes
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)

#### Scenario: A straight chain
- **WHEN** it runs for a branch of a straight chain
- **THEN** the whole stack lands from its last branch
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_of_a_straight_chain_lands_from_its_last_branch`)

#### Scenario: The app and brd answer the cases of the whole stack alike
- **WHEN** the cases of the stack and the whole stack are asked of the app and of `brd`
- **THEN** both answer alike
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

### Requirement: The Safe Push of the whole stack: brd safe-push --whole-stack is refused unless it can land

`brd safe-push <branch> --whole-stack` MUST be refused unless the stack branch is `built` (`stack_conflict`,
`stack_not_built`, exit 4), and both ways while a robot of the stack or of a branch of it runs (`robot_running`, exit 4)
([Agent CLI](../../cli/agent-cli/spec.md#brd-safe-push)).

#### Scenario: The stack branch conflicts
- **WHEN** the stack branch is not `built`, as it conflicts
- **THEN** `brd safe-push <branch> --whole-stack` is refused with the conflict (`stack_conflict`, else `stack_not_built`), exit 4
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_whole_stack_that_conflicts_is_refused_with_the_conflict`)

#### Scenario: A robot of the stack or of a branch of it runs
- **WHEN** a robot of the whole stack runs and a branch of it is asked, or a robot of a branch of it runs and the whole stack is asked
- **THEN** the Safe Push is refused as `robot_running`, exit 4
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)

### Requirement: The Safe Push of the whole stack: Push with Children of the root takes the stack branch along

Push with Children of the root, the app's and `brd push <root> --with-children`, MUST take the stack branch along
after the branches once it is on the remote or its pull request is open, only when the sync built it, so its pull
request follows the branches ([Stacked Branches](../stacked-branches/spec.md#pull-requests)), by one rule
(`domain_policy::push::stack_branch_along`: not while its push is held, nor while its remote has commits it lacks,
which `brd` pushes over only with `--force`).

#### Scenario: The rule of the stack branch along
- **WHEN** the rule decides whether the stack branch goes along
- **THEN** it goes along once published, only built, not while its push is held, nor while its remote has commits it lacks
  [@test] crates/domain-policy/src/push_test.rs (`the_stack_branch_goes_along_once_published_built_not_held_and_behind`)

#### Scenario: The stack branch is on the remote
- **GIVEN** the stack branch is on the remote and the sync built it
- **WHEN** `brd push <root> --with-children` runs
- **THEN** it pushes the stack branch after the branches
  [@test] crates/bd-remote/src/push_test.rs (`with_children_takes_the_stack_branch_along_once_it_is_on_the_remote`)

#### Scenario: Its pull request is open
- **GIVEN** the pull request of the stack branch is open
- **WHEN** Push with Children of the root runs
- **THEN** it takes the stack branch along, so its pull request follows the branches
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_with_children_takes_the_stack_branch_along_while_its_pull_request_is_open`)

#### Scenario: Its remote has commits it lacks
- **WHEN** the remote of the stack branch has commits it lacks
- **THEN** Push with Children leaves it, and `brd` pushes over them only with `--force`
  [@test] crates/domain-policy/src/push_test.rs (`the_stack_branch_goes_along_once_published_built_not_held_and_behind`)

#### Scenario: Its push is held
- **GIVEN** a Safe Push of the whole stack runs on the stack branch
- **WHEN** `brd push <root> --with-children` runs
- **THEN** it pushes the branches and leaves the stack branch as it is, saying why
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_with_children_leaves_the_stack_branch_of_a_running_whole_stack_safe_push`)

### Requirement: The Safe Push of the whole stack: brd robot --whole-stack prints its robot

`brd robot --whole-stack` MUST print the robot of the whole stack.

#### Scenario: From any branch of the stack
- **WHEN** `brd robot <branch> --whole-stack` runs for any branch of the stack
- **THEN** it prints the robot of the whole stack
  [@test] crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)

### Requirement: The Safe Push of the whole stack: A robot of a stack branch names the root of its stack

A robot started on a stack branch MUST name the root of its stack (`PatronusRobot::stack_root`) and no branch.

#### Scenario: The robot Patronus lists
- **WHEN** Patronus lists a robot started on a stack branch
- **THEN** it names the root of its stack and no branch
  [@test] crates/patronus/src/service_test.rs (`a_robot_of_the_stack_branch_names_the_root_of_its_stack_and_no_branch`)

### Requirement: The Safe Push of the whole stack: The robot of the whole stack belongs to no branch

A robot started on a stack branch MUST belong to no branch, not even by its revision, in `brd` and the session's view,
whose `robotId` the app reads (`RobotIndex::branch_robot` of `domain_policy::robot_match`);
`RobotIndex::stack_robot` MUST find the robot of the whole stack.

#### Scenario: The robot index
- **WHEN** the robot of a branch and the robot of the whole stack are found (`RobotIndex::branch_robot`, `RobotIndex::stack_robot`)
- **THEN** the robot of the stack branch is no branch's, not even by its revision, and is the robot of the whole stack of its root
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch`)

#### Scenario: The session's view
- **WHEN** the session's view holds the robot of the whole stack
- **THEN** no branch has it as its `robotId`, not even the root
  [@test] src/stores/patronus.test.ts (`holds every branch the whole-stack Safe Push lands`)

#### Scenario: brd robot
- **WHEN** `brd robot` prints the robot of the whole stack
- **THEN** it names the root of its stack (`stack_root`) and no branch
  [@test] crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)

### Requirement: The Safe Push of the whole stack: The pull request of a stack branch is the stack's

The pull request of a stack branch MUST be the stack's, by its root (`PullRequestsResult::stack_pull_requests`), never
a branch's; one that GitHub says exists MUST be found by its head.

#### Scenario: The pull requests of a stack branch and of its root
- **WHEN** the pull requests are listed
- **THEN** the pull request of the stack branch is the stack's, under its root, never the root branch's
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_pull_request_of_a_stack_branch_is_the_stacks_by_its_root`)

#### Scenario: GitHub says it exists
- **WHEN** GitHub says that the pull request of the stack branch exists
- **THEN** it is found by its head, and the start comments on it
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_pull_request_of_the_whole_stack_is_its_stack_branchs_and_the_start_comments_on_it`)

### Requirement: The Safe Push of the whole stack: The text of its pull request names every branch

Its text MUST be the stack branch's own description, else the root's title with the count of the branches built on it
and a section per branch, its description or the subjects of its commits, from what the sync read, cut to the
65,536 characters of a body and the 256 of a title GitHub takes, naming the branches left out.

#### Scenario: Each member by its description, else its commits
- **GIVEN** the stack branch has no description of its own
- **WHEN** the text of its pull request is made
- **THEN** it is the root's title with the count of the branches built on it
- **AND** a section per branch, its description or the subjects of its commits, from what the sync read
  [@test] crates/remote-facts/src/stack_pull_request_test.rs (`the_pull_request_of_the_whole_stack_names_each_member_by_its_description_else_its_commits`)

#### Scenario: The stack branch's own description
- **GIVEN** the stack branch has a description of its own
- **WHEN** the text of its pull request is made
- **THEN** it is that description
  [@test] crates/remote-facts/src/stack_pull_request_test.rs (`the_pull_request_of_the_whole_stack_names_each_member_by_its_description_else_its_commits`)

#### Scenario: A text too long for GitHub
- **WHEN** the text is longer than the 65,536 characters of a body or the 256 of a title GitHub takes
- **THEN** it is cut to them, naming the branches left out
  [@test] crates/remote-facts/src/stack_pull_request_test.rs (`a_text_too_long_for_github_is_cut_and_names_the_members_left_out`)

### Requirement: The Safe Push of the whole stack: Its readiness is asked of its stack branch

The readiness of the whole stack MUST ask Patronus about its stack branch (`ReviewReadinessBranch::source`, a stack
branch of the prefix, else refused), the answer under the key the caller gives.

#### Scenario: The readiness of the whole stack
- **WHEN** the readiness of the whole stack is asked
- **THEN** Patronus is asked about its stack branch, and the answer is kept under the key the caller gives
  [@test] crates/remote-facts/src/review_readiness_test.rs (`the_whole_stack_is_asked_as_its_stack_branch_under_the_key_of_the_caller`)

#### Scenario: Not a stack branch of the prefix
- **WHEN** the branch asked as a stack branch is not a stack branch of the prefix
- **THEN** the question is refused
  [@test] crates/remote-facts/src/review_readiness_test.rs (`the_whole_stack_is_asked_as_its_stack_branch_under_the_key_of_the_caller`)

### Requirement: The Safe Push of the whole stack: The app starts it as the Land of the whole stack

The start of the app MUST be the `Land` of the whole stack (`Subject::WholeStack`), whose robot starts from the stack
branch.

#### Scenario: The app starts the whole stack
- **WHEN** the app starts the Safe Push of the whole stack
- **THEN** it is the `Land` of `Subject::WholeStack`, whose robot starts from the stack branch
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: The whole stack in the app: It is named as brd names it

The whole stack MUST be named "the whole air stack" in a sentence and "Whole air stack" at its start, as `brd` names it
(`formatWholeStack`, `formatRobotSubject`), wherever the app names the robot or the pull request of a stack branch.

#### Scenario: The name of the whole stack
- **WHEN** the app names the whole stack of `air`
- **THEN** it is "the whole air stack" in a sentence and "Whole air stack" at its start, as `brd` names it
  [@test] src/lib/whole-stack.test.ts (`is named as Branch Deck names it`)

#### Scenario: The robot of a stack branch
- **WHEN** the app names the robot of a stack branch (`formatRobotSubject`)
- **THEN** it names the whole stack, at the start of a sentence too
  [@test] src/lib/patronus.test.ts (`is named the whole stack, at the start of a sentence too`)

### Requirement: The whole stack in the app: Its robot is the robot of the root's stack

The robot of the whole stack MUST be the root's stack's (`robotId` of its stack in the session's view,
`RobotIndex::stack_robot`: started on its stack branch, or a Safe Push of a temporary ref on the tip of the stack
branch).

#### Scenario: A robot on the stack branch or on a temporary ref at its tip
- **WHEN** a robot runs on the stack branch, or a Safe Push of a temporary ref on the tip of the stack branch runs
- **THEN** it is the robot of the whole stack of the root, the `robotId` of its stack in the session's view
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch`)

### Requirement: The whole stack in the app: Its robot is polled and told as the robot of a branch is

The robot of the whole stack MUST be polled while it runs, its problems and its failing read, as the robot of a branch
is; it MUST be told when it merged ("Whole air stack merged into master", in the toast, Activity and the OS
notification), failed ("Safe Push of the whole air stack failed", its pull request the step when it needs review) or
will fail.

#### Scenario: It merged
- **WHEN** the session polls the robot of the whole stack, which no branch has, and it merged
- **THEN** the toast, Activity and the OS notification say "Whole air stack merged into master"
  [@test] src/stores/patronus.test.ts (`closes the pull requests of the branches the whole stack landed`)

#### Scenario: It failed or will fail
- **WHEN** the robot of the whole stack failed, or will fail
- **THEN** the app tells it, "Safe Push of the whole air stack failed", its pull request the step when it needs review

### Requirement: The whole stack in the app: The landing overview lists it as the whole stack

The landing overview MUST list the robot of the whole stack as "Whole air stack", a click showing the root.

#### Scenario: The robot in the landing overview
- **WHEN** the robot of the whole stack runs
- **THEN** the landing overview lists it as "Whole air stack", and a click on it shows the root
  [@test] src/lib/robot-overview.test.ts (`lists the robot of the whole stack as the whole stack after the branches, a click showing the root, its failure as the move the view names, any other as finished`)

#### Scenario: A small stack
- **GIVEN** a small stack, without an island
- **WHEN** the robot of its whole stack runs
- **THEN** the robots of the landing overview name it "Whole auth stack"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the robot of the whole stack on the rows of a small stack, hold their pushes and cancel it from the root's menu`)

### Requirement: The whole stack in the app: The header of its island counts it as the root's

The header of its island MUST count the robot of the whole stack as no branch's: while it runs every branch it lands
is landing ("7 landing" with its ring, [Stack Island](../stack-island/spec.md#header)), a click on the count showing
the root.

#### Scenario: The count of the header
- **WHEN** the robot of the whole stack runs
- **THEN** the header of its island counts every branch it lands as landing, "7 landing", its ring on the count
- **AND** a click on the count shows the root
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should count the robot of the whole stack in the header of its island, a click on it showing the root`)

### Requirement: The whole stack in the app: Its failure asks for the user only on a built stack branch

The failure of the robot of the whole stack MUST ask for the user (the landing overview's attention, the root's "1 to fix"
of the island, the Dock badge) only on a stack branch the sync built (`domain_policy::attention::stack_attention`, the view's
`attention` of `stack:<root>`).

#### Scenario: A failed robot on a built stack branch
- **GIVEN** the sync built the stack branch
- **WHEN** the robot of the whole stack failed
- **THEN** the landing overview asks for the user, the island counts "1 to fix", the root's, and the Dock badge counts it
  [@test] crates/domain-policy/src/attention_test.rs (`the_failed_robot_of_a_whole_stack_needs_the_user_only_on_a_built_stack_branch_no_robot_of_a_branch_lands`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should count the failed robot of the whole stack only while the sync built its stack branch`)
  [@test] src/lib/stack-state.test.ts (`counts a fix of the whole stack, whose stack branch has no row, as the root's`)

#### Scenario: A failed robot on a stack branch not rebuilt
- **GIVEN** the stack branch is not rebuilt (`Stale`, kept for its remote branch once the stack no longer forks; `Blocked`; `Conflict`)
- **WHEN** the robot of the whole stack failed
- **THEN** its restart is refused and the stack lands in pieces
- **AND** the overview lists it as finished, and the island and the badge count nothing for it
  [@test] crates/domain-policy/src/attention_test.rs (`the_failed_robot_of_a_whole_stack_needs_the_user_only_on_a_built_stack_branch_no_robot_of_a_branch_lands`)
  [@test] src/lib/robot-overview.test.ts (`lists the robot of the whole stack as the whole stack after the branches, a click showing the root, its failure as the move the view names, any other as finished`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should count the failed robot of the whole stack only while the sync built its stack branch`)

### Requirement: The whole stack in the app: Its running robot holds every branch of the stack

While it runs and pushes into the target, the robot of the whole stack MUST hold every branch of the stack its commit
has (`PushHold.wholeStack`, by the source ref: its stack branch, not by a branch's revision; after a change of the stack
the content tells, [Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)) and land them.

#### Scenario: The rows of the branches it lands
- **WHEN** the robot of the whole stack runs and pushes into the target
- **THEN** every branch of the stack its commit has is held by the whole stack, by its stack branch, and its row says "lands with the whole air stack"
- **AND** their push and their Safe Push wait ("A Safe Push of the whole air stack is running: it lands 1 commit of auth with it")
  [@test] src/stores/patronus.test.ts (`holds every branch the whole-stack Safe Push lands`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_robot_of_the_whole_stack_lands_every_branch_of_it`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the robot of the whole stack on the rows of a small stack, hold their pushes and cancel it from the root's menu`)

#### Scenario: The stack changed since it started
- **WHEN** a branch of the stack changed since the robot started
- **THEN** the content tells which branches it holds ([Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs))

### Requirement: The whole stack in the app: The robot of a branch of the stack holds the whole stack

The robot of a branch of the stack MUST hold the whole stack's restart and start in turn ("A Safe Push of auth is
running: wait until it merges", the `landingMember` of the view); the start refusal is the rule of Rust
(`domain_policy::land::start_refusal`, the view's `landing.start` of the stack), after a stack branch the sync did not
build and the robot on its commit.

#### Scenario: A robot of a branch of the stack runs
- **WHEN** a robot of a branch of the stack runs
- **THEN** the restart and the start of the whole stack wait for it, "A Safe Push of auth is running: wait until it merges", the `landingMember` of the view
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_robot_of_the_whole_stack_lands_every_branch_of_it`)
  [@test] crates/domain-policy/src/land_test.rs (`the_whole_stack_says_its_build_first_and_its_own_robot_holds_or_is_replaced`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should refuse the restart of the failed robot of the whole stack once Safe Pushes of its branches landed every branch of it, on every surface that offers it`)

#### Scenario: The order of the start refusals
- **WHEN** the start of the whole stack is refused (`domain_policy::land::start_refusal`, `landing.start`)
- **THEN** a stack branch the sync did not build comes first, then the robot on its commit, then a robot of a branch of it that lands a part
  [@test] crates/domain-policy/src/land_test.rs (`a_whole_stack_waits_for_its_build_its_last_commit_then_a_member_that_lands_a_part`)

### Requirement: The whole stack in the app: A dry run of the whole stack holds nothing

A dry run of the whole stack MUST hold nothing.

#### Scenario: A dry run runs
- **WHEN** a dry run of the whole stack runs
- **THEN** no branch of the stack is held
  [@test] src/stores/patronus.test.ts (`holds nothing for a dry run of the whole stack`)

### Requirement: The whole stack in the app: Its actions are the actions of a robot

The actions of its robot MUST be the actions of a robot (`getRobotActionItems`, as Rust offers them): the restart works
on the stack branch, and pushes it first while it has work its remote lacks (the push step of `RestartRobot`, a running
robot canceled first), only while the sync built it; cancel, the prompts, Patronus, its pull request.

#### Scenario: The restart of a robot whose stack branch changed since
- **GIVEN** the sync built the stack branch, and it has work its remote lacks
- **WHEN** the user restarts the robot of the whole stack
- **THEN** a running robot is canceled first, then the stack branch is pushed (the push step of `RestartRobot`), then the robot restarts on it
  [@test] src/presenters/robot-actions.test.ts (`names what a restart of a robot of a branch changed since does: it cancels a running one, then pushes`)
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should run the restart of the robot of the whole stack once while its push runs: one push, one restart, one toast`)

#### Scenario: When the restart is offered
- **WHEN** Rust offers the actions of the robot
- **THEN** the restart is offered once Patronus tells why it failed and its approvals count, and cancel, the prompts, Patronus and its pull request beside it
  [@test] crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)

#### Scenario: Cancel, the prompt and Patronus of a small stack and an island
- **WHEN** the robot of the whole stack runs on a small stack, or failed on an island
- **THEN** "Whole Stack ›" of the root's menu cancels it, and the hover of the island's node offers its prompt and opens it in Patronus
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the robot of the whole stack on the rows of a small stack, hold their pushes and cancel it from the root's menu`, `should show the failed robot of the whole stack in the hover of the island's node, with its actions`)

### Requirement: The whole stack in the app: Its restart is refused once every branch of the stack landed

Once merged Safe Pushes landed every branch of the stack, its restart MUST be refused, with or without the push, of a
Dry Run too, with the reason its Safe Push Whole Stack says ("Landed in master with deploy and side-test",
`reasons::stack_landed` of `domain_policy`), on every surface that lists the actions (the "⋯" of the island, "Whole
Stack ›" of the menus of its branches).

#### Scenario: Merged Safe Pushes of its branches landed every branch of it
- **GIVEN** the robot of the whole stack failed
- **WHEN** merged Safe Pushes of its branches landed every branch of the stack
- **THEN** its restart, with or without the push, of a Dry Run too, is disabled with "Landed in master with deploy and side-test" (`reasons::stack_landed`) in the "⋯" of the island and in "Whole Stack ›" of the menus of its branches
- **AND** the header offers no step, and the landing overview lists the stack in master without its robot
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should refuse the restart of the failed robot of the whole stack once Safe Pushes of its branches landed every branch of it, on every surface that offers it`)

### Requirement: The whole stack in the app: A push of the stack branch alone shows as the push of the stack

A push of the stack branch alone (the push step of the restart, of Request Review… of its pull request, of the creation
of its pull request and of the start of its Safe Push; the "Push Branch" of a refused start of the whole stack, the
intent `Push` of the subject `wholeStack`, `pushWholeStack`) MUST show where the pushes of the stack show, one
indicator.

#### Scenario: The restart of its robot pushes it
- **WHEN** the restart of the robot of the whole stack pushes the stack branch
- **THEN** the push of the header of its island ([Stack Island](../stack-island/spec.md#header)) shows it, else the push button of the root's row of a small stack, "Pushing the whole stack 45%" with the line of git on hover
- **AND** the branches of the stack are not busy
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`)

#### Scenario: Request Review… of its pull request pushes it
- **WHEN** Request Review… of the pull request of the whole stack pushes the stack branch
- **THEN** the push of the header of its island shows it, one indicator
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it, the stack waiting from the click`)

#### Scenario: A refused start offers Push Branch
- **WHEN** the start of the whole stack is refused and the user chooses its "Push Branch"
- **THEN** it is the intent `Push` of the subject `wholeStack` (`pushWholeStack`), shown where the pushes of the stack show

### Requirement: The whole stack in the app: The start and the dialog show the push in their own button

The start of its Safe Push MUST show its push in its own button (the island's header, the root's slot of a small
stack: the spinner and "45%", named "Pushing 45%"), the dialog of its pull request in its Create button ("Pushing
45%"); one hook MUST decide which (`useStackBranchPushAlone`).

#### Scenario: The start from the header of an island
- **WHEN** the start of the Safe Push of the whole stack pushes its stack branch from the header of an island
- **THEN** the button of the header's slot shows the spinner and "45%", named "Pushing 45%"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

#### Scenario: The start from the root's menu of a small stack
- **WHEN** the start pushes the stack branch of a small stack
- **THEN** the root's slot shows the spinner and "45%"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)

#### Scenario: The dialog of its pull request
- **WHEN** Create of the "Create pull request" dialog of the stack pushes the stack branch
- **THEN** its Create button shows "Pushing 45%"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

### Requirement: The whole stack in the app: One push of the stack branch at a time

There MUST be one push of the stack branch at a time: a second request while one runs (another action's, or Push with
Children that takes it along) waits for it in the registry of the session (its ref), no push of its own, one
indicator; the same request joins it; then the request pushes only what the remote still lacks, nothing when the
remote has it all.

#### Scenario: A second request while one runs
- **WHEN** another action's push of the stack branch, or Push with Children that takes it along, is asked while a push of it runs
- **THEN** it waits for it in the registry of the session (its ref), with no push of its own and one indicator
- **AND** then it pushes only what the remote still lacks (what a sync rebuilt since; the stack branch alone after a Push with Children that failed for a branch), nothing when the remote has it all
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_of_a_whole_stack_is_pushed_alone_and_goes_along_once_on_the_remote`)

#### Scenario: The same request
- **WHEN** the same request is asked again while it runs
- **THEN** it joins the running one

### Requirement: The whole stack in the app: A second restart of the same robot is the same request

A second restart of the same robot MUST be the same request, one operation of the robot whose item runs meanwhile:
one push, one restart, one toast.

#### Scenario: The restart is chosen again while its push runs
- **WHEN** the user restarts the robot of the whole stack again while the push of its first restart runs
- **THEN** its item runs meanwhile, and there is one push, one restart, one toast
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should run the restart of the robot of the whole stack once while its push runs: one push, one restart, one toast`)

### Requirement: The whole stack in the app: Push with Children waits for a push of the stack branch alone

Push with Children MUST wait for a push of the stack branch alone and take it along only while the remote still lacks
it.

#### Scenario: Push with Children while the stack branch is pushed alone
- **WHEN** Push with Children of the root runs while a push of the stack branch alone runs
- **THEN** it waits for it, and takes the stack branch along only while the remote still lacks it
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_of_a_whole_stack_is_pushed_alone_and_goes_along_once_on_the_remote`)

### Requirement: The whole stack in the app: Its push is refused once every branch of the stack landed

Once merged Safe Pushes landed every branch of the stack, the stack branch carries only commits that are in the target:
its push MUST be refused ("Cannot push the whole air stack", "Landed in master with deploy and side-test: nothing of it
is left to push"), Push with Children MUST leave it, and "Create Pull Request of the Stack…" MUST be disabled with the
reason ([Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: Every branch of the stack landed
- **WHEN** merged Safe Pushes landed every branch of the stack
- **THEN** the push of the stack branch is refused: "Cannot push the whole air stack", "Landed in master with deploy and side-test: nothing of it is left to push"
- **AND** Push with Children leaves it and pushes what is left without the landed ones
- **AND** "Create Pull Request of the Stack…" is disabled with the reason
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

### Requirement: The whole stack in the app: The prompt of its failure names every branch and its commits

The prompt for an agent of its failure MUST name the stack branch and the commits of every branch of the stack, each
with its branch, by identity (`stack_robot_commit_map`: no mapping notes), a commit of a member the stack branch lacks
said as not synced; `brd robot --whole-stack` MUST print the same map.

#### Scenario: The commit map of the robot of the whole stack
- **WHEN** the commit map of the robot of the whole stack is made (`stack_robot_commit_map`)
- **THEN** it lists the commits of every member with its branch, by identity, without mapping notes
- **AND** a commit of a member the stack branch lacks is said as not synced
  [@test] crates/sync-core/src/robot_commits_test.rs (`a_robot_of_the_whole_stack_lists_the_commits_of_every_member_with_its_branch`)

#### Scenario: The prompt for an agent
- **WHEN** the user copies the prompt of the failure of the robot of the whole stack
- **THEN** it names the stack branch and every branch of the whole stack, with their commits
  [@test] src/lib/robot-prompt.test.ts (`names the stack branch and every branch of the whole stack for its robot: it built them all`)

#### Scenario: brd robot --whole-stack
- **WHEN** `brd robot --whole-stack` runs for a branch of the stack
- **THEN** it prints the same map
  [@test] crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)

### Requirement: The whole stack in the app: Its merge closes the pull requests of every branch of the stack

When it merged, the open pull requests of every branch of the stack MUST be closed with a comment ("Landed in master
with #12, the Safe Push of the whole air stack (user/stack/air), which holds this branch."), and the toast says it
("Safe Push · closes the pull requests of 6 branches: they landed with the whole stack").

#### Scenario: The robot of the whole stack merged
- **WHEN** the robot of the whole stack merged
- **THEN** the open pull requests of every branch of the stack it landed are closed with the comment "Landed in master with #12, the Safe Push of the whole air stack (user/stack/air), which holds this branch."
- **AND** the toast says "Safe Push · closes the pull requests of 6 branches: they landed with the whole stack"
  [@test] src/stores/patronus.test.ts (`closes the pull requests of the branches the whole stack landed`)

### Requirement: The whole stack in the app: Its merge closes only the pull requests of what it landed

The merge MUST close them while it ran on the stack branch as it is, else by the commits it tested against each branch
(a change of a branch since rebuilt the stack branch: the repository view by content, which the news of the merge
waits for), and only a pull request that holds nothing its branch lacks; never the pull request the robot started from
(`domain_policy::landed::pull_requests_to_close`, [Patronus](../patronus/spec.md#merged), the same for a piece).

#### Scenario: The stack branch as it is
- **WHEN** the robot of the whole stack merged on the stack branch as it is
- **THEN** the pull requests of its branches are closed, only one that holds nothing its branch lacks: its head is the branch's tip, or the remote branch has no commit the branch lacks
  [@test] crates/domain-policy/src/landed_test.rs (`closes_the_pull_requests_of_the_branches_a_merged_robot_landed_whole_not_of_its_own_branch`, `leaves_a_pull_request_with_a_commit_the_branch_lacks_one_of_a_branch_with_work_left_and_one_the_table_lacks`)

#### Scenario: A branch changed since
- **WHEN** a change of a branch since rebuilt the stack branch
- **THEN** the pull requests are closed by the commits the robot tested against each branch, the repository view by content, which the news of the merge waits for

#### Scenario: The pull request the robot started from
- **WHEN** the robot started from a pull request
- **THEN** that pull request is never closed by Branch Deck, the same for a piece
  [@test] crates/domain-policy/src/landed_test.rs (`leaves_the_pull_request_the_robot_started_from_to_patronus`)

### Requirement: The whole stack in the app: Every branch it landed says so until the update

Until the update archives them, every branch of the stack it landed (under the same conditions, also for a robot first
seen merged) MUST say so in its row, "Landed with the whole air stack" ([Patronus](../patronus/spec.md#merged)).

#### Scenario: The rows of the branches it landed
- **WHEN** the robot of the whole stack merged, or is first seen merged
- **THEN** every branch of the stack it landed says "Landed with the whole air stack" in its row, until the update archives them
  [@test] crates/sync-core/src/robot_landing_test.rs (`the_robot_of_the_whole_stack_landed_every_branch_of_it`)
  [@test] src/lib/patronus.test.ts (`names who landed a branch in its row: a branch built on it, the whole stack, its own robot with the work left`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer its Safe Push no more`)

### Requirement: The whole stack in the app: A branch it landed offers no Safe Push, Dry Run or push

Every branch of the stack it landed MUST offer no Safe Push, no Dry Run and no push: "Landed in master with the whole
air stack" ([Next Step](../next-step/spec.md#the-table), row 1); the "to push" counts of the header, the island and the
filter chip MUST leave them out.

#### Scenario: The actions of a branch it landed
- **WHEN** the whole stack landed a branch
- **THEN** its Safe Push, Dry Run and push are disabled with "Landed in master with the whole air stack" ([Next Step](../next-step/spec.md#the-table), row 1), the hover of its chip saying so
  [@test] src/presenters/reasons.test.ts (`says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer its Safe Push no more`, `should offer no Safe Push, Dry Run or push of the branches the whole stack landed, each saying why, the hover of their chip saying so`)

#### Scenario: The counts of what to push
- **WHEN** the whole stack landed some branches
- **THEN** the "to push" counts of the header, the island and the filter chip leave them out

### Requirement: The whole stack in the app: A branch changed since is read by content

Any change of a branch of the stack MUST rebuild the stack branch, so no hash is the robot's any more: what it landed
is then read by content ([Patronus](../patronus/spec.md#merged)), a commit added to a branch since the only work left
of it ("Landed with the whole air stack · 1 commit after it").

#### Scenario: A commit added to a branch since the merge
- **WHEN** a commit is added to a branch of the stack after the robot of the whole stack merged
- **THEN** what it landed is read by content, and the row says "Landed with the whole air stack · 1 commit after it"
  [@test] crates/repository-model/src/patronus_view_test.rs (`new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`)

### Requirement: The whole stack in the app: One builder lists the actions of the whole stack

One builder MUST list the actions of the whole stack (`getWholeStackMenuItems`), which the "⋯" of the stack island
renders, the menu of every branch of the stack under "Whole Stack ›", the root's whole stack (so their previews too),
and the title bar and "⋯" of the review of the whole stack ([Branch Review](../branch-review/spec.md#title-bar-footer-and-actions)):
every branch is a piece of it, for a small stack without an island as for an island.

#### Scenario: Whole Stack › on every branch of the stack
- **WHEN** the menu of a branch of a stack opens
- **THEN** it holds the root's whole stack under "Whole Stack ›", and a branch of no stack has none
  [@test] src/hooks/useBranchContextActions.test.ts (`is the submenu of the root's whole stack on every branch of the stack, none on a branch of no stack`)

#### Scenario: The surfaces of the whole stack
- **WHEN** the user opens the "⋯" of the island, the root's menu, or the menu of a branch of the stack
- **THEN** each lists the same actions of the whole stack, such as "Review Whole Stack", which opens the review of its stack branch
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should review the whole stack in the window of its stack branch, from the island's menu, the root's menu and the menu of a branch of it`)

### Requirement: The whole stack in the app: The actions it lists

The list MUST hold "Safe Push Whole Stack" and "Dry Run Whole Stack" ([below](#the-safe-push-of-the-whole-stack-in-the-app)),
"Open Pull Request #12" of its pull request and, while it is open and matches the stack, "Request Review…" of it,
else "Create Pull Request of the Stack…", Push with Children (the island's; the root's menu has it beside its push),
"Review Whole Stack" ([Branch Review](../branch-review/spec.md#opening)) and the actions of its robot.

#### Scenario: A whole stack with its pull request
- **GIVEN** the pull request of the whole stack is open and matches the stack
- **WHEN** the actions of the whole stack are listed
- **THEN** they hold "Safe Push Whole Stack", "Dry Run Whole Stack", "Open Pull Request #12", "Request Review…", Push with Children, "Review Whole Stack" and the actions of its robot
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

#### Scenario: A whole stack without a pull request
- **GIVEN** the whole stack has no pull request
- **WHEN** the actions of the whole stack are listed
- **THEN** they hold "Create Pull Request of the Stack…" in place of "Request Review…"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

### Requirement: The whole stack in the app: Request Review… of the pull request of the whole stack

"Request Review…" of the pull request of the whole stack MUST be the step of the whole stack when it is the one, else
`getRequestReviewItem` with the code owners of the readiness of `stack:<root>`; its popover at the header's slot of
the island, else at an anchor of the root's slot, also from the root's preview, which closes; its stack branch pushed
first.

#### Scenario: A small stack, from Whole Stack › of the root's menu
- **GIVEN** a small stack without an island, its pull request open
- **WHEN** the user chooses "Request Review…" in "Whole Stack ›" of the root's menu
- **THEN** its popover opens at the root's row
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask for a review of the pull request of a small stack from Whole Stack › of the root's menu, the popover at the root's row`)

#### Scenario: A small stack, from the ⋯ of the root's row and of its preview
- **GIVEN** a small stack without an island, its pull request open
- **WHEN** the user chooses "Request Review…" in the "⋯" of the root's row, or of its preview
- **THEN** its popover opens at the root's row, and the preview closes
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should open Request Review… of the pull request of a small stack from the ⋯ of its root's row and of its preview, the popover at the root's row`)

#### Scenario: An island
- **WHEN** the user chooses "Request Review…" in the "⋯" of the island or in the root's menu
- **THEN** its popover opens at the header's slot of the island
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask for a review of the pull request of the whole stack from the island's ⋯ and the root's menu, the popover at the header`)

#### Scenario: Its Safe Push runs and the header offers no step
- **GIVEN** the Safe Push of the whole stack runs, and the header offers no step
- **WHEN** the user chooses "Request Review…" of the whole stack
- **THEN** its popover still opens at the header
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should open Request Review… of the whole stack at the header while its Safe Push runs and the header offers no step`)

### Requirement: The whole stack in the app: Create Pull Request of the Stack… is disabled while it cannot be

"Create Pull Request of the Stack…" MUST be disabled with the reason while the sync didn't build the stack branch
(`WholeStackView.forking.notBuilt`), and once every branch of the stack landed (`reasons::stack_landed`).

#### Scenario: The stack branch is not built
- **WHEN** the sync didn't build the stack branch (`WholeStackView.forking.notBuilt`)
- **THEN** "Create Pull Request of the Stack…" is disabled with the reason
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

#### Scenario: Every branch of the stack landed
- **WHEN** every branch of the stack landed
- **THEN** "Create Pull Request of the Stack…" is disabled with the reason of `reasons::stack_landed`
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

### Requirement: The whole stack in the app: An open menu of the island follows the pull requests

An open "⋯" of the island MUST follow what the list reads: the pull requests that load after the sync change its items
while it is open (`StackIslandMenu`, its selector memoized on the stores it reads).

#### Scenario: The pull requests load after the sync
- **GIVEN** the "⋯" of the island is open
- **WHEN** the pull requests load after the sync
- **THEN** its items change while it is open
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should follow the pull requests in the island's ⋯ while it is open: they load after the sync`)

### Requirement: The whole stack in the app: Create Pull Request of the Stack… opens the dialog of the stack branch

"Create Pull Request of the Stack…" MUST open the "Create pull request" dialog of the stack branch into the baseline
("Whole air stack → master", "Holds the commits of every branch of the stack: air, bazel, bridge and 4 more: 7
branches · 14 commits"), its text from `stack_pull_request_text` (`get_new_stack_pull_request`: the descriptions in
one `git config`, the subjects as the rows show them).

#### Scenario: The dialog from the island's menu and the root's menu
- **WHEN** the user chooses "Create Pull Request of the Stack…" in the island's menu or the root's menu
- **THEN** the "Create pull request" dialog of the stack branch opens, "Whole air stack → master", "Holds the commits of every branch of the stack: air, bazel, bridge and 4 more: 7 branches · 14 commits"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

#### Scenario: The text of the new pull request
- **WHEN** the dialog asks for the text of the new pull request (`get_new_stack_pull_request`)
- **THEN** it is from `stack_pull_request_text`, the descriptions read in one `git config`, the subjects as the rows show them
  [@test] crates/remote-facts/src/process_budget_test.rs (`the_text_of_a_new_pull_request_of_the_whole_stack`)

### Requirement: The whole stack in the app: Create pushes the stack branch alone, then creates the pull request

Create MUST push the stack branch alone (the push step of the intent `CreatePullRequest`, leased on its remote head),
then create the pull request of its head.

#### Scenario: Create in the dialog of the whole stack
- **WHEN** the user chooses Create in the dialog of the whole stack
- **THEN** the stack branch alone is pushed, leased on its remote head
- **AND** the pull request of its head is created, and opens from both menus
  [@test] crates/sync-core/src/push_test.rs (`the_whole_stack_pushes_its_stack_branch`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

### Requirement: The whole stack in the app: The dialog asks the code owners of the whole stack

The dialog MUST ask the code owners of the whole stack to review, as the dialog of a branch asks the branch's: those
Patronus suggests for the stack branch (the readiness of `stack:<root>`), preselected, while the sync built it.

#### Scenario: The code owners Patronus suggests
- **GIVEN** the sync built the stack branch
- **WHEN** the header creates the pull request of the whole stack
- **THEN** the dialog preselects the code owners Patronus suggests for the stack branch (the readiness of `stack:<root>`), as the step of a branch does
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does, the move made from the click`)

### Requirement: The whole stack in the app: A pull request that no longer matches is never closed by itself

A pull request of the whole stack that no longer matches it (a landing took the root: a Safe Push of a branch in a
piece, the fetched baseline or the merged pull request of the root, as the session knows what landed the root, also
after a restart; or the stack no longer forks and its stack branch stays for its remote branch) MUST never be closed by
itself, and its stack branch stays.

#### Scenario: A piece landed the root
- **WHEN** a Safe Push of a branch in a piece landed the root
- **THEN** the pull request of the whole stack is not closed, and its stack branch stays
  [@test] src/stores/patronus.test.ts (`says that a piece that landed the root leaves the pull request of the whole stack, never closed by itself`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say that the pull request of the whole stack no longer matches it once a piece landed the root, and close it only when asked`)

#### Scenario: The baseline or the merged pull request of the root landed it, or the stack no longer forks
- **WHEN** the fetched baseline or the merged pull request of the root landed the root, as the session knows it, also after a restart, or the stack no longer forks and its stack branch stays for its remote branch
- **THEN** the pull request of the whole stack is not closed by itself

### Requirement: The whole stack in the app: The surfaces say why its pull request no longer matches

The merge toast, the root's preview, the island's header and the description of its items MUST say "#12 no longer
matches the air stack: auth landed" ("air landed in origin/master", "#7 of air is merged", "it no longer forks"); the
page words what landed the root.

#### Scenario: What landed the root, or that it no longer forks
- **WHEN** Rust offers the close of the pull request of the whole stack
- **THEN** the page says why it no longer matches: "#12 no longer matches the air stack: auth landed", "air landed in origin/master", "#7 of air is merged" or "it no longer forks"
  [@test] src/lib/whole-stack.test.ts (`says why its pull request no longer matches it while Rust offers its close: what landed the root, or that it no longer forks`)

#### Scenario: The merge toast of a piece
- **WHEN** a piece that landed the root merged
- **THEN** its toast says "#12 no longer matches the air stack: auth landed"
  [@test] src/stores/patronus.test.ts (`says that a piece that landed the root leaves the pull request of the whole stack, never closed by itself`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say that the pull request of the whole stack no longer matches it once a piece landed the root, and close it only when asked`)

### Requirement: The whole stack in the app: Close Pull Request asks in the root's row before it closes

"Close Pull Request #12…" (the preview's notice, the island's header, "⋯") MUST ask in the root's row before it closes
it with a comment that says why.

#### Scenario: The close from the preview's notice or the menu
- **WHEN** the user chooses "Close Pull Request #12…" in the preview's notice or "⋯"
- **THEN** a confirmation in the root's row asks first, and the pull request is closed with a comment that says why only when asked
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say that the pull request of the whole stack no longer matches it once a piece landed the root, and close it only when asked`)

#### Scenario: The close from the island's header
- **WHEN** the pull request no longer matches the stack
- **THEN** the island's header offers "Close Pull Request #12…"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should offer the close of the pull request of the whole stack in the island header once it no longer matches`)

### Requirement: The whole stack in the app: The close waits for the first sync on the frame of an earlier run

On the frame of an earlier run the close MUST wait for the first sync ("Waits for the first sync", in its hover).

#### Scenario: The rows of the frame
- **GIVEN** the window shows the frame of an earlier run
- **WHEN** the pull request of a stack that is no more is offered its close
- **THEN** the close waits for the first sync, "Waits for the first sync" in its hover, in the island header and its menu
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on the rows of the frame that the close of the pull request of the whole stack waits for the first sync, in the island header and its menu`)
  [@test] tests/e2e/startup/warm-start.spec.ts (`says the close of a pull request of a stack that is no more waits for the first sync on the rows of the frame`)
  [@test] crates/repository-model/src/state_test.rs (`the_pull_request_of_a_stack_that_is_no_more_closes_and_waits_for_the_first_sync_on_the_frame`)

### Requirement: The whole stack in the app: The close waits while another operation holds the state

While another operation holds the state of #12, the confirmation's Close Pull Request MUST be disabled with why.

#### Scenario: Another process holds the state of the pull request
- **WHEN** another operation holds the state of #12
- **THEN** the confirmation's Close Pull Request is disabled with why, and closes it after
  [@test] crates/domain-policy/src/actions_test.rs (`the_close_of_the_pull_request_of_a_whole_stack_runs_once_and_waits_for_what_holds_its_state`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should hold the confirmation of the close of the pull request of the whole stack while another process holds its state, and close it after`)

### Requirement: The whole stack in the app: Rust offers the close only of a pull request that no longer matches

Rust MUST decide it: it offers the close (`close-stack-pull-request`) only of an open pull request that no longer
matches the stack, so a surface that presents the actions of the stack never offers the close of one that matches.

#### Scenario: An open pull request that matches the stack and one that does not
- **WHEN** Rust lists the actions of the whole stack
- **THEN** it offers the close (`close-stack-pull-request`) of an open pull request once a landing took the root or the stack no longer forks
- **AND** never of one that matches the stack
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

### Requirement: The whole stack in the app: No new pull request of the whole stack while the root landed

While the root landed, a new pull request of the whole stack MUST NOT be created: it would show a stack that is no
more from its start ("Create Pull Request of the Stack…" disabled with what landed the root).

#### Scenario: The root landed
- **WHEN** the root of the stack landed
- **THEN** "Create Pull Request of the Stack…" is disabled with what landed the root
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

### Requirement: The Safe Push of the whole stack in the app: It starts as the start of a branch, under its key

The app MUST start the Safe Push and the Dry Run of the whole stack as the start of a branch is
([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)), its subject the key `stack:<root>`
(`requestSafePush`, `readSafePush`; the decision of `domain_policy::land::decide`, `RepositoryView.lands` under the
key): its facts are the stack branch, the robot of the whole stack and the profile of its last one, its pull request
and the readiness of the whole stack.

#### Scenario: The start of the whole stack from the header
- **WHEN** the user starts the Safe Push of the whole stack
- **THEN** the app asks it under the key `stack:<root>`, the decision of Rust under that key, on the facts of the whole stack
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: The Safe Push of the whole stack in the app: It always opens its popover, which says what lands

It lands more than a row, so it MUST always open its popover ("Safe Push of the whole air stack"): what lands ("Lands
7 branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and side-test.", the main line first, then
the side branches, `whole_stack_members`), and "Pushes only its stack branch user-name/stack/air.".

#### Scenario: The popover from the header of an island
- **WHEN** the user starts the whole stack from the header of its island
- **THEN** the popover "Safe Push of the whole air stack" opens at the header, saying what lands and "Pushes only its stack branch user-name/stack/air."
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

#### Scenario: The popover of a small stack
- **WHEN** the user starts the whole stack of a small stack with its side branch from the root's menu
- **THEN** its popover opens at the root, saying what lands
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)

#### Scenario: The sentence of what lands
- **WHEN** the popover says what lands
- **THEN** it says "Lands 7 branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and side-test.", the main line first, then the side branches, by `whole_stack_members`
  [@test] src/lib/whole-stack.test.ts (`lands its branches in one robot, the side branches named: they are what only it lands`)

### Requirement: The Safe Push of the whole stack in the app: The popover says what stays on the remote

Its popover MUST say "Pushes only its stack branch user-name/stack/air." and what stays only of the branches on the
remote ("…: the branches of the stack stay as they are on the remote.", "…: air and bazel stay as they are on the
remote.", `describeStackBranchPush`).

#### Scenario: Every branch of the stack on the remote
- **WHEN** every branch of the stack is on the remote
- **THEN** the popover says "…: the branches of the stack stay as they are on the remote."
  [@test] src/lib/whole-stack.test.ts (`pushes only its stack branch for its Safe Push, and says that its branches stay only of those on the remote`)

#### Scenario: Some branches of the stack on the remote
- **WHEN** only some branches of the stack are on the remote
- **THEN** the popover names only those: "…: air and bazel stay as they are on the remote."
  [@test] src/lib/whole-stack.test.ts (`pushes only its stack branch for its Safe Push, and says that its branches stay only of those on the remote`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say in the popover of the whole stack only what holds: the branches on the remote stay, no approvals where none are needed`)

### Requirement: The Safe Push of the whole stack in the app: The popover says that its approvals count in its pull request

With its pull request open and its approvals needed, its popover MUST say "Approvals count in #12, the pull request of
the whole stack." (never beside "No code-owner review needed.").

#### Scenario: Patronus judges its approvals
- **GIVEN** the pull request of the whole stack is open
- **WHEN** Patronus judges its approvals
- **THEN** the popover says "Approvals count in #12, the pull request of the whole stack."
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say in the popover of the whole stack that its approvals count in its pull request while Patronus judges them`)

#### Scenario: No approvals are needed
- **WHEN** no code-owner review is needed
- **THEN** the popover says "No code-owner review needed." and nothing of the approvals of its pull request
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say in the popover of the whole stack only what holds: the branches on the remote stay, no approvals where none are needed`)

### Requirement: The Safe Push of the whole stack in the app: The popover offers the start, Dry Run and Cancel

Its popover MUST offer the profile, the review, the robot of the whole stack it cancels; Dry Run, Cancel and the start
(Enter).

#### Scenario: The rest of the popover
- **WHEN** the popover of the whole stack is open
- **THEN** it shows the profile, the review and the robot of the whole stack it cancels
- **AND** it offers Dry Run, Cancel and the start, which Enter chooses
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: The Safe Push of the whole stack in the app: The start checks its gate, pushes the stack branch, starts it

The start (`execute` of the `Land` of the whole stack) MUST check its landing gate first (the landing needs of the view
are of branches: the engine runs the gate of the whole stack, `operations::land_session`), push the stack branch alone
while its remote lacks it (leased, a robot it replaces canceled first), then start the robot from it, or comment on its
pull request.

#### Scenario: Its build needs a branch outside it
- **WHEN** the build of the whole stack needs a branch outside it
- **THEN** the start is refused by the gate of the whole stack, with the restack and Start Anyway
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway`)

#### Scenario: The start pushes the stack branch and starts the robot
- **GIVEN** the remote of the stack branch lacks it
- **WHEN** the user starts the whole stack
- **THEN** the stack branch alone is pushed, leased, a robot it replaces canceled first
- **AND** the robot starts from it, or the start comments on its pull request
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: The Safe Push of the whole stack in the app: A straight chain starts its last branch

A straight chain, which has no stack branch, MUST start its last branch.

#### Scenario: The whole stack of a straight chain
- **WHEN** the user starts the whole stack of a straight chain
- **THEN** the start of its last branch runs

### Requirement: The Safe Push of the whole stack in the app: It is asked from the header, the root's menu and popovers

It MUST be asked from the header of an island (its slot, "Safe Push Whole Stack" and "Dry Run Whole Stack" of its "⋯",
S on the focused header), the popover at the header's slot ([Stack Island](../stack-island/spec.md#header)), from
"Whole Stack ›" of the root's menu, and from the Safe Push popover of the root and of any branch of a stack that forks
("Land the whole stack: Safe Push Whole Stack", [Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: The header of an island
- **WHEN** the user chooses the slot of the header, "Safe Push Whole Stack" or "Dry Run Whole Stack" of its "⋯", or presses S on the focused header
- **THEN** the popover opens at the header's slot
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`, `should start the whole stack from the island's ⋯ and with S on its header, the popover at the header`, `should check the whole stack in a Dry Run from the island's ⋯, its popover at the header`)

#### Scenario: Whole Stack › of the root's menu of a small stack
- **WHEN** the user chooses it in "Whole Stack ›" of the root's menu of a small stack
- **THEN** the popover opens at the root's row (`getWholeStackStartItems`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)

#### Scenario: The Safe Push popover of a branch of the stack
- **WHEN** the Safe Push popover of the root or of any branch of a stack that forks is open
- **THEN** it offers "Land the whole stack: Safe Push Whole Stack", which opens the popover of the whole stack at the header of the root's island, else at the root's row
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover`)

### Requirement: The Safe Push of the whole stack in the app: Its steps show where its popover opens

Its steps MUST show where its popover opens, as the start of a branch shows them in its slot: "Checking…" while the
facts load, the spinner and "45%" of its push, "Starting…", in the button of the header's slot, else in the root's slot
of a small stack, in place of the root's own step while the start runs (`NextStepSlot`); the push button of the root's
row stays the root's own meanwhile (`useStackBranchPushAlone`).

#### Scenario: The steps in the header's slot
- **WHEN** the start of the whole stack of an island runs
- **THEN** the button of the header's slot shows "Checking…" while the facts load, the spinner and "45%" of its push, then "Starting…"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

#### Scenario: The steps in the root's slot of a small stack
- **WHEN** the start of the whole stack of a small stack runs
- **THEN** the root's slot shows its steps in place of the root's own step (`NextStepSlot`)
- **AND** the push button of the root's row stays the root's own
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)

### Requirement: The Safe Push of the whole stack in the app: It lands only the rest after pieces landed

After merged Safe Pushes landed branches of it (a piece that landed its main line, `getLandedBy`), it MUST land only
the rest and say so: its label counts the rest ("Safe Push" for one branch left), its hover, menu item and popover
"Lands 1 branch · 1 commit into master in one Safe Push: session; auth, login and oauth landed with oauth"
(`StackLanding.rest`, `describeWholeStackLanding`); the stack branch still carries the landed commits, which Patronus
finds in the target.

#### Scenario: A piece landed its main line
- **WHEN** a merged Safe Push of a piece landed the main line of the stack
- **THEN** the label of the whole stack counts the rest, "Safe Push" for one branch left
- **AND** its hover, menu item and popover say "Lands 1 branch · 1 commit into master in one Safe Push: session; auth, login and oauth landed with oauth"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should land only what is left of the whole stack after a piece landed its main line, and offer nothing once every branch of it landed`)

### Requirement: The Safe Push of the whole stack in the app: Nothing is left once every branch landed

Once every branch of it landed, nothing is left: Safe Push Whole Stack and Dry Run Whole Stack MUST be disabled with
"Landed in master with oauth and the whole auth stack" ([Next Step](../next-step/spec.md#the-whole-stack)).

#### Scenario: Every branch of the stack landed
- **WHEN** merged Safe Pushes landed every branch of the stack
- **THEN** Safe Push Whole Stack and Dry Run Whole Stack are disabled with "Landed in master with oauth and the whole auth stack"
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should land only what is left of the whole stack after a piece landed its main line, and offer nothing once every branch of it landed`)

### Requirement: The Safe Push of the whole stack in the app: Its next step is the table of its stack branch

Its next step MUST be the table of [Next Step](../next-step/spec.md#the-whole-stack) on its facts, as Rust decides it
for its stack branch (`nextSteps` of the view under `stack:<root>`, its node beside it; the page words both,
`useStackNextStep`, `useStackLandingNode`).

#### Scenario: The rows of the table on the facts of the stack branch
- **WHEN** Rust decides the next step of the whole stack
- **THEN** it takes the rows of the table on the facts of its stack branch, under `stack:<root>`, its node beside it
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)

### Requirement: The Safe Push of the whole stack in the app: Its next step is disabled while it cannot land

Its next step MUST be disabled with the reason while the sync didn't build its stack branch ("The stack branch
conflicts at 0caf9066 in config.js: the stack lands in pieces", a Dry Run too), while a robot of a branch of it runs,
while its approvals are missing in its own pull request.

#### Scenario: The stack branch conflicts
- **WHEN** the stack branch conflicts
- **THEN** its Safe Push and its Dry Run are disabled with "The stack branch conflicts at 0caf9066 in config.js: the stack lands in pieces"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)

#### Scenario: A robot of a branch of it runs, or its approvals are missing
- **WHEN** a robot of a branch of the stack runs, or the approvals of the whole stack are missing in its own pull request
- **THEN** its next step is disabled with the reason
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)

### Requirement: The Safe Push of the whole stack in the app: Its readiness is asked while its header or root shows

Its readiness MUST be asked while the header of its island or its root's row shows
([Next Step](../next-step/spec.md#the-readiness-store)), so a small stack's menu and popovers know the missing
approvals before a click.

#### Scenario: The approvals of a small stack are missing
- **GIVEN** the root's row of a small stack shows
- **WHEN** the approvals of the whole stack are missing
- **THEN** "Safe Push Whole Stack" is disabled with "Code owners must approve in a pull request: IJPL Code Platform" beside "Create Pull Request of the Stack…", before a click
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask for the approvals of the whole stack of a small stack while its root's row shows, and offer its pull request where they are missing`)

#### Scenario: Before the answer
- **WHEN** Patronus has not answered yet
- **THEN** "Safe Push Whole Stack" is disabled with "Waits for Patronus to check the approvals" ([Facts without an answer](../next-step/spec.md#facts-without-an-answer))
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask for the approvals of the whole stack of a small stack while its root's row shows, and offer its pull request where they are missing`)

### Requirement: The Safe Push of the whole stack in the app: S on the header says why it cannot start

S on the header MUST say it in the notice, whose action is "Create Pull Request of the Stack…".

#### Scenario: S without its pull request
- **WHEN** the user presses S on the header while the whole stack can't start without its pull request
- **THEN** the notice says why, and its action "Create Pull Request of the Stack…" creates it
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on S why the whole stack can't start without its pull request, and create it from the notice`)

### Requirement: The Safe Push of the whole stack in the app: A stack branch not built lands the stack in pieces

A stack branch not built MUST land the stack in pieces: the "⋯" of the island offers the Safe Push of the last branch
of its main line, and so do the notice of S and the popover ("Land it in pieces").

#### Scenario: The stack branch conflicts
- **WHEN** the stack branch is not built, as it conflicts
- **THEN** the "⋯" of the island, the notice of S and the popover ("Land it in pieces") offer the Safe Push of the last branch of its main line
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

### Requirement: The Safe Push of the whole stack in the app: Its approvals count in its own pull request only

Its approvals MUST count in its own pull request only: the island's header says "Approvals count in #12, the pull
request of the whole stack", apart from the pull requests of the branches; a branch of it without a pull request of its
own, whose approvals are missing, shows a quiet "In #12" that opens it; the popover of a piece of the stack says that
the approvals of #12 don't count for it.

#### Scenario: The header and a branch without its own pull request
- **GIVEN** the pull request #12 of the whole stack is open
- **WHEN** the island shows
- **THEN** its header says "Approvals count in #12, the pull request of the whole stack", apart from the pull requests of the branches
- **AND** a branch of it without a pull request of its own, whose approvals are missing, shows a quiet "In #12" that opens it
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it`)

#### Scenario: The readiness key and the popover of a piece
- **WHEN** the readiness of the whole stack is kept, and the popover of a piece of the stack opens
- **THEN** the readiness is under a key no branch has, and the popover of the piece says that the approvals of #12 don't count for it
  [@test] src/lib/whole-stack.test.ts (`has the readiness of its stack branch under a key no branch has, and its approvals only in its own pull request`)

### Requirement: The Safe Push of the whole stack in the app: Request Review… pushes its stack branch first

Request Review… of its pull request MUST push its stack branch first.

#### Scenario: Request Review… of the pull request of the whole stack
- **WHEN** the user chooses Request Review… of the pull request of the whole stack
- **THEN** the stack branch is pushed first
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it, the stack waiting from the click`)

### Requirement: The Safe Push of the whole stack in the app: Approvals in its pull request are news

Approvals that arrive in its pull request MUST be news: "Ilya Korennoy approved the whole air stack" with "Safe Push
Whole Stack".

#### Scenario: An approval arrives
- **WHEN** an approval arrives in the pull request of the whole stack
- **THEN** the news says "Ilya Korennoy approved the whole air stack" with "Safe Push Whole Stack"
  [@test] src/lib/approval-news.test.ts (`names the whole stack for the approvals of its pull request`)

### Requirement: The Safe Push of the whole stack in the app: Its merge closes the pull requests of its branches

When its robot merged, the pull requests of its branches MUST be closed (above), in the app as the user sees it.

#### Scenario: Its Safe Push merged
- **WHEN** the Safe Push of the whole stack merged
- **THEN** the pull requests of every branch of the whole stack are closed, as the user sees it in the app
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should close the pull requests of every branch of the whole stack once its Safe Push merged`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/stack_sync_test.rs`, `stack_branch_test.rs`, `process_budget_test.rs`,
  `patronus_start_test.rs`; `crates/domain-policy/src/robot_match_test.rs`;
  `crates/remote-facts/src/stack_pull_request_test.rs`, `process_budget_test.rs`;
  `crates/bd-remote/src/status_test.rs`, `safe_push_test.rs`,
  `push_test.rs`, `robot_test.rs`.
- `pnpm test:unit`: `src/stores/branch-sync.test.ts`, `src/lib/patronus.test.ts`, `src/lib/whole-stack.test.ts`,
  `crates/domain-policy/src/landed_test.rs`, `src/stores/patronus.test.ts`, `crates/domain-policy/src/next_step_test.rs`.
- `pnpm e2e tests/e2e/stack-island`: the whole stack in the app (templates `stack_island`, `stack_side_conflict`);
  `tests/e2e/stacked-branches/stacked-branches.spec.ts` for a small stack.
- `pnpm perf:monorepo`: the full and the unchanged `brd sync` of the pinned monorepo clone, one stack that forks.

## References

- [Landing Work into Master](../../../../docs/design/landing.md): why the whole stack is one more virtual branch
- [Git Operations](../../../../docs/design/git-operations.md): "Stack branch", the build and the ref rules
- [Stacked Branches](../stacked-branches/spec.md): how branches stack
- [Stack Island](../stack-island/spec.md): the header the whole stack's Safe Push is in
- [Next Step](../next-step/spec.md#the-whole-stack): the step of the whole stack
