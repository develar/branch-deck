---
name: Stacked Branches
description: Requirements for stacked branches, where a `(parent/child)` prefix builds the child's virtual branch on its parent's, a parent without commits passed to the branch below it, problems and blocked parents reported per branch, the root row heading its stack, which collapses into it, and the setting opt-in per repository.
style: plain-1
targets:
  - ../../crates/sync-core/src/stack.rs
  - ../../crates/sync-core/src/commit_grouper.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/branch_processor.rs
  - ../../crates/sync-core/src/autosquash.rs
  - ../../crates/sync-core/src/remote_status.rs
  - ../../crates/branch-integration/src/detector.rs
  - ../../crates/branch-integration/src/cache.rs
  - ../../crates/branch-integration/src/rebase.rs
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/sync-core/src/check.rs
  - ../../crates/sync-types/src/stack.rs
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/stack-lanes.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/branch/StackLanes.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/components/branch/InlineUngroupConfirm.tsx
  - ../../src/components/branch/StackChildrenNote.tsx
  - ../../src/components/branch/InlineRenameBranchInput.tsx
  - ../../crates/sync-core/src/stack_operations.rs
  - ../../src/lib/commit.ts
  - ../../src/lib/stack.ts
  - ../../src/components/commit/PrefixChangeHint.tsx
  - ../../src/components/shared/InlineBranchTarget.tsx
  - ../../src/hooks/useBranchNameValidation.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/lib/push-summary.ts
  - ../../src/lib/git/push.ts
  - ../../src/components/branch/StackBaseLabel.tsx
  - ../../src/components/branch/StackChip.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/lib/push-up-to.ts
  - ../../src/components/commit/PushUpToMark.tsx
  - ../../src/hooks/useCommitMoveList.ts
  - ../../crates/sync-core/src/push.rs
  - ../../src/lib/stack-state.ts
  - ../../src/lib/stack-collapse.ts
  - ../../src/stores/stack-collapse.ts
  - ../../src/stores/settings/repositorySettings.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/components/shared/MenuItems.tsx
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/lib/git/push-toast.tsx
  - ../../src/lib/git/new-pull-request.ts
  - ../../crates/sync-core/src/pull_request_writes.rs
---

# Stacked Branches

Status: Draft
Date: 2026-10-15

## Summary

A commit `(auth/login) Add form` belongs to the branch `login`, and `login` stacks on `auth`. Its virtual branch is
built on the new head of `auth`'s virtual branch, so a pull request for `login` contains only its own commits on top
of `auth`. Deeper prefixes declare the ancestry (`(auth/login/oauth)`), and a bare `(login)` joins `login` wherever
its parent is declared. A branch whose prefixes declare no parent, an issue branch and an issue group too, takes its
stack parent from `refs/branch-deck/stack-parents` ([Stack Parents](stack-parents.spec.md)): Stack On, Unstack and
`bd group` with a path write it there, without a history rewrite. A parent without commits (landed, unapplied, mistyped) is passed: the child is built on the
nearest ancestor with commits, or the baseline. A branch that can't be placed, or whose parent can't be built, is
reported and not rebuilt; the other branches still sync. The branch table draws the stacks as a graph, and the branch
menu stacks, unstacks and renames branches.

## Goals

- Review a feature in dependent steps, each a branch with its own commits.
- Keep linear development on the main branch: the prefixes and the stack parents are the stack state.
- The history decides where a branch is built: a parent without commits (landed, unapplied, mistyped) doesn't stop its
  children; the base change is visible in the row, the alert and the push toast; only the user restacks a branch
  (Unstack).

## Non-goals

- Pull requests of a whole stack at once: "Create Pull Request…" creates the pull request of one branch into its base
  ([Pull Requests](pull-requests.spec.md#creating-a-pull-request)). Branch Deck retargets an open pull request and
  closes the ones a Safe Push landed ([Pull requests](#pull-requests)).

## Requirements

### Opt-in

- Stacks must be on only when `git config branchdeck.stacks` is true. Without the setting, a `/` in a prefix must stay
  part of a flat branch name and every event must be as before, with a null `stack`.
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`without_stacks_a_slash_is_part_of_a_flat_name`, `stacks_are_off_unless_configured`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`without_stacks_branches_grouped_has_no_stack`)

### Prefix syntax

- The last level of a prefix must name the branch and the level before it its parent. Each level must be sanitized
  like a flat branch name. Empty levels at the ends must be dropped; an empty level in the middle (`(a//b)`) is a
  syntax problem of the last level's branch.
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`stack_prefixes_split_into_sanitized_levels`, `an_empty_stack_level_is_a_syntax_error`, `a_syntax_error_is_reported_on_the_last_level`)
- Commits must be grouped by the leaf, and every level pair must declare an edge: `(auth/login/oauth)` also declares
  `auth` as the parent of `login`. An issue-key branch is a root unless the stack parents name its parent
  ([Stack Parents](stack-parents.spec.md#placement)).
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`groups_by_leaf_and_records_the_declared_parents`, `a_parent_without_commits_is_missing`)

### Problems

- These problems must mark only the affected branch, with the `Error` status, naming the commits; the branch is not
  rebuilt:
  - commits declare different parents;
  - the parents form a cycle (every branch of the cycle);
  - `(a/a)`;
  - two branches differ only in case (both);
  - a syntax problem.
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`conflicting_parents_name_their_commits`, `a_branch_can_not_be_its_own_parent`, `a_cycle_marks_each_of_its_branches`, `leaves_that_differ_only_in_case_collide`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_problem_is_not_built_and_a_missing_parent_is_passed`)

### Placement

- A child must be built on the new head of its active parent's virtual branch, also when a child commit comes before
  every parent commit on the main branch (`childPrecedesParent`).
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)
  [@test] ../../crates/sync-core/src/stack_test.rs (`a_child_is_built_on_its_active_parent`)
- A parent without commits must be `Unapplied` with an unapplied branch of its name, `Integrated` when its branch is
  integrated, else `Missing`. Its branch is its virtual branch, which the sync is about to archive, else its newest
  archived branch; a detection note says whether it is integrated. The state is a label (the row, `bd status`); the
  placement doesn't depend on it.
  [@test] ../../crates/sync-core/src/stack_test.rs (`reads_the_state_of_parents_without_commits_from_their_branches`)
- The branch of a parent without commits (its virtual branch, else its newest archived one) must be detected before the
  branches are placed by the rule of the archive step: when it has no note, a `NotIntegrated` note the baseline moved on
  from (incrementally), or a note without a usable baseline (in full; an archived branch only within the retention
  period), so the first sync after the parent's merge already labels the parent landed, also when the parent was
  archived a sync before. The sync writes the note, and its archiving reports the parent from it; `bd status` and
  `bd check` detect without writing.
  [@test] ../../crates/sync-core/src/stack_test.rs (`a_parent_whose_virtual_branch_lost_its_commits_is_detected_before_the_placement`)
  [@test] ../../crates/integration-detection-tests/src/stack_tests.rs (`the_first_sync_after_the_parent_is_integrated_builds_the_child_on_the_baseline`)
  [@test] ../../crates/integration-detection-tests/src/recheck_tests.rs (`a_stale_note_on_the_virtual_tip_does_not_hide_an_integrated_parent`, `a_stale_note_on_the_archived_parent_does_not_hide_an_integrated_parent`)
  [@test] ../../crates/bd-cli/src/status_test.rs (`a_parent_integrated_since_the_last_sync_is_integrated_before_the_next_one`)
- A parent without commits, whatever its state, must move the child to its nearest active ancestor, or the baseline,
  with `prBaseChanged`: the local history is linear on the baseline, so the child's commits can be picked there like
  those of any flat branch (or conflict like them).
  [@test] ../../crates/sync-core/src/stack_test.rs (`an_integrated_parent_moves_the_child_to_the_nearest_active_ancestor`, `a_parent_without_commits_moves_the_child_to_the_nearest_ancestor_with_commits`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`an_unapplied_parent_is_reported_before_the_sync_starts`, `a_problem_is_not_built_and_a_missing_parent_is_passed`)
- A parent with a problem and a blocked parent must block the child: the `Blocked` status, the reason as its text, its
  commits blocked, its virtual branch left as it was.
  [@test] ../../crates/sync-core/src/stack_test.rs (`a_parent_with_a_problem_blocks_its_children`)
- A parent that conflicts or fails while it is built must block its descendants the same way.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_parent_conflict_blocks_its_descendants`)

### Sync

- A child must start once its parent's head is written, without holding a slot of the git process limit while it
  waits. Ready branches must start longest chain first.
- A new parent commit must move the children onto the parent's new head; an unchanged stack must reuse every commit.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)

### Remote status

- The unpushed commits of a child must exclude its parent's commits (`^<parent>` next to the baseline exclusion),
  after a sync and after a push of the child. In the sync the parent is the new head the child was built on, not the
  parent's ref: a child can finish before its parent, whose ref is then absent (a new stack) or at its old head. After
  a push the parent's ref is current. A remote status that can't be computed is logged as a warning, and the branch
  keeps the status it had.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`the_unpushed_commits_of_a_child_exclude_its_parent`)
  [@test] ../../crates/sync-core/src/branch_processor_test.rs (`a_child_finished_before_its_parent_excludes_the_parent_by_the_head_it_was_built_on`)
- A child whose parent (or any branch below it) was amended or rebuilt since the child was pushed has the parent's
  commits as they were on its remote branch. Those must be restacked, not dropped: their identity (author email,
  author time, subject without the prefix) is that of a commit of a branch it is built on. The status reads
  `restacked` (`4↑ · restacked`), "Rebuilt on the new auth: the remote has older versions of 2 of its commits.", the
  push is a `primary` Force Push that asks nothing, also with nothing of the child's own to push, and Push Stack takes
  such a branch along ([Outgoing Commits](../repository/outgoing.spec.md#what-is-outgoing)). The sync builds the
  identities of every branch's ancestors once, from the commits it grouped, along the chain of effective bases; a push
  outside the sync reads them from the parent's virtual branch with one `rev-list`, only while a remote commit
  matches nothing of the child. `bd` reports them as `remote.restacked` and pushes such a branch without `--force`
  ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`a_child_of_an_amended_parent_is_restacked`)
  [@test] ../../src/lib/stack.test.ts (`pushes an amended or restacked ancestor with the stack and asks nothing: its push replaces older versions`)
  [@test] ../../crates/bd-cli/src/push_test.rs (`amended_and_restacked_commits_push_without_force`)

### Integration detection

- A rebuilt child must get a stack note (`refs/notes/branch-deck/stack`) on its tip naming the commit it was built on,
  written with the mapping notes in one `fast-import` run.
- Detection of an archived child must look only at the commits after that base: `^<base>` in the cherry-mark scans and
  the merge count, the base as the merge base of the squash check. A child whose own commits are upstream is
  integrated even if its parent is not.
  [@test] ../../crates/integration-detection-tests/src/stack_tests.rs (`an_archived_child_is_integrated_when_its_own_commits_are`)
- The result of a stacked child must be cached like any other: a `NotIntegrated` note records the baseline it was
  checked against and the commits without an equivalent, and is checked again against the baseline commits since then
  when the baseline moves ([Integration Detection](../../docs/design/integration-detection.md)).
  [@test] ../../crates/integration-detection-tests/src/stack_tests.rs (`an_archived_child_counts_only_its_own_commits_and_is_cached_with_its_orphaned_commits`)
  [@test] ../../crates/integration-detection-tests/src/recheck_tests.rs (`a_not_integrated_note_is_rechecked_when_the_baseline_moves`, `partial_integration_is_tracked_incrementally`, `a_recheck_compares_only_the_baseline_commits_since_the_note`, `a_v1_note_within_retention_is_detected_afresh`, `a_v1_note_beyond_retention_is_reported_as_is`, `a_rewound_baseline_falls_back_to_a_full_detection`, `two_archived_names_with_one_tip_are_rechecked_once`, `a_fast_forwarded_branch_names_its_tip_as_the_integration_commit`)
  [@test] ../../crates/integration-detection-tests/src/cache_tests.rs (`a_not_integrated_note_records_its_baseline_and_orphaned_commits`, `a_final_result_records_no_baseline`, `a_version_1_note_has_no_check_data`)

### Pull requests

- The pull request base of a stacked branch must be its parent's pushed branch; of a root, the baseline as the remote
  names it (`master`). A parent that isn't on the remote (its remote status says so; one not loaded yet stays the
  base) can't be a base: until it is pushed the base is the baseline, and the pull request includes the parent's
  commits; once it is pushed, the retarget below is offered. "Copy PR Base" in the branch menu must copy it, and
  "Create Pull Request…" creates the pull request with it as the base ([Pull Requests](pull-requests.spec.md#creating-a-pull-request)).
  [@test] ../../src/lib/stack.test.ts (`targets the parent's branch, or the baseline on the remote`, `targets the baseline while the parent isn't on the remote`)
  [@test] ../../src/lib/pull-request.test.ts (`opens the pull request form of the pushed branch into its base`)
- After a push, the toast is the push toast of any branch ([Pull Requests](pull-requests.spec.md#creating-a-pull-request)):
  it says what the push did and offers "Create Pull Request…" (into the base) or "Open Pull Request". A stacked branch's
  toast also names the base ("Pushed oauth" / "1 commit · created the remote branch · pull request base: login"); a
  root's names none, with stacked branches on, every branch has a stack, and a root's base is the baseline as for any
  branch.
  [@test] ../../src/lib/git/push.test.ts (`offers to create the pull request of a branch without one, into the baseline`, `names the parent of a stacked branch and creates its pull request into the parent's branch`, `creates the pull request of a stacked branch into the baseline while its parent isn't on the remote`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and name the pull request base`)
- An open pull request whose base (`baseRefName`) is not the expected one, e.g. after its parent was integrated or
  pushed for the first time, or the branch was restacked, must be retargeted to it (`PATCH pulls/<n>` with the new
  `base`, as `gh pr edit … --base …`) right
  before a push of its branch: pushed onto another base, a pull request into the old one shows the wrong commits, all
  of the baseline since. The push toast says so ("1 commit · created the remote branch · retargeted pull request
  #802 to master"). A failed retarget is a "Cannot retarget pull request #802" error and the push still runs; its
  toast then says "retarget it" with "Copy gh pr edit". "Copy Retarget Command" in the copy group of the branch menu
  copies the command before a push.
  [@test] ../../src/lib/git/push.test.ts (`retargets a pull request with another base before the push`, `pushes and copies the command that retargets a pull request when the retarget fails`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request whose base is not the expected one before the push`)
  [@test] ../../crates/sync-core/src/pull_request_writes_test.rs (`retargets_a_pull_request_of_the_repository_of_the_branches`, `a_failure_says_what_github_said`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
- The push button of a branch whose ancestors have work for the remote must push the stack (`buttonStackPushOrder`):
  the pull request of the branch is right only once they are on the remote. The compact button counts the commits the
  pushes send ("↑ 3"), in the strongest colour of them; its name is `Push Stack (3 branches)`, and so is the
  push of the branch menu, the same push ([Branch Row](branch-row.spec.md#branch-menu)). The push starts above the nearest ancestor a running Safe Push holds ([Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)):
  its push won't land; its description lists the pushes, "Pushes 3 branches, parents first:"
  and a line per branch ("login · 2 commits · creates the remote branch", "auth · 1 commit · force push"). A force
  push says what it replaces or drops in the words of the push alone ([Outgoing Commits](../repository/outgoing.spec.md#push)):
  "auth · 1 commit · force push, replaces 1 commit amended here", "login · force push, replaces the older versions of
  auth's commits", "session · 1 commit · force push, drops 1 remote commit". Below a rule, "login isn't on the remote
  yet. Pushing oauth alone works; a pull request includes login's commits until login is pushed." ("login has commits
  the remote lacks. … includes them until login is pushed." for a parent on the remote, the nearest ancestor with
  work) and a `Push oauth Only` button. Preview `P` pushes as the button does. While the stack push runs, the button
  shows the branch it pushes ("Pushing login 45%") and a second click does nothing. The row doesn't name the parent:
  the stack graph shows it, and the description of the button explains Push Stack.
  [@test] ../../src/hooks/usePushButton.test.ts (`pushes the stack while an ancestor has work for the remote, and offers the push of the branch alone`, `says what each force push of a stack push replaces, as the push of the branch alone does`, `takes the colour of an ancestor when none of the branch's commits are the user's`)
  [@test] ../../src/lib/remote-status.test.ts (`names it in the line of a stack push in the same words, or the remote commits the push drops`)
  [@test] ../../src/lib/git/push.test.ts (`pushes nothing more from a row whose stack push runs`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and name the pull request base`, `should push a stacked branch alone from the description of its button`)
- The button pushes the branch alone, and its description offers `Push Stack (3 branches)` instead, while the branch's own push drops remote commits (it asks first, Force Push) and while the stack above
  the nearest ancestor whose push drops remote commits has only the branch: that ancestor asks first in its own row,
  and a stack pushed around it would leave a stale branch in the middle.
  [@test] ../../src/hooks/usePushButton.test.ts (`offers Push Stack while the button pushes the branch alone: its push drops remote commits, or an ancestor's does`)
  [@test] ../../src/lib/stack.test.ts (`starts the stack push of the button above an ancestor whose push drops remote commits`, `pushes the stack from the button only while the branch's own push asks nothing`)
- "Push Stack" (the button, or in that description) must push the ancestors with unpushed work, root first, then the
  branch, and stop at a failure. The branch menu pushes as the button does, never the whole stack past an ancestor
  whose push drops remote commits: that ancestor asks first in its own row. `bd push` takes them along too ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).
  [@test] ../../src/lib/stack.test.ts (`pushes the ancestors that have work for the remote first, root first`, `lists the branches a branch is built on, root first`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row`)
- A push of several branches, "Push Stack" or "Push with Children", is one toast, also when it stopped at a failure
  (then for the branches pushed before it, and the failure's own error toast): "Pushed auth, login and oauth" (more than
  three: "Pushed auth, login, oauth and 2 more") / "3 commits · created 2 remote branches · force pushed auth", the
  pull request of a pushed branch that needs a retarget, the robots the pushes restarted. The action is the pull
  request action of the branch the user pushed from ("Create Pull Request…" into its base, or "Open Pull Request"),
  unless a retarget comes first. Activity has an entry per branch. A push of one branch is its own push toast.
  [@test] ../../src/lib/git/push.test.ts (`is one toast for the branches, with the pull request of the branch the stack was pushed from`)
  [@test] ../../src/lib/push-summary.test.ts (`says what the pushes of a stack did together`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and name the pull request base`)
- The push button of a branch whose descendants have work for the remote must stay as it is too: it pushes only the
  branch. Its description must offer the children below a rule: "oauth is stacked on login and has commits to push."
  ("oauth and session are …", "session and 2 more branches are … and have commits to push."), then a
  `Push with Children (4 branches)` button, named as in the branch menu. A branch whose own push is up to date offers
  it only in the menu. Below the other push of a stacked branch when both apply.
  [@test] ../../src/hooks/usePushButton.test.ts (`offers Push with Children while branches stacked on it have work for the remote`, `offers the push of the branch alone before Push with Children for a branch in the middle of a stack`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a parent with its children`)
- "Push with Children" must push the branch if it has work for the remote, then its descendants that have work,
  parents first, and stop at a failure. A descendant whose push drops remote commits is left out: it asks first in its
  own row; so is a branch a running Safe Push holds. It is offered only while it pushes more than the branch itself,
  and names what it pushes: "Push with Children (4 branches)", "Push with Children (oauth)" for one child. While a Safe
  Push holds the branch it is disabled with the reason: every Safe Push of the stack lands it.
  [@test] ../../src/lib/stack.test.ts (`pushes the branch and its children that have work for the remote, parents first`, `leaves out the branches a running Safe Push holds, and starts a stack push above them`, `names Push with Children by what it pushes, and offers it only beyond the branch itself`)
  [@test] ../../src/lib/git/push.test.ts (`starts above the branches a running Safe Push holds`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a parent with its children`)

- A Safe Push of a stacked branch lands the branches it is built on with it, and a Safe Push of a parent moves its
  children onto the target: the Safe Push form says both and leads root first ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)).
  A branch that builds alone gets "Unstack from <parent>" first instead, and a branch that needs commits it can take
  gets "Move 1 Commit Here" first.
  While it runs, the branches it lands say "lands with oauth" in their Status cell and offer no push and no Safe Push of their
  own; the lines of the graph from the root down to it are in the colour of the robot, its node has the ring of the
  robot. Before the start, the same path is highlighted while the pointer is on the Safe Push slot and while the form
  is open ([Branch Row](branch-row.spec.md#tree)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)
  When a robot of a stacked branch merged, the open pull requests of the ancestors it landed are closed with a comment
  ([Patronus](patronus.spec.md#merged)).

### Push Up to Here

- A commit the remote lacks must offer "Push Up to Here": its push arrow in the commit lines of a stack island
  (`PushUpToMark`) opens what it sends, and the menu of a commit in a commit list pushes it (`pushUpToPlan`). It
  pushes the branches the commit is built on that have work for the remote, whole (the branch is built on their tips),
  root first, then the branch up to the commit (`PushBranchParams.tip`); the commits after it stay unpushed. The last
  commit the remote lacks is the push of the whole branch. The popover says it in the words of Push Stack: "Pushes 3
  branches, parents first:", a line per push, "bridge · 1 of 2 commits · creates the remote branch"; its button
  "Push Up to Here" marks, while hovered, the nodes of the branches and the commits it sends. One toast says what was
  pushed, the part of the branch counted.
  [@test] ../../src/lib/push-up-to.test.ts (`pushes the branches the commit is built on whole, root first, then its branch up to it`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`, `should push up to a commit from its menu in the full list`)
- Only a commit on the fast-forward tail can be a cut: the branch is not on the remote, or the remote has an older
  state of it. A branch rewritten since its push pushes whole: "login was rewritten since its push: it pushes whole."
  The backend checks it with one `rev-list` (`check_push_tip`): the commit is one of the branch's own, after its
  parent or the baseline, and the remote head an older one of them; else nothing is pushed.
  [@test] ../../src/lib/push-up-to.test.ts (`cuts only the commits the remote lacks of a branch the remote has an older state of`)
  [@test] ../../crates/sync-core/src/push_test.rs (`a_push_up_to_a_commit_leaves_the_commits_after_it_unpushed`, `a_push_up_to_a_commit_refuses_one_outside_the_branch_and_a_branch_rewritten_since_its_push`)
- It must say why it can't, with a link that shows the branch in the way: an ancestor blocked or failed to sync ("auth
  can't be pushed: blocked by core."), an ancestor whose push drops remote commits ("auth's push drops 2 remote
  commits: Force Push auth first.": it asks first in its own row), a branch a running Safe Push lands ("A Safe Push of
  oauth lands all of auth."); the menu item is disabled with the reason.
  [@test] ../../src/lib/push-up-to.test.ts (`stops at an ancestor that can't be pushed or asks first, and at a branch a running Safe Push holds`)

### Messages

- `groupFor` must read a prefix as the backend does: the leaf, its parent and an empty level.
  [@test] ../../src/lib/commit.test.ts (`reads %j as the backend stack_path does`, `keeps a slash in a flat name without stacked branches`)
- The commit message editor must say when an edit restacks the branch, and warn when it would make the branch's
  parents conflict, also with its stack parent, or name a parent without commits, likely a typo: "api has no commits:
  docs is built on master until it has".
  [@test] ../../src/lib/stack.test.ts (`names the new branch and its parent`, `warns about a parent the other commits of the branch don't declare`, `says a single-commit branch is restacked or unstacked`, `warns about a parent without commits`, `warns about a parent level that differs from the stored parent`)

### Fixups

- With stacks on, a fixup's prefix must agree with its target's when both name the same leaf, so `fixup! (login) X`
  finds `(auth/login) X`.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`with_stacks_a_fixup_names_the_leaf_of_its_target`, `with_stacks_another_leaf_does_not_agree`, `the_plan_reads_the_stacks_setting_of_the_repository`)

### UI

- The branch table must show each branch under the branch it stacks on, depth-first; among siblings the shortest
  chain comes first (ties newest first), so the longest chain comes last and continues its parent's line. It draws
  each stack as a graph, root on top, instead of a staircase: a straight chain is one line, a side child forks off on
  a lane of its own, and the expanded commits of a branch sit on its line ([Branch Row](branch-row.spec.md#tree)). A
  branch whose parent has no row, or whose parents form a cycle, stays at the top level.
  [@test] ../../src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`, `keeps a branch whose parent has no row, or whose parents form a cycle, at the top level`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`, `should carry the commits of a branch on its lane, into the lines of its children`)
- Each row must carry its place in the stack graph (`lanes`, `src/lib/stack-lanes.ts`): its lane, the lane a side
  child forks from, the line up into its node and down from it, and the lanes passing it. A straight chain stays on
  lane 0; at a fork the last child continues the parent's lane and each other child takes the lowest free lane for
  its subtree, which follows the parent at once, so lines never cross; a lane is free again after its subtree. Each
  stack starts at lane 0, so separate stacks share no line. Without a branch stacked on another, no row has lanes.
  [@test] ../../src/lib/stack-lanes.test.ts (`keeps a straight chain on lane 0`, `continues the longest chain on the parent's lane and forks the others off it, shortest first`, `opens a lane per fork inside a side subtree, and frees it after the subtree`, `keeps the given order of children with chains of the same length: the oldest is the main line`)
  [@test] ../../src/stores/branch-table-rows.test.ts (`keeps a straight chain on one lane and forks a leaf sibling and a side chain off it`, `starts each stack on lane 0, so separate stacks share no line`, `has no lanes while no branch is stacked on another`)
- The review window of a stacked branch must show its stack from the baseline as segments: the branch on its parent
  first, Shift+click extends the part, "Whole Stack" shows what Patronus checks ([Branch Review](branch-review.spec.md#the-stack-and-the-commits)).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- Issue families must be formed from the stack roots only; a family member and the stack on it are indented one
  level (`familyIndent`), and the stack keeps its lanes inside it: lanes mean "built on", the indent "belongs to the
  issue". A stacked branch of another issue than its parent shows its issue as a chip, unless the branch is named
  after it. Without stacked branches the table has no lanes; a stack and an issue family each read as one block
  ([Branch Row](branch-row.spec.md#tree), the dividers).
  [@test] ../../src/stores/branch-table-rows.test.ts (`forms issue families from the stack roots only and marks a child of another issue`, `indents a family member and the stack on it one level, with the lanes of the stack inside`, `shows no chip for a stacked branch named after its issue`)
  [@test] ../../src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`)
- Under the filter of the table ([Branch Filter](branch-filter.spec.md)), the rows of the matches only
  (`useVisibleBranchTableRows`) must include the branches they stack on, dimmed unless they are matches themselves, and
  lay the graph out over the rows that remain, so a match deep in a stack stays connected to its root, also in a
  collapsed stack: the filter wins over the collapse ([Branch Filter](branch-filter.spec.md#the-filter-and-the-rest-of-the-table)).
  The header counters and the selection read all rows.
  [@test] ../../src/stores/branch-table-rows.test.ts (`prunes the rows to the visible branches and the branches they stack on, which are dimmed`, `prunes an issue family to its visible members`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)
- A stack of 5 branches or 40 commits at the top level of the table must be an island of its own below it, its header
  heading the stack, its commits on the lanes ([Stack Island](stack-island.spec.md)); the rules below are of a stack
  in the table, and an island collapses by the same `collapsedStacks`.
- The root row of a stack must head it, so a stack of 22 branches doesn't read as 22 rows. A stack is the tree of the
  branches built on a root; what a branch's ancestors are to it is "built on", and a Safe Push "lands … with it". After
  the root's name a chip sizes the stack, "22 branches", and the tooltip says the rest: "Stack of vm-lane-daemon · 22
  branches · 152 commits · 4 to push · Collapse Stack ⌥←" (`StackChip`). A click on it, ⌥← from any row of the stack
  (the focus moves to the root) and "Collapse Stack" of the branch menu collapse the stack into its root; the click,
  ⌥→ and "Expand Stack" expand it. A held key collapses once. A collapsed root has no line down from its node, a short
  line fading out below it instead (`lane-collapsed`), and its chip adds the strongest state the hidden rows hold, what
  waits for the user first: "· 1 conflict" or "· 1 error" (`destructive`), "· 1 blocked" (`warning`), "· 1 failed"
  (`destructive`), "· 1 needs review" (`warning`), then "· 1 running" with the ring and "· 3 to push" (the push
  colour). Each counts as the surface that tells it for all branches (`summarizeStackState`): "failed" a robot that
  failed on the branch as it is, as the robots indicator and Attention count it (one of an earlier state, fixed since,
  counts nothing: its row offers a new Safe Push), "needs review" one that failed for missing approvals, "to push" by the
  rule of To Push. Stacks are expanded by default; a collapsed one
  is remembered per repository (`collapsedStacks`, by root; a rename of the root keeps it, a stack on another branch or
  a merged root leaves the list), and the table keeps the lane column of the expanded stacks, so no name moves.
  Collapsing closes the inline form of a hidden row; a reveal of a hidden branch, or an inline form opened for one
  elsewhere, expands its stack. The header counters, the outgoing overview, the robots and Your Turn count every branch.
  [@test] ../../src/stores/branch-table-rows.test.ts (`collapses a stack to its root and keeps the lane count of the expanded table`, `marks no root collapsed whose stack has no other row, and finds the root from any row of a stack`, `keeps only the collapsed roots that still head a stack`)
  [@test] ../../src/lib/stack-state.test.ts (`counts a failed robot as the header does: on the branch as it is, apart from one that needs review`, `counts the push by the rule of the outgoing counts: not a landed branch, nor one without commits`, `tells a conflict from another sync error`, `puts what waits for the user before what runs, and the push last`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should head a stack with its size and collapse it into its root, per repository`, `should name in the chip of a collapsed stack only the robots that failed on its branches as they are`, `should collapse and expand a stack with Alt+arrows from any of its rows`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show a match deep in a collapsed stack, and expand the stack for a reveal`)
- A side stack, a branch forked off the lane of its parent with a branch stacked on it, must have the chip of a stack
  root on its row ("2 branches", "· 1 to push" while collapsed, `sideStackSizes`), which collapses the side stack into
  it, in the table and in an island, for the session. A side stack with nothing for the remote and nothing wrong
  (pushed, no error, not blocked) is collapsed until the user expands it. ⌥← works as ← in a tree: from a row of an
  expanded side stack it collapses the nearest one, its root takes the focus, and the next ⌥← the whole stack; ⌥→ on
  a collapsed side stack expands it. A reveal of a branch it hides expands it. The main line has no chip.
  [@test] ../../src/stores/branch-table-rows.test.ts (`collapses a side stack into its root, and keeps a quiet one collapsed until the user expands it`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should collapse a side stack into its root, from its chip and with Alt+Left before the whole stack`)
- The branch menu must offer "Stack On…", a submenu of the other branches without the branch's descendants and its
  parent (disabled with "No other branch to stack on" when none is left), and "Unstack from <parent>" for a stacked branch (`restack_branch`, with an undo), and "Collapse Stack" (⌥←)
  or "Expand Stack" (⌥→) on a branch of a stack while no filter is active. Stack On and Unstack write the stack
  parents and rewrite only the commits whose prefix declares another parent ([Stack Parents](stack-parents.spec.md#restack-unstack-and-rename)),
  so an issue branch and an issue group are stacked too. A toast without a moved commit names no count: "Stacked docs
  on oauth".
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a branch on another one and unstack it`, `should not offer a branch's own descendants to stack on`, `should stack an issue branch without a rewrite and undo it`)
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`restack_writes_the_store_and_rewrites_only_differing_parent_levels`, `unstack_removes_the_line_and_the_parent_levels`, `renaming_a_parent_renames_it_in_its_children`)
- Rename of a parent must keep its children stacked on it; Ungroup and Unapply of a parent must name its children:
  "oauth stays stacked on login and is built on master until login has commits again." See
  [Move Commits](move-commits.spec.md).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should rename a parent with its children following`, `should build a child on the baseline once its parent has no commits, and unstack it`)
- A branch built elsewhere because its declared parent has no commits (`prBaseChanged`) must say so in the third
  column of its Status cell: a muted "on master" (or the ancestor it is built on) with the reason as its tooltip ("login
  landed · oauth is built on master"; "login has no commits", "login is unapplied"). Its push button is enabled.
  Expanded, an alert says "Built on master: login landed. The pull request base is master." (`info` for a landed
  parent, `warning` for one without commits, likely a typo, neutral for an unapplied one) with an
  "Unstack from login" link: the undoable change of the branch menu, the only restack, by the user. The
  push toast offers the retarget of an open pull request ([Pull requests](#pull-requests)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)
  [@test] ../../src/lib/stack.test.ts (`says why the branch isn't built on its parent`, `is passed only when the branch is built elsewhere`)
- A blocked branch must show a muted `blocked by <parent>` status after a `warning` dot with the reason as its tooltip,
  a disabled push button (in the row, the Outgoing overview and the preview), no share in the outgoing counts, and,
  expanded, a `warning` alert with a link that reveals the parent.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
  [@test] ../../src/lib/stack.test.ts (`leaves out a blocked branch: it offers no push`)
- Where the stack graph isn't visible, a stacked branch must say what it is built on ("on auth", "on master" while its
  parent has no commits): the Outgoing overview and the branch preview. The branch row says it only for a branch built
  elsewhere ("on master"); the graph shows the parent.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)
- A push of a stacked branch must count only its own commits as unpushed afterwards (`parentBranch`).

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/stack_test.rs`, `stack_sync_test.rs`, `commit_grouper_test.rs`,
  `autosquash_test.rs`.
- `pnpm test:unit`: `src/lib/stack-lanes.test.ts`, `src/stores/branch-table-rows.test.ts`, `src/stores/stack-view.test.ts`.
- `pnpm e2e`: `tests/e2e/stacked-branches`, `tests/e2e/branch-filter` (a collapsed stack under the filter).
- `git config branchdeck.stacks true` in a repository, then commit with `(parent/child)` prefixes.

## Open Questions / Risks

- A parent integrated by a squash merge is `Missing` under the default rebase detection: its child is labelled "has no
  commits" instead of "landed"; the placement is the same.

## References

- [Branch Row](branch-row.spec.md): the stack graph and the blocked row
- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Git Operations](../../docs/design/git-operations.md)
- [Fixups](../commit/fixups.spec.md)
