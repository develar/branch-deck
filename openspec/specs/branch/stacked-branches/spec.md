---
name: Stacked Branches
description: Requirements for stacked branches, where a `(parent/child)` prefix builds the child's virtual branch on its parent's, a parent without commits passed to the branch below it, problems and blocked parents reported per branch, and the root row heading its stack, which collapses into it; always on, without a setting.
style: plain-1
targets:
  - crates/history-rewrite/src/stack.rs
  - crates/history-rewrite/src/commit_grouper.rs
  - crates/sync-core/src/sync.rs
  - crates/sync-core/src/branch_processor.rs
  - crates/history-rewrite/src/autosquash.rs
  - crates/sync-core/src/remote_status.rs
  - crates/branch-integration/src/detector.rs
  - crates/branch-integration/src/detection_notes.rs
  - crates/branch-integration/src/rebase.rs
  - crates/branch-integration/src/merge.rs
  - crates/branch-integration/src/squash.rs
  - crates/sync-core/src/grouping.rs
  - crates/trial-check/src/check.rs
  - crates/domain-types/src/stack.rs
  - crates/domain-types/src/rewrite.rs
  - crates/domain-types/src/session.rs
  - crates/domain-policy/src/github_stack.rs
  - crates/domain-policy/src/facts.rs
  - crates/domain-policy/src/stack.rs
  - crates/repository-model/src/derive.rs
  - crates/repository-model/src/table.rs
  - src/model/structure.ts
  - src/model/store.ts
  - src/presenters/row-status.ts
  - src/stores/branch-sync.ts
  - src/lib/stack-lanes.ts
  - src/components/branch/BranchTableCard.tsx
  - src/components/branch/BranchNameCell.tsx
  - src/components/branch/ActiveBranchExpanded.tsx
  - src/components/branch/StackProblemAlert.tsx
  - src/lib/branch-drop.ts
  - src/components/branch/StackLanes.tsx
  - src/components/branch/BranchActions.tsx
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useStackActions.ts
  - src/components/branch/InlineUngroupConfirm.tsx
  - src/components/branch/StackChildrenNote.tsx
  - src/components/branch/InlineRenameBranchInput.tsx
  - crates/operations/src/rewrite.rs
  - crates/history-rewrite/src/regroup.rs
  - src/presenters/rewrite.ts
  - src/lib/stack.ts
  - src/components/commit/PrefixChangeHint.tsx
  - src/components/shared/InlineBranchTarget.tsx
  - src/hooks/useBranchNameValidation.ts
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - src/lib/push-summary.ts
  - crates/domain-policy/src/push.rs
  - src/lib/push-run.ts
  - crates/operations/src/push.rs
  - crates/sync-core/src/push.rs
  - src/components/branch/StackBaseLabel.tsx
  - crates/sync-core/src/repository_config.rs
  - src/components/branch/StackChip.tsx
  - src/components/branch/StackIslandCard.tsx
  - src/components/branch/SafePushPopover.tsx
  - src/presenters/push.ts
  - src/components/commit/PushUpToMark.tsx
  - src/hooks/useCommitMoveList.ts
  - src/lib/stack-state.ts
  - src/hooks/useStackStates.ts
  - src/lib/stack-collapse.ts
  - src/stores/stack-collapse.ts
  - src/stores/settings/repositorySettings.ts
  - src/hooks/useBranchRowKeys.ts
  - src/components/shared/MenuItems.tsx
  - src/components/branch/BranchSelectionBar.tsx
  - src/components/branch/BranchPushButton.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/branch/LandingSpanLabel.tsx
  - src/components/branch/LandedChip.tsx
  - src/lib/git/push-toast.tsx
  - src/presenters/pull-request-url.ts
  - src/lib/pull-request-dialog.ts
  - src/lib/pull-request-run.ts
  - src/components/branch/CreatePullRequestDialog.tsx
  - crates/remote-facts/src/pull_request_writes.rs
  - crates/operations/src/pull_request.rs
  - src/hooks/useBaselineOnRemote.ts
---

# Stacked Branches

Status: Draft
Date: 2026-10-15

## Purpose

A commit `(auth/login) Add form` belongs to the branch `login`, and `login` stacks on `auth`. Its virtual branch is
built on the new head of `auth`'s virtual branch. Its pull request goes into the baseline, as every pull request of a
virtual branch, and holds `auth`'s commits with its own: what its Safe Push lands ([Landing](../../../../docs/design/landing.md)).
Deeper prefixes declare the ancestry (`(auth/login/oauth)`), and a bare `(login)` joins `login` wherever
its parent is declared. A branch whose prefixes declare no parent, an issue branch and an issue group too, takes its
stack parent from `refs/branch-deck/stack-parents` ([Stack Parents](../stack-parents/spec.md)): Stack On, Unstack and
`brd group` with a path write it there, without a history rewrite. A parent without commits (landed, unapplied, mistyped) is passed: the child is built on the
nearest ancestor with commits, or the baseline. A branch that can't be placed, or whose parent can't be built, is
reported and not rebuilt; the other branches still sync. The branch table draws the stacks as a graph, and the branch
menu stacks, unstacks and renames branches.

## Goals

- Land a feature into master in dependent pieces, each a branch with its own commits on the ones it is built on.
- Keep linear development on the main branch: the prefixes and the stack parents are the stack state.
- The history decides where a branch is built: a parent without commits (landed, unapplied, mistyped) doesn't stop its
  children; the base change is visible in the row and the alert; only the user restacks a branch (Unstack).

## Non-goals

- A pull request into a parent's branch outside a stack of GitHub: Patronus merges a pull request into its base and
  counts only the approvals of the pull request that started it, and GitHub merges one into its base by the rules of
  that base, so such a pull request can never land ([Landing](../../../../docs/design/landing.md#alternatives-considered)).
  On the route of Patronus every pull request of a virtual branch goes into the baseline; elsewhere a straight chain is
  a stack of GitHub, whose layers go into their parents' branches and are judged by the rules of the target
  ([Stacks of GitHub](#stacks-of-github)). An open one into a virtual branch outside such a stack is retargeted
  ([Pull requests](#pull-requests)); one GitHub merged into a parent's branch landed nothing: the next step and
  `brd push` say so (`mergedIntoBranch`, `merged_elsewhere`), and a new pull request into the baseline lands it
  ([Next Step](../next-step/spec.md)).
- The pull requests of every branch of a stack at once: "Create Pull Request…" creates the pull request of one branch
  ([Pull Requests](../pull-requests/spec.md#creating-a-pull-request)), of a layer of a stack of GitHub with those of the
  layers below it that have none, as a stack of GitHub has a pull request on every layer. A stack that forks has one
  more, of its whole stack ([Whole Stack](../whole-stack/spec.md)). Branch Deck closes the pull requests a Safe Push landed
  ([Pull requests](#pull-requests)).

## Concepts

### Stacks of GitHub

On a route without Patronus GitHub's own stacks land a piece in one move with the review of each layer by its own diff
([Stacks of GitHub](../../../../docs/design/github-stacks.md)).

## Requirements

### Requirement: Always on: A slash in a prefix always declares a stack

A `/` in a prefix MUST always declare a stack: `(auth/login)` builds `login` on top of `auth` in every repository,
without a setting.

#### Scenario: A stacked prefix without any config
- **GIVEN** a repository without any setting of stacks
- **WHEN** a commit has the prefix `(auth/login)`
- **THEN** it groups into `login`, whose declared parent is `auth`, so `login` is built on top of `auth`
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`a_slash_in_a_prefix_stacks_without_any_config`)

### Requirement: Always on: An old stacks setting is ignored

A set `branchdeck.stacks` MUST be ignored; branches an older version grouped under a flat name with a `/` are not
migrated.

#### Scenario: The setting is set
- **GIVEN** `branchdeck.stacks` is set
- **WHEN** the commits are grouped
- **THEN** a `/` in a prefix still declares a stack
- **AND** a branch an older version grouped under a flat name with a `/` is not migrated

### Requirement: Prefix syntax: The last level names the branch, the one before it its parent

The last level of a prefix MUST name the branch and the level before it its parent. Each level MUST be sanitized like
a flat branch name.

#### Scenario: A prefix splits into sanitized levels
- **WHEN** a prefix has levels, such as `(auth/login/oauth)` or levels with spaces
- **THEN** the last level names the branch and the level before it its parent
- **AND** each level is sanitized like a flat branch name
  [@test] crates/domain-types/src/stack_test.rs (`stack_prefixes_split_into_sanitized_levels`)

### Requirement: Prefix syntax: Empty levels at the ends are dropped, one in the middle is a problem

Empty levels at the ends MUST be dropped; an empty level in the middle (`(a//b)`) MUST be a syntax problem of the last
level's branch.

#### Scenario: An empty level at an end
- **WHEN** a prefix has an empty level at its start or its end
- **THEN** that level is dropped
  [@test] crates/domain-types/src/stack_test.rs (`stack_prefixes_split_into_sanitized_levels`)

#### Scenario: An empty level in the middle
- **WHEN** a prefix has an empty level in the middle, as `(a//b)`
- **THEN** it is a syntax problem of the last level's branch
  [@test] crates/domain-types/src/stack_test.rs (`an_empty_stack_level_is_a_syntax_error`)
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`a_syntax_error_is_reported_on_the_last_level`)

### Requirement: Prefix syntax: Commits group by the leaf, every level pair declares an edge

Commits MUST be grouped by the leaf, and every level pair MUST declare an edge: `(auth/login/oauth)` also declares
`auth` as the parent of `login`.

#### Scenario: Deeper prefixes declare the parents of their levels
- **WHEN** commits have the prefixes `(auth/login)`, `(login)` and `(auth/login/oauth)`
- **THEN** each commit groups into the branch of its leaf
- **AND** `(auth/login/oauth)` also declares `auth` as the parent of `login`
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`groups_by_leaf_and_records_the_declared_parents`, `a_parent_without_commits_is_missing`)

### Requirement: Prefix syntax: An issue-key branch is a root

An issue-key branch MUST be a root unless the stack parents name its parent
([Stack Parents](../stack-parents/spec.md#placement)).

#### Scenario: An issue-key branch without a stored parent
- **GIVEN** the stack parents name no parent of an issue-key branch
- **WHEN** the commits are grouped
- **THEN** the issue-key branch is a root
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`a_syntax_error_is_reported_on_the_last_level`)

### Requirement: Problems: A stack problem marks only the affected branch

These problems MUST mark only the affected branch, with the `Error` status, naming the commits; the branch is not
rebuilt: commits declare different parents; the parents form a cycle (every branch of the cycle); `(a/a)`; two
branches differ only in case (both); a syntax problem.

#### Scenario: Commits declare different parents
- **WHEN** commits of a branch declare different parents
- **THEN** only that branch has the `Error` status, naming the commits
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`conflicting_parents_name_their_commits`)

#### Scenario: The parents form a cycle
- **WHEN** the declared parents form a cycle
- **THEN** every branch of the cycle has the `Error` status, naming the commits
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`a_cycle_marks_each_of_its_branches`)

#### Scenario: A branch is its own parent
- **WHEN** a commit has the prefix `(a/a)`
- **THEN** the branch `a` has the `Error` status, naming the commits
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`a_branch_can_not_be_its_own_parent`)

#### Scenario: Two branches differ only in case
- **WHEN** two branches differ only in case
- **THEN** both have the `Error` status, naming the commits
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`leaves_that_differ_only_in_case_collide`)

#### Scenario: A syntax problem
- **WHEN** a prefix has a syntax problem
- **THEN** the branch of its last level has the `Error` status, naming the commits

#### Scenario: A branch with a problem is not rebuilt
- **WHEN** the sync meets a branch with a problem
- **THEN** that branch is not rebuilt
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_problem_is_not_built_and_a_missing_parent_is_passed`)

### Requirement: Problems: Rust says each problem once, for the app and brd

Each problem MUST be said once, by Rust, for the app and `brd` (`problem_view`, `StackProblemView`): a label (the
status of the row), a reason naming the branch ("login has conflicting parents"), a message without commands and its
fixes (`StackFix`).

#### Scenario: The view of conflicting parents
- **WHEN** a branch `login` has conflicting parents
- **THEN** its problem has a label, the reason "login has conflicting parents", a message without commands and its fixes
  [@test] crates/history-rewrite/src/stack_test.rs (`each_problem_offers_the_fixes_that_remove_it`)

### Requirement: Problems: The fixes of a problem remove it and add none

The fixes MUST be only those that remove the problem and add none.

#### Scenario: The fixes of each problem
- **WHEN** a branch has a problem
- **THEN** its fixes are: conflicting parents, Stack On each parent except one stacked on the branch (a cycle), or
  Unstack; a cycle, Unstack; `(a/a)`, Unstack while no other parent is declared, else Reword of its commits; a case
  collision, Rename; an empty level, Reword of its commits
  [@test] crates/history-rewrite/src/stack_test.rs (`each_problem_offers_the_fixes_that_remove_it`)

#### Scenario: Each restack a problem offers removes it
- **WHEN** a restack that a problem offers as its fix runs
- **THEN** the problem is gone
  [@test] crates/sync-core/src/stack_operations_test.rs (`each_restack_a_stack_problem_offers_removes_it`)

### Requirement: Problems: brd prints the fixes as commands

`brd` MUST print the fixes as commands after the message (`fix: `brd restack login auth` or `brd unstack login``).

#### Scenario: A problem in the output of brd
- **WHEN** `brd` reports a branch with a problem
- **THEN** it prints the message, then the fixes as commands
  [@test] crates/history-rewrite/src/stack_test.rs (`each_problem_offers_the_fixes_that_remove_it`)

### Requirement: Problems: A child of a branch with a problem is blocked

A child of a branch with a problem MUST be blocked with the reason `problem`, its text naming the problem
("Blocked: parent login has a stack problem (conflicting parents)"), not a failure.

#### Scenario: The parent has conflicting parents
- **GIVEN** a parent `login` with conflicting parents
- **WHEN** the sync places its child
- **THEN** the child is blocked with the reason `problem`, not a failure
- **AND** its text is "Blocked: parent login has a stack problem (conflicting parents)"
  [@test] crates/history-rewrite/src/stack_test.rs (`a_parent_with_a_problem_blocks_its_children`)
  [@test] crates/bd-remote/src/status_test.rs (`stacks_in_status`)

### Requirement: Problems: A regroup that would introduce a problem is refused

A regroup that would introduce a problem MUST be refused by its reason, without a fix: the plan changed nothing yet.

#### Scenario: The plan introduces a problem
- **WHEN** the plan of a regroup would introduce a problem
- **THEN** the regroup is refused by the reason of the problem, without a fix
  [@test] crates/sync-core/src/regroup_test.rs (`refuses_problems_the_plan_introduces_and_keeps_existing_ones`)

### Requirement: Placement: A child is built on the new head of its active parent

A child MUST be built on the new head of its active parent's virtual branch, also when a child commit comes before
every parent commit on the main branch (`childPrecedesParent`).

#### Scenario: A child on its active parent
- **GIVEN** a stack `auth`, `login` on it and `oauth` on `login`
- **WHEN** the sync builds the stack
- **THEN** each child is built on the new head of its parent's virtual branch
  [@test] crates/history-rewrite/src/stack_test.rs (`a_child_is_built_on_its_active_parent`)
  [@test] crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)

#### Scenario: A child commit comes before every parent commit
- **WHEN** a child commit comes before every parent commit on the main branch
- **THEN** the child is still built on the new head of its parent's virtual branch, marked `childPrecedesParent`
  [@test] crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)

### Requirement: Placement: A parent without commits takes its state from its branch

A parent without commits MUST be `Unapplied` with an unapplied branch of its name, `Integrated` when its branch is
integrated, else `Missing`. Its branch is its virtual branch, which the sync is about to archive, else its newest
archived branch; a detection note says whether it is integrated.

#### Scenario: The states of parents without commits
- **WHEN** a parent has no commits
- **THEN** it is `Unapplied` when an unapplied branch of its name exists
- **AND** `Integrated` when its branch (its virtual branch, else its newest archived branch) is integrated by its
  detection note
- **AND** `Missing` else
  [@test] crates/history-rewrite/src/stack_test.rs (`reads_the_state_of_parents_without_commits_from_their_branches`)

### Requirement: Placement: The state of a parent without commits is only a label

The state MUST be a label (the row, `brd status`); the placement MUST NOT depend on it.

#### Scenario: Any state places the child the same way
- **WHEN** a parent without commits is `Unapplied`, `Integrated` or `Missing`
- **THEN** the row and `brd status` show the state
- **AND** the child is placed the same way for each

### Requirement: Placement: The branch of a parent without commits is detected before the placement

The branch of a parent without commits (its virtual branch, else its newest archived one) MUST be detected before the
branches are placed by the rule of the archive step: when it has no note, a `NotIntegrated` note the baseline moved
on from (incrementally), or a note without a usable baseline (in full; an archived branch only within the retention
period), so the first sync after the parent's merge already labels the parent landed, also when the parent was
archived a sync before.

#### Scenario: A virtual branch that lost its commits, without a note
- **GIVEN** a parent's commits are integrated and its virtual branch has no note
- **WHEN** the first sync after the parent's merge runs
- **THEN** the branch is detected before the placement, the parent is labelled landed, and the child is built on the
  baseline
  [@test] crates/history-rewrite/src/stack_test.rs (`a_parent_whose_virtual_branch_lost_its_commits_is_detected_before_the_placement`)
  [@test] crates/sync-core/src/detection_stack_test.rs (`the_first_sync_after_the_parent_is_integrated_builds_the_child_on_the_baseline`)

#### Scenario: A stale note the baseline moved on from
- **GIVEN** a `NotIntegrated` note the baseline moved on from, on the parent's virtual tip or on its archived branch,
  archived a sync before
- **WHEN** the sync places the branches
- **THEN** the branch is detected again, incrementally, and the note does not hide an integrated parent
  [@test] crates/sync-core/src/detection_recheck_test.rs (`a_stale_note_on_the_virtual_tip_does_not_hide_an_integrated_parent`, `a_stale_note_on_the_archived_parent_does_not_hide_an_integrated_parent`)

#### Scenario: A note without a usable baseline
- **GIVEN** a note of the parent's branch without a usable baseline
- **WHEN** the sync places the branches
- **THEN** the branch is detected in full, an archived branch only within the retention period

### Requirement: Placement: The sync writes the note of the parent, brd only reads

The sync MUST write the note, and its archiving reports the parent from it; `brd status` and `brd check` MUST detect
without writing.

#### Scenario: The sync detects the parent
- **WHEN** the sync detects the branch of a parent without commits
- **THEN** it writes the note, and its archiving reports the parent from it
  [@test] crates/sync-core/src/detection_stack_test.rs (`the_first_sync_after_the_parent_is_integrated_builds_the_child_on_the_baseline`)

#### Scenario: brd status before the next sync
- **GIVEN** a parent integrated since the last sync
- **WHEN** `brd status` or `brd check` runs
- **THEN** the parent is integrated before the next sync, detected without writing a note
  [@test] crates/bd-remote/src/status_test.rs (`a_parent_integrated_since_the_last_sync_is_integrated_before_the_next_one`)

### Requirement: Placement: A parent without commits moves the child to its nearest active ancestor

A parent without commits, whatever its state, MUST move the child to its nearest active ancestor, or the baseline,
with `prBaseChanged`: the local history is linear on the baseline, so the child's commits can be picked there like
those of any flat branch (or conflict like them).

#### Scenario: An integrated or a missing parent
- **WHEN** the parent of a child is integrated, or has no commits
- **THEN** the child is built on its nearest active ancestor, or the baseline, with `prBaseChanged`
  [@test] crates/history-rewrite/src/stack_test.rs (`an_integrated_parent_moves_the_child_to_the_nearest_active_ancestor`, `a_parent_without_commits_moves_the_child_to_the_nearest_ancestor_with_commits`)
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_problem_is_not_built_and_a_missing_parent_is_passed`)

#### Scenario: An unapplied parent
- **WHEN** the parent of a child is unapplied
- **THEN** the child is built on the baseline with `prBaseChanged`, reported before the sync starts
  [@test] crates/sync-core/src/stack_sync_test.rs (`an_unapplied_parent_is_reported_before_the_sync_starts`)

#### Scenario: The parent without commits is itself on a blocked branch
- **GIVEN** a declared parent without commits, stacked on a branch with a problem
- **WHEN** the sync places the child
- **THEN** the child passes the parent without commits, which stays its declared parent, and the branch below blocks it
  [@test] crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)
  [@test] crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_blocked_branch_follows_its_declared_parent_not_what_blocks_it`)

### Requirement: Placement: A parent with a problem or a blocked parent blocks the child

A parent with a problem and a blocked parent MUST block the child: the `Blocked` status, the reason as its text, its
commits blocked, its virtual branch left as it was.

#### Scenario: A parent with a problem
- **GIVEN** a parent with a stack problem
- **WHEN** the sync places its child
- **THEN** the child has the `Blocked` status, the reason as its text, its commits blocked, its virtual branch left as
  it was
  [@test] crates/history-rewrite/src/stack_test.rs (`a_parent_with_a_problem_blocks_its_children`)

#### Scenario: A blocked parent
- **GIVEN** a parent blocked by a branch below it
- **WHEN** the sync places its child
- **THEN** the child is blocked the same way
  [@test] crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)
  [@test] crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_blocked_branch_follows_its_declared_parent_not_what_blocks_it`)

### Requirement: Placement: A parent that conflicts or fails blocks its descendants

A parent that conflicts or fails while it is built MUST block its descendants the same way.

#### Scenario: The parent conflicts
- **WHEN** a parent conflicts or fails while it is built
- **THEN** its descendants have the `Blocked` status, the reason as their text, their virtual branches left as they
  were
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_parent_conflict_blocks_its_descendants`)

### Requirement: Sync: A child starts once its parent's head is written

A child MUST start once its parent's head is written, without holding a slot of the git process limit while it waits.

#### Scenario: A child waits for its parent
- **WHEN** the sync builds a child whose parent is still being built
- **THEN** the child starts once its parent's head is written
- **AND** it holds no slot of the git process limit while it waits

### Requirement: Sync: Ready branches start longest chain first

Ready branches MUST start longest chain first.

#### Scenario: Several branches are ready
- **WHEN** several branches are ready to be built
- **THEN** the one with the longest chain starts first

### Requirement: Sync: A new parent commit moves the children onto its new head

A new parent commit MUST move the children onto the parent's new head.

#### Scenario: The parent gets a commit
- **WHEN** a parent gets a new commit and the sync runs
- **THEN** its children are built on the parent's new head
  [@test] crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)

### Requirement: Sync: An unchanged stack reuses every commit

An unchanged stack MUST reuse every commit.

#### Scenario: Nothing changed
- **WHEN** the sync runs again on an unchanged stack
- **THEN** every commit is reused
  [@test] crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)

### Requirement: Remote status: The unpushed commits of a child exclude its parent's commits

The unpushed commits of a child MUST exclude its parent's commits (`^<parent>` next to the baseline exclusion), after a
sync and after a push of the child.

#### Scenario: After a sync and after a push of the child
- **WHEN** a sync builds a child, or the child is pushed
- **THEN** its unpushed commits exclude its parent's commits, by `^<parent>` next to the baseline exclusion
  [@test] crates/sync-core/src/stack_sync_test.rs (`the_unpushed_commits_of_a_child_exclude_its_parent`)

### Requirement: Remote status: In the sync the parent is the head the child was built on

In the sync the parent MUST be the new head the child was built on, not the parent's ref: a child can finish before
its parent, whose ref is then absent (a new stack) or at its old head. After a push the parent's ref is current.

#### Scenario: A child finishes before its parent
- **WHEN** a child finishes in the sync before its parent, whose ref is absent (a new stack) or at its old head
- **THEN** its unpushed commits exclude the parent by the new head the child was built on
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_child_finished_before_its_parent_excludes_the_parent_by_the_head_it_was_built_on`)

#### Scenario: After a push
- **WHEN** the child is pushed
- **THEN** its unpushed commits exclude the parent by the parent's ref, which is current
  [@test] crates/sync-core/src/stack_sync_test.rs (`the_unpushed_commits_of_a_child_exclude_its_parent`)

### Requirement: Remote status: A remote status that can't be computed keeps the old one

A remote status that can't be computed MUST be logged as a warning, and the branch keeps the status it had.

#### Scenario: The remote status fails
- **WHEN** the remote status of a child can't be computed
- **THEN** it is logged as a warning
- **AND** the branch keeps the status it had

### Requirement: Remote status: The older versions of the parent's commits are restacked

A child whose parent (or any branch below it) was amended, reworded or rebuilt since the child was pushed has the
parent's commits as they were on its remote branch. Those MUST be restacked, not dropped (a reworded one, which the
remote has by content, too: its message there is the old one): their identity (author email, author time, subject
without the prefix) is that of a commit of a branch it is built on.

#### Scenario: An amended parent
- **GIVEN** a child pushed, then its parent amended
- **WHEN** the remote status of the child is computed
- **THEN** the older versions of the parent's commits on the child's remote branch are restacked, not dropped
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_of_an_amended_parent_is_restacked`)

#### Scenario: A reworded parent
- **GIVEN** a child pushed, then a commit of its parent reworded
- **WHEN** the remote status of the child is computed
- **THEN** the reworded commit, which the remote has by content with its old message, is restacked too
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_of_a_reworded_parent_is_restacked`)

### Requirement: Remote status: A restacked child says so and its push asks nothing

The status MUST read `restacked` (`4↑ · restacked`), "Rebuilt on the new auth: the remote has older versions of 2 of
its commits.", the push MUST be a `primary` Force Push that asks nothing, also with nothing of the child's own to push,
and Push Stack takes such a branch along ([Outgoing Commits](../../repository/outgoing/spec.md#what-is-outgoing)).

#### Scenario: The row of a restacked child
- **GIVEN** a child with restacked commits of its parent
- **WHEN** its row shows
- **THEN** its status reads `restacked` (`4↑ · restacked`), "Rebuilt on the new auth: the remote has older versions of
  2 of its commits."
- **AND** its push is a `primary` Force Push that asks nothing, also with nothing of the child's own to push

#### Scenario: Push Stack takes it along
- **WHEN** a Push Stack of a branch above it runs
- **THEN** it takes the restacked branch along
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)

### Requirement: Remote status: The identities of the ancestors are built once

The sync MUST build the identities of every branch's ancestors once, from the commits it grouped, along the chain of
effective bases; a push outside the sync reads them from the parent's virtual branch with one `rev-list`, only while a
remote commit matches nothing of the child.

#### Scenario: In the sync
- **WHEN** the sync computes the remote statuses
- **THEN** it builds the identities of every branch's ancestors once, from the commits it grouped, along the chain of
  effective bases

#### Scenario: A push outside the sync
- **WHEN** a push outside the sync computes the remote status of a child
- **THEN** it reads the identities from the parent's virtual branch with one `rev-list`, only while a remote commit
  matches nothing of the child

### Requirement: Remote status: brd reports restacked commits and pushes without force

`brd` MUST report them as `remote.restacked` and push such a branch without `--force`
([Agent CLI](../../cli/agent-cli/spec.md#brd-push)).

#### Scenario: brd push of a restacked child
- **GIVEN** a child with amended and restacked commits
- **WHEN** `brd` reports and pushes it
- **THEN** it reports them as `remote.restacked`
- **AND** the push runs without `--force`
  [@test] crates/bd-remote/src/push_test.rs (`amended_and_restacked_commits_push_without_force`)

### Requirement: Integration detection: A rebuilt child gets a stack note

A rebuilt child MUST get a stack note (`refs/notes/branch-deck/stack`) on its tip naming the commit it was built on,
written with the mapping notes in one `fast-import` run.

#### Scenario: The sync rebuilds a child
- **WHEN** the sync rebuilds a child
- **THEN** its tip gets a stack note (`refs/notes/branch-deck/stack`) naming the commit it was built on
- **AND** the note is written with the mapping notes in one `fast-import` run

### Requirement: Integration detection: An archived child is detected by the commits after its base

Detection of an archived child MUST look only at the commits after that base: `^<base>` in the cherry-mark walk and
the merge count, the base as the merge base of the squash check.

#### Scenario: A merged stacked branch
- **GIVEN** an archived child merged into the baseline
- **WHEN** it is detected
- **THEN** the merge count counts only its own commits, after its base
  [@test] crates/sync-core/src/detection_merge_test.rs (`a_merged_stacked_branch_counts_only_its_own_commits`)

#### Scenario: A rebased or squashed stacked branch
- **WHEN** an archived child is detected by the cherry-mark walk or the squash check
- **THEN** the walk excludes `^<base>` and the squash check takes the base as its merge base

### Requirement: Integration detection: A child whose own commits are upstream is integrated

A child whose own commits are upstream MUST be integrated even if its parent is not.

#### Scenario: Its own commits are upstream, its parent's are not
- **GIVEN** an archived child whose own commits are upstream and whose parent's are not
- **WHEN** it is detected
- **THEN** it is integrated
  [@test] crates/sync-core/src/detection_stack_test.rs (`an_archived_child_is_integrated_when_its_own_commits_are`)

### Requirement: Integration detection: The result of a stacked child is cached like any other

The result of a stacked child MUST be cached like any other: a `NotIntegrated` note records the baseline it was
checked against and the commits without an equivalent, and is checked again against the baseline commits since then
when the baseline moves ([Integration Detection](../../../../docs/design/integration-detection.md)).

#### Scenario: A stacked child not integrated yet
- **WHEN** an archived child is detected as not integrated
- **THEN** it counts only its own commits
- **AND** its `NotIntegrated` note records the baseline it was checked against and the commits without an equivalent
  [@test] crates/sync-core/src/detection_stack_test.rs (`an_archived_child_counts_only_its_own_commits_and_is_cached_with_its_orphaned_commits`)
  [@test] crates/branch-integration/src/detection_notes_test.rs (`a_not_integrated_note_records_its_baseline_and_orphaned_commits`, `a_final_result_records_no_baseline`, `a_version_1_note_has_no_check_data`)

#### Scenario: The baseline moves
- **GIVEN** a `NotIntegrated` note
- **WHEN** the baseline moves
- **THEN** the note is checked again against the baseline commits since then, as the note of any other branch is
  [@test] crates/sync-core/src/detection_recheck_test.rs (`a_not_integrated_note_is_rechecked_when_the_baseline_moves`, `partial_integration_is_tracked_incrementally`, `a_recheck_compares_only_the_baseline_commits_since_the_note`, `a_v1_note_within_retention_is_detected_afresh`, `a_v1_note_beyond_retention_is_reported_as_is`, `a_rewound_baseline_falls_back_to_a_full_detection`, `two_archived_names_with_one_tip_are_rechecked_once`, `a_fast_forwarded_branch_names_its_tip_as_the_integration_commit`)

### Requirement: Pull requests: Every pull request goes into the baseline

The pull request of every virtual branch, stacked or not, MUST go into the baseline as the remote names it (`master`,
`pullRequestBaseOf`): a stacked branch's holds the commits of the branches it is built on with its own, what its Safe
Push lands, so its approvals are the ones Patronus counts ([Landing](../../../../docs/design/landing.md)).

#### Scenario: The base of a stacked branch and of a flat one
- **WHEN** the base of the pull request of a virtual branch is decided, stacked or not
- **THEN** it is the baseline as the remote names it (`master`)
  [@test] src/lib/stack.test.ts (`is the baseline as the remote names it for every branch, stacked or not`)

### Requirement: Pull requests: Create Pull Request goes into the baseline and says what it holds

"Create Pull Request…" MUST create it into the baseline ([Pull Requests](../pull-requests/spec.md#creating-a-pull-request)),
and its dialog MUST say what it holds ("Includes the commits of auth and login, which oauth is built on: 3 branches · 12
commits").

#### Scenario: The pull request of a stacked branch
- **WHEN** the user opens "Create Pull Request…" of `oauth`, built on `auth` and `login`
- **THEN** the pull request is created into the baseline
- **AND** its dialog says "Includes the commits of auth and login, which oauth is built on: 3 branches · 12 commits"
  [@test] src/lib/pull-request-dialog.test.ts (`creates the pull request of a stacked branch into the baseline, with the commits of the branches it is built on`)

#### Scenario: A branch built on the baseline
- **WHEN** the user opens "Create Pull Request…" of a branch built on the baseline
- **THEN** its dialog includes nothing more
  [@test] src/lib/pull-request-dialog.test.ts (`includes nothing more for a branch built on the baseline`)

#### Scenario: The form of a pushed branch
- **WHEN** the pull request form of a pushed branch opens
- **THEN** it goes into its base, the baseline
  [@test] src/lib/pull-request.test.ts (`opens the pull request form of the pushed branch into its base`)

### Requirement: Pull requests: brd push prints gh pr create into the baseline

`brd push` MUST print its `gh pr create` with `--base master` ([Agent CLI](../../cli/agent-cli/spec.md#brd-push)).

#### Scenario: brd push of a stacked branch without a pull request
- **WHEN** `brd push` pushes a stacked branch
- **THEN** it prints the `gh pr create` of its pull request with `--base master`

### Requirement: Pull requests: The toast after a push is the push toast of any branch

After a push, the toast MUST be the push toast of any branch ([Pull Requests](../pull-requests/spec.md#creating-a-pull-request)):
it says what the push did and offers "Create Pull Request…" (into the baseline) or "Open Pull Request". It names no
base: every branch has the same.

#### Scenario: A branch without a pull request
- **WHEN** the push of a branch without a pull request succeeds
- **THEN** its toast says what the push did and offers "Create Pull Request…" into the baseline, naming no base
  [@test] src/lib/push-run.test.ts (`offers to create the pull request of a branch without one, into the baseline`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)

#### Scenario: A branch with a pull request
- **WHEN** the push of a branch with a pull request succeeds
- **THEN** its toast offers "Open Pull Request"

### Requirement: Pull requests: A pull request into a virtual branch is retargeted before the push

An open pull request whose base (`baseRefName`) is a virtual branch of the user (`{prefix}/virtual/…`,
`PullRequestBase::Virtual` of `domain_policy::pull_request::base_of`) MUST be retargeted to the baseline
(`PATCH pulls/<n>` with the new `base`, as `gh pr edit … --base …`) right before a push of its branch, each branch of a
stack push too. Its approvals don't count until then, and Patronus would merge it into that branch.

#### Scenario: A pull request into the branch of its parent
- **GIVEN** an open pull request into the branch of its parent, opened when pull requests went into the parent's
  branch, or of a parent that landed
- **WHEN** its branch is pushed, alone or in a stack push
- **THEN** the pull request is retargeted to the baseline right before the push
  [@test] src/lib/push-run.test.ts (`retargets a pull request into a virtual branch to the baseline before the push, and says it in the toast of the push`)
  [@test] crates/test-server/src/github_host_test.rs (`a_push_retargets_the_pull_request_of_its_branch_out_of_a_virtual_branch_first`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)

#### Scenario: The retarget on GitHub
- **WHEN** a pull request is retargeted
- **THEN** it is `PATCH pulls/<n>` with the new `base`, as `gh pr edit … --base …`
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`retargets_a_pull_request_of_the_repository_of_the_branches`)
  [@test] crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)

### Requirement: Pull requests: A pull request into another base is left as it is

A pull request into another base (a release branch) MUST be left as it is: that base may be meant
([Next Step](../next-step/spec.md), row 5′).

#### Scenario: A pull request into a release branch
- **GIVEN** an open pull request into a release branch
- **WHEN** its branch is pushed
- **THEN** the pull request is left as it is

### Requirement: Pull requests: The push button says each retarget before the push

Before the push, the description of the push button MUST say each retarget ("Retargets #8 to master." per pull
request, "Retargets #8 of login to master." for a stack push), and in `warning` for one with approvals: "Retargets #8
to master: GitHub may dismiss its approvals."

#### Scenario: The description of a push that retargets
- **GIVEN** a branch, or a branch of a stack push, whose pull request goes into a virtual branch
- **WHEN** the description of the push button shows
- **THEN** it says "Retargets #8 to master." per pull request, "Retargets #8 of login to master." for a stack push
- **AND** for one with approvals, in `warning`: "Retargets #8 to master: GitHub may dismiss its approvals."
  [@test] src/presenters/push-button.test.ts (`says which pull requests into a virtual branch the push retargets first, and warns when GitHub may dismiss approvals`)

### Requirement: Pull requests: The toast and Activity say each retarget

The push toast MUST say it after ("1 commit · created the remote branch · retargeted pull request #802 to master"),
and Activity MUST record each: "Retargeted #802 to master" / "login · it went into auth", with "Open Pull Request"
([Activity](../../repository/activity/spec.md)).

#### Scenario: A push that retargeted a pull request
- **WHEN** a push that retargeted a pull request succeeds
- **THEN** its toast says "1 commit · created the remote branch · retargeted pull request #802 to master"
- **AND** Activity records "Retargeted #802 to master" / "login · it went into auth", with "Open Pull Request"
  [@test] src/lib/push-run.test.ts (`retargets a pull request into a virtual branch to the baseline before the push, and says it in the toast of the push`)

### Requirement: Pull requests: A failed retarget does not stop the push

A failed retarget MUST be a "Cannot retarget pull request #802" error and the push still runs; its toast then says
"the pull request goes into auth: retarget it to master" with "Copy gh pr edit".

#### Scenario: GitHub refuses the retarget
- **WHEN** the retarget of a pull request fails
- **THEN** it is a "Cannot retarget pull request #802" error, and the push still runs
- **AND** the toast of the push says "the pull request goes into auth: retarget it to master" with "Copy gh pr edit"
  [@test] src/lib/push-run.test.ts (`pushes and copies the command that retargets a pull request when the retarget fails`)
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_failure_says_what_github_said`)

### Requirement: Pull requests: Copy Retarget Command copies the command before a push

"Copy Retarget Command" in the copy group of the branch menu MUST copy the command before a push.

#### Scenario: The branch menu of a branch whose pull request goes into a virtual branch
- **WHEN** the user chooses "Copy Retarget Command" in the copy group of the branch menu
- **THEN** the command that retargets the pull request is copied, before any push

### Requirement: Pull requests: brd push and brd safe-push retarget the same way

`brd push` and `brd safe-push` MUST retarget the same way ([Agent CLI](../../cli/agent-cli/spec.md#brd-push)).

#### Scenario: brd pushes a branch whose pull request goes into a virtual branch
- **WHEN** `brd push` or `brd safe-push` pushes a branch whose pull request goes into a virtual branch
- **THEN** it retargets the pull request to the baseline right before the push

### Requirement: Pull requests: The push button pushes the stack while ancestors have work

The push button of a branch whose ancestors have work for the remote MUST push the stack (`BranchPush.button`,
`domain_policy::push::branch_push`): the remote branches of its ancestors, and their pull requests, stay what the
branch is built on.

#### Scenario: An ancestor has work for the remote
- **GIVEN** a stacked branch whose ancestor has work for the remote
- **WHEN** the user presses its push button
- **THEN** it pushes the stack, parents first
  [@test] src/presenters/push-button.test.ts (`pushes the stack while an ancestor has work for the remote, and offers the push of the branch alone`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)

### Requirement: Pull requests: The button of a stack push counts what it sends

The compact button MUST count the commits the pushes send ("↑ 3"), in the strongest colour of them; its name is
`Push Stack (3 branches)`, and so is the push of the branch menu, the same push
([Branch Row](../branch-row/spec.md#branch-menu)).

#### Scenario: The compact button of a stack push
- **WHEN** the push button of a branch pushes the stack
- **THEN** it counts the commits the pushes send ("↑ 3"), in the strongest colour of them, also the colour of an
  ancestor when none of the branch's commits are the user's
- **AND** its name is `Push Stack (3 branches)`, as is the push of the branch menu
  [@test] src/presenters/push-button.test.ts (`takes the colour of an ancestor when none of the branch's commits are the user's`)

### Requirement: Pull requests: A stack push starts above the nearest held ancestor

The push MUST start above the nearest ancestor a running Safe Push holds ([Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)):
its push won't land.

#### Scenario: A Safe Push holds an ancestor
- **GIVEN** a running Safe Push holds an ancestor of the branch
- **WHEN** the push of the stack is planned
- **THEN** it starts above that ancestor

### Requirement: Pull requests: The description of a stack push lists its pushes

The description of the button MUST list the pushes, "Pushes 3 branches, parents first:" and a line per branch. A
force push MUST say what it replaces or drops in the words of the push alone
([Outgoing Commits](../../repository/outgoing/spec.md#push)).

#### Scenario: A line per branch
- **WHEN** the description of a stack push shows
- **THEN** it says "Pushes 3 branches, parents first:" and a line per branch ("login · 2 commits · creates the remote
  branch", "auth · 1 commit · force push")
  [@test] src/presenters/push-button.test.ts (`pushes the stack while an ancestor has work for the remote, and offers the push of the branch alone`)

#### Scenario: A force push in the stack
- **WHEN** a branch of the stack push is a force push
- **THEN** its line says what it replaces or drops, in the words of its push alone: "auth · 1 commit · force push,
  replaces 1 commit amended here", "login · force push, replaces the older versions of auth's commits", "session · 1
  commit · force push, drops 1 remote commit"
  [@test] src/presenters/push-button.test.ts (`says what each force push of a stack push replaces, as the push of the branch alone does`)
  [@test] src/lib/remote-status.test.ts (`names it in the line of a stack push in the same words, or the remote commits the push drops`)

### Requirement: Pull requests: A stack push offers the push of the branch alone as the second way

Below a rule, the description MUST offer the push of the branch alone, with a `Push oauth Only` button, which is also
the item after the push in the branch menu (`push-branch-only`, no key; its "⋯" in the preview and the review window
too): the second way, never the lead ([Landing](../../../../docs/design/landing.md)).

#### Scenario: A parent not on the remote yet
- **GIVEN** `oauth` on `login`, which isn't on the remote yet
- **WHEN** the description of the push button of `oauth` shows
- **THEN** below a rule it says "login isn't on the remote yet. Pushing oauth alone works: its pull request includes
  login's commits either way." and offers a `Push oauth Only` button
  [@test] src/presenters/push-button.test.ts (`pushes the stack while an ancestor has work for the remote, and offers the push of the branch alone`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stacked branch alone from the description of its button`)

#### Scenario: A parent on the remote with work
- **GIVEN** the nearest ancestor with work, `login`, is on the remote
- **WHEN** the description of the push button of `oauth` shows
- **THEN** it says "login has commits the remote lacks. Pushing oauth alone works: its pull request includes them
  either way, and the remote branch of login stays as it is."

#### Scenario: The branch menu
- **WHEN** the user opens the branch menu of a stacked branch whose push pushes the stack
- **THEN** the push of the branch alone (`push-branch-only`, no key) is the item after the push, also in "⋯" of the
  preview and the review window
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stacked branch alone from its menu, the second way after the push of the stack`)
  [@test] src/hooks/useBranchContextActions.test.ts (`offers the push of the branch alone after the push of the stack, as the description of the button does`)

### Requirement: Pull requests: P of the preview pushes as the button does

Preview `P` MUST push as the button does.

#### Scenario: P in the preview of a stacked branch
- **WHEN** the user presses `P` in the preview of a stacked branch
- **THEN** it pushes as the push button of the branch does

### Requirement: Pull requests: A stack push is the intent Push with the scope withAncestors

The push MUST be the intent `Push` with the scope `withAncestors` (`operations::push`): its plan sends the ancestors
above the nearest one whose push drops remote commits, then the branch.

#### Scenario: The plan of a stack push
- **WHEN** a push of a stack runs
- **THEN** it is one push of the branches it covers: the ancestors above the nearest one whose push drops remote
  commits, then the branch
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_stack_is_one_push_of_the_branches_it_covers`)

### Requirement: Pull requests: A branch whose push drops remote commits by its plan asks first

A branch whose own push drops remote commits by the time it is planned (a fetch brought a commit of someone else after
the row showed it) MUST ask first: its push button opens its description with them, whose Force Push runs the push of
the stack again with that drop accepted, never forced, over the remote head the row shows at the click.

#### Scenario: A fetch brought a commit of someone else
- **GIVEN** a fetch brought a commit of someone else to the remote branch of a stacked branch after the row showed it
- **WHEN** the user presses its push button, which pushes the stack
- **THEN** the push button opens its description with the remote commits the push drops
- **AND** its Force Push runs the push of the stack again with that drop accepted, never forced, over the remote head
  the row shows at the click
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask in the row of a stacked branch whose push drops remote commits by the time its button's push of the stack is planned, and push the stack over the remote head the row shows`)

### Requirement: Pull requests: A running stack push shows its progress on every branch it pushes

While it runs, the button MUST name it ("Pushing 3 branches 45%", the refs its operation reserves), the rows of the
branches it pushes show the same progress on their own push buttons, and a second click joins it (the same intent).

#### Scenario: A stack push runs
- **WHEN** a push of a stack runs
- **THEN** the button says "Pushing 3 branches 45%", and the rows of the branches it pushes show the same progress
- **AND** a second click joins it
  [@test] src/lib/stack.test.ts (`names what a stack push sends while it runs: its one push of several branches, or the one branch`)

### Requirement: Pull requests: A push of a branch meanwhile waits for the stack push

A push of one of its branches meanwhile (its own row's button, the push step of a Safe Push start, a pull request, a
review request) MUST wait for it in the registry of the session and be decided again on what it left: it pushes only
what the remote still lacks, never an invented "The remote branch changed", so no branch is sent twice.

#### Scenario: Two pushes of one ref
- **GIVEN** a push of a stack runs
- **WHEN** a push of one of its branches starts
- **THEN** it waits for the stack push, then pushes only what the remote still lacks
- **AND** it says no "The remote branch changed"
  [@test] crates/operations/src/push_test.rs (`the_pushes_of_one_ref_wait_for_each_other_and_both_push`)

### Requirement: Pull requests: The row of a stacked branch doesn't name the parent

The row MUST NOT name the parent: the stack graph shows it, and the description of the button explains Push Stack.

#### Scenario: A stacked branch whose button pushes the stack
- **WHEN** the row of a stacked branch shows a push of the stack
- **THEN** it names no parent; the stack graph shows it, and the description of the button explains Push Stack

### Requirement: Pull requests: The button pushes the branch alone while a push in the stack asks first

The button MUST push the branch alone, and its description offer `Push Stack (3 branches)` instead, while the branch's
own push drops remote commits (it asks first, Force Push) and while the stack above the nearest ancestor whose push
drops remote commits has only the branch: that ancestor asks first in its own row, and a stack pushed around it would
leave a stale branch in the middle.

#### Scenario: The branch's own push drops remote commits
- **WHEN** the push of the branch itself drops remote commits
- **THEN** its button pushes the branch alone, asking first (Force Push), and its description offers
  `Push Stack (3 branches)`
  [@test] src/presenters/push-button.test.ts (`offers Push Stack while the button pushes the branch alone: its push drops remote commits, or an ancestor's does`)

#### Scenario: The nearest ancestor whose push drops remote commits is the parent
- **WHEN** the stack above the nearest ancestor whose push drops remote commits has only the branch
- **THEN** the button pushes the branch alone, and its description offers `Push Stack (3 branches)`
- **AND** that ancestor asks first in its own row
  [@test] src/presenters/push-button.test.ts (`offers Push Stack while the button pushes the branch alone: its push drops remote commits, or an ancestor's does`)
  [@test] crates/domain-policy/src/push_test.rs (`the_push_button_and_an_action_start_above_the_nearest_ancestor_that_asks_first`)

### Requirement: Pull requests: Push Stack pushes the ancestors with unpushed work, root first

"Push Stack" (the button, or in that description) MUST push the ancestors with unpushed work, root first, then the
branch.

#### Scenario: Push Stack of a branch in a stack
- **WHEN** the user presses "Push Stack"
- **THEN** the ancestors with unpushed work, the chain of the effective bases, are pushed root first, then the branch
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)
  [@test] crates/domain-policy/src/stack_test.rs (`the_ancestors_are_the_chain_of_the_effective_bases_in_the_table`)

### Requirement: Pull requests: The branch menu never pushes past an ancestor that asks first

The branch menu MUST push as the button does, never the whole stack past an ancestor whose push drops remote commits:
that ancestor asks first in its own row.

#### Scenario: An ancestor's push drops remote commits
- **GIVEN** an ancestor whose push drops remote commits
- **WHEN** the user chooses the push in the branch menu
- **THEN** it pushes the branch alone above that ancestor, which asks first in its own row
  [@test] src/hooks/useBranchContextActions.test.ts (`pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row`)

### Requirement: Pull requests: Push Stack of the description covers the drops it named

`Push Stack (3 branches)` of the description of a button that pushes the branch alone MUST cover every ancestor with
work up to the nearest one a Safe Push holds, the ones that drop remote commits too: the intent forced (`force`), as
the description named each drop.

#### Scenario: Push Stack from the description
- **GIVEN** a button that pushes the branch alone, as an ancestor asks first
- **WHEN** the user presses `Push Stack (3 branches)` in its description
- **THEN** every ancestor with work up to the nearest one a Safe Push holds is pushed, the ones that drop remote
  commits too, by the intent forced (`force`)
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_stack_leaves_an_ancestor_that_asks_first_unless_forced`)

### Requirement: Pull requests: brd push takes the ancestors along

`brd push` MUST take them along too ([Agent CLI](../../cli/agent-cli/spec.md#brd-push)).

#### Scenario: brd push of a stacked branch
- **WHEN** `brd push` pushes a branch whose ancestors have unpushed work
- **THEN** it pushes the ancestors with it, root first

### Requirement: Pull requests: A push of several branches is one atomic push

A push of several branches (Push Stack, Push with Children, Push Up to Here) MUST be one atomic push of all of them, as
`brd push` pushes ([Git Operations](../../../../docs/design/git-operations.md#push-and-land)): all of them or none,
never a stack pushed up to a failure, which would leave a stale branch in the middle.

#### Scenario: Push Stack
- **WHEN** a push of a stack runs
- **THEN** its branches go in one atomic push, parents first, by the processes of the core push
  [@test] crates/sync-core/src/push_test.rs (`a_stack_pushes_its_branches_in_one_push`)
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_stack_runs_the_processes_of_the_core_push`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_atomic_push_of_a_stack`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`)

#### Scenario: Push with Children
- **WHEN** Push with Children of a parent runs
- **THEN** the parent and its children go in one atomic push
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a parent with its children`)

#### Scenario: Push Up to Here
- **WHEN** a push up to a commit runs
- **THEN** the branches below go whole, and the branch up to the commit, in one atomic push
  [@test] crates/sync-core/src/push_test.rs (`a_stack_push_up_to_a_commit_pushes_the_branches_below_whole`)
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`)

### Requirement: Pull requests: A push of several branches waits for the running sync

A sync of the session that runs MUST be awaited first, and the branches decided and pushed as it leaves them.

#### Scenario: A sync runs when the push starts
- **GIVEN** a sync of the session runs
- **WHEN** a push of several branches starts
- **THEN** it waits for the sync, then decides the branches and pushes them as the sync leaves them

#### Scenario: The push sends the branches as it decided them
- **GIVEN** a push decided its branches
- **WHEN** a sync rebuilds one of them meanwhile
- **THEN** the push sends the commit it decided on
  [@test] crates/operations/src/push_test.rs (`a_push_that_sends_holds_no_lock_and_sends_the_commit_it_froze`)

### Requirement: Pull requests: A remote branch that moved refuses the whole push

Each branch MUST be leased on the remote head its row shows; a remote branch that moved since refuses the whole push
and every branch is fetched.

#### Scenario: A remote branch of the stack moved
- **GIVEN** a remote branch of the stack moved since its row showed it
- **WHEN** the push of the stack runs
- **THEN** the whole push is refused, naming that branch, and every branch is fetched
  [@test] crates/sync-core/src/push_test.rs (`a_moved_lease_refuses_every_branch_and_names_it`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack in one push, refused whole when a remote branch of it moved`)

### Requirement: Pull requests: A moved remote without new commits is pushed over at once

When that remote has no commits its branch lacks, nobody's work is lost: Rust MUST push again over the fetched heads at
once, once, as the push of one branch does ([Outgoing Commits](../../repository/outgoing/spec.md#push)); else its
toast says what that one has now ("login changed on the remote", "… Nothing was pushed.") and offers its push over it.

#### Scenario: The moved remote has no commits the branch lacks
- **WHEN** the remote branch that moved has no commits its branch lacks
- **THEN** Rust pushes again over the fetched heads at once, once
  [@test] crates/operations/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks`)

#### Scenario: The moved remote has commits the branch lacks
- **WHEN** the remote branch that moved has commits its branch lacks
- **THEN** nothing is pushed again, and the toast says what that one has now ("login changed on the remote", "…
  Nothing was pushed.") and offers its push over it
  [@test] crates/operations/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks`)

### Requirement: Pull requests: A branch a running Safe Push holds refuses the push of several branches

A branch a running Safe Push holds MUST refuse the push with the reason.

#### Scenario: A Safe Push holds a branch of the push
- **GIVEN** a running Safe Push holds a branch of a push of several branches
- **WHEN** the push runs
- **THEN** it is refused with the reason

### Requirement: Pull requests: A push of several branches retargets as the push of one

A pull request into a virtual branch MUST be retargeted before the push, as for one branch.

#### Scenario: A branch of the push has a pull request into a virtual branch
- **WHEN** a push of several branches runs and one of them has a pull request into a virtual branch
- **THEN** that pull request is retargeted to the baseline before the push

### Requirement: Pull requests: A push of several branches is one toast

A push of several branches, "Push Stack" or "Push with Children", MUST be one toast that names the branches and says
what the pushes did together: the commits, the remote branches, each pull request the push retargeted and one whose
retarget failed, the robots the pushes restarted.

#### Scenario: The toast of a stack push
- **WHEN** a push of several branches succeeds
- **THEN** one toast says "Pushed branches auth, login and oauth" / "3 commits · created 2 remote branches · force
  pushed auth", each pull request the push retargeted ("retargeted pull request #802 of auth to master") and one whose
  retarget failed, and the robots the pushes restarted
  [@test] src/lib/push-summary.test.ts (`says what the pushes of a stack did together`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)

#### Scenario: The names of many branches
- **WHEN** the push sends up to four branches, or five and more
- **THEN** up to four are named, "Pushed branches auth, session, login and oauth"
- **AND** from five on "Pushed branches auth, login, oauth and 2 more", never "and 1 more" (`joinNames`)
  [@test] src/lib/text.test.ts (`names up to four, so 'and 1 more' never stands where one name would, and counts the rest from five on`)

### Requirement: Pull requests: The action of a stack push toast is that of the pushed branch

The action of the toast MUST be the pull request action of the branch the user pushed from ("Create Pull Request…" into
the baseline, or "Open Pull Request"), unless a retarget comes first.

#### Scenario: A stack push from a branch without a pull request
- **WHEN** a push of several branches from a branch without a pull request succeeds
- **THEN** the action of its toast is "Create Pull Request…" into the baseline, or "Open Pull Request" for one with a pull
  request, unless a retarget comes first
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)

### Requirement: Pull requests: Activity has an entry per branch of a stack push

Activity MUST have an entry per branch of a push of several branches.

#### Scenario: Activity after a stack push
- **WHEN** a push of several branches succeeds
- **THEN** Activity has an entry per branch

### Requirement: Pull requests: A push of one branch is its own push toast

A push of one branch MUST be its own push toast.

#### Scenario: A push of one branch
- **WHEN** a push sends one branch
- **THEN** its toast is the push toast of that branch

### Requirement: Pull requests: The push button of a parent pushes only the branch

The push button of a branch whose descendants have work for the remote MUST stay as it is too: it pushes only the
branch. Its description MUST offer the children below a rule, then a `Push with Children (4 branches)` button, named as
in the branch menu.

#### Scenario: Children with work for the remote
- **GIVEN** branches stacked on `login` have commits to push
- **WHEN** the description of the push button of `login` shows
- **THEN** the button pushes only `login`
- **AND** below a rule the description says "oauth is stacked on login and has commits to push." ("oauth and session
  are …", "session and 2 more branches are … and have commits to push."), then a `Push with Children (4 branches)`
  button
  [@test] src/presenters/push-button.test.ts (`offers Push with Children while branches stacked on it have work for the remote`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a parent with its children`)

### Requirement: Pull requests: Push with Children comes below the other push of a stacked branch

Push with Children MUST come below the other push of a stacked branch when both apply.

#### Scenario: A branch in the middle of a stack
- **GIVEN** a branch in the middle of a stack whose ancestors and descendants have work for the remote
- **WHEN** the description of its push button shows
- **THEN** the push of the branch alone comes first, Push with Children below it
  [@test] src/presenters/push-button.test.ts (`offers the push of the branch alone before Push with Children for a branch in the middle of a stack`)

### Requirement: Pull requests: A pushed root has Push with Children as its push button

A branch whose own push is up to date (the root of a stack pushed before its children) MUST have Push with Children as
its push button, the push of the header of an island (`ChildrenPushButton`), named "Push with Children (3 branches)",
its hover the pushes it sends, the graph circling their nodes.

#### Scenario: The root of a stack is pushed, its children are not
- **GIVEN** the root of a stack is on the remote and its children have commits to push
- **WHEN** its row shows
- **THEN** its push button is Push with Children: "↑ 3", the commits it sends, in the push colour, then the muted
  "with children" where the branch view is at least 56rem wide (`data-wide` of the view,
  [Branch Row](../branch-row/spec.md#actions))
- **AND** its name is "Push with Children (3 branches)", its hover the pushes it sends, the graph circling their nodes
  [@test] src/presenters/push-button.test.ts (`offers Push with Children while the branch itself is on the remote: its own push is disabled, this one isn't`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer Push with Children while a sync runs and when the root is pushed`)

### Requirement: Pull requests: P and the push item stay the push of the branch

P and the push item of its menu MUST stay the push of the branch, disabled as up to date; the root row of an island
leaves it to the header.

#### Scenario: P on a pushed root
- **GIVEN** a pushed root whose push button is Push with Children
- **WHEN** the user presses P or opens its menu
- **THEN** P and the push item are the push of the branch, disabled as up to date
  [@test] src/presenters/push-button.test.ts (`offers Push with Children while the branch itself is on the remote: its own push is disabled, this one isn't`)

#### Scenario: The root row of an island
- **WHEN** the root of an island is pushed and its children are not
- **THEN** its row leaves Push with Children to the header of the island

### Requirement: Pull requests: Push with Children of a pushed root waits for Patronus and GitHub

It MUST show once Patronus and GitHub answered, as a first push waits for them: a robot of theirs holds the children, a
merged one landed them, an open pull request of the whole stack takes its stack branch along. The settled layout holds
its track meanwhile, and while it pushes the track keeps its width ("⟳ 45% with children").

#### Scenario: GitHub answers late
- **GIVEN** a pushed root with children to push
- **WHEN** GitHub has not answered yet
- **THEN** the settled layout holds the push track, and Push with Children shows once it answered, as wide as it was
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should hold the push track of a pushed root while GitHub answers, its Push with Children as wide as it was`)

#### Scenario: It pushes
- **WHEN** Push with Children of a pushed root runs
- **THEN** the track keeps its width ("⟳ 45% with children")

### Requirement: Pull requests: Push with Children pushes the branch and its descendants with work

"Push with Children" MUST push the branch if it has work for the remote, then its descendants that have work, parents
first, in the one atomic push of a stack.

#### Scenario: A parent with children to push
- **GIVEN** a parent and descendants with work for the remote
- **WHEN** the user runs "Push with Children" of the parent
- **THEN** the parent, if it has work, then its descendants that have work go in one atomic push, parents first
  [@test] crates/domain-policy/src/push_test.rs (`push_with_children_covers_the_branch_and_its_descendants`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a parent with its children`)

### Requirement: Pull requests: Push with Children leaves out what asks first or is held

A descendant whose push drops remote commits MUST be left out: it asks first in its own row; so MUST be a branch a
running Safe Push holds.

#### Scenario: A descendant whose push drops remote commits
- **GIVEN** a descendant whose push drops remote commits
- **WHEN** Push with Children runs
- **THEN** that descendant is left out, and asks first in its own row
  [@test] crates/domain-policy/src/push_test.rs (`push_with_children_leaves_out_what_asks_first_is_held_or_landed`)

#### Scenario: A branch a running Safe Push holds
- **GIVEN** a running Safe Push holds a branch of the stack
- **WHEN** Push with Children runs
- **THEN** that branch is left out
  [@test] crates/domain-policy/src/push_test.rs (`push_with_children_leaves_out_what_asks_first_is_held_or_landed`)

### Requirement: Pull requests: The branch's own drop asks first in Push with Children

The branch's own push that drops remote commits MUST ask first: its push button opens its description with them,
whose Force Push runs Push with Children again with that drop accepted, never forced, so a descendant whose drop nobody
showed stays out, over the remote head the row shows at the click (the branch leases on it, and its drops are counted
on the head the session has).

#### Scenario: The branch's push drops remote commits
- **GIVEN** the push of the branch itself drops remote commits
- **WHEN** the user runs Push with Children
- **THEN** its push button opens its description with them
- **AND** its Force Push runs Push with Children again with that drop accepted, never forced, and a descendant whose
  drop nobody showed stays out
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask in the row of the branch before Push with Children drops its remote commits, and leave out a child whose drop nobody showed`)
  [@test] src/lib/push-run.test.ts (`of a stack asks first in the push button of the branch that drops remote commits, whose Force Push runs it again with that drop accepted`)
  [@test] crates/operations/src/push_test.rs (`push_with_children_run_on_the_accepted_drop_leaves_a_child_that_drops_remote_commits_out`)

#### Scenario: Over the remote head the row shows at the click
- **WHEN** the Force Push runs on the accepted drop
- **THEN** it goes only over the remote head the row shows at the click, on which the branch leases and its drops are
  counted on the head the session has
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_stack_on_an_accepted_drop_goes_only_over_the_remote_head_its_drops_are_counted_on`)

### Requirement: Pull requests: A plan of Push with Children that asks more asks again

A plan that asks more by then (the remote got another commit) MUST ask again. Nothing is pushed before that, and a plan
that asks of more than one branch MUST be refused, each drop named (D12).

#### Scenario: The remote got another commit
- **GIVEN** the user accepted a drop with Force Push
- **WHEN** the plan asks more by then, as the remote got another commit
- **THEN** it asks again, nothing pushed before that, then pushes over the remote head the row shows at the click
  [@test] src/lib/push-run.test.ts (`of a stack whose plan asks more after the Force Push than its description showed asks again, then pushes over the remote head the row shows at the click`)

#### Scenario: The plan asks of more than one branch
- **WHEN** the plan of Push with Children drops remote commits of more than one branch
- **THEN** it is refused, each drop named (D12), and nothing is pushed
  [@test] src/lib/push-run.test.ts (`of a stack that drops remote commits of more than one branch is refused, each drop named`)

### Requirement: Pull requests: Push with Children names what it pushes

Push with Children MUST name what it pushes: "Push with Children (4 branches)", "Push with Children (oauth)" for one
child.

#### Scenario: Several children and one
- **WHEN** Push with Children is offered
- **THEN** it is "Push with Children (4 branches)" for several branches, "Push with Children (oauth)" for one child
  [@test] src/lib/stack.test.ts (`names Push with Children by what it pushes`)

### Requirement: Pull requests: Every branch with children offers Push with Children

Every branch with branches stacked on it MUST offer it, in the branch menu beside the push of the branch
([Branch Row](../branch-row/spec.md#branch-menu)), also while a sync runs: the push waits for the sync and computes what
to push on the branches as it leaves them.

#### Scenario: The branch menu of a parent
- **WHEN** the user opens the branch menu of a branch with branches stacked on it
- **THEN** it offers Push with Children beside the push of the branch, labelled by what it pushes
  [@test] src/hooks/useBranchContextActions.test.ts (`is offered beside the push where Rust offers it, with its label of what it pushes, and the stack push over it`)

#### Scenario: A sync runs
- **GIVEN** a sync runs
- **WHEN** the user runs Push with Children
- **THEN** the push waits for the sync and computes what to push on the branches as it leaves them
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer Push with Children while a sync runs and when the root is pushed`)

### Requirement: Pull requests: Push with Children is disabled with the reason, never hidden

Push with Children MUST be disabled with the reason, never hidden, while it would push nothing beyond the branch, while
another push that sends the branch holds it and while a running Safe Push holds the branch: every Safe Push of the
stack lands it.

#### Scenario: Nothing beyond the branch to push
- **WHEN** Push with Children would push nothing beyond the branch
- **THEN** it is disabled with the reason: "Nothing to push: the stack is on the remote", "Only auth has commits to
  push: its own push sends them", "login asks first in its row: the push drops remote commits"; "Wait for the sync to
  finish" while the sync computes the remote statuses
  [@test] crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)
  [@test] src/presenters/actions.test.ts (`words every blocker of the catalog as the page says it`, `says why Push with Children leaves the stack branch of the whole stack as it is, and names the children that ask first`)

#### Scenario: Another push sends the branch
- **WHEN** another push that sends the branch holds it
- **THEN** Push with Children is disabled with "Pushing branch login…", and shows running while its own push runs
  [@test] crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)

#### Scenario: A running Safe Push holds the branch
- **WHEN** a running Safe Push holds the branch
- **THEN** Push with Children is disabled with the reason
  [@test] src/presenters/push-button.test.ts (`offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it`)
  [@test] crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)

### Requirement: Pull requests: A branch a merged Safe Push landed whole is left out

A branch a merged Safe Push landed whole MUST only be left out, its own branch included: the branches built on it push
without it, from its menu, the island's header and the description of its push ("session is stacked on auth and has
commits to push.", "Push with Children (session)").

#### Scenario: The children of a landed branch
- **GIVEN** a merged Safe Push landed `auth` whole and `session` on it has commits to push
- **WHEN** the user pushes from the menu of `auth`, the island's header or the description of its push
- **THEN** `session` is pushed without `auth`: "session is stacked on auth and has commits to push.", "Push with
  Children (session)"
  [@test] src/presenters/push-button.test.ts (`offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it`)
  [@test] crates/bd-remote/src/landed_test.rs (`push_with_children_leaves_out_what_merged_safe_pushes_landed_and_refuses_once_nothing_else_is_left`)
  [@test] crates/domain-policy/src/push_test.rs (`push_with_children_leaves_out_what_asks_first_is_held_or_landed`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

#### Scenario: The branches the whole stack landed
- **GIVEN** a merged Safe Push of the whole stack landed its branches
- **WHEN** the rows of those branches show until the update
- **THEN** they offer no push of their own, each saying why
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer its Safe Push no more`, `should offer no Safe Push, Dry Run or push of the branches the whole stack landed, each saying why, the hover of their chip saying so`)

### Requirement: Pull requests: Nothing is left once a branch and every branch on it landed

Once it and every branch built on it landed, nothing MUST be left: "Landed in master with deploy and side-test: nothing
of air and its children is left to push" ([Patronus](../patronus/spec.md#merged)).

#### Scenario: Every branch of the stack landed
- **GIVEN** merged Safe Pushes landed `air` and every branch built on it
- **WHEN** Push with Children of `air` is asked
- **THEN** it is refused: "Landed in master with deploy and side-test: nothing of air and its children is left to push"
  [@test] crates/bd-remote/src/landed_test.rs (`push_with_children_leaves_out_what_merged_safe_pushes_landed_and_refuses_once_nothing_else_is_left`)

### Requirement: Pull requests: Push with Children of a forked root takes the whole stack branch along

Push with Children of the root of a stack that forks MUST also push the stack branch of its whole stack
(`{prefix}/stack/<root>`, [Whole Stack](../whole-stack/spec.md)) in the same atomic push, after the branches, leased on
the remote head of its remote status, so the pull request of the whole stack follows its branches.

#### Scenario: The root of a forked stack whose pull request of the whole stack is open
- **GIVEN** the root of a stack that forks, whose stack branch has work for the remote
- **WHEN** the user runs Push with Children of the root, from its push, its menu or the header of its island
- **THEN** the stack branch goes in the same atomic push, after the branches, leased on the remote head of its remote
  status
  [@test] crates/sync-core/src/push_test.rs (`the_stack_branch_goes_with_the_branches_and_a_moved_one_is_named_as_the_stack_branch`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should take the whole stack branch along in Push with Children of the root once its pull request is open, from the push and the menu`, `should push a parent with its children`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu`)

### Requirement: Pull requests: The whole stack branch goes along only while built, published and not held

The stack branch MUST go along only while the sync built it (never stale, conflicted or blocked), once it is on the
remote or its pull request is open, while its remote lacks commits of it and has none it lacks, and never while it is
held (`domain_policy::push::stack_branch_along`: its own robot runs, or every branch of the stack landed,
[Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: The conditions of the stack branch
- **WHEN** Push with Children of the root is planned
- **THEN** the stack branch goes along only while the sync built it, once it is on the remote or its pull request is
  open, while its remote lacks commits of it and has none it lacks, and not while it is held
  [@test] crates/domain-policy/src/push_test.rs (`the_stack_branch_goes_along_once_published_built_not_held_and_behind`)

#### Scenario: Every branch of the stack landed
- **GIVEN** every branch of the stack landed
- **WHEN** the header of the island shows
- **THEN** the push of the whole stack branch is held, and what is left pushes without the landed ones
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

### Requirement: Pull requests: The whole stack branch counts as one more branch and is named

The stack branch MUST count as one more branch and the hover MUST name it: "Push with Children (5 branches)", "Push
with Children (the whole stack branch)" when only it has work.

#### Scenario: The names of a push that takes the stack branch along
- **WHEN** Push with Children of the root takes the stack branch along
- **THEN** it is "Push with Children (5 branches)", "Push with Children (the whole stack branch)" when only it has work
- **AND** the menu item says "Also pushes the whole stack branch user-name/stack/auth, the head of #12", and the
  description of the root's push "The whole stack branch user-name/stack/auth goes along."
  [@test] src/lib/stack.test.ts (`names Push with Children by what it pushes`)
  [@test] src/hooks/useBranchContextActions.test.ts (`takes the stack branch of the whole stack of a root along, counted and named, and says why it doesn't in Rust's words`)
  [@test] src/presenters/push-button.test.ts (`names the stack branch of the whole stack that Push with Children of a root takes along`)

#### Scenario: The toast and Activity
- **WHEN** a push took the stack branch along
- **THEN** its toast says "… · pushed the whole stack branch" ("Pushed the whole stack branch of auth" alone, with
  "Open Pull Request" of the stack's)
- **AND** Activity records "Pushed the whole stack branch of auth"

### Requirement: Pull requests: Nothing to push is said only while the stack branch has nothing either

"Nothing to push" MUST be said only while the stack branch has nothing to push either: one on no remote and in no pull
request is "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe
Push of the whole stack", one the sync didn't build says why.

#### Scenario: Why the stack branch stays
- **GIVEN** the branches of the stack are on the remote
- **WHEN** Push with Children of the root shows
- **THEN** a stack branch on no remote and in no pull request says "The branches of the stack are on the remote. Its
  stack branch goes there with the pull request or the Safe Push of the whole stack"
- **AND** one the sync didn't build says why, in Rust's words, and the children that ask first are named
  [@test] src/presenters/actions.test.ts (`words every blocker of the catalog as the page says it`, `says why Push with Children leaves the stack branch of the whole stack as it is, and names the children that ask first`)
  [@test] src/hooks/useBranchContextActions.test.ts (`takes the stack branch of the whole stack of a root along, counted and named, and says why it doesn't in Rust's words`)
  [@test] crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)

### Requirement: Pull requests: A refused push names a moved stack branch as the stack branch

A refused push MUST name a stack branch that moved as the stack branch, apart from its root ("Cannot push the whole
stack branch of auth").

#### Scenario: The stack branch moved on the remote
- **GIVEN** the remote stack branch moved
- **WHEN** Push with Children of the root runs
- **THEN** the push is refused, naming "Cannot push the whole stack branch of auth", apart from its root
  [@test] crates/sync-core/src/push_test.rs (`the_stack_branch_goes_with_the_branches_and_a_moved_one_is_named_as_the_stack_branch`)

### Requirement: Pull requests: brd push --with-children takes the stack branch the same way

`brd push <root> --with-children` MUST follow the same rule ([Agent CLI](../../cli/agent-cli/spec.md#brd-push)).

#### Scenario: brd push of a forked root with its children
- **WHEN** `brd push <root> --with-children` runs on the root of a stack that forks
- **THEN** it takes the stack branch of the whole stack along by the same rule

### Requirement: Pull requests: A Safe Push of a stacked branch lands it with what it is built on

A Safe Push of a stacked branch MUST land it with the branches it is built on, in one robot: its button says so ("Safe
Push · 3"), and its start always confirms, the popover at the slot saying it first: "Lands oauth with auth and login,
which it is built on: 3 branches · 12 commits into master."; Enter starts it
([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: The Safe Push of oauth on auth and login
- **WHEN** the user presses the Safe Push of `oauth`, built on `auth` and `login`
- **THEN** its button says "Safe Push · 3", and the popover at the slot says "Lands oauth with auth and login, which it
  is built on: 3 branches · 12 commits into master."
- **AND** Enter starts one robot that lands the three branches
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`)

### Requirement: Pull requests: The branches built on a landing branch stay

The branches built on it MUST stay, and the popover says that the next sync after the merge builds them on the target.

#### Scenario: A branch with branches built on it
- **GIVEN** branches built on the branch of the Safe Push
- **WHEN** its popover shows
- **THEN** it says that the next sync after the merge builds them on the target, and they stay

### Requirement: Pull requests: Every surface asks the branch's own Safe Push start

Its slot, "⋯", S, the preview and the review window MUST ask the branch's own start, its popover at its own row, never
the root's.

#### Scenario: From the slot, ⋯ and the menu
- **WHEN** the user starts the Safe Push of a stacked branch from "⋯" of its row or Safe Push With Profile… of its menu
- **THEN** the branch's own start is asked, its popover at its own row
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on from the ⋯ of its row and Safe Push With Profile… of its menu`)

#### Scenario: From S and the review window
- **WHEN** the user presses S on the row, or starts it in the review window
- **THEN** the branch's own start is asked, never the root's
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on from S and the review window`)

#### Scenario: From the preview
- **WHEN** the user starts it from the footer of its preview, or presses S in it
- **THEN** the branch's own start is asked, the popover at its row
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on from the footer of its preview and S in it, the popover at its row`)

### Requirement: Pull requests: Landing a stacked branch alone is the second way

Landing the branch alone MUST be the second way: "Unstack from <parent>" for a branch that builds alone, "Move 1 Commit
Here" for one that needs commits it can take.

#### Scenario: A stacked branch that applies alone
- **WHEN** a stacked branch builds alone on the baseline
- **THEN** it offers "Unstack from <parent>" as the second way, after its Safe Push with what it is built on
- **AND** one that needs commits it can take offers "Move 1 Commit Here"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer to land only a stacked branch that applies alone`)

### Requirement: Pull requests: The branches a running Safe Push lands say so

While it runs, the branches it lands MUST say "lands with oauth" in their Status cell and offer no push and no Safe
Push of their own; the lines of the graph from the root down to it are in the colour of the robot, its node has the
ring of the robot.

#### Scenario: A running Safe Push of oauth
- **GIVEN** a running Safe Push of `oauth` on `auth` and `login`
- **WHEN** the table shows
- **THEN** `auth` and `login` say "lands with oauth" in their Status cell and offer no push and no Safe Push of their own
- **AND** the lines of the graph from the root down to `oauth` are in the colour of the robot, and its node has the
  ring of the robot
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)

### Requirement: Pull requests: A branch a Safe Push lands keeps what it is itself

A branch it lands MUST keep what it is itself: a conflict of a branch below says "merge conflict", its node a fix (the
conflict came after the push, and the robot lands the remote copy); the branch the robot runs on, blocked by that
parent, says the robot first with "Blocked: parent auth has a merge conflict" beside it, its node landing
([Branch Row](../branch-row/spec.md#status-cell)).

#### Scenario: A parent conflicts after the push
- **GIVEN** a running Safe Push of a branch, and a parent `auth` that conflicts since the push
- **WHEN** the table shows
- **THEN** `auth` says "merge conflict", its node a fix
- **AND** the branch the robot runs on says the robot first with "Blocked: parent auth has a merge conflict" beside it,
  its node landing
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show a running Safe Push of a blocked branch first, its block beside it, and its parent's conflict as the fix`)

### Requirement: Pull requests: The landing path is highlighted before the start

Before the start, the same path MUST be highlighted while the pointer is on the Safe Push slot and while the popover is
open ([Branch Row](../branch-row/spec.md#tree)).

#### Scenario: The pointer on the Safe Push slot
- **WHEN** the pointer is on the Safe Push slot of a stacked branch, or its popover is open
- **THEN** the lines of the graph from the root down to the branch are highlighted

### Requirement: Pull requests: A running Safe Push lands the branches its commit has

The branches it lands MUST be those its commit has: a branch a restack placed below it after the start says no "lands
with", pushes and starts its own Safe Push, and the colour of the robot starts below it
([Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: A restack after the start
- **GIVEN** a running Safe Push, and a branch a restack placed below it after the start
- **WHEN** the table shows
- **THEN** that branch says no "lands with", pushes and starts its own Safe Push
- **AND** the colour of the robot starts below it
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a branch a restack placed below a running Safe Push after it started, and hold what its commit has`)

### Requirement: Pull requests: A merged robot closes the pull requests of the ancestors it landed

When a robot of a stacked branch merged, the open pull requests of the ancestors it landed MUST be closed with a
comment ([Patronus](../patronus/spec.md#merged)).

#### Scenario: The robot of oauth merged
- **GIVEN** open pull requests of `auth` and `login`
- **WHEN** the robot of `oauth`, built on them, merged
- **THEN** their pull requests are closed with a comment

### Requirement: Pull requests: The rows a merged Safe Push landed offer nothing until the update

Until the update archives them, the rows of the ancestors a merged robot landed MUST say "Landed with oauth" and offer
no Safe Push, no Dry Run and no push, nor does the branch it ran on ("Merged").

#### Scenario: After the merge, before the update
- **WHEN** the robot of `oauth` merged, and the update has not archived its branches yet
- **THEN** the rows of the ancestors it landed say "Landed with oauth" and offer no Safe Push, no Dry Run and no push
- **AND** the row of `oauth` says "Merged" and offers none of them either
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`, `should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

### Requirement: Pull requests: A branch built on a landed ancestor lands without it

A branch built on a landed ancestor MUST land without it ("Safe Push" where it was "Safe Push · 2").

#### Scenario: A branch on a landed ancestor
- **GIVEN** a branch built on an ancestor a merged Safe Push landed
- **WHEN** its row shows
- **THEN** its button says "Safe Push" where it was "Safe Push · 2"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`)

### Requirement: Pull requests: A new commit of a landed ancestor is the only work left

A new commit of an ancestor MUST rebuild the branches built on it: they stay landed by content, the new commit the only
work left of its branch ("Landed with oauth · 1 commit after it").

#### Scenario: A commit added to a landed ancestor
- **GIVEN** a merged Safe Push landed an ancestor
- **WHEN** a new commit of that ancestor rebuilds the branches built on it
- **THEN** they stay landed by content, and the ancestor says "Landed with oauth · 1 commit after it"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

### Requirement: Pull requests: brd safe-push lands a stacked branch with its ancestors

`brd safe-push` MUST land a stacked branch the same way, with its ancestors in one robot
([Agent CLI](../../cli/agent-cli/spec.md#brd-safe-push)).

#### Scenario: brd safe-push of a stacked branch
- **WHEN** `brd safe-push` starts on a stacked branch
- **THEN** one robot lands the branch with its ancestors

### Requirement: Stacks of GitHub: The layers are the branches of a straight chain

The layers MUST be the branches of a straight chain of two or more by declared parent, on the route of a push or of a
merge of the pull request, where GitHub's schema has stacks (`PullRequestsView.githubStacks`;
`RepositoryView.githubLayers`, `domain_policy::github_stack::chain_of`). Each layer names its chain, the branch below
it and the branches below it without an open pull request.

#### Scenario: A straight chain and a branch alone
- **GIVEN** the route of a push or of a merge of the pull request, where GitHub's schema has stacks
- **WHEN** branches form a straight chain of two or more by declared parent
- **THEN** they are the layers of a stack of GitHub
- **AND** a branch alone has none
  [@test] crates/domain-policy/src/github_stack_test.rs (`a_straight_chain_is_a_stack_of_github_and_a_branch_alone_is_none`)

#### Scenario: A stack that forks
- **WHEN** a stack forks
- **THEN** it has no layers
  [@test] crates/domain-policy/src/github_stack_test.rs (`a_stack_that_forks_is_none`)

#### Scenario: The route of Patronus
- **WHEN** the route is that of Patronus
- **THEN** no chain has layers at all

#### Scenario: What landed and what ends the chain
- **WHEN** branches at the bottom of the chain landed whole, or a branch of it is parked, blocked or built on another
  base than its declared parent
- **THEN** the chain starts above the branches that landed whole
- **AND** a parked branch, a blocked one or one built on another base than its declared parent ends it
  [@test] crates/domain-policy/src/github_stack_test.rs (`the_chain_starts_above_what_landed`, `a_parked_a_blocked_or_a_rebased_branch_ends_the_chain`)

#### Scenario: The view names the layers
- **WHEN** the session derives its view of a straight chain
- **THEN** `RepositoryView.githubLayers` names each layer, its chain, the branch below it and the branches below it
  without an open pull request
  [@test] crates/repository-model/src/derive_test.rs (`the_view_names_the_layers_of_the_stacks_of_github_of_a_straight_chain`)

### Requirement: Stacks of GitHub: A member of a stack of GitHub is a layer whatever the session says

An open pull request into a virtual branch that GitHub lists in a stack into the target MUST be a layer of it
(`PullRequestBase::Layer`), whatever the session says of the chain: GitHub's membership decides, and no push, start or
next step retargets it; GitHub refuses the retarget of a member.

#### Scenario: GitHub lists the pull request in a stack
- **GIVEN** an open pull request into a virtual branch that GitHub lists in a stack into the target
- **WHEN** its base is decided
- **THEN** it is `PullRequestBase::Layer`, whatever the session says of the chain
- **AND** no push, start or next step retargets it
  [@test] crates/domain-policy/src/pull_request_test.rs (`a_member_of_a_stack_of_github_into_the_target_is_a_layer`)

### Requirement: Stacks of GitHub: A pull request into the branch below is a layer for a push

One into the branch below it in its chain that GitHub lists in no stack yet MUST be a layer too for a push
(`layer_base_of`), which links the stack instead.

#### Scenario: A pull request into the branch below, in no stack yet
- **GIVEN** an open pull request into the branch below it in its chain, which GitHub lists in no stack yet
- **WHEN** its branch is pushed
- **THEN** it is a layer for the push, which links the stack instead of retargeting it

### Requirement: Stacks of GitHub: Create Pull Request of a layer opens the layers below it

Create Pull Request of a layer MUST open the pull requests of the layers below it that have none, bottom first, each
into the branch below it, the bottom into the target, with the text of its own commits; then its own into its
parent's branch, with the text of its own commits against that branch; then link the stack (`LinkStack`).

#### Scenario: Create Pull Request of the top layer
- **GIVEN** a straight chain `auth` → `login` → `oauth` without pull requests
- **WHEN** the user creates the pull request of `oauth`
- **THEN** the pull requests of `auth` (into the target) and `login` (into `auth`) are opened first, each with the text
  of its own commits
- **AND** then that of `oauth` into `login`, with the text of its own commits against that branch, then the stack is
  linked
  [@test] crates/operations/src/pull_request_test.rs (`the_creation_of_a_layer_opens_the_layers_below_it_its_own_into_its_parent_and_links_the_stack`, `the_link_of_a_stack_makes_the_stack_of_its_layers`)
  [@test] src/lib/pull-request-dialog.test.ts (`creates the pull request of a layer of a stack of GitHub into the branch below it, with its own commits`)
  [@test] tests/e2e/stacked-branches/github-stacks.spec.ts (`should create the pull request of a layer into its parent's branch with those of the layers below it, and link the stack`)

### Requirement: Stacks of GitHub: The dialog of a layer says what it creates

The dialog MUST say it under the title: "A layer of the GitHub stack auth → login → oauth: it shows only the commits of
oauth, into login. Also creates the pull requests of auth and login."

#### Scenario: The dialog of the top layer
- **WHEN** the dialog of Create Pull Request of `oauth` opens
- **THEN** under the title it says "A layer of the GitHub stack auth → login → oauth: it shows only the commits of
  oauth, into login. Also creates the pull requests of auth and login."
  [@test] src/lib/pull-request-dialog.test.ts (`creates the pull request of a layer of a stack of GitHub into the branch below it, with its own commits`)

### Requirement: Stacks of GitHub: The link brings the stack of GitHub in line with the chain

The link of a stack MUST bring the stack of GitHub in line with the chain by the open pull requests of its branches,
in the order GitHub requires (`stack_writes`).

#### Scenario: Layers outside a stack
- **WHEN** the layers are in no stack of GitHub
- **THEN** the link makes one of them
  [@test] crates/domain-policy/src/github_stack_test.rs (`layers_outside_a_stack_make_one`)

#### Scenario: A stack in line, new layers on top
- **WHEN** the stack is in line with the chain, the layers left above a partial merge included
- **THEN** the link writes nothing
- **AND** new layers go on top of it, retargeted first when they go into another virtual branch
  [@test] crates/domain-policy/src/github_stack_test.rs (`a_stack_in_line_needs_nothing_and_new_layers_go_on_top`, `the_layers_left_after_a_partial_merge_are_in_line`)

#### Scenario: A stack out of line
- **WHEN** the stack is out of line otherwise
- **THEN** the link unstacks what is there, retargets, and makes a new stack
  [@test] crates/domain-policy/src/github_stack_test.rs (`a_stack_out_of_line_is_made_anew`)

#### Scenario: Where the layers end
- **WHEN** the chain has a branch without a pull request, or one into the target or another base above the bottom (a
  pull request of before, which the user takes in)
- **THEN** the layers end there
- **AND** fewer than two make no stack

### Requirement: Stacks of GitHub: A refusal of GitHub leaves the pull requests as they are

A refusal of GitHub MUST fail the effect and leave the pull requests as they are.

#### Scenario: GitHub refuses a write of the link
- **WHEN** GitHub refuses a write of the link of a stack
- **THEN** the effect fails and the pull requests stay as they are

### Requirement: Stacks of GitHub: A push of a layer links its stack

A push of a layer MUST link its stack after the push while the stack is out of line.

#### Scenario: A push of a layer whose stack is out of line
- **WHEN** a layer whose stack is out of line is pushed
- **THEN** its stack is linked after the push
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_layer_links_its_stack_and_one_github_moved_waits_for_the_update`)

### Requirement: Stacks of GitHub: A layer GitHub moved onto the target waits for the update

A layer GitHub moved onto the target after the layers below it merged, while the branch is still built on them, MUST
refuse the push with "GitHub moved #9 of oauth onto master after the layers below it merged: Update Project first"
(`LayerRebased`): its push would put the merged layers back over GitHub's rebase, whose copies have the same patches.

#### Scenario: GitHub moved the layer after the layers below it merged
- **GIVEN** the layers below `oauth` merged and GitHub moved #9 of `oauth` onto `master`, while the branch is still
  built on them
- **WHEN** `oauth` is pushed
- **THEN** the push is refused with "GitHub moved #9 of oauth onto master after the layers below it merged: Update
  Project first" (`LayerRebased`)
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_layer_links_its_stack_and_one_github_moved_waits_for_the_update`)
  [@test] crates/domain-policy/src/github_stack_test.rs (`a_layer_github_moved_onto_the_target_waits_for_the_update`)

### Requirement: Stacks of GitHub: Link GitHub Stack is offered for a layer out of line

"Link GitHub Stack" (`link-github-stack`, `Intent::LinkStack`) MUST be offered in the menu of a layer whose stack is
out of line with its chain, an old pull request into the target above the bottom joining it (`stack_writes` with
`join`).

#### Scenario: An old pull request into the target above the bottom
- **GIVEN** a layer whose stack is out of line with its chain, and an old pull request into the target above the bottom
- **WHEN** the user opens the menu of the layer
- **THEN** it offers "Link GitHub Stack", and only that link joins the old pull request to the stack
  [@test] crates/domain-policy/src/github_stack_test.rs (`an_old_pull_request_into_the_target_joins_the_stack_only_when_the_user_links_it`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_layer_whose_stack_is_out_of_line_offers_link_github_stack`)

### Requirement: Stacks of GitHub: Link GitHub Stack pushes, retargets, then links

"Link GitHub Stack" MUST push what the chain lacks on the remote, retarget that pull request into the branch below it,
then link the stack, and tell it ("Linked the GitHub stack of login").

#### Scenario: The user links the stack from the menu
- **WHEN** the user chooses "Link GitHub Stack" in the menu of the row
- **THEN** what the chain lacks on the remote is pushed, the old pull request is retargeted into the branch below it,
  and the stack is linked
- **AND** it tells "Linked the GitHub stack of login"; in line, it has nothing to link
  [@test] crates/operations/src/pull_request_test.rs (`link_github_stack_joins_an_old_pull_request_and_has_nothing_to_link_in_line`)
  [@test] tests/e2e/stacked-branches/github-stacks.spec.ts (`should take an old pull request into master into the stack with Link GitHub Stack from the menu of the row`)

### Requirement: Stacks of GitHub: Link Stack is the next step of a layer in no stack

It MUST be the next step of a layer into the branch below it that GitHub lists in no stack (row 5, "Link Stack" in the
slot), whose merge would land nothing.

#### Scenario: A layer GitHub lists in no stack
- **GIVEN** a layer into the branch below it that GitHub lists in no stack
- **WHEN** its next step is decided
- **THEN** it is row 5, "Link Stack" in the slot
- **AND** the slot links the stack
  [@test] crates/domain-policy/src/next_step_test.rs (`row_5_links_the_stack_of_a_layer_github_lists_in_none`)
  [@test] src/presenters/next-step.test.ts (`row 5: links the stack of GitHub of a layer GitHub lists in none, short in its column`)
  [@test] tests/e2e/stacked-branches/github-stacks.spec.ts (`should link the stack of a layer outside it from the slot of the row, as its merge would land nothing`)

### Requirement: Push Up to Here: A commit the remote lacks offers Push Up to Here

A commit the remote lacks MUST offer "Push Up to Here", as the view says it (`BranchPush.upTo`: the commits that offer
it, what it pushes first and what stops it, `domain_policy::push::up_to_push`, sent only where it differs from the
common answer).

#### Scenario: The push arrow and the menu of a commit
- **GIVEN** a commit the remote lacks
- **WHEN** the user clicks its push arrow after the subject in the commit list of a branch (`PushUpToMark`), in the
  table, a stack island or the Parked card
- **THEN** it opens what it sends (`pushUpToIn`)
- **AND** the menu of the commit pushes it
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`, `should push up to a commit from its menu in the full list`)

#### Scenario: The view sends the answer only where it differs
- **WHEN** the session derives the view of a branch
- **THEN** `BranchPush.upTo` is sent only where it differs from the common answer
  [@test] crates/repository-model/src/derive_test.rs (`the_view_sends_push_up_to_here_only_where_it_differs_from_the_common_answer`)

### Requirement: Push Up to Here: The press pushes the branches below whole, then the branch up to the commit

The press MUST be the intent `Push` with the scope `upTo` of the commit's copy, which Rust plans again on the facts of
its run: the branches the commit is built on that have work for the remote, whole (the branch is built on their tips),
root first, then the branch up to the commit, frozen instead of its tip (its tip witnessed), in the one atomic push of
a stack.

#### Scenario: Push Up to Here of a commit of a stacked branch
- **WHEN** the user presses "Push Up to Here" of a commit of a stacked branch
- **THEN** the branches it is built on that have work for the remote are pushed whole, root first, then the branch up
  to the commit, in one atomic push
  [@test] crates/operations/src/push_test.rs (`push_up_to_here_pushes_the_branch_up_to_the_commit_after_what_it_is_built_on`)
  [@test] crates/sync-core/src/push_test.rs (`a_stack_push_up_to_a_commit_pushes_the_branches_below_whole`)

### Requirement: Push Up to Here: The commits after the cut stay unpushed, counted as the user's

The commits after it MUST stay unpushed, counted as the user's as a sync counts them (the branch's author).

#### Scenario: After a push up to a commit
- **WHEN** a push up to a commit succeeded
- **THEN** the commits after it stay unpushed, counted as the user's as a sync counts them
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`)

### Requirement: Push Up to Here: The last commit the remote lacks is the push of the whole branch

The last commit the remote lacks MUST be the push of the whole branch.

#### Scenario: Push Up to Here of the last commit
- **WHEN** the user pushes up to the last commit the remote lacks
- **THEN** it is the push of the whole branch

### Requirement: Push Up to Here: A commit a sync rebuilt since has no plan

A commit a sync rebuilt since (another copy) MUST have no plan: nothing is pushed (`plan_changed`).

#### Scenario: A sync rebuilt the commit after the press
- **WHEN** a sync rebuilt the commit since, as another copy
- **THEN** nothing is pushed (`plan_changed`)

### Requirement: Push Up to Here: The popover says it in the words of Push Stack

The popover MUST say it in the words of Push Stack: "Pushes 3 branches, parents first:", a line per push, "bridge · 1
of 2 commits · creates the remote branch"; its button "Push Up to Here" marks, while hovered, the nodes of the branches
and the commits it sends. One toast says what was pushed, the part of the branch counted.

#### Scenario: The popover of a push up to a commit
- **WHEN** the popover of a push up to a commit opens
- **THEN** it says "Pushes 3 branches, parents first:" and a line per push, "bridge · 1 of 2 commits · creates the
  remote branch"
- **AND** while its button "Push Up to Here" is hovered, the nodes of the branches and the commits it sends are marked
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`)

#### Scenario: The toast
- **WHEN** the push up to a commit succeeded
- **THEN** one toast says what was pushed, the part of the branch counted
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`)

### Requirement: Push Up to Here: Only a commit on the fast-forward tail can be a cut

Only a commit on the fast-forward tail SHALL be a cut: the branch is not on the remote, or the remote has an older state
of it (its remote status is a fast-forward). The commit is one of the branch's own the remote lacks, of the tip the
plan witnesses; else nothing is pushed.

#### Scenario: The branch is not on the remote, or the remote has an older state
- **WHEN** the remote status of the branch is a fast-forward, or the branch is not on the remote
- **THEN** a commit of the branch's own the remote lacks, of the tip the plan witnesses, can be a cut
  [@test] crates/domain-policy/src/push_test.rs (`push_up_to_here_pushes_the_ancestors_whole_and_stops_at_what_cant_go`)

#### Scenario: Another commit
- **WHEN** the commit is not one of the branch's own the remote lacks, of the tip the plan witnesses
- **THEN** nothing is pushed

### Requirement: Push Up to Here: A branch rewritten since its push pushes whole

A branch rewritten since its push MUST push whole: "login was rewritten since its push: it pushes whole"
(`Reason::RewrittenSincePush`).

#### Scenario: A rewritten branch
- **GIVEN** `login` was rewritten since its push
- **WHEN** Push Up to Here of one of its commits is asked
- **THEN** it says "login was rewritten since its push: it pushes whole" (`Reason::RewrittenSincePush`)
  [@test] crates/domain-policy/src/push_test.rs (`push_up_to_here_pushes_the_ancestors_whole_and_stops_at_what_cant_go`)

### Requirement: Push Up to Here: It says why it can't, with a link to the branch in the way

It MUST say why it can't, in the words of the reason Rust gives, with a link that shows the branch in the way; the
menu item is disabled with the reason.

#### Scenario: An ancestor blocked or failed to sync
- **WHEN** an ancestor is blocked or failed to sync
- **THEN** it says "auth can't be pushed: blocked by core", with a link that shows the branch in the way
  [@test] crates/domain-policy/src/push_test.rs (`push_up_to_here_pushes_the_ancestors_whole_and_stops_at_what_cant_go`)

#### Scenario: An ancestor whose push drops remote commits
- **WHEN** the push of an ancestor drops remote commits
- **THEN** it says "The remote branch of auth has 2 commits that auth lacks: push it from its row, which shows them":
  it asks first in its own row
  [@test] crates/domain-policy/src/push_test.rs (`push_up_to_here_pushes_the_ancestors_whole_and_stops_at_what_cant_go`)

#### Scenario: A branch a running Safe Push lands
- **WHEN** a running Safe Push lands a branch of the push
- **THEN** it says why, and the menu item is disabled with the reason

#### Scenario: A parked branch or one built on it
- **WHEN** the branch is parked or built on a parked branch
- **THEN** it says so in the words of its push button ("Parked: Unpark it to push", "Built on parked core: Unpark core
  to push", [Parked Branches](../parked-branches/spec.md#the-app)), and the menu item is disabled with the reason
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

### Requirement: Push Up to Here: A commit a merged Safe Push landed has no push of its own

A commit a merged Safe Push landed (`patronus_view::landed_commits`: every commit of a landed branch but those added
after the landed ones, [Patronus](../patronus/spec.md#merged)) MUST be in the target, as a commit the remote has: one
answer of the view for every surface (`BranchPush.upTo.commits`), so it has no push arrow and no "Push Up to Here" in
its menu.

#### Scenario: The commits a merged Safe Push landed
- **GIVEN** a merged Safe Push landed the commits of a branch, and a commit was added after them
- **WHEN** the commit list shows
- **THEN** the landed commits have no push arrow and no "Push Up to Here" in their menu
- **AND** only the commit added after them offers it
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should show no push arrow on the commits a merged Safe Push landed, only on a commit added after them`, `should offer Push Up to Here in the full list only for a commit added after the ones a merged Safe Push landed, without the landed branches`)

### Requirement: Push Up to Here: A branch landed whole is no step of a push up to a commit

A branch it landed whole MUST be no step of a push up to a commit built on it: the branch is pushed without it, as its
own push leaves it.

#### Scenario: A commit built on a landed branch
- **GIVEN** a branch a merged Safe Push landed whole, and a commit built on it
- **WHEN** the user pushes up to that commit
- **THEN** the landed branch is no step of the push
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should offer Push Up to Here in the full list only for a commit added after the ones a merged Safe Push landed, without the landed branches`)

### Requirement: Messages: The grouping preview reads a prefix as the grouping does

The grouping preview of a typed message or branch name (`preview_group`) MUST read a prefix as the grouping does: the
leaf, its parent and an empty level.

#### Scenario: The preview of a stacked prefix
- **WHEN** the user types a message or a branch name with a stacked prefix
- **THEN** the preview reads the leaf and the parent of the prefix, else the stack parent, and an empty level, as the
  grouping does
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_with_stacked_branches_reads_the_parent_of_the_prefix_else_the_stack_parent`)

#### Scenario: The preview of a subject
- **WHEN** the user types a subject
- **THEN** the preview groups it as the grouping does: its prefix, else the issue group of its key, else the key
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_of_a_subject_is_its_prefix_else_the_issue_group_of_its_key_else_the_key`)

### Requirement: Messages: The editor says when an edit restacks the branch

The commit message editor MUST say when an edit restacks the branch.

#### Scenario: An edit that restacks
- **WHEN** an edit of a message changes the parent level of its prefix
- **THEN** the editor names the new branch and its parent
- **AND** for a single-commit branch, it says the branch is restacked or unstacked
  [@test] src/lib/stack.test.ts (`names the new branch and its parent`, `says a single-commit branch is restacked or unstacked`)

### Requirement: Messages: The editor warns when an edit makes the parents conflict

The commit message editor MUST warn when an edit would make the branch's parents conflict, also with its stack parent.

#### Scenario: A parent the other commits don't declare
- **WHEN** the edit declares a parent the other commits of the branch don't declare
- **THEN** the editor warns that the parents would conflict
  [@test] src/lib/stack.test.ts (`warns about a parent the other commits of the branch don't declare`)

#### Scenario: A parent level that differs from the stack parent
- **WHEN** the edit declares a parent level that differs from the stored parent
- **THEN** the editor warns that the parents would conflict
  [@test] src/lib/stack.test.ts (`warns about a parent level that differs from the stored parent`)

### Requirement: Messages: The editor warns about a parent without commits

The commit message editor MUST warn when an edit would name a parent without commits, likely a typo: "api has no
commits: docs is built on master until it has".

#### Scenario: A parent without commits
- **WHEN** the edit names `api` as the parent of `docs`, and `api` has no commits
- **THEN** the editor warns "api has no commits: docs is built on master until it has"
  [@test] src/lib/stack.test.ts (`warns about a parent without commits`)

### Requirement: Fixups: A fixup agrees with its target by the leaf of the prefix

A fixup's prefix MUST agree with its target's when both name the same leaf, so `fixup! (login) X` finds
`(auth/login) X`.

#### Scenario: A fixup names the leaf of its target
- **WHEN** a fixup `fixup! (login) X` and a commit `(auth/login) X` name the same leaf
- **THEN** the fixup finds its target, also in the plan of the squash
  [@test] crates/history-rewrite/src/autosquash_test.rs (`with_stacks_a_fixup_names_the_leaf_of_its_target`, `the_plan_matches_a_fixup_by_the_leaf_of_its_prefix`)

#### Scenario: Another leaf
- **WHEN** the prefix of a fixup names another leaf than its target's
- **THEN** they do not agree
  [@test] crates/history-rewrite/src/autosquash_test.rs (`with_stacks_another_leaf_does_not_agree`)

### Requirement: UI: The branches come newest first by where each begins

The branches MUST come newest first by where each begins in the history ahead of the baseline, for the table and
`brd status` alike (`prepare_branches_for_ui`): the branch whose oldest commit is the newest comes first, a stack where
its root comes, an issue family where its first member comes. No time decides it.

#### Scenario: The order of the table and of brd status
- **WHEN** the table or `brd status` lists the branches
- **THEN** the branch whose oldest commit is the newest comes first, a stack where its root comes, an issue family where
  its first member comes
  [@test] crates/sync-core/src/grouping_test.rs (`the_branches_come_newest_first_by_their_oldest_commit_and_no_rewrite_of_their_commits_moves_one`)

### Requirement: UI: A rewrite keeps every commit it keeps in its place

A rewrite MUST keep every commit it keeps in its place, but give each commit above the oldest one it rewrites a new
committer time: an order by the newest commit moved the rows of the whole table after the user's own squash of one
fixup, and the pointer that pressed Squash was over the push button of another branch.

#### Scenario: The rewrites keep the order
- **WHEN** a rewrite runs
- **THEN** a reword, an amend and a move of a hunk keep each one, a squash keeps the oldest commit, a fixup folds into
  its older target, Update Project and a pick keep the order of the commits
- **AND** no row moves
  [@test] crates/sync-core/src/grouping_test.rs (`the_branches_come_newest_first_by_their_oldest_commit_and_no_rewrite_of_their_commits_moves_one`)

#### Scenario: The squash of the fixup of the branch that began first
- **WHEN** the user squashes the fixup of the branch that began first
- **THEN** every row stays in its place
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should keep every row in its place when the fixup of the branch that began first is squashed`)

### Requirement: UI: A new commit moves no row, a move places only what changed

A commit added to a branch MUST move no row either; a branch of new commits comes first, a branch grouped of older
commits (a name for unassigned commits) where its oldest commit is, and a move to another branch places again only
the branches whose oldest commit changes.

#### Scenario: New commits and older ones
- **WHEN** a commit is added to a branch, a branch of new commits appears, or a branch is grouped of older commits
- **THEN** the added commit moves no row, the branch of new commits comes first, and the branch of older commits comes
  where its oldest commit is
  [@test] crates/sync-core/src/grouping_test.rs (`the_branches_come_newest_first_by_their_oldest_commit_and_no_rewrite_of_their_commits_moves_one`)

#### Scenario: A move to another branch
- **WHEN** commits move to another branch
- **THEN** only the branches whose oldest commit changes are placed again

### Requirement: UI: The table shows each branch under the branch it stacks on

The branch table MUST show each branch under the branch it stacks on, depth-first; among siblings the shortest chain
comes first (ties newest first), so the longest chain comes last and continues its parent's line.

#### Scenario: A stack in the table
- **WHEN** the table shows a stack
- **THEN** each branch is under its parent, depth-first, wherever the parent is in the order
- **AND** among siblings the shortest chain comes first (ties newest first), the longest last
  [@test] src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)

### Requirement: UI: The table draws each stack as a graph

The table MUST draw each stack as a graph, root on top, instead of a staircase: a straight chain is one line, a side
child forks off on a lane of its own, and the expanded commits of a branch sit on its line
([Branch Row](../branch-row/spec.md#tree)).

#### Scenario: The commits of an expanded branch
- **WHEN** a branch of a stack is expanded
- **THEN** its commits sit on its lane, which goes on into the lines of its children
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should carry the commits of a branch on its lane, into the lines of its children`)

### Requirement: UI: A branch without a parent row or in a cycle stays at the top level

A branch whose parent has no row, or whose parents form a cycle, MUST stay at the top level.

#### Scenario: No parent row, a cycle
- **WHEN** the parent of a branch has no row, or its parents form a cycle
- **THEN** the branch stays at the top level
  [@test] src/stores/branch-table-rows.test.ts (`keeps a branch whose parent has no row, or whose parents form a cycle, at the top level`)

### Requirement: UI: Where each branch is in its stack is Rust's

Where each branch is MUST be Rust's, for the page and `brd` alike (`domain_policy::stack`, placed once per grouping by
`repository_model::derive` as the structure of the branch in the session's view). The page lays the rows, their depth
and their lanes out of it and decides none of it; a patch carries the structure of a branch only when it changed.

#### Scenario: A grouping places every branch
- **WHEN** a grouping arrives
- **THEN** the session places every branch once, with its section, in its view
  [@test] crates/repository-model/src/derive_test.rs (`a_grouping_places_every_branch_with_its_section`)

#### Scenario: A patch of the view
- **WHEN** the view is patched
- **THEN** the patch carries the structure of a branch only when it changed
  [@test] crates/repository-model/src/derive_test.rs (`a_patch_carries_only_the_structures_that_changed`)

#### Scenario: The page lays the rows out
- **WHEN** the page lays out the table
- **THEN** it takes the children and the descendants of a branch from the structure and decides none of it
  [@test] src/stores/branch-table-rows.test.ts (`lists the children and the descendants of a branch`)

### Requirement: UI: The structure of a branch names its parent, children, ancestors and main line

Its parent MUST be the branch of the table it declares (a `(parent/child)` prefix or its stored parent), so a branch
whose declared parent has no commits is a root of its own; its children in the order of the table, and its
descendants, each after its parent; its ancestors, the chain of its effective bases among the branches of the table;
the main line below it, at each fork the child with the longest chain, the last of children with equal ones.

#### Scenario: Children and descendants
- **WHEN** branches declare their parents
- **THEN** the children and the descendants follow the declared parents in the order of the table, each after its
  parent
  [@test] crates/domain-policy/src/stack_test.rs (`children_and_descendants_follow_the_declared_parents_in_the_order_of_the_table`)

#### Scenario: A declared parent the table lacks
- **WHEN** the declared parent of a branch has no commits, so the table lacks it
- **THEN** the branch is a root of its own
  [@test] crates/domain-policy/src/stack_test.rs (`a_declared_parent_the_table_lacks_makes_a_root`)

#### Scenario: The ancestors
- **WHEN** the ancestors of a branch are asked
- **THEN** they are the chain of its effective bases among the branches of the table
  [@test] crates/domain-policy/src/stack_test.rs (`the_ancestors_are_the_chain_of_the_effective_bases_in_the_table`)

#### Scenario: The main line
- **WHEN** a stack forks
- **THEN** the main line takes the child with the longest chain at each fork, the last of children with equal ones
- **AND** a cycle of declared parents ends the descendants and the main line
  [@test] crates/domain-policy/src/stack_test.rs (`the_main_line_takes_the_longest_chain_at_each_fork_and_the_last_of_equal_ones`, `a_cycle_of_declared_parents_ends_the_descendants_and_the_main_line`)

### Requirement: UI: Each row carries its place in the stack graph

Each row MUST carry its place in the stack graph (`lanes`, `src/lib/stack-lanes.ts`): its lane, the lane a side child
forks from, the line up into its node and down from it, and the lanes passing it.

#### Scenario: The rows of a stack
- **WHEN** the rows of a stack are laid out
- **THEN** each carries its lane, the lane a side child forks from, the lines up into its node and down from it, and
  the lanes passing it
  [@test] src/stores/branch-table-rows.test.ts (`keeps a straight chain on one lane and forks a leaf sibling and a side chain off it`)

### Requirement: UI: The lanes of a stack never cross

A straight chain MUST stay on lane 0; at a fork the last child continues the parent's lane and each other child takes
the lowest free lane for its subtree, which follows the parent at once, so lines never cross; a lane is free again
after its subtree.

#### Scenario: A straight chain
- **WHEN** a stack is a straight chain
- **THEN** it stays on lane 0
  [@test] src/lib/stack-lanes.test.ts (`keeps a straight chain on lane 0`)

#### Scenario: A fork
- **WHEN** a stack forks
- **THEN** the last child, the longest chain, continues the parent's lane, and the others fork off it, shortest first,
  each on the lowest free lane for its subtree
  [@test] src/lib/stack-lanes.test.ts (`continues the longest chain on the parent's lane and forks the others off it, shortest first`, `keeps the given order of children with chains of the same length: the oldest is the main line`)
  [@test] src/stores/branch-table-rows.test.ts (`keeps a straight chain on one lane and forks a leaf sibling and a side chain off it`)

#### Scenario: A fork inside a side subtree
- **WHEN** a side subtree forks
- **THEN** it opens a lane per fork, which is free again after the subtree
  [@test] src/lib/stack-lanes.test.ts (`opens a lane per fork inside a side subtree, and frees it after the subtree`)

### Requirement: UI: Each stack starts at lane 0

Each stack MUST start at lane 0, so separate stacks share no line.

#### Scenario: Separate stacks
- **WHEN** the table has separate stacks
- **THEN** each starts at lane 0, and they share no line
  [@test] src/stores/branch-table-rows.test.ts (`starts each stack on lane 0, so separate stacks share no line`)

### Requirement: UI: Without a stack every row has lane 0 alone

Without a branch stacked on another, every row MUST have lane 0 alone (`alone`): the node of the branch, no line, its
commits newest first.

#### Scenario: No branch is stacked on another
- **WHEN** no branch is stacked on another
- **THEN** every row has lane 0 alone (`alone`): the node of the branch, no line, its commits newest first
  [@test] src/stores/branch-table-rows.test.ts (`puts every branch on one lane while no branch is stacked on another: its node says where it is on its way into master`)

### Requirement: UI: The review window shows the stack of a stacked branch as segments

The review window of a stacked branch MUST show its stack from the baseline as segments: the branch on its parent
first, Shift+click extends the part, "From Root" shows what its Safe Push lands and Patronus checks
([Branch Review](../branch-review/spec.md#the-stack-and-the-commits)).

#### Scenario: The review of a stacked branch
- **WHEN** the review window of a stacked branch opens
- **THEN** it shows the branch on its parent first
- **AND** Shift+click extends the part, and "From Root" shows what its Safe Push lands and Patronus checks
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: UI: Issue families are formed from the stack roots only

Issue families MUST be formed from the stack roots only.

#### Scenario: A stack under an issue
- **WHEN** the table forms issue families
- **THEN** it forms them from the stack roots only, the branches of an issue nested under the issue branch wherever it
  is in the order
  [@test] src/stores/branch-table-rows.test.ts (`forms issue families from the stack roots only and marks a child of another issue`)
  [@test] src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`)

### Requirement: UI: A family member and the stack on it are indented one level

A family member and the stack on it MUST be indented one level (`familyIndent`), and the stack keeps its lanes inside
it: lanes mean "built on", the indent "belongs to the issue".

#### Scenario: A stack on a family member
- **WHEN** a member of an issue family has a stack on it
- **THEN** the member and the stack are indented one level, with the lanes of the stack inside
  [@test] src/stores/branch-table-rows.test.ts (`indents a family member and the stack on it one level, with the lanes of the stack inside`)

### Requirement: UI: A stacked branch of another issue shows its issue as a chip

A stacked branch of another issue than its parent MUST show its issue as a chip, unless the branch is named after it.

#### Scenario: A child of another issue
- **WHEN** a stacked branch belongs to another issue than its parent
- **THEN** it shows its issue as a chip
  [@test] src/stores/branch-table-rows.test.ts (`forms issue families from the stack roots only and marks a child of another issue`)

#### Scenario: A stacked branch named after its issue
- **WHEN** the stacked branch is named after its issue
- **THEN** it shows no chip
  [@test] src/stores/branch-table-rows.test.ts (`shows no chip for a stacked branch named after its issue`)

### Requirement: UI: A stack and an issue family each read as one block

A stack and an issue family MUST each read as one block ([Branch Row](../branch-row/spec.md#tree), the dividers).

#### Scenario: The dividers of the table
- **WHEN** the table shows stacks and issue families
- **THEN** each reads as one block

### Requirement: UI: Under the filter the matches keep the branches they stack on

Under the filter of the table ([Branch Filter](../branch-filter/spec.md)), the rows of the matches only
(`useVisibleBranchTableRows`) MUST include the branches they stack on, dimmed unless they are matches themselves, and
lay the graph out over the rows that remain, so a match deep in a stack stays connected to its root, also in a
collapsed stack: the filter wins over the collapse ([Branch Filter](../branch-filter/spec.md#the-filter-and-the-rest-of-the-table)).

#### Scenario: A search matches a branch deep in a stack
- **WHEN** the filter matches a branch deep in a stack
- **THEN** the rows are the matches and, dimmed, the branches they stack on, the graph laid out over them
  [@test] src/stores/branch-table-rows.test.ts (`prunes the rows to the visible branches and the branches they stack on, which are dimmed`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

#### Scenario: An issue family under the filter
- **WHEN** the filter matches some members of an issue family
- **THEN** the family is pruned to its visible members
  [@test] src/stores/branch-table-rows.test.ts (`prunes an issue family to its visible members`)

### Requirement: UI: The header counters and the selection read all rows under the filter

The header counters and the selection MUST read all rows.

#### Scenario: A filter is active
- **WHEN** a filter hides rows
- **THEN** the header counters and the selection still read all rows

### Requirement: UI: A big stack at the top level is an island of its own

A stack of 5 branches or 40 commits at the top level of the table MUST be an island of its own below it, its header
heading the stack, its rows those of the table ([Stack Island](../stack-island/spec.md)); the rules below are of a
stack in the table, and an island collapses by the same `collapsedStacks`.

#### Scenario: A stack of 5 branches
- **WHEN** a stack at the top level of the table has 5 branches or 40 commits
- **THEN** it is an island of its own below the table, its header heading the stack, its rows those of the table
- **AND** it collapses by the same `collapsedStacks`

### Requirement: UI: The root row of a stack heads it with a chip

The root row of a stack MUST head it, so a stack of 22 branches doesn't read as 22 rows. A stack is the tree of the
branches built on a root; what a branch's ancestors are to it is "built on", and a Safe Push "lands … with it". After
the root's name a chip (`StackChip`) MUST size the stack, "22 branches", and its tooltip say the rest: where the
branches stacked on the root are on their way into master.

#### Scenario: The chip of a stack root
- **WHEN** the table shows the root of a stack
- **THEN** after its name a chip says "22 branches"
- **AND** its tooltip says "Stack of vm-lane-daemon · 22 branches · 152 commits · 3 your step · 18 waiting · Collapse
  Stack ⌥←"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should head a stack with its size and collapse it into its root, per repository`)

### Requirement: UI: A stack collapses into its root and expands

A click on the chip, ⌥← from any row of the stack (the focus moves to the root) and "Collapse Stack" of the branch menu
MUST collapse the stack into its root; the click, ⌥→ and "Expand Stack" MUST expand it. A held key collapses once.

#### Scenario: The chip
- **WHEN** the user clicks the chip of a root, then clicks it again
- **THEN** the stack collapses into its root, then expands
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should head a stack with its size and collapse it into its root, per repository`)

#### Scenario: Alt+arrows from any row of the stack
- **WHEN** the user presses ⌥← on any row of the stack, then ⌥→
- **THEN** the stack collapses into its root, which takes the focus, then expands
- **AND** a held key collapses once
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should collapse and expand a stack with Alt+arrows from any of its rows`)
  [@test] src/stores/branch-table-rows.test.ts (`marks no root collapsed whose stack has no other row, and finds the root from any row of a stack`)

#### Scenario: The branch menu
- **WHEN** the user chooses "Collapse Stack", then "Expand Stack" in the branch menu
- **THEN** the stack collapses into its root, then expands

### Requirement: UI: A collapsed root shows the strongest state its hidden rows hold

A collapsed root MUST have no line down from its node, a short line fading out below it instead (`lane-collapsed`), and
its chip MUST add the strongest state the hidden rows hold, by the nodes of their rows (`summarizeStackState`,
[Landing](../../../../docs/design/landing.md)), in the colour of the node, the user's moves first. They are the words of
the landing overview and of the filter chips; a push is no state of its own.

#### Scenario: The states of a collapsed stack
- **WHEN** a stack is collapsed
- **THEN** its root has a short line fading out below its node (`lane-collapsed`) instead of a line down
- **AND** its chip adds, by the nodes of the hidden rows, "· 1 to fix" (`destructive`: a conflict, an error, a robot
  that failed on the branch as it is, changes requested), "· 2 your step" (`primary`), "· 3 ready" (`success`), then
  "· 4 landing" with the ring of the robot, "· 2 waiting" (muted: reviewers, the parent, a robot that holds it) and "·
  1 in master" (`info`)
  [@test] src/lib/stack-state.test.ts (`counts the rows of a stack by their nodes, and nothing of a branch without one`, `puts the user's moves first, a fix before a step before ready, then what lands, waits and is in master, in the colours of the nodes`)

### Requirement: UI: What lands never hides behind a stronger state

What lands MUST never hide behind a stronger state: "· 1 to fix · 2 landing", the landing with its ring beside the
fix.

#### Scenario: A landing beside a fix
- **GIVEN** a collapsed stack with a branch to fix and two branches landing
- **WHEN** its chip shows
- **THEN** it says "· 1 to fix · 2 landing", the landing with its ring beside the fix
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should keep the landing of a collapsed stack beside the fix of another of its branches`)

### Requirement: UI: A robot of an earlier state, fixed since, makes no fix

A robot of an earlier state, fixed since, MUST make no fix: the row offers a new Safe Push.

#### Scenario: A robot failed on an earlier state of a branch
- **GIVEN** a robot failed on an earlier state of a branch of a collapsed stack, fixed since
- **WHEN** the chip of the stack shows
- **THEN** it counts no fix for that robot, only the robots that failed on its branches as they are
- **AND** the row offers a new Safe Push
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should name in the chip of a collapsed stack only the robots that failed on its branches as they are`)

### Requirement: UI: A collapsed stack is remembered per repository

Stacks MUST be expanded by default; a collapsed one is remembered per repository (`collapsedStacks`, by root; a rename
of the root keeps it, a stack on another branch or a merged root leaves the list), and the table keeps the lane column
of the expanded stacks, so no name moves.

#### Scenario: Collapse, then reopen the repository
- **WHEN** the user collapses a stack and opens the repository again
- **THEN** the stack is still collapsed, and other repositories keep their own
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should head a stack with its size and collapse it into its root, per repository`)

#### Scenario: The lane column of a collapsed stack
- **WHEN** a stack collapses into its root
- **THEN** the table keeps the lane count of the expanded table, so no name moves
  [@test] src/stores/branch-table-rows.test.ts (`collapses a stack to its root and keeps the lane count of the expanded table`)

#### Scenario: The list of collapsed roots
- **WHEN** a collapsed root is renamed, no longer heads a stack, or merged
- **THEN** a rename keeps it in the list; a stack on another branch or a merged root leaves the list
  [@test] src/stores/branch-table-rows.test.ts (`keeps only the collapsed roots that still head a stack`)

### Requirement: UI: A collapse closes hidden inline forms, a reveal expands

Collapsing MUST close the inline form of a hidden row; a reveal of a hidden branch, or an inline form opened for one
elsewhere, MUST expand its stack.

#### Scenario: A reveal of a branch in a collapsed stack
- **GIVEN** a collapsed stack
- **WHEN** a branch it hides is revealed, or an inline form is opened for one elsewhere
- **THEN** its stack expands
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show a match deep in a collapsed stack, and expand the stack for a reveal`)

#### Scenario: Collapsing with an open inline form
- **GIVEN** an inline form open on a row of a stack
- **WHEN** the stack collapses and hides the row
- **THEN** the form closes

### Requirement: UI: The counters, the landing overview and Your Turn count every branch

The header counters, the landing overview (a stack is one row there, its root's) and Your Turn MUST count every
branch.

#### Scenario: A collapsed stack
- **WHEN** a stack is collapsed
- **THEN** the header counters, the landing overview (a stack is one row there, its root's) and Your Turn still count
  every branch

### Requirement: UI: A side stack has the chip of a stack root

A side stack, a branch forked off the lane of its parent with a branch stacked on it, MUST have the chip of a stack
root on its row ("2 branches", "· 1 to push" while collapsed, `sideStackSizes`), which collapses the side stack into
it, in the table and in an island, for the session. The main line has no chip.

#### Scenario: The chip of a side stack
- **WHEN** a branch forked off the lane of its parent has a branch stacked on it
- **THEN** its row has the chip "2 branches" ("· 1 to push" while collapsed), which collapses the side stack into it
  [@test] src/stores/branch-table-rows.test.ts (`collapses a side stack into its root, and keeps a quiet one collapsed until the user expands it`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should collapse a side stack into its root, from its chip and with Alt+Left before the whole stack`)

#### Scenario: The main line
- **WHEN** a branch is on the main line of its stack
- **THEN** it has no chip of a side stack

### Requirement: UI: A quiet side stack is collapsed until the user expands it

A side stack with nothing for the remote and nothing wrong (pushed, no error, not blocked) MUST be collapsed until the
user expands it.

#### Scenario: A pushed side stack without problems
- **WHEN** a side stack is pushed, has no error and is not blocked
- **THEN** it is collapsed until the user expands it
  [@test] src/stores/branch-table-rows.test.ts (`collapses a side stack into its root, and keeps a quiet one collapsed until the user expands it`)

### Requirement: UI: Alt+Left collapses the nearest side stack first

⌥← MUST work as ← in a tree: from a row of an expanded side stack it collapses the nearest one, its root takes the
focus, and the next ⌥← the whole stack; ⌥→ on a collapsed side stack expands it. A reveal of a branch it hides MUST
expand it.

#### Scenario: Alt+Left on a row of a side stack
- **WHEN** the user presses ⌥← on a row of an expanded side stack, then again
- **THEN** the side stack collapses first, its root taking the focus, then the whole stack
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should collapse a side stack into its root, from its chip and with Alt+Left before the whole stack`)

#### Scenario: Alt+Right and a reveal
- **WHEN** the user presses ⌥→ on a collapsed side stack, or a branch it hides is revealed
- **THEN** the side stack expands

### Requirement: UI: The branch menu offers Stack On

The branch menu MUST offer "Stack On…", a submenu of the other branches without the branch's descendants and its parent
(disabled with "No other branch to stack on" when none is left).

#### Scenario: Stack a branch on another one
- **WHEN** the user chooses a branch in "Stack On…" of the branch menu
- **THEN** the branch is stacked on it
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a branch on another one and unstack it`)

#### Scenario: The submenu lacks the descendants and the parent
- **WHEN** the user opens "Stack On…"
- **THEN** it lists the other branches without the branch's descendants and its parent
- **AND** with none left it is disabled with "No other branch to stack on"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should not offer a branch's own descendants to stack on`)

### Requirement: UI: The branch menu offers Unstack of a stacked branch

The branch menu MUST offer "Unstack from <parent>" for a stacked branch (the rewrite `unstack` of the engine, with an
undo).

#### Scenario: Unstack a stacked branch
- **WHEN** the user chooses "Unstack from <parent>" of a stacked branch
- **THEN** the rewrite `unstack` runs, with an undo
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a branch on another one and unstack it`)
  [@test] crates/sync-core/src/stack_operations_test.rs (`unstack_removes_the_line_and_the_parent_levels`)

### Requirement: UI: The branch menu offers Collapse Stack or Expand Stack

The branch menu MUST offer "Collapse Stack" (⌥←) or "Expand Stack" (⌥→) on a branch of a stack while no filter is
active.

#### Scenario: A branch of a stack without a filter
- **GIVEN** no filter is active
- **WHEN** the user opens the menu of a branch of a stack
- **THEN** it offers "Collapse Stack" (⌥←), or "Expand Stack" (⌥→) for a collapsed one

### Requirement: UI: Stack On and Unstack write the stack parents

Stack On and Unstack MUST write the stack parents and rewrite only the commits whose prefix declares another parent
([Stack Parents](../stack-parents/spec.md#restack-unstack-and-rename)), so an issue branch and an issue group are
stacked too.

#### Scenario: A restack of a branch with prefixes
- **WHEN** Stack On runs on a branch whose commits have parent levels in their prefixes
- **THEN** it writes the stack parents and rewrites only the commits whose prefix declares another parent
  [@test] crates/sync-core/src/stack_operations_test.rs (`restack_writes_the_store_and_rewrites_only_differing_parent_levels`)

#### Scenario: An issue branch
- **WHEN** the user stacks an issue branch
- **THEN** it is stacked without a rewrite, and Undo takes it back
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack an issue branch without a rewrite and undo it`)

### Requirement: UI: A toast of a restack without a moved commit names no count

A toast without a moved commit MUST name no count: "Stacked docs on oauth".

#### Scenario: Stack On without a rewrite
- **WHEN** Stack On moves no commit
- **THEN** its toast says "Stacked docs on oauth"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack an issue branch without a rewrite and undo it`)

### Requirement: UI: The selection bar offers Stack Together

Selecting two or more branches MUST show "Stack Together" on the selection bar. Its tooltip says "Stacks the selected
branches, the oldest at the bottom".

#### Scenario: Two branches selected
- **WHEN** the user selects two or more branches
- **THEN** the selection bar shows "Stack Together", its tooltip "Stacks the selected branches, the oldest at the
  bottom"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack the selected branches together from the selection bar, the oldest at the bottom, and undo it`)

### Requirement: UI: Stack Together stacks the branches oldest first

Stack Together MUST stack them oldest-first (the oldest commit at the bottom), keep an existing ancestor edge and a
moved branch's children, and use the `stack-on` blocker for every selected branch.

#### Scenario: Stack Together of several branches
- **WHEN** the user presses "Stack Together"
- **THEN** one rewrite stacks every named branch oldest-first, the oldest commit at the bottom, keeping an existing
  ancestor edge and a moved branch's children
  [@test] crates/operations/src/rewrite_test.rs (`stack_together_is_one_rewrite_of_every_named_branch`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack the selected branches together from the selection bar, the oldest at the bottom, and undo it`)

#### Scenario: The stack-on blocker
- **WHEN** two or more branches are selected
- **THEN** Stack Together is offered only while Stack On is available for each selected branch
  [@test] src/lib/branch-drop.test.ts (`stacks two or more selected branches together when Stack On is available for each`)

### Requirement: UI: A Stack Together is one toast, one Undo and one Activity entry

A successful click MUST clear the selection, show one toast with one Undo and one Activity entry, and the table shows
the new chain after its sync.

#### Scenario: After Stack Together
- **WHEN** Stack Together succeeds
- **THEN** the selection is cleared, one toast with one Undo and one Activity entry name what it did to every selected
  branch
- **AND** the table shows the new chain after its sync, and Undo takes it back
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack the selected branches together from the selection bar, the oldest at the bottom, and undo it`)
  [@test] src/presenters/rewrite.test.ts (`words a rewrite by what it does to what it names`)
  [@test] src/presenters/operations.test.ts (`names every selected branch as a row the rewrite works on`)

### Requirement: UI: A rename of a parent keeps its children stacked on it

Rename of a parent MUST keep its children stacked on it.

#### Scenario: Rename a parent
- **WHEN** the user renames a parent
- **THEN** its children follow and stay stacked on it
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should rename a parent with its children following`)
  [@test] crates/sync-core/src/stack_operations_test.rs (`renaming_a_parent_renames_it_in_its_children`)

### Requirement: UI: Ungroup and Unapply of a parent name its children

Ungroup and Unapply of a parent MUST name its children: "oauth stays stacked on login and is built on master until
login has commits again." See [Move Commits](../move-commits/spec.md).

#### Scenario: The confirmation of an ungroup of a parent
- **WHEN** the user ungroups or unapplies `login`, with `oauth` stacked on it
- **THEN** the confirmation says "oauth stays stacked on login and is built on master until login has commits again."
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

### Requirement: UI: A branch built elsewhere says so in its Status cell

A branch built elsewhere because its declared parent has no commits (`prBaseChanged`) MUST say so in the third column
of its Status cell: a muted "on master" (or the ancestor it is built on) with the reason as its tooltip. Its push
button is enabled.

#### Scenario: The parent landed or has no commits
- **WHEN** the declared parent of `oauth` has no commits
- **THEN** the third column of its Status cell says a muted "on master" (or the ancestor it is built on), with the
  reason as its tooltip: "login landed · oauth is built on master"; "login has no commits", "login is unapplied"
- **AND** its push button is enabled
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)
  [@test] src/lib/stack.test.ts (`says why the branch isn't built on its parent`, `is passed only when the branch is built elsewhere`)

### Requirement: UI: The alert of a branch built elsewhere offers Unstack

Expanded, an alert MUST say "Built on master: login landed." (`info` for a landed parent, `warning` for one without
commits, likely a typo, neutral for an unapplied one) with an "Unstack from login" link: the undoable change of the
branch menu, its very item, the only restack, by the user.

#### Scenario: The alert of oauth on a landed login
- **WHEN** the user expands `oauth`, whose parent `login` landed
- **THEN** an `info` alert says "Built on master: login landed." with an "Unstack from login" link
- **AND** the link runs the undoable Unstack of the branch menu
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

#### Scenario: A parent without commits or an unapplied one
- **WHEN** the parent has no commits, or is unapplied
- **THEN** the alert is `warning` for one without commits, likely a typo, neutral for an unapplied one

### Requirement: UI: The Unstack of the alert runs once

While it runs, the link MUST be disabled with a spinner and say "Unstacking oauth…"; a second click runs nothing.

#### Scenario: Two clicks on Unstack
- **WHEN** the user clicks "Unstack from login" in the alert, then again
- **THEN** the link is disabled with a spinner and says "Unstacking oauth…" until the sync shows the branch
- **AND** the second click runs nothing
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack once from the alert, its spinner on the button until the sync shows the branch`)

### Requirement: UI: The alert names no pull request base

The alert MUST name no pull request base: every pull request goes into the baseline, and the push retargets an open
one into the branch of login ([Pull requests](#pull-requests)).

#### Scenario: A branch built elsewhere with an open pull request
- **GIVEN** `oauth`, built on master, has an open pull request into the branch of `login`
- **WHEN** its alert shows
- **THEN** it names no pull request base, and the push retargets the pull request to the baseline

### Requirement: UI: A branch with a stack problem shows its label and keeps its commits

A branch with a stack problem MUST show the label of the problem as its `destructive` status badge, never "internal
error", its message on hover, and keep its commits in the row and the preview, the commits to reword marked "Error".

#### Scenario: The badge of a problem
- **WHEN** a branch has a stack problem
- **THEN** its `destructive` status badge says "conflicting parents", "parent cycle", "its own parent", "case clash" or
  "empty prefix level", never "internal error", its message on hover
- **AND** its commits stay in the row and the preview, the commits to reword marked "Error"
  [@test] tests/e2e/stacked-branches/stack-problems.spec.ts (`should name conflicting parents, keep the commits and stack on a parent from the alert`)

### Requirement: UI: The alert of a stack problem has a link per fix

Expanded, and in the preview in place of "Sync failed", a `destructive` alert MUST say the message, no `brd` command,
with a link per fix (`StackProblemAlert`), each the very item of the branch menu, with its spinner while it runs; in
the preview a fix closes the preview first. A reword is the commit's "Reword…".

#### Scenario: Stack On from the alert
- **WHEN** the user expands a branch with conflicting parents
- **THEN** a `destructive` alert says the message, no `brd` command, with a link per fix: "Stack On ci-traces", "Unstack"
  ("Unstack from lane-b" of a branch with a parent), "Rename…"
- **AND** a link runs the very item of the branch menu, with its spinner while it runs
  [@test] tests/e2e/stacked-branches/stack-problems.spec.ts (`should name conflicting parents, keep the commits and stack on a parent from the alert`, `should cut a branch out of a parent cycle from its alert`)

#### Scenario: Rename from the alert
- **WHEN** the name of a branch differs from another only in case
- **THEN** the "Rename…" link of its alert opens the rename of the branch
  [@test] tests/e2e/stacked-branches/stack-problems.spec.ts (`should open the rename of a branch whose name differs from another only in case`)

#### Scenario: The preview
- **WHEN** the preview of a branch with a stack problem opens
- **THEN** the alert stands in place of "Sync failed" with the fixes, and a fix closes the preview first
  [@test] tests/e2e/stacked-branches/stack-problems.spec.ts (`should show the problem with its fixes in the preview, a fix closing the preview`)

### Requirement: UI: The branch menu offers the fixes of a stack problem enabled

The branch menu MUST offer the same fixes enabled: "Stack On…" lists the parents of the fixes (the stack parent too),
"Unstack" a branch the problem left without a parent.

#### Scenario: The menu of a branch with conflicting parents
- **WHEN** the user opens the menu of a branch with conflicting parents
- **THEN** "Stack On…" lists the parents of the fixes, the stack parent too, and Unstack is enabled
  [@test] tests/e2e/stacked-branches/stack-problems.spec.ts (`should offer the fixes in the menu and block the other rewrites with the problem`, `should unstack a branch with conflicting parents from the menu`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_rewrite_needs_commits_and_a_build_never_a_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest`)

### Requirement: UI: The other actions of a branch with a stack problem say its reason

Every other rewrite, Unapply, Review Changes and Safe Push MUST say the reason ("publish-viewer has conflicting
parents") instead of "The branch failed to sync".

#### Scenario: Another rewrite of a branch with a problem
- **WHEN** the user opens the menu of a branch with a stack problem
- **THEN** every other rewrite, Unapply, Review Changes and Safe Push are disabled with "publish-viewer has conflicting
  parents" instead of "The branch failed to sync"
  [@test] tests/e2e/stacked-branches/stack-problems.spec.ts (`should offer the fixes in the menu and block the other rewrites with the problem`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_rewrite_needs_commits_and_a_build_never_a_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest`)

### Requirement: UI: A drag takes the fixes of a stack problem too

A drag MUST take the fixes too (`stackOnDrop`, `unstackDrop`).

#### Scenario: A drag of a branch with a problem
- **WHEN** the user drags a branch with a stack problem
- **THEN** the rewrites the problem blocks are refused by the reason Rust gives, and its fixes (`stackOnDrop`,
  `unstackDrop`) run
  [@test] src/lib/branch-drop.test.ts (`blocks the rewrites of a branch with a stack problem by the reason Rust gives, and runs the fixes it offers`)

### Requirement: UI: A blocked branch says what blocks it

A blocked branch MUST show a muted `blocked by <parent>` status after a `warning` dot with the reason as its tooltip, a
disabled push button (in the row and the preview), and, expanded, a `warning` alert with a link that reveals the
parent.

#### Scenario: The children of a parent that doesn't build
- **GIVEN** a parent that doesn't build
- **WHEN** the table shows its children
- **THEN** each shows a muted `blocked by <parent>` status after a `warning` dot, the reason as its tooltip, and a
  disabled push button in the row and the preview
- **AND** expanded, a `warning` alert with a link that reveals the parent
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)

### Requirement: UI: A blocked branch has no share in a push of the stack

A blocked branch MUST have no share in a push of the stack (`domain_policy::push::push_plan` leaves a branch the last
sync did not build out, `Kept::NotBuilt`).

#### Scenario: Push Stack over a blocked branch
- **WHEN** a push of the stack covers a branch the last sync did not build
- **THEN** that branch is left out (`Kept::NotBuilt`)
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)

### Requirement: UI: Where the graph isn't visible a stacked branch says what it is built on

Where the stack graph isn't visible, a stacked branch MUST say what it is built on ("on auth", "on master" while its
parent has no commits): the branch preview; the landing overview shows the stack as one row, its root's. The branch row
says it only for a branch built elsewhere ("on master"); the graph shows the parent.

#### Scenario: The preview and the row
- **WHEN** the preview of a stacked branch shows
- **THEN** it says "on auth", or "on master" while its parent has no commits
- **AND** the branch row says it only for a branch built elsewhere ("on master"); the graph shows the parent
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)

#### Scenario: The landing overview
- **WHEN** the landing overview shows a stack
- **THEN** it shows the stack as one row, its root's

### Requirement: UI: A push of a stacked branch counts only its own commits as unpushed

A push of a stacked branch MUST count only its own commits as unpushed afterwards (`parentBranch`).

#### Scenario: After the push of a child
- **WHEN** a stacked branch is pushed
- **THEN** only its own commits count as unpushed afterwards, its parent's excluded (`parentBranch`)

## Testing / Local Run

- `pnpm test:backend`: `crates/history-rewrite/src/stack_test.rs`, `stack_sync_test.rs`, `commit_grouper_test.rs`,
  `autosquash_test.rs`.
- `pnpm test:unit`: `src/lib/stack-lanes.test.ts`, `src/stores/branch-table-rows.test.ts`, `src/stores/stack-view.test.ts`.
- `pnpm e2e`: `tests/e2e/stacked-branches`, `tests/e2e/branch-filter` (a collapsed stack under the filter).
- In any repository, commit with `(parent/child)` prefixes.

## Open Questions / Risks

- A parent integrated by a squash merge is `Missing` under the default rebase detection: its child is labelled "has no
  commits" instead of "landed"; the placement is the same.

## References

- [Branch Row](../branch-row/spec.md): the stack graph and the blocked row
- [Virtual Branches](../../../../docs/design/virtual-branches.md)
- [Git Operations](../../../../docs/design/git-operations.md)
- [Fixups](../../commit/fixups/spec.md)
