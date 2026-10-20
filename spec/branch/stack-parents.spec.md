---
name: Stack Parents
description: Requirements for stored stack parents, a branch to parent map in `refs/branch-deck/stack-parents` that stacks a branch without a history rewrite, so that an issue branch and an issue group can be stacked and a regroup writes no parent level into a prefix.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/stack_parents.rs
  - ../../crates/history-rewrite/src/store_lines.rs
  - ../../crates/history-rewrite/src/issue_groups.rs
  - ../../crates/history-rewrite/src/commit_grouper.rs
  - ../../crates/history-rewrite/src/stack.rs
  - ../../crates/history-rewrite/src/regroup.rs
  - ../../crates/history-rewrite/src/move_plan.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/check.rs
  - ../../crates/sync-core/src/dependencies.rs
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/git-ops/src/commit_writer.rs
  - ../../crates/git-ops/src/reword_commits.rs
  - ../../crates/branch-integration/src/detector.rs
  - ../../crates/sync-types/src/lib.rs
  - ../../crates/sync-types/src/stack.rs
  - ../../crates/bd-cli/src/rewrite.rs
  - ../../crates/bd-cli/src/status.rs
  - ../../crates/bd-cli/src/suggest.rs
  - ../../crates/bd-cli/src/safe_push_command.rs
  - ../../crates/bd-cli/src/clustering.rs
  - ../../crates/bd-cli/src/error.rs
  - ../../src/lib/commit.ts
  - ../../src/lib/stack.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/hooks/useStackIndependence.ts
  - ../../src/hooks/useMoveCommits.ts
  - ../../src/lib/patronus-stack.ts
  - ../../src/components/commit/PrefixChangeHint.tsx
---

# Stack Parents

Status: Implemented
Date: 2026-10-16

## Summary

A stack parent is stored, not written into the subjects. The ref `refs/branch-deck/stack-parents` holds one
`child<TAB>parent` line per stacked branch, by branch name, next to the issue groups in
`refs/branch-deck/issue-groups`. A branch whose commits declare no parent level takes its parent from this map. So
`bd group` with the plan path `auth/login` writes the line `login<TAB>auth`, and a commit that needs a prefix gets the
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
- Stay safe next to an older version of the app or `bd`: an older write of the issue groups must not lose a stored
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

## Requirements

### Store

- The map must live in `refs/branch-deck/stack-parents`: a commit whose tree holds the file `stack-parents`, one
  `child<TAB>parent` line per child, sorted by child. Each write must be a new commit on the previous one. A missing
  ref, or a commit without the file, is an empty map. It is a ref of its own because a store commit replaces its whole
  tree (`deleteall`): a file next to `issue-groups` would be lost by every write of an older version.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`reads_nothing_without_the_file_and_the_written_commit_after_the_transaction`)
- The `cat-file --batch` that reads the issue groups must read the map too: the stack setting, the issue groups and
  the stack parents cost two git processes.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`the_stack_setting_and_both_maps_cost_two_git_processes`)
- A line whose child or parent is not a branch name without `/`, whose child is its parent, or whose child is listed
  twice must be a problem: it is reported and never used. A write must keep it as it is, unless the write sets or
  removes its child.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`bad_lines_are_problems_and_never_used`)
- The ref is shared by every branch and worktree. So a write must change only the children that it names. A child that
  no branch on this HEAD reaches must stay, and `bd status` lists it in `idle_stack_parents`.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_write_on_one_worktree_keeps_the_parents_of_another`)
- Without `git config branchdeck.stacks`, the map must be read and not used: every branch stays a root, a `/` in a
  plan path is part of a flat name, and a restack or an `unstack` is `invalid_plan`. A rename does not touch the map.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`without_stacks_the_map_is_not_used_and_not_written`)

### Placement

- The parent of a branch must be the parent that its prefixes declare, else its line in the map, else none. This
  applies to a prefix branch, an issue branch and an issue group alike. An issue branch is a root only when the map
  names no parent for it. `parent_source` is `prefix` when a prefix declares the parent (the map may agree), else
  `store`.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_stored_parent_stacks_a_prefix_branch_an_issue_branch_and_an_issue_group`)
- A stored parent that differs from a parent that a prefix declares must be the problem "commits declare different
  parents", which names the commits of each parent and the line of the map, and the `bd restack <branch> <parent>` of
  each that keeps it; `bd status` says the same (`<branch> has conflicting parents …`). The branch is not rebuilt.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_stored_parent_that_differs_from_a_prefix_is_a_conflict`)
  [@test] ../../crates/bd-cli/src/status_test.rs (`stacks_in_status`)
- A cycle through the map, or through the map and the prefixes, must be the cycle problem of every branch of the cycle,
  also through an ancestor without commits.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_cycle_through_the_map_marks_each_of_its_branches`)
- A stored parent without commits must be placed as a declared one: `Unapplied`, `Integrated` or `Missing`, and the
  child is built on its nearest ancestor with commits, or the baseline. The chain goes on through the map: with
  `login<TAB>auth` and `auth<TAB>core`, `login` is built on `core` while `auth` has no commits. Only the lines that
  this chain reaches are placed; the others are idle.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_stored_parent_without_commits_is_passed`)
- The sync, `bd status`, `bd commits`, `bd check`, `bd suggest`, the regroup, the fixup plan, the landed commits, the
  update check and `groupFor` in the app must read the same map.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_sync_reads_the_map_and_never_writes_a_line_it_can_not_prove_dead`)
  [@test] ../../src/lib/commit.test.ts (`groupFor with stored parents`)

### Cleanup

- A sync must drop the line of a child once the newest archived branch of its name is integrated into the baseline:
  its commits are on the baseline, whatever worktree grouped them. A newer archive that is not integrated (the branch
  of another worktree that this sync archived) keeps the line, and so does a child without an archive. An archive
  `login-2` is one of `login` only on a day with an archive `login` (the archive takes the suffix only when the name is
  taken that day); else it is the branch `login-2`. The lines where such a branch is only the parent stay. This is the
  only write of a sync: one compare-and-swap write with its own message, only when a line goes, best effort (the next
  sync tries again).
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`the_newest_archive_of_a_child_decides_whether_it_is_integrated`, `a_sync_reads_the_map_and_never_writes_a_line_it_can_not_prove_dead`)
  [@test] ../../crates/bd-cli/src/status_test.rs (`a_sync_drops_the_stack_parent_of_an_integrated_branch`)

### Writing the map

- A plan path with a parent level (`auth/login`) must write the line `login<TAB>auth`. A deeper path (`a/b/c`) must
  write `b<TAB>a` and `c<TAB>b`. The commits must go where the path `login` puts them: a commit already in `login`
  without a prefix (its issue key or issue group) keeps its subject, the other ones get the bare leaf `(login)`, never
  a parent level; a commit whose prefix is that path already keeps it. Only a move creates issue groups
  ([Move Commits](move-commits.spec.md)).
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_plan_path_writes_the_parent_and_a_bare_leaf_prefix`)
- A plan that stacks only branches whose commits need no prefix (issue branches, issue groups, prefix branches whose
  prefixes declare no parent or the same parent) must rewrite no commit, honour `--expect-head`, and move only the stack
  parents ref, with a check that HEAD did not move.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`stacking_whole_issue_branches_rewrites_nothing`)
- A commit that the plan does not name and whose prefix declares another parent for a child that the plan stacks or
  unstacks must lose the levels above the child, in the same rewrite: `(docs/login)` becomes `(login)`, and the deeper
  path `(x/login/oauth)` of another branch becomes `(login/oauth)`. The levels above become stack parents of their own
  (`(core/x/login)` keeps `x<TAB>core`), so no other branch is unstacked. A prefix that declares the same parent stays.
  A commit that the plan names and gives the bare leaf keeps the levels above its leaf as stack parents the same way,
  unless the plan gives the commits of such a level the bare leaf too.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_differing_parent_level_becomes_a_bare_leaf`, `a_named_commit_that_gets_the_bare_leaf_keeps_the_levels_above_as_stack_parents`)
- `unstack: [branches]` in the plan must remove the line of each branch and the parent levels of its commits, by the
  rule above; a bare plan path (`login`) never removes a line. A branch that is neither in the map nor declared by a
  prefix is `invalid_plan`; an idle line is removed although no commit is in its branch here.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)
- A plan must move HEAD, the issue groups ref and the stack parents ref, each one that it changes, in one
  `update-ref --stdin`, with compare-and-swap on each, under the repository lock. The undo token must join the parts
  of the changed refs with `+`, in this order: `H1..H2`, `groups:G1..G2` and `stacks:S1..S2`. The `stacks` part follows
  the rules of the `groups` part: full ids, `S2` a write on `S1`, zeros for a missing ref, and `stacks_moved` (exit 3)
  when the ref moved. Only drops of a sync on top of `S2` don't count as a move: the undo writes the map of `S1` without
  the dropped children on top of them. A token of an older version stays valid.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`, `an_undo_passes_over_the_drops_of_a_sync`)
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`stack_parents_plan_and_undo`)
- These plans must be `invalid_plan`: a path whose leaf is its parent; two paths that give a child two parents; a
  child both stacked and unstacked; a plan whose result has a cycle or a conflict with a prefix.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`invalid_stack_parent_plans_are_refused`)

### Restack, unstack and rename

- `bd restack <branch> <parent>` must write the line of `<branch>`, and rewrite only the commits whose prefix declares
  another parent for it.
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`restack_writes_the_store_and_rewrites_only_differing_parent_levels`)
- `bd unstack <branch>` must remove the line of `<branch>` and the parent levels of its commits; a root is
  `invalid_plan`.
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`unstack_removes_the_line_and_the_parent_levels`)
- `bd rename <old> <new>` must rename `<old>` in the map as a child, and as the parent of a child with commits on this
  HEAD, in the same transaction as the other changes of the rename: a child of another worktree keeps its parent. As a
  child, `<old>` is renamed also when it is only a level of the prefixes here, without commits of its own.
  Merging into a branch with another stack parent is `invalid_plan`.
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`renaming_a_branch_renames_it_in_the_stored_parents`)
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`a_write_on_one_worktree_keeps_the_parents_of_another`, `renaming_a_level_without_commits_renames_its_stack_parent`)
- `bd restack` and `bd unstack` of an issue branch and of an issue group must be valid, and rewrite nothing.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)

### A stacked branch that builds alone

The map does not record why a branch is stacked. `bd suggest` writes a line because a commit needs a commit of the
parent, and Stack On writes the same line on purpose. A later move can take the needed commit into the child, and then
the line stays without a need. So the app and `bd` offer an unstack and never decide it.

- `stack_independence` must build the branch alone, as the plan `unstack: [branch]` makes it a root, and write commit
  objects only: no ref, note or lock. A clean build is `builds` with the declared parent. With `branchdeck.symbols on`
  it must name in `uses` each name a commit of the branch uses that a commit of a declared ancestor adds (the edges by
  symbols), once per name, and say `symbols: true`, also when none is used.
  [@test] ../../crates/sync-core/src/check_test.rs (`a_stacked_branch_that_applies_on_the_baseline_builds_alone`, `a_stacked_branch_that_uses_a_name_its_parent_adds_names_the_use`)
- A conflict must be `needs`, with the first conflicting commit of the branch and its files. `parent_commits` must be
  the commits of the declared ancestors that the commits of the branch need by the line dependencies, and the commits
  of the ancestors that those need, oldest first. A root is `invalid_plan`.
  [@test] ../../crates/sync-core/src/check_test.rs (`a_stacked_branch_that_conflicts_alone_names_the_commit_of_its_parent_it_needs`, `a_root_is_not_stacked`)
- `parent_commits` must hold whole fixup units, because a move takes them whole: a needed target brings its fixups,
  and a needed fixup brings its target and the other fixups of it.
  [@test] ../../crates/sync-core/src/check_test.rs (`a_needed_commit_brings_its_fixups_and_a_needed_fixup_its_target`)
- `movable` must be false when a commit left in a branch whose build holds a needed commit depends on one of
  `parent_commits`: an ancestor that owns a needed commit, or a branch built on such an ancestor, but not this branch
  or a branch built on it. After a move, that branch would need this one. A branch of another stack never holds a
  needed commit, so its dependencies don't count.
  [@test] ../../crates/sync-core/src/check_test.rs (`a_needed_commit_is_not_movable_when_a_commit_left_in_the_parent_depends_on_it`, `a_commit_of_another_stack_that_depends_on_a_needed_commit_keeps_it_movable`)
- The trial build is on the base the sync builds on, not on the newest target: the words say "the baseline".
- The line dependencies do not see a compile dependency; the edges by symbols see the uses of names, not what a
  compiler sees. A clean trial build proves only that the lines apply, and a Dry Run stays the proof.
- The Safe Push popover of a stacked branch lands it with its parents; to land it alone, it offers "Unstack from
  <parent>" when it builds alone, and "Move 1 Commit Here" when it needs commits that can move, as a second way below
  the landing, never the lead ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)). The toast of a move
  offers the unstack of a stacked branch of the move that builds alone now ([Move
  Commits](move-commits.spec.md#after-a-move)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer to land only a stacked branch that applies alone`, `should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)

### `bd status` and `bd suggest`

- `bd status` must print for each stacked branch its `parent` and `parent_source`: `prefix` or `store`. It must also
  print `idle_stack_parents` and `stack_parent_problems`.
  [@test] ../../crates/bd-cli/src/status_test.rs (`stored_parents_in_status`)
- `bd suggest` must read the stored parents as declared parents. A restacked cluster names its path also when no
  commit moves (`"a/b": []`), a cluster unstacked from its stack parent goes into `unstack`, and a commit with the bare
  leaf stays where it is. Its trial sync must apply the plan as `bd group` does, so that `rewritten` counts only the
  commits that need a prefix.
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`a_stacked_plan_stacks_an_issue_group_and_rewrites_a_deeper_path`, `with_stacked_branches_a_branch_stacks_on_the_branch_it_needs`)
- `bd suggest` must mark a cluster `stacked_without_need` when its stored parent stays and it depends on no cluster of
  that parent's lineage. The plan must keep the parent.
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`a_stored_parent_without_need_is_reported_and_kept`)
- `bd safe-push <branch> --check` of a stacked branch must print the trial build of the branch alone as `alone`:
  `builds` with "`bd unstack <branch>` makes it a root that lands alone", or the commits of its ancestors that it
  needs, with their branch. Its Safe Push lands it with its ancestors either way; the unstack is the user's choice.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_stacked_branch_lands_its_parents_with_it`, `a_stacked_branch_that_needs_a_commit_of_its_parent_to_land_alone_names_it`)

### App

- Stack On and Unstack from in the branch menu must write the map, and rewrite only as `bd restack` and `bd unstack`
  do. Their toast names a count only when commits moved: "Stacked ABC-2 on ABC-1".
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack an issue branch without a rewrite and undo it`)
- `groupFor` and `src/lib/stack.ts` must read the map from the `branchesGrouped` event. The commit editor must warn
  when an edited prefix declares a parent that differs from the stored one, and say nothing when it drops a parent
  level that the map repeats.
  [@test] ../../src/lib/stack.test.ts (`warns about a parent level that differs from the stored parent`)
- A move to a stacked target must take a whole issue by the issue groups, not by the prefix.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`a_whole_issue_joins_a_stacked_target_without_a_rewrite`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/stack_parents_test.rs`, `crates/sync-core/src/stack_operations_test.rs`,
  `crates/sync-core/src/check_test.rs`, `crates/bd-cli/src/safe_push_test.rs`,
  `crates/sync-core/src/move_commits_test.rs`, `crates/bd-cli/src/status_test.rs`, `crates/bd-cli/src/suggest_test.rs`,
  `crates/bd-cli/src/rewrite_test.rs`.
- `pnpm test:unit`: `src/lib/commit.test.ts`, `src/lib/stack.test.ts`.
- `pnpm e2e`: `tests/e2e/stacked-branches/stacked-branches.spec.ts` on the `stacked_issues` template.

## Risks

- `git log` no longer shows a stored parent. `bd status` and the app show it, with `parent_source`.
- Every worktree shares the map, by branch name. Stacking a branch in one worktree stacks the branch of that name in
  every worktree. The issue groups already behave so.
- A peer that edits a subject can add a parent level that differs from the map. That is a reported conflict, not a
  silent restack.
- An older version ignores `refs/branch-deck/stack-parents`. It shows a stored stack as flat branches and syncs the
  child on the baseline without the commits of its parent (it may conflict), and its `bd push` retargets the pull
  request of the child onto the baseline, until the newer version syncs and pushes again. It never loses a stored
  parent. Install the new app and `bd` together (`pnpm install-app`, then `bd install` for the skill).
- git keeps no reflog for the stack parents ref. The parent chain of its commits is its history.

## References

- [Stacked Branches](stacked-branches.spec.md)
- [Issue Groups](issue-groups.spec.md)
- [Move Commits](move-commits.spec.md)
- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Git Operations](../../docs/design/git-operations.md)
