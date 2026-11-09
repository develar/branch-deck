---
name: Stack Parents
description: Requirements for stored stack parents, a branch to parent map in `refs/branch-deck/stack-parents` that stacks a branch without a history rewrite, so that an issue branch and an issue group can be stacked and a regroup writes no parent level into a prefix.
style: plain-1
targets:
  - crates/history-rewrite/src/stack_parents.rs
  - crates/history-rewrite/src/store_lines.rs
  - crates/history-rewrite/src/issue_groups.rs
  - crates/history-rewrite/src/commit_grouper.rs
  - crates/history-rewrite/src/stack.rs
  - crates/history-rewrite/src/regroup.rs
  - crates/history-rewrite/src/move_plan.rs
  - crates/operations/src/rewrite.rs
  - crates/sync-core/src/sync.rs
  - crates/trial-check/src/check.rs
  - crates/trial-check/src/dependencies.rs
  - crates/sync-core/src/grouping.rs
  - crates/git-ops/src/commit_writer.rs
  - crates/git-ops/src/reword_commits.rs
  - crates/branch-integration/src/detector.rs
  - crates/domain-types/src/lib.rs
  - crates/domain-types/src/stack.rs
  - crates/bd-rewrite/src/rewrite.rs
  - crates/bd-remote/src/status.rs
  - crates/bd-rewrite/src/suggest.rs
  - crates/bd-remote/src/safe_push_command.rs
  - crates/commit-clustering/src/clustering.rs
  - crates/domain-types/src/error.rs
  - crates/domain-types/src/rewrite.rs
  - crates/bd-cli/src/lib.rs
  - src/lib/stack.ts
  - src/stores/branch-sync.ts
  - src/hooks/useStackActions.ts
  - src/components/branch/BranchSelectionBar.tsx
  - src/lib/branch-drop.ts
  - src/hooks/useStackIndependence.ts
  - src/hooks/useMoveCommits.ts
  - src/lib/patronus-stack.ts
  - src/components/commit/PrefixChangeHint.tsx
  - src/presenters/rewrite.ts
---

# Stack Parents

Status: Implemented
Date: 2026-10-16

## Purpose

A stack parent is stored, not written into the subjects. The ref `refs/branch-deck/stack-parents` holds one
`child<TAB>parent` line per stacked branch, by branch name, next to the issue groups in
`refs/branch-deck/issue-groups`. A branch whose commits declare no parent level takes its parent from this map. So
`brd group` with the plan path `auth/login` writes the line `login<TAB>auth`, and a commit that needs a prefix gets the
bare `(login)`. An issue branch and an issue group can now be stacked: `IJAI-4047<TAB>IJAI-3992` stacks the issue
branch `IJAI-4047` with no prefix at all. A prefix with a parent level (`(auth/login)`) still declares the parent, as
before.

The cause is the cost of a stack today. Stacking is the main reason why a regroup rewrites history: every commit of a
stacked branch gets `(parent/leaf)`, and every restack rewrites them again. In one monorepo checkout, stacking the
issue branch `IJAI-4047` onto `IJAI-3992` rewrote 37 subjects. That moved the shared `master`, changed the hashes that
a peer session had landed, and needed a handshake with that session. With the stored parent, the same regroup moves
only the stack parents ref.

## Goals

- Stack, restack and unstack a branch with no history rewrite when its commits need no prefix to be grouped.
- Let an issue branch and an issue group be stacked, so that their subjects keep the rule "issue id plus one sentence".
- Read and write the stack parents as the issue groups: in the same read, in the same ref transaction as HEAD, with
  compare-and-swap, and with one undo token for all of it.
- Stay safe next to an older version of the app or `brd`: an older write of the issue groups must not lose a stored
  parent.
- Keep every existing prefix valid: a parent level in a prefix declares the parent as before.

## Non-goals

- Group a commit without an issue key without a prefix. It still gets `(leaf)`. A store of commit assignments is a
  separate change.
- Push or fetch the store. Its ref is outside `refs/heads` and outside the default refspecs, as the issue groups: a
  new clone or another machine does not see the stored parents.
- Move the existing parent levels from the subjects into the store on its own. They stay valid, and a restack onto
  another parent turns them into bare leaves.
- A minimum version in the store: the versions without stored parents ignore the ref and could not read one.

## Concepts

### A stacked branch that builds alone

The map does not record why a branch is stacked. `brd suggest` writes a line because a commit needs a commit of the
parent, and Stack On writes the same line on purpose. A later move can take the needed commit into the child, and then
the line stays without a need.

## Requirements

### Requirement: Store: The map lives in its own ref, one sorted line per child

The map MUST live in `refs/branch-deck/stack-parents`: a commit whose tree holds the file `stack-parents`, one
`child<TAB>parent` line per child, sorted by child. Each write MUST be a new commit on the previous one. It is a ref of
its own because a store commit replaces its whole tree (`deleteall`): a file next to `issue-groups` would be lost by
every write of an older version.

#### Scenario: A write is a new commit of the ref
- **WHEN** a plan writes the line of a child
- **THEN** the ref is a new commit on the previous one whose file `stack-parents` holds one `child<TAB>parent` line
  per child, sorted by child
  [@test] crates/sync-core/src/stack_parents_test.rs (`reads_nothing_without_the_file_and_the_written_commit_after_the_transaction`)

### Requirement: Store: A missing ref or file is an empty map

A missing ref, or a commit without the file, SHALL be an empty map.

#### Scenario: No ref, or a commit of the ref without the file
- **WHEN** the ref is missing, or its commit holds no file `stack-parents`
- **THEN** the map is empty
  [@test] crates/sync-core/src/stack_parents_test.rs (`reads_nothing_without_the_file_and_the_written_commit_after_the_transaction`)

### Requirement: Store: One git process reads the issue groups and the map

The `cat-file --batch` that reads the issue groups MUST read the map too: the issue groups and the stack parents cost
one git process.

#### Scenario: Both maps are read
- **WHEN** the issue groups and the stack parents are read
- **THEN** they cost one git process
  [@test] crates/sync-core/src/stack_parents_test.rs (`both_maps_cost_one_git_process`)

### Requirement: Store: A bad line is a problem and never used

A line whose child or parent is not a branch name without `/`, whose child is its parent, or whose child is listed
twice MUST be a problem: it is reported and never used. A write MUST keep it as it is, unless the write sets or
removes its child.

#### Scenario: Bad lines in the map
- **GIVEN** a line whose child or parent is not a branch name without `/`, a line whose child is its parent, and a
  child listed twice
- **WHEN** the map is read
- **THEN** each is a problem, reported and never used
- **AND** a write keeps it as it is, unless the write sets or removes its child
  [@test] crates/sync-core/src/stack_parents_test.rs (`bad_lines_are_problems_and_never_used`)

### Requirement: Store: A write changes only the children it names

The ref is shared by every branch and worktree. So a write MUST change only the children that it names. A child that
no branch on this HEAD reaches MUST stay, and `brd status` lists it in `idle_stack_parents`.

#### Scenario: A write on one worktree keeps the line of another
- **GIVEN** a line of a child that only another worktree has
- **WHEN** this worktree writes the line of its own child
- **THEN** the line of the other worktree stays
- **AND** it is idle here, listed in `idle_stack_parents`
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_write_on_one_worktree_keeps_the_parents_of_another`)

### Requirement: Placement: The parent is the declared one, else the stored one

The parent of a branch MUST be the parent that its prefixes declare, else its line in the map, else none. This applies
to a prefix branch, an issue branch and an issue group alike. An issue branch is a root only when the map names no
parent for it. `parent_source` is `prefix` when a prefix declares the parent (the map may agree), else `store`.

#### Scenario: A stored parent stacks a prefix branch, an issue branch and an issue group
- **GIVEN** a line of the map for a prefix branch, an issue branch and an issue group, whose prefixes declare no parent
- **WHEN** the branches are placed
- **THEN** each takes the parent of its line, with `parent_source` `store`
- **AND** an issue branch without a line is a root
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_stored_parent_stacks_a_prefix_branch_an_issue_branch_and_an_issue_group`)

#### Scenario: The prefix and the map agree
- **GIVEN** a branch whose prefix declares the parent that its line of the map names too
- **WHEN** the branch is placed
- **THEN** `parent_source` is `prefix`
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_stored_parent_stacks_a_prefix_branch_an_issue_branch_and_an_issue_group`)

### Requirement: Placement: A stored parent that differs from a prefix is a problem

A stored parent that differs from a parent that a prefix declares MUST be the problem "commits declare different
parents", which names the commits of each parent and the line of the map ("docs (its stack parent)"), and the
`brd restack <branch> <parent>` of each that keeps it or the `brd unstack <branch>` that keeps none; `brd status` says
the same. The branch is not rebuilt.

#### Scenario: The prefix declares another parent than the map
- **GIVEN** a commit whose prefix declares the parent `auth`, and the line of the map that names `docs`
- **WHEN** the branches are placed
- **THEN** the branch has the problem "commits declare different parents", naming the commits of `auth` and "docs (its stack parent)"
- **AND** the fixes are the `brd restack <branch> <parent>` of each parent and the `brd unstack <branch>`, and the
  branch is not rebuilt
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_stored_parent_that_differs_from_a_prefix_is_a_conflict`)

#### Scenario: `brd status` says the same
- **WHEN** `brd status` runs on such a branch
- **THEN** it says the same problem with the same fixes
  [@test] crates/bd-remote/src/status_test.rs (`stacks_in_status`)

#### Scenario: The app offers the fixes as buttons
- **WHEN** the app shows the problem
- **THEN** it offers the same fixes as buttons ([Stacked Branches](../stacked-branches/spec.md#problems))

### Requirement: Placement: A cycle through the map marks each of its branches

A cycle through the map, or through the map and the prefixes, MUST be the cycle problem of every branch of the cycle,
also through an ancestor without commits.

#### Scenario: A cycle through the map and the prefixes
- **GIVEN** a cycle through the map, or through the map and the prefixes, maybe through an ancestor without commits
- **WHEN** the branches are placed
- **THEN** every branch of the cycle has the cycle problem
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_cycle_through_the_map_marks_each_of_its_branches`)

### Requirement: Placement: A stored parent without commits is placed as a declared one

A stored parent without commits MUST be placed as a declared one: `Unapplied`, `Integrated` or `Missing`, and the
child is built on its nearest ancestor with commits, or the baseline. The chain goes on through the map. Only the
lines that this chain reaches are placed; the others are idle.

#### Scenario: The chain goes on through the map
- **GIVEN** `login<TAB>auth` and `auth<TAB>core`, while `auth` has no commits
- **WHEN** the branches are placed
- **THEN** `login` keeps the declared parent `auth`, its state `Missing`
- **AND** `login` is built on `core`, its nearest ancestor with commits
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_stored_parent_without_commits_is_passed`)

#### Scenario: A line that the chain does not reach
- **GIVEN** a line of the map whose child no branch on this HEAD reaches
- **WHEN** the branches are placed
- **THEN** it is not placed, and it is idle

### Requirement: Placement: Every reader reads the same map

The sync, `brd status`, `brd commits`, `brd check`, `brd suggest`, the regroup, the fixup plan, the landed commits, the
update check and the grouping preview of the app (`preview_group`) MUST read the same map.

#### Scenario: A sync reads the map
- **GIVEN** a stored parent
- **WHEN** a sync runs
- **THEN** it reads the map and builds the stacked child
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_sync_reads_the_map_and_never_writes_a_line_it_can_not_prove_dead`)

#### Scenario: The grouping preview reads the map
- **WHEN** the grouping preview of the app groups a commit whose prefix declares no parent
- **THEN** it takes the parent of the prefix, else the stack parent
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_with_stacked_branches_reads_the_parent_of_the_prefix_else_the_stack_parent`)

### Requirement: Cleanup: A sync drops the line of an integrated child

A sync MUST drop the line of a child once the newest archived branch of its name is integrated into the baseline: its
commits are on the baseline, whatever worktree grouped them. The lines where such a branch is only the parent stay.

#### Scenario: The child is integrated
- **GIVEN** a stacked child whose commits upstream took, and whose newest archive is integrated
- **WHEN** a sync runs
- **THEN** it drops the line of that child
- **AND** the other lines stay, also a line where that branch is only the parent
  [@test] crates/bd-remote/src/status_test.rs (`a_sync_drops_the_stack_parent_of_an_integrated_branch`)
  [@test] crates/sync-core/src/stack_parents_test.rs (`the_newest_archive_of_a_child_decides_whether_it_is_integrated`)

### Requirement: Cleanup: Only the newest archive of the child's name decides

A newer archive that is not integrated (the branch of another worktree that this sync archived) MUST keep the line,
and so MUST a child without an archive. An archive `login-2` is one of `login` only on a day with an archive `login`
(the archive takes the suffix only when the name is taken that day); else it is the branch `login-2`.

#### Scenario: A newer archive that is not integrated
- **GIVEN** an integrated archive of a child, and a newer archive of its name that is not integrated
- **WHEN** a sync decides the lines to drop
- **THEN** the line stays
  [@test] crates/sync-core/src/stack_parents_test.rs (`the_newest_archive_of_a_child_decides_whether_it_is_integrated`)

#### Scenario: A child without an archive
- **GIVEN** a line of a child that has no archive
- **WHEN** a sync runs
- **THEN** the line stays
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_sync_reads_the_map_and_never_writes_a_line_it_can_not_prove_dead`)

#### Scenario: An archive with a suffix
- **GIVEN** an integrated archive `login-2`
- **WHEN** that day has no archive `login`
- **THEN** it is the branch `login-2`, and the line of `login` stays
- **AND** on a day with an archive `login`, it is an archive of `login`
  [@test] crates/sync-core/src/stack_parents_test.rs (`the_newest_archive_of_a_child_decides_whether_it_is_integrated`)

### Requirement: Cleanup: The drop is the only write of a sync

The drop MUST be the only write of a sync: one compare-and-swap write with its own message, only when a line goes,
best effort (the next sync tries again).

#### Scenario: No line goes
- **WHEN** a sync finds no line to drop
- **THEN** it writes nothing
  [@test] crates/sync-core/src/stack_parents_test.rs (`the_newest_archive_of_a_child_decides_whether_it_is_integrated`, `a_sync_reads_the_map_and_never_writes_a_line_it_can_not_prove_dead`)

#### Scenario: A line goes
- **WHEN** a sync drops a line
- **THEN** it is one compare-and-swap write with its own message, which keeps every other line
- **AND** when the write fails, the next sync tries again
  [@test] crates/sync-core/src/stack_parents_test.rs (`the_newest_archive_of_a_child_decides_whether_it_is_integrated`)

### Requirement: Writing the map: A plan path with a parent level writes its lines

A plan path with a parent level (`auth/login`) MUST write the line `login<TAB>auth`. A deeper path (`a/b/c`) MUST
write `b<TAB>a` and `c<TAB>b`.

#### Scenario: A path of one and of two parent levels
- **WHEN** a plan names the path `auth/login`, or the deeper path `a/b/c`
- **THEN** it writes the line `login<TAB>auth`, or `b<TAB>a` and `c<TAB>b`
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_plan_path_writes_the_parent_and_a_bare_leaf_prefix`)

### Requirement: Writing the map: The commits of a path get at most the bare leaf

The commits MUST go where the path `login` puts them: a commit already in `login` without a prefix (its issue key or
issue group) keeps its subject, the other ones get the bare leaf `(login)`, never a parent level; a commit whose prefix
is that path already keeps it. Only a move creates issue groups ([Move Commits](../move-commits/spec.md)).

#### Scenario: A commit that needs a prefix gets the bare leaf
- **WHEN** a plan path with a parent level names a commit that is not in its branch
- **THEN** the commit gets the bare leaf, never a parent level
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_plan_path_writes_the_parent_and_a_bare_leaf_prefix`)

#### Scenario: A commit already in the branch without a prefix keeps its subject
- **WHEN** a plan path with a parent level names the commits of an issue branch
- **THEN** they keep their subjects, and only the stack parents move
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`stack_parents_plan_and_undo`)

#### Scenario: A commit whose prefix is the path already
- **WHEN** a plan path names a commit whose prefix is that path already
- **THEN** the commit keeps its prefix

### Requirement: Writing the map: A stack of branches that need no prefix rewrites nothing

A plan that stacks only branches whose commits need no prefix (issue branches, issue groups, prefix branches whose
prefixes declare no parent or the same parent) MUST rewrite no commit, honour `--expect-head`, and move only the stack
parents ref, with a check that HEAD did not move.

#### Scenario: Whole issue branches are stacked
- **WHEN** a plan stacks an issue branch on another one with `--expect-head`
- **THEN** no commit is rewritten and HEAD does not move
- **AND** only the stack parents ref moves
  [@test] crates/sync-core/src/stack_parents_test.rs (`stacking_whole_issue_branches_rewrites_nothing`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`stack_parents_plan_and_undo`)

#### Scenario: HEAD moved since the expected one
- **WHEN** HEAD moved since the `--expect-head` of a plan that writes only the stack parents ref
- **THEN** the plan is refused
  [@test] crates/sync-core/src/stack_parents_test.rs (`stacking_whole_issue_branches_rewrites_nothing`)

### Requirement: Writing the map: A commit outside the plan loses a differing parent level

A commit that the plan does not name and whose prefix declares another parent for a child that the plan stacks or
unstacks MUST lose the levels above the child, in the same rewrite. The levels above become stack parents of their
own, so no other branch is unstacked. A prefix that declares the same parent stays.

#### Scenario: A differing parent level becomes a bare leaf
- **WHEN** a plan stacks `login` on `auth`
- **THEN** `(docs/login)` becomes `(login)`, and the deeper path `(x/login/oauth)` of another branch becomes `(login/oauth)`
- **AND** `(core/x/login)` keeps `x<TAB>core`
- **AND** a prefix that declares the same parent stays
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_differing_parent_level_becomes_a_bare_leaf`)

### Requirement: Writing the map: A named commit given the bare leaf keeps its upper levels

A commit that the plan names and gives the bare leaf MUST keep the levels above its leaf as stack parents the same
way, unless the plan gives the commits of such a level the bare leaf too.

#### Scenario: The levels above the leaf stay as stack parents
- **GIVEN** the commit `(core/x/login)`
- **WHEN** a plan names it in the path `auth/login`
- **THEN** it gets the bare leaf `(login)`, and `x` stays on `core`
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_named_commit_that_gets_the_bare_leaf_keeps_the_levels_above_as_stack_parents`)

### Requirement: Writing the map: `unstack` removes the line and the parent levels

`unstack: [branches]` in the plan MUST remove the line of each branch and the parent levels of its commits, by the rule
above; a bare plan path (`login`) never removes a line.

#### Scenario: A branch is unstacked
- **WHEN** a plan has `unstack: [branches]`
- **THEN** the line of each branch goes, and its commits lose their parent levels
  [@test] crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)

#### Scenario: A bare plan path
- **WHEN** a plan names the bare path `login` of a stacked branch
- **THEN** its line stays

### Requirement: Writing the map: `unstack` of a branch that is not stacked

A branch that is neither in the map nor declared by a prefix MUST be `invalid_plan` in `unstack`; an idle line is
removed although no commit is in its branch here.

#### Scenario: A branch that is not stacked
- **WHEN** a plan unstacks a branch that is neither in the map nor declared by a prefix
- **THEN** the plan is `invalid_plan`
  [@test] crates/sync-core/src/stack_parents_test.rs (`invalid_stack_parent_plans_are_refused`)

#### Scenario: An idle line
- **WHEN** a plan unstacks the child of an idle line
- **THEN** the line is removed, although no commit is in its branch here
  [@test] crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)

### Requirement: Writing the map: A plan moves its refs in one transaction

A plan MUST move HEAD, the issue groups ref and the stack parents ref, each one that it changes, in one
`update-ref --stdin`, with compare-and-swap on each, under the repository lock.

#### Scenario: A plan that changes HEAD and both stores
- **WHEN** a plan rewrites commits, writes issue groups and writes stack parents
- **THEN** HEAD and both refs move in one ref transaction
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

### Requirement: Writing the map: The undo token holds a `stacks` part

The undo token MUST join the parts of the changed refs with `+`, in this order: `H1..H2`, `groups:G1..G2` and
`stacks:S1..S2`. The `stacks` part follows the rules of the `groups` part: full ids, `S2` a write on `S1`, zeros for a
missing ref, and `stacks_moved` (exit 3) when the ref moved. A token of an older version stays valid.

#### Scenario: A mixed plan and its undo
- **WHEN** a plan changes HEAD, the issue groups and the stack parents
- **THEN** its undo token holds `+groups:` and `+stacks:`, and the undo puts all three back
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`stack_parents_plan_and_undo`)

#### Scenario: A token is read strictly
- **WHEN** a token is read
- **THEN** its parts are valid only in the order `H1..H2`, `groups:G1..G2`, `stacks:S1..S2`, with full ids or zeros
- **AND** the tokens of an older version stay valid
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)

### Requirement: Writing the map: The undo passes over the drops of a sync

Only drops of a sync on top of `S2` MUST NOT count as a move: the undo writes the map of `S1` without the dropped
children on top of them.

#### Scenario: A sync dropped a line after the plan
- **WHEN** a sync dropped the line of an integrated child on top of `S2`
- **THEN** the undo writes the map of `S1` without the dropped child
  [@test] crates/sync-core/src/stack_parents_test.rs (`an_undo_passes_over_the_drops_of_a_sync`)

#### Scenario: Another write after the plan
- **WHEN** a regroup wrote the map on top of `S2`
- **THEN** the undo is `stacks_moved`
  [@test] crates/sync-core/src/stack_parents_test.rs (`an_undo_passes_over_the_drops_of_a_sync`)

### Requirement: Writing the map: Invalid stack parent plans are refused

These plans MUST be `invalid_plan`: a path whose leaf is its parent; two paths that give a child two parents; a child
both stacked and unstacked; a plan whose result has a cycle or a conflict with a prefix.

#### Scenario: A plan that gives no valid map
- **WHEN** a plan has a path whose leaf is its parent, two paths that give a child two parents, a child both stacked
  and unstacked, or a result with a cycle or a conflict with a prefix
- **THEN** it is `invalid_plan`, and nothing is written
  [@test] crates/sync-core/src/stack_parents_test.rs (`invalid_stack_parent_plans_are_refused`)

### Requirement: Restack, unstack, Stack Together and rename: `brd restack` writes the line

`brd restack <branch> <parent>` MUST write the line of `<branch>`, and rewrite only the commits whose prefix declares
another parent for it.

#### Scenario: A restack onto another parent
- **WHEN** `brd restack <branch> <parent>` runs
- **THEN** the line of `<branch>` names `<parent>`
- **AND** only the commits whose prefix declares another parent for it are rewritten
  [@test] crates/sync-core/src/stack_operations_test.rs (`restack_writes_the_store_and_rewrites_only_differing_parent_levels`)

### Requirement: Restack, unstack, Stack Together and rename: `brd unstack` removes the line

`brd unstack <branch>` MUST remove the line of `<branch>` and the parent levels of its commits; a root is
`invalid_plan`.

#### Scenario: A stacked branch is unstacked
- **WHEN** `brd unstack <branch>` runs on a stacked branch
- **THEN** the line of `<branch>` and the parent levels of its commits go
  [@test] crates/sync-core/src/stack_operations_test.rs (`unstack_removes_the_line_and_the_parent_levels`)

#### Scenario: A root
- **WHEN** `brd unstack <branch>` runs on a root
- **THEN** it is `invalid_plan`
  [@test] crates/sync-core/src/stack_operations_test.rs (`unstack_removes_the_line_and_the_parent_levels`)

### Requirement: Restack, unstack, Stack Together and rename: Stack Together is one rewrite of `restack_core`

`brd stack-together <branch> <branch>...` and the app's selection-bar action MUST use the same `restack_core` as Stack
On and Unstack, through one `RewriteIntent::StackTogether` with one stack-parent update and one undo.

#### Scenario: Every named branch in one rewrite
- **WHEN** Stack Together runs on several branches
- **THEN** it is one rewrite of every named branch, with one stack-parent update and one undo
  [@test] crates/operations/src/rewrite_test.rs (`stack_together_is_one_rewrite_of_every_named_branch`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`stack_together_stacks_branches_oldest_first_and_undoes_once`)

#### Scenario: The cost of a restack
- **WHEN** Stack Together runs
- **THEN** it uses the same number of git processes as a restack
  [@test] crates/bd-cli/src/process_budget_test.rs (`stack_together_uses_the_same_number_of_git_processes_as_restack`)

### Requirement: Restack, unstack, Stack Together and rename: Stack Together chains the branches oldest first

The core MUST order branches by their oldest commit, keep an existing ancestor edge, move a selected branch from its
former parent without moving its children, refuse a cycle or an already-complete chain, and rewrite only commits whose
prefixes declare another parent.

#### Scenario: The oldest at the bottom, without a rewrite
- **WHEN** Stack Together runs on branches whose prefixes declare no other parent
- **THEN** they are chained by their oldest commit, the oldest at the bottom, and no commit is rewritten
  [@test] crates/history-rewrite/src/regroup_test.rs (`stack_together_chains_branches_oldest_at_the_bottom_without_a_rewrite`)

#### Scenario: A selected branch with a parent and children
- **WHEN** a selected branch is stacked on another parent and has children
- **THEN** it moves from its former parent, its children stay on it, and an existing ancestor edge is kept
  [@test] crates/sync-core/src/stack_operations_test.rs (`stack_together_moves_a_branch_and_keeps_its_existing_parent_and_children`)

#### Scenario: Invalid or already stacked branches, and cycles
- **WHEN** the selection would make a cycle, or is a complete chain already
- **THEN** it is refused
  [@test] crates/history-rewrite/src/regroup_test.rs (`stack_together_refuses_invalid_or_already_stacked_branches_and_cycles`)

### Requirement: Restack, unstack, Stack Together and rename: A rename renames the branch in the map

`brd rename <old> <new>` MUST rename `<old>` in the map as a child, and as the parent of a child with commits on this
HEAD, in the same transaction as the other changes of the rename: a child of another worktree keeps its parent. As a
child, `<old>` is renamed also when it is only a level of the prefixes here, without commits of its own.

#### Scenario: As a child and as a parent
- **WHEN** a stacked branch with a stacked child is renamed
- **THEN** it is renamed in its own line and in the line of its child, in the transaction of the rename
  [@test] crates/sync-core/src/stack_operations_test.rs (`renaming_a_branch_renames_it_in_the_stored_parents`)

#### Scenario: A child of another worktree
- **GIVEN** a child of another worktree whose parent is `<old>`
- **WHEN** `brd rename <old> <new>` runs here
- **THEN** that child keeps its parent
  [@test] crates/sync-core/src/stack_parents_test.rs (`a_write_on_one_worktree_keeps_the_parents_of_another`)

#### Scenario: A level without commits
- **GIVEN** a stacked branch that is only a level of the prefixes here, without commits of its own
- **WHEN** it is renamed
- **THEN** its line is renamed too
  [@test] crates/sync-core/src/stack_parents_test.rs (`renaming_a_level_without_commits_renames_its_stack_parent`)

### Requirement: Restack, unstack, Stack Together and rename: A merge into another stack parent is refused

Merging into a branch with another stack parent MUST be `invalid_plan`.

#### Scenario: The rename target has another stack parent
- **WHEN** `brd rename <old> <new>` merges `<old>` into a branch with another stack parent
- **THEN** it is `invalid_plan`

### Requirement: Restack, unstack, Stack Together and rename: An issue branch and group restack without a rewrite

`brd restack` and `brd unstack` of an issue branch and of an issue group MUST be valid, and rewrite nothing.

#### Scenario: An issue group and an issue branch
- **WHEN** `brd restack` and `brd unstack` run on an issue group, and `brd restack` on an issue branch
- **THEN** each is valid, and HEAD does not move
  [@test] crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)

### Requirement: Restack, unstack, Stack Together and rename: The app's rewrites run through the engine

The app's Stack On, Unstack and Rename SHALL be the rewrites `restack`, `unstack` and `rename` of the engine
(`operations::rewrite`, over the same cores), named on the table the window shows: a second click of the same one runs
one rewrite with one outcome, another rewrite waits behind it in the queue of the session, and its toast with Undo
comes once the sync it asked showed the branch.

#### Scenario: A second click of the same one
- **WHEN** the same unstack is asked again (a double click, three clicks from two windows)
- **THEN** it runs one rewrite with one outcome
  [@test] crates/operations/src/rewrite_test.rs (`a_triple_unstack_is_one_rewrite_with_one_outcome`)

#### Scenario: The toast after the sync
- **WHEN** the unstack is pressed in the alert of a branch
- **THEN** it runs once, its spinner on the button
- **AND** its toast with Undo comes once the sync it asked showed the branch
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack once from the alert, its spinner on the button until the sync shows the branch`)

#### Scenario: Another rewrite
- **WHEN** another rewrite is asked while one runs
- **THEN** it waits behind it in the queue of the session

### Requirement: Restack, unstack, Stack Together and rename: An agent's unstack beside the app's changes nothing more

`brd unstack` of an agent beside the app's MUST wait for the repository lock and change nothing more.

#### Scenario: The app and `brd` unstack the same branch
- **WHEN** the app and an agent's `brd unstack` unstack the same branch at once
- **THEN** `brd unstack` waits for the repository lock and changes nothing more: one rewrite, journaled once by each
  [@test] crates/bd-cli/src/cross_process_test.rs (`the_triple_unstack_of_the_app_and_bd_is_one_rewrite_journaled_once_by_each`)

### Requirement: Restack, unstack, Stack Together and rename: An unstack on a seeded table waits for the first sync

Unstack pressed on the table a start shows from the frame of the start before (`RepositoryView.cached`, until the
first sync of the session commits) MUST NOT be lost or refused: the rewrite is admitted at once, holding HEAD, and
waits in the session for that sync (the step `WaitingForSync`; the sync the session started as it opened serves both),
nothing written meanwhile; then it runs on the table that sync read, and asks its own sync after it.

#### Scenario: The first sync commits
- **GIVEN** the table of a start shown from `RepositoryView.cached`
- **WHEN** Unstack is pressed before the first sync of the session commits
- **THEN** the rewrite is admitted at once and waits in the step `WaitingForSync`, nothing written
- **AND** once that sync committed, it runs on the table that sync read and asks its own sync after it
  [@test] crates/operations/src/rewrite_test.rs (`an_unstack_on_a_seeded_table_waits_for_the_first_sync_and_runs_once_it_committed`)

### Requirement: Restack, unstack, Stack Together and rename: Stop Waiting for the first sync cancels the unstack

Stop Waiting MUST end the wait for the first sync with `canceled`, nothing written, the first sync going on.

#### Scenario: Stop Waiting
- **GIVEN** an unstack that waits for the first sync of the session
- **WHEN** the user chooses Stop Waiting
- **THEN** the rewrite ends with `canceled`, nothing written, and the first sync goes on
  [@test] crates/operations/src/rewrite_test.rs (`a_stop_waiting_for_the_first_sync_is_canceled_and_the_sync_goes_on`)

### Requirement: A stacked branch that builds alone: The unstack is offered, never decided

As the map does not record why a branch is stacked, the app and `brd` MUST offer an unstack and MUST NOT decide it.

#### Scenario: A line that stays without a need
- **GIVEN** a stacked branch whose line stays after a move took the needed commit into it
- **WHEN** the app or `brd` finds that it builds alone
- **THEN** they offer its unstack, and never unstack it themselves

### Requirement: A stacked branch that builds alone: `stack_independence` builds the branch alone

`stack_independence` MUST build the branch alone, as the plan `unstack: [branch]` makes it a root, and write commit
objects only: no ref, note or lock. A clean build is `builds` with the declared parent.

#### Scenario: A stacked branch that applies on the baseline
- **WHEN** `stack_independence` builds a stacked branch whose commits apply without its parent
- **THEN** it is `builds` with the declared parent
- **AND** only commit objects are written: no ref, note or lock
  [@test] crates/trial-check/src/check_test.rs (`a_stacked_branch_that_applies_on_the_baseline_builds_alone`)

### Requirement: A stacked branch that builds alone: The edges by symbols name the uses

With `branchdeck.symbols on` it MUST name in `uses` each name a commit of the branch uses that a commit of a declared
ancestor adds (the edges by symbols), once per name, and say `symbols: true`, also when none is used.

#### Scenario: A branch that uses a name its parent adds
- **GIVEN** `branchdeck.symbols on`
- **WHEN** a commit of the branch uses a name that a commit of its parent adds
- **THEN** `uses` names the use once, with `symbols: true`
  [@test] crates/trial-check/src/check_test.rs (`a_stacked_branch_that_uses_a_name_its_parent_adds_names_the_use`)

#### Scenario: No name is used
- **GIVEN** `branchdeck.symbols on`
- **WHEN** the branch uses no name of its ancestors
- **THEN** it still says `symbols: true`
  [@test] crates/trial-check/src/check_test.rs (`a_stacked_branch_that_applies_on_the_baseline_builds_alone`)

### Requirement: A stacked branch that builds alone: A conflict names the commits it needs

A conflict MUST be `needs`, with the first conflicting commit of the branch and its files. `parent_commits` MUST be
the commits of the declared ancestors that the commits of the branch need by the line dependencies, and the commits of
the ancestors that those need, oldest first.

#### Scenario: A stacked branch that conflicts alone
- **WHEN** a stacked branch conflicts when built alone
- **THEN** it is `needs`, with the first conflicting commit of the branch and its files
- **AND** `parent_commits` names the commits of its ancestors that it needs, oldest first
  [@test] crates/trial-check/src/check_test.rs (`a_stacked_branch_that_conflicts_alone_names_the_commit_of_its_parent_it_needs`)

### Requirement: A stacked branch that builds alone: A root is not stacked

`stack_independence` of a root MUST be `invalid_plan`.

#### Scenario: The check of a root
- **WHEN** `stack_independence` runs on a root
- **THEN** it is `invalid_plan`
  [@test] crates/trial-check/src/check_test.rs (`a_root_is_not_stacked`)

### Requirement: A stacked branch that builds alone: `parent_commits` holds whole fixup units

`parent_commits` MUST hold whole fixup units, because a move takes them whole: a needed target brings its fixups, and
a needed fixup brings its target and the other fixups of it.

#### Scenario: A needed target and a needed fixup
- **WHEN** the branch needs a target, or a fixup, of an ancestor
- **THEN** the target brings its fixups, and the fixup brings its target and the other fixups of it
  [@test] crates/trial-check/src/check_test.rs (`a_needed_commit_brings_its_fixups_and_a_needed_fixup_its_target`)

### Requirement: A stacked branch that builds alone: `movable` is false when a commit left behind needs it

`movable` MUST be false when a commit left in a branch whose build holds a needed commit depends on one of
`parent_commits`: an ancestor that owns a needed commit, or a branch built on such an ancestor, but not this branch or
a branch built on it. After a move, that branch would need this one. A branch of another stack never holds a needed
commit, so its dependencies don't count.

#### Scenario: A commit left in the parent depends on a needed commit
- **WHEN** a commit left in the ancestor that owns a needed commit depends on it
- **THEN** `movable` is false
  [@test] crates/trial-check/src/check_test.rs (`a_needed_commit_is_not_movable_when_a_commit_left_in_the_parent_depends_on_it`)

#### Scenario: A commit of another stack depends on a needed commit
- **WHEN** only a commit of a branch of another stack depends on a needed commit
- **THEN** `movable` stays true
  [@test] crates/trial-check/src/check_test.rs (`a_commit_of_another_stack_that_depends_on_a_needed_commit_keeps_it_movable`)

### Requirement: A stacked branch that builds alone: The trial build is on the base of the sync

The trial build SHALL be on the base the sync builds on, not on the newest target: the words say "the baseline".

#### Scenario: The words of the trial build
- **WHEN** the result of the trial build is shown
- **THEN** it was built on the base the sync builds on, and the words say "the baseline"

### Requirement: A stacked branch that builds alone: A clean trial build proves only that the lines apply

The line dependencies do not see a compile dependency; the edges by symbols see the uses of names, not what a
compiler sees. A clean trial build SHALL prove only that the lines apply, and a Dry Run stays the proof.

#### Scenario: A clean trial build
- **WHEN** a stacked branch builds alone
- **THEN** that proves only that its lines apply, and a Dry Run stays the proof

### Requirement: A stacked branch that builds alone: The Safe Push popover offers to land it alone

The Safe Push popover of a stacked branch SHALL land it with its parents; to land it alone, it MUST offer "Unstack from
<parent>" when it builds alone, and "Move 1 Commit Here" when it needs commits that can move, as a second way below
the landing, never the lead ([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: A stacked branch that applies alone
- **WHEN** the Safe Push popover of a stacked branch that builds alone opens
- **THEN** it lands the branch with its parents, and offers "Unstack from <parent>" below the landing
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer to land only a stacked branch that applies alone`)

#### Scenario: A stacked branch that needs a commit that can move
- **WHEN** the Safe Push popover of a stacked branch that needs commits that can move opens
- **THEN** it offers "Move 1 Commit Here" below the landing, never as the lead
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)

### Requirement: A stacked branch that builds alone: The toast of a move offers the unstack

The toast of a move MUST offer the unstack of a stacked branch of the move that builds alone now
([Move Commits](../move-commits/spec.md#after-a-move)).

#### Scenario: After the move of the needed commit
- **WHEN** a move takes the commit that a stacked branch needs into it
- **THEN** the toast of the move offers the unstack of that branch, which builds alone now
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)

### Requirement: `brd status` and `brd suggest`: `brd status` prints the parent and its source

`brd status` MUST print for each stacked branch its `parent` and `parent_source`: `prefix` or `store`. It MUST also
print `idle_stack_parents` and `stack_parent_problems`.

#### Scenario: The status of stored parents
- **GIVEN** stacked branches, a stored parent among them, an idle line and a bad line
- **WHEN** `brd status` runs
- **THEN** each stacked branch has its `parent` and its `parent_source`, `prefix` or `store`
- **AND** the output has `idle_stack_parents` and `stack_parent_problems`
  [@test] crates/bd-remote/src/status_test.rs (`stored_parents_in_status`)

### Requirement: `brd status` and `brd suggest`: `brd suggest` reads the stored parents as declared

`brd suggest` MUST read the stored parents as declared parents. A restacked cluster names its path also when no
commit moves (`"a/b": []`), a cluster unstacked from its stack parent goes into `unstack`, and a commit with the bare
leaf stays where it is.

#### Scenario: A restacked cluster whose commits do not move
- **WHEN** `brd suggest` stacks a cluster on the branch it needs, and no commit moves
- **THEN** the plan names its path with no commits (`"a/b": []`), and the commit with the bare leaf stays where it is
  [@test] crates/bd-rewrite/src/suggest_test.rs (`with_stacked_branches_a_branch_stacks_on_the_branch_it_needs`)

#### Scenario: A cluster unstacked from its stack parent
- **WHEN** `brd suggest` unstacks a cluster from its stack parent
- **THEN** the cluster goes into `unstack`

### Requirement: `brd status` and `brd suggest`: The trial sync of `brd suggest` applies the plan as `brd group`

The trial sync of `brd suggest` MUST apply the plan as `brd group` does, so that `rewritten` counts only the commits
that need a prefix.

#### Scenario: An issue group stacked on what it needs
- **WHEN** `brd suggest` stacks an issue group on the branch it needs
- **THEN** `rewritten` is 0, and a deeper path is rewritten only where its commits need a prefix
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_stacked_plan_stacks_an_issue_group_and_rewrites_a_deeper_path`)

### Requirement: `brd status` and `brd suggest`: A stored parent without need is reported and kept

`brd suggest` MUST mark a cluster `stacked_without_need` when its stored parent stays and it depends on no cluster of
that parent's lineage. The plan MUST keep the parent.

#### Scenario: A stacked cluster that needs nothing of its parent
- **GIVEN** a stored parent of a cluster that depends on no cluster of that parent's lineage
- **WHEN** `brd suggest` runs
- **THEN** the cluster is `stacked_without_need`, and the plan keeps the parent
- **AND** once it depends on its parent, the mark goes
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_stored_parent_without_need_is_reported_and_kept`)

### Requirement: `brd status` and `brd suggest`: `brd safe-push --dry-run` prints the trial build alone

`brd safe-push <branch> --dry-run` of a stacked branch MUST print the trial build of the branch alone as `alone`:
`builds` with "`brd unstack <branch>` makes it a root that lands alone", or the commits of its ancestors that it needs,
with their branch. Its Safe Push lands it with its ancestors either way; the unstack is the user's choice.

#### Scenario: A stacked branch that builds alone
- **WHEN** `brd safe-push <branch> --dry-run` runs on a stacked branch that builds alone
- **THEN** `alone` is `builds` with "`brd unstack <branch>` makes it a root that lands alone"
- **AND** the Safe Push lands it with its parents
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_lands_its_parents_with_it`)

#### Scenario: A stacked branch that needs a commit of its parent
- **WHEN** `brd safe-push <branch> --dry-run` runs on a stacked branch that needs commits of its ancestors to land alone
- **THEN** `alone` names those commits with their branch
- **AND** the Safe Push lands it with its ancestors
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_that_needs_a_commit_of_its_parent_to_land_alone_names_it`)

### Requirement: App: Stack On and Unstack from write the map

Stack On and Unstack from in the branch menu MUST write the map, and rewrite only as `brd restack` and `brd unstack`
do. Their toast names a count only when commits moved: "Stacked ABC-2 on ABC-1".

#### Scenario: An issue branch is stacked without a rewrite
- **WHEN** the user stacks an issue branch with Stack On in the branch menu
- **THEN** the map is written and no commit is rewritten
- **AND** the toast says "Stacked ABC-2 on ABC-1", without a count, and its Undo unstacks it
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack an issue branch without a rewrite and undo it`)

### Requirement: App: Stack Together from the selection bar writes one chain

Stack Together from the selection bar MUST write one oldest-first chain, show one success toast and undo, and clear the
selection after the sync shows the new parent map.

#### Scenario: The selected branches are stacked together
- **WHEN** the user chooses Stack Together in the selection bar
- **THEN** the selected branches are one chain, the oldest at the bottom, with one success toast and its undo
- **AND** the selection clears after the sync shows the new parent map
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack the selected branches together from the selection bar, the oldest at the bottom, and undo it`)

### Requirement: App: A dragged branch stacks on a chip and unstacks on the zone

A dragged branch MUST stack the same way when dropped on the "Stack on ‹name›" chip of a row, and unstack when dropped
on the unstack zone of the drag bar ([Move Commits](../move-commits/spec.md#drag-and-drop)); the chip refuses the
branch's parent and a branch stacked on it, as the menu does not offer them.

#### Scenario: Drop on the chip, then on the unstack zone
- **WHEN** a dragged branch is dropped on the "Stack on ‹name›" chip of a row
- **THEN** it is stacked on that row's branch
- **AND** dropped on the unstack zone of the drag bar, it is unstacked
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a dragged branch dropped on the Stack on chip of a row, refuse a cycle there, and unstack it on the unstack zone`)

#### Scenario: The chip refuses a cycle
- **WHEN** the branch is dragged over the chip of its parent, or of a branch stacked on it
- **THEN** the chip refuses it
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a dragged branch dropped on the Stack on chip of a row, refuse a cycle there, and unstack it on the unstack zone`)

### Requirement: App: The preview and the table header read the same map

The grouping preview (`preview_group`, the map of the session's last grouping) and `src/lib/stack.ts` (the map of the
table header) MUST read the same map.

#### Scenario: The grouping preview and the table header
- **WHEN** the grouping preview and the table header place a stacked branch
- **THEN** both take its parent from the same map

### Requirement: App: The commit editor warns about a differing parent level

The commit editor MUST warn when an edited prefix declares a parent that differs from the stored one, and say nothing
when it drops a parent level that the map repeats.

#### Scenario: An edited prefix and the stored parent
- **WHEN** an edited prefix declares a parent that differs from the stored one
- **THEN** the commit editor warns
- **AND** when the edit drops a parent level that the map repeats, it says nothing
  [@test] src/lib/stack.test.ts (`warns about a parent level that differs from the stored parent`)

### Requirement: App: A move to a stacked target takes a whole issue by the issue groups

A move to a stacked target MUST take a whole issue by the issue groups, not by the prefix.

#### Scenario: A whole issue joins a stacked target
- **WHEN** the commits of a whole issue move to a stacked target
- **THEN** they join it by the issue groups, not by the prefix, and their subjects stay
  [@test] crates/sync-core/src/move_commits_test.rs (`a_whole_issue_joins_a_stacked_target_without_a_rewrite`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/stack_parents_test.rs`, `crates/sync-core/src/stack_operations_test.rs`,
  `crates/trial-check/src/check_test.rs`, `crates/bd-remote/src/safe_push_test.rs`,
  `crates/sync-core/src/move_commits_test.rs`, `crates/bd-remote/src/status_test.rs`, `crates/bd-rewrite/src/suggest_test.rs`,
  `crates/bd-rewrite/src/rewrite_test.rs`.
- `pnpm test:unit`: `src/lib/stack.test.ts`; the grouping preview: `crates/history-rewrite/src/commit_grouper_test.rs`.
- `pnpm e2e`: `tests/e2e/stacked-branches/stacked-branches.spec.ts` on the `stacked_issues` template.

## Risks

- `git log` no longer shows a stored parent. `brd status` and the app show it, with `parent_source`.
- Every worktree shares the map, by branch name. Stacking a branch in one worktree stacks the branch of that name in
  every worktree. The issue groups already behave so.
- A peer that edits a subject can add a parent level that differs from the map. That is a reported conflict, not a
  silent restack.
- An older version ignores `refs/branch-deck/stack-parents`. It shows a stored stack as flat branches and syncs the
  child on the baseline without the commits of its parent (it may conflict), and its `brd push` retargets the pull
  request of the child onto the baseline, until the newer version syncs and pushes again. It never loses a stored
  parent. Install the new app and `brd` together (`pnpm install-app`, then `brd install` for the skill).
- git keeps no reflog for the stack parents ref. The parent chain of its commits is its history.

## References

- [Stacked Branches](../stacked-branches/spec.md)
- [Issue Groups](../issue-groups/spec.md)
- [Move Commits](../move-commits/spec.md)
- [Virtual Branches](../../../../docs/design/virtual-branches.md)
- [Git Operations](../../../../docs/design/git-operations.md)
