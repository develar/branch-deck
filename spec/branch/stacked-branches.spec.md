---
name: Stacked Branches
description: Requirements for stacked branches, where a `(parent/child)` prefix builds the child's virtual branch on its parent's, a parent without commits passed to the branch below it, problems and blocked parents reported per branch, the root row heading its stack, which collapses into it, and the setting opt-in per repository.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/stack.rs
  - ../../crates/history-rewrite/src/commit_grouper.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/branch_processor.rs
  - ../../crates/history-rewrite/src/autosquash.rs
  - ../../crates/sync-core/src/remote_status.rs
  - ../../crates/branch-integration/src/detector.rs
  - ../../crates/branch-integration/src/detection_notes.rs
  - ../../crates/branch-integration/src/rebase.rs
  - ../../crates/branch-integration/src/merge.rs
  - ../../crates/branch-integration/src/squash.rs
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/trial-check/src/check.rs
  - ../../crates/domain-types/src/stack.rs
  - ../../crates/domain-types/src/session.rs
  - ../../crates/domain-policy/src/facts.rs
  - ../../crates/domain-policy/src/stack.rs
  - ../../crates/repository-model/src/derive.rs
  - ../../crates/repository-model/src/table.rs
  - ../../src/model/structure.ts
  - ../../src/model/store.ts
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/stack-lanes.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/branch/StackProblemAlert.tsx
  - ../../src/lib/branch-drop.ts
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
  - ../../src/presenters/push-button.ts
  - ../../src/hooks/useBranchPushes.ts
  - ../../src/lib/push-summary.ts
  - ../../crates/domain-policy/src/push.rs
  - ../../src/lib/push-run.ts
  - ../../crates/operations/src/push.rs
  - ../../crates/sync-core/src/push.rs
  - ../../src/components/branch/StackBaseLabel.tsx
  - ../../crates/sync-core/src/repository_config.rs
  - ../../src/components/branch/StackChip.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/presenters/push.ts
  - ../../src/components/commit/PushUpToMark.tsx
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/lib/stack-state.ts
  - ../../src/hooks/useStackStates.ts
  - ../../src/lib/stack-collapse.ts
  - ../../src/stores/stack-collapse.ts
  - ../../src/stores/settings/repositorySettings.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/components/shared/MenuItems.tsx
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/branch/LandedChip.tsx
  - ../../src/lib/git/push-toast.tsx
  - ../../src/presenters/pull-request-url.ts
  - ../../src/lib/pull-request-dialog.ts
  - ../../src/lib/pull-request-run.ts
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../crates/remote-facts/src/pull_request_writes.rs
---

# Stacked Branches

Status: Draft
Date: 2026-10-15

## Summary

A commit `(auth/login) Add form` belongs to the branch `login`, and `login` stacks on `auth`. Its virtual branch is
built on the new head of `auth`'s virtual branch. Its pull request goes into the baseline, as every pull request of a
virtual branch, and holds `auth`'s commits with its own: what its Safe Push lands ([Landing](../../docs/design/landing.md)).
Deeper prefixes declare the ancestry (`(auth/login/oauth)`), and a bare `(login)` joins `login` wherever
its parent is declared. A branch whose prefixes declare no parent, an issue branch and an issue group too, takes its
stack parent from `refs/branch-deck/stack-parents` ([Stack Parents](stack-parents.spec.md)): Stack On, Unstack and
`bd group` with a path write it there, without a history rewrite. A parent without commits (landed, unapplied, mistyped) is passed: the child is built on the
nearest ancestor with commits, or the baseline. A branch that can't be placed, or whose parent can't be built, is
reported and not rebuilt; the other branches still sync. The branch table draws the stacks as a graph, and the branch
menu stacks, unstacks and renames branches.

## Goals

- Land a feature into master in dependent pieces, each a branch with its own commits on the ones it is built on.
- Keep linear development on the main branch: the prefixes and the stack parents are the stack state.
- The history decides where a branch is built: a parent without commits (landed, unapplied, mistyped) doesn't stop its
  children; the base change is visible in the row and the alert; only the user restacks a branch (Unstack).

## Non-goals

- A pull request into a parent's branch (stacked pull requests that show only the branch's own commits): Patronus
  merges a pull request into its base and counts only the approvals of the pull request that started it, so such a
  pull request can never land ([Landing](../../docs/design/landing.md#alternatives-considered)). Every pull request of a
  virtual branch goes into the baseline; an open one into a virtual branch is retargeted ([Pull requests](#pull-requests)).
- The pull requests of every branch of a stack at once: "Create Pull Request…" creates the pull request of one branch
  ([Pull Requests](pull-requests.spec.md#creating-a-pull-request)). A stack that forks has one more, of its whole
  stack ([Whole Stack](whole-stack.spec.md)). Branch Deck closes the pull requests a Safe Push landed
  ([Pull requests](#pull-requests)).

## Requirements

### Opt-in

- Stacks must be on only when `git config branchdeck.stacks` is true (Settings → Repository, Stacked branches, writes
  it to `.git/config`; [Settings](../repository/settings.spec.md#repository)). Without the setting, a `/` in a prefix
  must stay part of a flat branch name and every event must be as before, with a null `stack`.
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`without_stacks_a_slash_is_part_of_a_flat_name`, `stacks_are_off_unless_configured`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`without_stacks_branches_grouped_has_no_stack`)
- Without stacks, `(auth)` beside `(auth/login)` is two flat branches whose refs git cannot hold together
  (`<prefix>/virtual/auth` is a file where `<prefix>/virtual/auth/login` needs a directory): the sync fails with
  "Failed to sync branch auth/login" (or `auth`, by the order of the writes) and git's ref error (`cannot lock ref`
  or `couldn't set`). Turning stacks off after a stacked sync archives the stacked child (`login`) and fails the same
  way on `auth/login`.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`without_stacks_a_parent_and_its_child_prefix_fail_the_sync`, `turning_stacks_off_fails_the_sync_on_the_flat_child`)
- Settings → Repository doesn't offer stacks off while it would fail so: it names the stacked branches whose flat
  names clash and says to unstack them first ([Settings](../repository/settings.spec.md#repository)).
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`with_stacks_on_the_read_names_the_flat_branches_that_would_clash`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should not offer to turn stacked branches off while their flat names would clash, and say what to do`)

### Prefix syntax

- The last level of a prefix must name the branch and the level before it its parent. Each level must be sanitized
  like a flat branch name. Empty levels at the ends must be dropped; an empty level in the middle (`(a//b)`) is a
  syntax problem of the last level's branch.
  [@test] ../../crates/domain-types/src/stack_test.rs (`stack_prefixes_split_into_sanitized_levels`, `an_empty_stack_level_is_a_syntax_error`)
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`a_syntax_error_is_reported_on_the_last_level`)
- Commits must be grouped by the leaf, and every level pair must declare an edge: `(auth/login/oauth)` also declares
  `auth` as the parent of `login`. An issue-key branch is a root unless the stack parents name its parent
  ([Stack Parents](stack-parents.spec.md#placement)).
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`groups_by_leaf_and_records_the_declared_parents`, `a_parent_without_commits_is_missing`)

### Problems

- These problems must mark only the affected branch, with the `Error` status, naming the commits; the branch is not
  rebuilt:
  - commits declare different parents;
  - the parents form a cycle (every branch of the cycle);
  - `(a/a)`;
  - two branches differ only in case (both);
  - a syntax problem.
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`conflicting_parents_name_their_commits`, `a_branch_can_not_be_its_own_parent`, `a_cycle_marks_each_of_its_branches`, `leaves_that_differ_only_in_case_collide`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_problem_is_not_built_and_a_missing_parent_is_passed`)
- Each problem must be said once, by Rust, for the app and `bd` (`problem_view`, `StackProblemView`): a label (the
  status of the row), a reason naming the branch ("login has conflicting parents"), a message without commands and its
  fixes (`StackFix`). The fixes must be only those that remove the problem and add none: conflicting parents, Stack On
  each parent except one stacked on the branch (a cycle), or Unstack; a cycle, Unstack; `(a/a)`, Unstack while no
  other parent is declared, else Reword of its commits; a case collision, Rename; an empty level, Reword of its commits.
  `bd` prints the fixes as commands after the message (`fix: `bd restack login auth` or `bd unstack login``).
  [@test] ../../crates/history-rewrite/src/stack_test.rs (`each_problem_offers_the_fixes_that_remove_it`)
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`each_restack_a_stack_problem_offers_removes_it`)
- A child of a branch with a problem must be blocked with the reason `problem`, its text naming the problem
  ("Blocked: parent login has a stack problem (conflicting parents)"), not a failure.
  [@test] ../../crates/history-rewrite/src/stack_test.rs (`a_parent_with_a_problem_blocks_its_children`)
  [@test] ../../crates/bd-remote/src/status_test.rs (`stacks_in_status`)
- A regroup that would introduce a problem must be refused by its reason, without a fix: the plan changed nothing yet.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`refuses_problems_the_plan_introduces_and_keeps_existing_ones`)

### Placement

- A child must be built on the new head of its active parent's virtual branch, also when a child commit comes before
  every parent commit on the main branch (`childPrecedesParent`).
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)
  [@test] ../../crates/history-rewrite/src/stack_test.rs (`a_child_is_built_on_its_active_parent`)
- A parent without commits must be `Unapplied` with an unapplied branch of its name, `Integrated` when its branch is
  integrated, else `Missing`. Its branch is its virtual branch, which the sync is about to archive, else its newest
  archived branch; a detection note says whether it is integrated. The state is a label (the row, `bd status`); the
  placement doesn't depend on it.
  [@test] ../../crates/history-rewrite/src/stack_test.rs (`reads_the_state_of_parents_without_commits_from_their_branches`)
- The branch of a parent without commits (its virtual branch, else its newest archived one) must be detected before the
  branches are placed by the rule of the archive step: when it has no note, a `NotIntegrated` note the baseline moved on
  from (incrementally), or a note without a usable baseline (in full; an archived branch only within the retention
  period), so the first sync after the parent's merge already labels the parent landed, also when the parent was
  archived a sync before. The sync writes the note, and its archiving reports the parent from it; `bd status` and
  `bd check` detect without writing.
  [@test] ../../crates/history-rewrite/src/stack_test.rs (`a_parent_whose_virtual_branch_lost_its_commits_is_detected_before_the_placement`)
  [@test] ../../crates/sync-core/src/detection_stack_test.rs (`the_first_sync_after_the_parent_is_integrated_builds_the_child_on_the_baseline`)
  [@test] ../../crates/sync-core/src/detection_recheck_test.rs (`a_stale_note_on_the_virtual_tip_does_not_hide_an_integrated_parent`, `a_stale_note_on_the_archived_parent_does_not_hide_an_integrated_parent`)
  [@test] ../../crates/bd-remote/src/status_test.rs (`a_parent_integrated_since_the_last_sync_is_integrated_before_the_next_one`)
- A parent without commits, whatever its state, must move the child to its nearest active ancestor, or the baseline,
  with `prBaseChanged`: the local history is linear on the baseline, so the child's commits can be picked there like
  those of any flat branch (or conflict like them).
  [@test] ../../crates/history-rewrite/src/stack_test.rs (`an_integrated_parent_moves_the_child_to_the_nearest_active_ancestor`, `a_parent_without_commits_moves_the_child_to_the_nearest_ancestor_with_commits`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`an_unapplied_parent_is_reported_before_the_sync_starts`, `a_problem_is_not_built_and_a_missing_parent_is_passed`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)
  [@test] ../../crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_blocked_branch_follows_its_declared_parent_not_what_blocks_it`)
- A parent with a problem and a blocked parent must block the child: the `Blocked` status, the reason as its text, its
  commits blocked, its virtual branch left as it was.
  [@test] ../../crates/history-rewrite/src/stack_test.rs (`a_parent_with_a_problem_blocks_its_children`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)
  [@test] ../../crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_blocked_branch_follows_its_declared_parent_not_what_blocks_it`)
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
- A child whose parent (or any branch below it) was amended, reworded or rebuilt since the child was pushed has the
  parent's commits as they were on its remote branch. Those must be restacked, not dropped (a reworded one, which the
  remote has by content, too: its message there is the old one): their identity (author email,
  author time, subject without the prefix) is that of a commit of a branch it is built on. The status reads
  `restacked` (`4↑ · restacked`), "Rebuilt on the new auth: the remote has older versions of 2 of its commits.", the
  push is a `primary` Force Push that asks nothing, also with nothing of the child's own to push, and Push Stack takes
  such a branch along ([Outgoing Commits](../repository/outgoing.spec.md#what-is-outgoing)). The sync builds the
  identities of every branch's ancestors once, from the commits it grouped, along the chain of effective bases; a push
  outside the sync reads them from the parent's virtual branch with one `rev-list`, only while a remote commit
  matches nothing of the child. `bd` reports them as `remote.restacked` and pushes such a branch without `--force`
  ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`a_child_of_an_amended_parent_is_restacked`, `a_child_of_a_reworded_parent_is_restacked`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`amended_and_restacked_commits_push_without_force`)

### Integration detection

- A rebuilt child must get a stack note (`refs/notes/branch-deck/stack`) on its tip naming the commit it was built on,
  written with the mapping notes in one `fast-import` run.
- Detection of an archived child must look only at the commits after that base: `^<base>` in the cherry-mark walk and
  the merge count, the base as the merge base of the squash check. A child whose own commits are upstream is
  integrated even if its parent is not.
  [@test] ../../crates/sync-core/src/detection_stack_test.rs (`an_archived_child_is_integrated_when_its_own_commits_are`)
  [@test] ../../crates/sync-core/src/detection_merge_test.rs (`a_merged_stacked_branch_counts_only_its_own_commits`)
- The result of a stacked child must be cached like any other: a `NotIntegrated` note records the baseline it was
  checked against and the commits without an equivalent, and is checked again against the baseline commits since then
  when the baseline moves ([Integration Detection](../../docs/design/integration-detection.md)).
  [@test] ../../crates/sync-core/src/detection_stack_test.rs (`an_archived_child_counts_only_its_own_commits_and_is_cached_with_its_orphaned_commits`)
  [@test] ../../crates/sync-core/src/detection_recheck_test.rs (`a_not_integrated_note_is_rechecked_when_the_baseline_moves`, `partial_integration_is_tracked_incrementally`, `a_recheck_compares_only_the_baseline_commits_since_the_note`, `a_v1_note_within_retention_is_detected_afresh`, `a_v1_note_beyond_retention_is_reported_as_is`, `a_rewound_baseline_falls_back_to_a_full_detection`, `two_archived_names_with_one_tip_are_rechecked_once`, `a_fast_forwarded_branch_names_its_tip_as_the_integration_commit`)
  [@test] ../../crates/branch-integration/src/detection_notes_test.rs (`a_not_integrated_note_records_its_baseline_and_orphaned_commits`, `a_final_result_records_no_baseline`, `a_version_1_note_has_no_check_data`)

### Pull requests

- The pull request of every virtual branch, stacked or not, must go into the baseline as the remote names it
  (`master`, `getPullRequestBase`): a stacked branch's holds the commits of the branches it is built on with its own,
  what its Safe Push lands, so its approvals are the ones Patronus counts ([Landing](../../docs/design/landing.md)).
  "Create Pull Request…" creates it into the baseline ([Pull Requests](pull-requests.spec.md#creating-a-pull-request)),
  and its dialog says what it holds ("Includes the commits of auth and login, which oauth is built on: 3 branches · 12
  commits"); `bd push` prints its `gh pr create` with `--base master` ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).
  [@test] ../../src/lib/stack.test.ts (`is the baseline as the remote names it for every branch, stacked or not`)
  [@test] ../../src/lib/pull-request-dialog.test.ts (`creates the pull request of a stacked branch into the baseline, with the commits of the branches it is built on`, `includes nothing more for a branch built on the baseline`)
  [@test] ../../src/lib/pull-request.test.ts (`opens the pull request form of the pushed branch into its base`)
- After a push, the toast is the push toast of any branch ([Pull Requests](pull-requests.spec.md#creating-a-pull-request)):
  it says what the push did and offers "Create Pull Request…" (into the baseline) or "Open Pull Request". It names no
  base: every branch has the same.
  [@test] ../../src/lib/push-run.test.ts (`offers to create the pull request of a branch without one, into the baseline`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
- An open pull request whose base (`baseRefName`) is a virtual branch of the user (`{prefix}/virtual/…`,
  `PullRequestBase::Virtual` of `domain_policy::pull_request::base_of`: the branch of its parent, opened when pull requests went into the parent's branch, or of a
  parent that landed) must be retargeted to the baseline (`PATCH pulls/<n>` with the new `base`, as
  `gh pr edit … --base …`) right before a push of its branch, each branch of a stack push too. Its approvals don't
  count until then, and Patronus would merge it into that branch. A pull request into another base (a release branch)
  is left as it is: that base may be meant ([Next Step](next-step.spec.md), row 5′). Before the push, the description
  of the push button says each retarget ("Retargets #8 to master." per pull request, "Retargets #8 of login to master."
  for a stack push), and in `warning` for one with approvals: "Retargets #8 to master: GitHub may dismiss its
  approvals." The push toast says it after ("1 commit · created the remote branch · retargeted pull request #802 to
  master"), and Activity records each: "Retargeted #802 to master" / "login · it went into auth", with "Open Pull
  Request" ([Activity](../repository/activity.spec.md)). A failed retarget is a "Cannot retarget pull request #802"
  error and the push still runs; its toast then says "the pull request goes into auth: retarget it to master" with
  "Copy gh pr edit". "Copy Retarget Command" in the copy group of the branch menu copies the command before a push.
  `bd push` and `bd safe-push` retarget the same way ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).
  [@test] ../../src/lib/push-run.test.ts (`retargets a pull request into a virtual branch to the baseline before the push, and says it in the toast of the push`, `pushes and copies the command that retargets a pull request when the retarget fails`)
  [@test] ../../crates/test-server/src/github_host_test.rs (`a_push_retargets_the_pull_request_of_its_branch_out_of_a_virtual_branch_first`)
  [@test] ../../src/presenters/push-button.test.ts (`says which pull requests into a virtual branch the push retargets first, and warns when GitHub may dismiss approvals`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)
  [@test] ../../crates/remote-facts/src/pull_request_writes_test.rs (`retargets_a_pull_request_of_the_repository_of_the_branches`, `a_failure_says_what_github_said`)
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
- The push button of a branch whose ancestors have work for the remote must push the stack (`BranchPush.button`, `domain_policy::push::branch_push`):
  the remote branches of its ancestors, and their pull requests, stay what the branch is built on. The compact button counts the commits the
  pushes send ("↑ 3"), in the strongest colour of them; its name is `Push Stack (3 branches)`, and so is the
  push of the branch menu, the same push ([Branch Row](branch-row.spec.md#branch-menu)). The push starts above the nearest ancestor a running Safe Push holds ([Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)):
  its push won't land; its description lists the pushes, "Pushes 3 branches, parents first:"
  and a line per branch ("login · 2 commits · creates the remote branch", "auth · 1 commit · force push"). A force
  push says what it replaces or drops in the words of the push alone ([Outgoing Commits](../repository/outgoing.spec.md#push)):
  "auth · 1 commit · force push, replaces 1 commit amended here", "login · force push, replaces the older versions of
  auth's commits", "session · 1 commit · force push, drops 1 remote commit". Below a rule, "login isn't on the remote
  yet. Pushing oauth alone works: its pull request includes login's commits either way." ("login has commits the
  remote lacks. Pushing oauth alone works: its pull request includes them either way, and the remote branch of login
  stays as it is." for a parent on the remote, the nearest ancestor with work) and a `Push oauth Only` button, which is
  also the item after the push in the branch menu (`push-branch-only`, no key; its "⋯" in the preview and the review
  window too): the second way, never the lead ([Landing](../../docs/design/landing.md)). Preview
  `P` pushes as the button does. The push is the intent `Push` with the scope `withAncestors` (`operations::push`): its
  plan sends the ancestors above the nearest one whose push drops remote commits, then the branch. A branch whose own
  push drops remote commits by the time it is planned (a fetch brought a commit of someone else after the row showed
  it) asks first: its push button opens its description with them, whose Force Push runs the push of the stack again
  with that drop accepted, never forced, over the remote head the row shows at the click. While it runs, the
  button names it ("Pushing 3 branches 45%", the refs its operation reserves), the rows of the branches it pushes show
  the same progress on their own push buttons, and a second click joins it (the same intent). A push of one of its
  branches meanwhile (its own row's button, the push step of a Safe Push start, a pull request, a review request) waits
  for it in the registry of the session and is decided again on what it left: it pushes only what the remote still
  lacks, never an invented "The remote branch changed", so no branch is sent twice. The row doesn't name the parent:
  the stack graph shows it, and the description of the button explains Push Stack.
  [@test] ../../src/presenters/push-button.test.ts (`pushes the stack while an ancestor has work for the remote, and offers the push of the branch alone`, `says what each force push of a stack push replaces, as the push of the branch alone does`, `takes the colour of an ancestor when none of the branch's commits are the user's`)
  [@test] ../../src/lib/remote-status.test.ts (`names it in the line of a stack push in the same words, or the remote commits the push drops`)
  [@test] ../../src/lib/stack.test.ts (`names what a stack push sends while it runs: its one push of several branches, or the one branch`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`, `should push a stacked branch alone from the description of its button`, `should push a stacked branch alone from its menu, the second way after the push of the stack`, `should ask in the row of a stacked branch whose push drops remote commits by the time its button's push of the stack is planned, and push the stack over the remote head the row shows`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`offers the push of the branch alone after the push of the stack, as the description of the button does`)
  [@test] ../../crates/operations/src/push_test.rs (`a_push_of_a_stack_is_one_push_of_the_branches_it_covers`, `the_pushes_of_one_ref_wait_for_each_other_and_both_push`)
- The button pushes the branch alone, and its description offers `Push Stack (3 branches)` instead, while the branch's own push drops remote commits (it asks first, Force Push) and while the stack above
  the nearest ancestor whose push drops remote commits has only the branch: that ancestor asks first in its own row,
  and a stack pushed around it would leave a stale branch in the middle.
  [@test] ../../src/presenters/push-button.test.ts (`offers Push Stack while the button pushes the branch alone: its push drops remote commits, or an ancestor's does`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`the_push_button_and_an_action_start_above_the_nearest_ancestor_that_asks_first`)
- "Push Stack" (the button, or in that description) must push the ancestors with unpushed work, root first, then the
  branch. The branch menu pushes as the button does, never the whole stack past an ancestor whose push drops remote
  commits: that ancestor asks first in its own row. `Push Stack (3 branches)` of the description of a button that
  pushes the branch alone covers every ancestor with work up to the nearest one a Safe Push holds, the ones that drop
  remote commits too: the intent forced (`force`), as the description named each drop. `bd push` takes them along too
  ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)
  [@test] ../../crates/domain-policy/src/stack_test.rs (`the_ancestors_are_the_chain_of_the_effective_bases_in_the_table`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row`)
  [@test] ../../crates/operations/src/push_test.rs (`a_push_of_a_stack_leaves_an_ancestor_that_asks_first_unless_forced`)
- A push of several branches (Push Stack, Push with Children, Push Up to Here) must be one atomic push of all of them,
  as `bd push` pushes ([Git Operations](../../docs/design/git-operations.md#push-and-land)): all of them or none, never
  a stack pushed up to a failure, which would leave a stale branch in the middle. A sync of the session that runs is
  awaited first, and the branches are decided and pushed as it leaves them. Each branch is leased on the remote head its row shows; a remote branch
  that moved since refuses the whole push and every branch is fetched. When that remote has no commits its branch lacks,
  nobody's work is lost: Rust pushes again over the fetched heads at once, once, as the push of one branch does
  ([Outgoing Commits](../repository/outgoing.spec.md#push)); else its toast says what that one has now ("login changed
  on the remote", "… Nothing was pushed.") and offers its push over it. A branch a running Safe Push holds refuses the
  push with the reason. A pull request into a virtual branch is retargeted before the push, as for one branch.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_stack_pushes_its_branches_in_one_push`, `a_moved_lease_refuses_every_branch_and_names_it`, `a_stack_push_up_to_a_commit_pushes_the_branches_below_whole`)
  [@test] ../../crates/operations/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks`, `a_push_of_a_stack_runs_the_processes_of_the_core_push`, `a_push_that_sends_holds_no_lock_and_sends_the_commit_it_froze`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`an_atomic_push_of_a_stack`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`)
  [@test] ../../tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack in one push, refused whole when a remote branch of it moved`, `should push a parent with its children`)
- A push of several branches, "Push Stack" or "Push with Children", is one toast: "Pushed auth, login and oauth" (up
  to four named, "Pushed auth, session, login and oauth"; from five on "Pushed auth, login, oauth and 2 more", never
  "and 1 more", `joinNames`) / "3 commits · created 2 remote branches · force pushed auth",
  each pull request the push retargeted ("retargeted pull request #802 of auth to master") and one whose retarget
  failed, the robots the pushes restarted. The action is the pull request action of the branch the user pushed from
  ("Create Pull Request…" into the baseline, or "Open Pull Request"),
  unless a retarget comes first. Activity has an entry per branch. A push of one branch is its own push toast.
  [@test] ../../src/lib/push-summary.test.ts (`says what the pushes of a stack did together`)
  [@test] ../../src/lib/text.test.ts (`names up to four, so 'and 1 more' never stands where one name would, and counts the rest from five on`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
- The push button of a branch whose descendants have work for the remote must stay as it is too: it pushes only the
  branch. Its description must offer the children below a rule: "oauth is stacked on login and has commits to push."
  ("oauth and session are …", "session and 2 more branches are … and have commits to push."), then a
  `Push with Children (4 branches)` button, named as in the branch menu. Below the other push of a stacked branch
  when both apply. A branch whose own push is up to date (the root of a stack pushed before its children) has Push
  with Children as its push button, the push of the header of an island (`ChildrenPushButton`): "↑ 3", the commits it
  sends, in the push colour, then the muted "with children" where the branch view is at least 56rem wide
  (`@4xl/branch-view`), its name "Push with Children (3 branches)", its hover the pushes it sends, the graph circling
  their nodes. P and the push item of its menu stay
  the push of the branch, disabled as up to date; the root row of an island leaves it to the header. It shows once
  Patronus and GitHub answered, as a first push waits for them: a robot of theirs holds the children, a merged one
  landed them, an open pull request of the whole stack takes its stack branch along. The settled layout holds its track
  meanwhile, and while it pushes the track keeps its width ("⟳ 45% with children").
  [@test] ../../tests/e2e/branch-row-layout/late-facts.spec.ts (`should hold the push track of a pushed root while GitHub answers, its Push with Children as wide as it was`)
  [@test] ../../src/presenters/push-button.test.ts (`offers Push with Children while branches stacked on it have work for the remote`, `offers the push of the branch alone before Push with Children for a branch in the middle of a stack`, `offers Push with Children while the branch itself is on the remote: its own push is disabled, this one isn't`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a parent with its children`, `should offer Push with Children while a sync runs and when the root is pushed`)
- "Push with Children" must push the branch if it has work for the remote, then its descendants that have work,
  parents first, in the one atomic push of a stack. A descendant whose push drops remote commits is left out: it asks first in its
  own row; so is a branch a running Safe Push holds. The branch's own push that drops remote commits asks first: its
  push button opens its description with them, whose Force Push runs Push with Children again with that drop
  accepted, never forced, so a descendant whose drop nobody showed stays out, over the remote head the row shows at the
  click (the branch leases on it, and its drops are counted on the head the session has); a plan that asks more by then
  (the remote got another commit) asks again. Nothing is pushed before that, and a plan that asks of more than one branch is
  refused, each drop named (D12). It names what it
  pushes: "Push with Children (4 branches)", "Push with Children (oauth)" for one child. Every branch with branches
  stacked on it offers it, in the branch menu beside
  the push of the branch ([Branch Row](branch-row.spec.md#branch-menu)), also while a sync runs: the push waits for the
  sync and computes what to push on the branches as it leaves them. It is disabled with the reason, never hidden,
  while it would push nothing beyond the branch ("Nothing to push: the stack is on the remote", "Only auth has
  commits to push: its own push sends them", "login asks first in its row: the push drops remote commits"; "Wait for
  the sync to finish" while the sync computes the remote statuses), while the stack is being pushed ("The stack is being
  pushed") and while a running Safe Push holds the branch: every Safe Push of the stack lands it. A branch a merged
  Safe Push landed whole is only left out, its own branch included: the branches built on it push without it, from its
  menu, the island's header and the description of its push ("session is stacked on auth and has commits to push.",
  "Push with Children (session)"). Once it and every branch built on it landed, nothing is left: "Landed in master
  with deploy and side-test: nothing of air and its children is left to push" ([Patronus](patronus.spec.md#merged)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask in the row of the branch before Push with Children drops its remote commits, and leave out a child whose drop nobody showed`)
  [@test] ../../src/lib/push-run.test.ts (`of a stack asks first in the push button of the branch that drops remote commits, whose Force Push runs it again with that drop accepted`, `of a stack whose plan asks more after the Force Push than its description showed asks again, then pushes over the remote head the row shows at the click`, `of a stack that drops remote commits of more than one branch is refused, each drop named`)
  [@test] ../../crates/operations/src/push_test.rs (`push_with_children_run_on_the_accepted_drop_leaves_a_child_that_drops_remote_commits_out`, `a_push_of_a_stack_on_an_accepted_drop_goes_only_over_the_remote_head_its_drops_are_counted_on`)
- Push with Children of the root of a stack that forks must also push the stack branch of its whole stack
  (`{prefix}/stack/<root>`, [Whole Stack](whole-stack.spec.md)) in the same atomic push, after the branches, leased
  on the remote head of its remote status, so the pull request of the whole stack follows its branches: only while the
  sync built it (never stale, conflicted or blocked), once it is on the remote or its pull request is open, while its
  remote lacks commits of it and has none it lacks, and never while it is held (`domain_policy::push::stack_branch_along`: its own robot
  runs, or every branch of the stack landed, [Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)). It counts as one more branch and the
  hover names it: "Push with Children (5 branches)", "Push with Children (the whole stack branch)" when only it has
  work, "Also pushes the whole stack branch user-name/stack/auth, the head of #12" on the menu item, "The whole stack
  branch user-name/stack/auth goes along." in the description of the root's push, its toast "… · pushed the whole
  stack branch" ("Pushed the whole stack branch of auth" alone, with "Open Pull Request" of the stack's), Activity
  "Pushed the whole stack branch of auth". "Nothing to push" is said only while the stack branch has nothing to push
  either: one on no remote and in no pull request is "The branches of the stack are on the remote. Its stack branch
  goes there with the pull request or the Safe Push of the whole stack", one the sync didn't build says why. A refused
  push names a stack branch that moved as the stack branch, apart from its root ("Cannot push the whole stack branch of
  auth"). `bd push <root> --with-children` follows the same rule ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).
  [@test] ../../crates/domain-policy/src/push_test.rs (`the_stack_branch_goes_along_once_published_built_not_held_and_behind`)
  [@test] ../../src/lib/stack.test.ts (`names Push with Children by what it pushes`)
  [@test] ../../src/presenters/actions.test.ts (`words every blocker of the catalog as the page says it`, `says why Push with Children leaves the stack branch of the whole stack as it is, and names the children that ask first`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`takes the stack branch of the whole stack of a root along, counted and named, and says why it doesn't in Rust's words`)
  [@test] ../../src/presenters/push-button.test.ts (`names the stack branch of the whole stack that Push with Children of a root takes along`)
  [@test] ../../crates/sync-core/src/push_test.rs (`the_stack_branch_goes_with_the_branches_and_a_moved_one_is_named_as_the_stack_branch`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should take the whole stack branch along in Push with Children of the root once its pull request is open, from the push and the menu`, `should push a parent with its children`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu`, `should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)
  [@test] ../../src/presenters/push-button.test.ts (`offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer no Safe Push or push of them`)
  [@test] ../../crates/bd-remote/src/landed_test.rs (`push_with_children_leaves_out_what_merged_safe_pushes_landed_and_refuses_once_nothing_else_is_left`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_with_children_covers_the_branch_and_its_descendants`, `push_with_children_leaves_out_what_asks_first_is_held_or_landed`, `push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)
  [@test] ../../src/lib/stack.test.ts (`names Push with Children by what it pushes`)
  [@test] ../../src/presenters/actions.test.ts (`words every blocker of the catalog as the page says it`, `says why Push with Children leaves the stack branch of the whole stack as it is, and names the children that ask first`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`is offered beside the push where Rust offers it, with its label of what it pushes, and the stack push over it`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a parent with its children`, `should offer Push with Children while a sync runs and when the root is pushed`)

- A Safe Push of a stacked branch lands it with the branches it is built on, in one robot: its button says so ("Safe
  Push · 3"), and its start always confirms, the popover at the slot saying it first: "Lands oauth with auth and login,
  which it is built on: 3 branches · 12 commits into master."; Enter starts it
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)). The branches built on it stay, and the popover
  says that the next sync after the merge builds them on the target. Its slot, "⋯", S, the preview and the review
  window ask the branch's own start, its popover at its own row, never the root's. Landing the branch alone is the second way:
  "Unstack from <parent>" for a branch that builds alone, "Move 1 Commit Here" for one that needs commits it can take.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`, `should offer to land only a stacked branch that applies alone`)
  While it runs, the branches it lands say "lands with oauth" in their Status cell and offer no push and no Safe Push of their
  own; the lines of the graph from the root down to it are in the colour of the robot, its node has the ring of the
  robot. Before the start, the same path is highlighted while the pointer is on the Safe Push slot and while the
  popover is open ([Branch Row](branch-row.spec.md#tree)). The branches it lands are those its commit has: a branch a
  restack placed below it after the start says no "lands with", pushes and starts its own Safe Push, and the colour of
  the robot starts below it ([Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a branch a restack placed below a running Safe Push after it started, and hold what its commit has`)
  When a robot of a stacked branch merged, the open pull requests of the ancestors it landed are closed with a comment
  ([Patronus](patronus.spec.md#merged)), and until the update archives them their rows say "Landed with oauth" and
  offer no Safe Push, no Dry Run and no push, nor does the branch it ran on ("Merged"); a branch built on a landed
  ancestor lands without it ("Safe Push" where it was "Safe Push · 2"). A new commit of an ancestor rebuilds the
  branches built on it: they stay landed by content, the new commit the only work left of its branch ("Landed with
  oauth · 1 commit after it"). `bd safe-push` lands a stacked branch the same way, with its ancestors in one robot
  ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`, `should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

### Push Up to Here

- A commit the remote lacks must offer "Push Up to Here", as the view says it (`BranchPush.upTo`: the commits that
  offer it, what it pushes first and what stops it, `domain_policy::push::up_to_push`, sent only where it differs from
  the common answer): its push arrow after the subject in the commit list of a branch (`PushUpToMark`), in the table, a
  stack island and the Parked card, opens what it sends (`pushUpToIn`), and the menu of the commit pushes it. The press
  is the intent `Push` with the scope `upTo` of the commit's copy, which Rust plans again on the facts of its run: the
  branches the commit is built on that have work for the remote, whole (the branch is built on their tips), root first,
  then the branch up to the commit, frozen instead of its tip (its tip witnessed), in the one atomic push of a stack;
  the commits after it stay unpushed, counted as the user's as a sync counts them (the branch's author). The last
  commit the remote lacks is the push of the whole branch. A commit a sync
  rebuilt since (another copy) has no plan: nothing is pushed (`plan_changed`). The popover says it in the words of Push Stack: "Pushes 3
  branches, parents first:", a line per push, "bridge · 1 of 2 commits · creates the remote branch"; its button
  "Push Up to Here" marks, while hovered, the nodes of the branches and the commits it sends. One toast says what was
  pushed, the part of the branch counted.
  [@test] ../../tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`, `should push up to a commit from its menu in the full list`)
  [@test] ../../crates/operations/src/push_test.rs (`push_up_to_here_pushes_the_branch_up_to_the_commit_after_what_it_is_built_on`)
  [@test] ../../crates/sync-core/src/push_test.rs (`a_stack_push_up_to_a_commit_pushes_the_branches_below_whole`)
- Only a commit on the fast-forward tail can be a cut: the branch is not on the remote, or the remote has an older
  state of it (its remote status is a fast-forward). A branch rewritten since its push pushes whole: "login was
  rewritten since its push: it pushes whole" (`Reason::RewrittenSincePush`). The commit is one of the branch's own the
  remote lacks, of the tip the plan witnesses; else nothing is pushed.
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_up_to_here_pushes_the_ancestors_whole_and_stops_at_what_cant_go`)
  [@test] ../../crates/repository-model/src/derive_test.rs (`the_view_sends_push_up_to_here_only_where_it_differs_from_the_common_answer`)
- It must say why it can't, in the words of the reason Rust gives, with a link that shows the branch in the way: an
  ancestor blocked or failed to sync ("auth can't be pushed: blocked by core"), an ancestor whose push drops remote
  commits ("The remote branch of auth has 2 commits that auth lacks: push it from its row, which shows them": it asks
  first in its own row), a branch a running Safe Push lands, a parked branch or one built on it, in the words of its
  push button ("Parked: Unpark it to push", "Built on parked core: Unpark core to push",
  [Parked Branches](parked-branches.spec.md#the-app)); the menu item is disabled with the reason.
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_up_to_here_pushes_the_ancestors_whole_and_stops_at_what_cant_go`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)
- A commit a merged Safe Push landed (`isLandedCommit`: every commit of a landed branch but those added after the
  landed ones, [Patronus](patronus.spec.md#merged)) is in the target, as a commit the remote has: one answer of the view
  for every surface (`BranchPush.upTo.commits`), so it has no push arrow and no "Push Up to Here" in its menu. A branch it landed whole is
  no step of a push up to a commit built on it: the branch is pushed without it, as its own push leaves it.
  [@test] ../../tests/e2e/stacked-branches/push-up-to.spec.ts (`should offer Push Up to Here in the full list only for a commit added after the ones a merged Safe Push landed, without the landed branches`, `should show no push arrow on the commits a merged Safe Push landed, only on a commit added after them`)

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
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`with_stacks_a_fixup_names_the_leaf_of_its_target`, `with_stacks_another_leaf_does_not_agree`, `the_plan_reads_the_stacks_setting_of_the_repository`)

### UI

- The branch table must show each branch under the branch it stacks on, depth-first; among siblings the shortest
  chain comes first (ties newest first), so the longest chain comes last and continues its parent's line. It draws
  each stack as a graph, root on top, instead of a staircase: a straight chain is one line, a side child forks off on
  a lane of its own, and the expanded commits of a branch sit on its line ([Branch Row](branch-row.spec.md#tree)). A
  branch whose parent has no row, or whose parents form a cycle, stays at the top level.
  [@test] ../../src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`, `keeps a branch whose parent has no row, or whose parents form a cycle, at the top level`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`, `should carry the commits of a branch on its lane, into the lines of its children`)
- Where each branch is must be Rust's, for the page and `bd` alike (`domain_policy::stack`, placed once per grouping
  by `repository_model::derive` as the structure of the branch in the session's view): its parent is the branch of the
  table it declares (a `(parent/child)` prefix or its stored parent), so a branch whose declared parent has no commits
  is a root of its own; its children in the order of the table, and its descendants, each after its parent; its
  ancestors, the chain of its effective bases among the branches of the table; the main line below it, at each fork
  the child with the longest chain, the last of children with equal ones. The page lays the rows, their depth and
  their lanes out of it and decides none of it; a patch carries the structure of a branch only when it changed.
  [@test] ../../crates/domain-policy/src/stack_test.rs (`children_and_descendants_follow_the_declared_parents_in_the_order_of_the_table`, `a_declared_parent_the_table_lacks_makes_a_root`, `the_ancestors_are_the_chain_of_the_effective_bases_in_the_table`, `the_main_line_takes_the_longest_chain_at_each_fork_and_the_last_of_equal_ones`, `a_cycle_of_declared_parents_ends_the_descendants_and_the_main_line`)
  [@test] ../../crates/repository-model/src/derive_test.rs (`a_grouping_places_every_branch_with_its_section`, `a_patch_carries_only_the_structures_that_changed`)
  [@test] ../../src/stores/branch-table-rows.test.ts (`lists the children and the descendants of a branch`)
- Each row must carry its place in the stack graph (`lanes`, `src/lib/stack-lanes.ts`): its lane, the lane a side
  child forks from, the line up into its node and down from it, and the lanes passing it. A straight chain stays on
  lane 0; at a fork the last child continues the parent's lane and each other child takes the lowest free lane for
  its subtree, which follows the parent at once, so lines never cross; a lane is free again after its subtree. Each
  stack starts at lane 0, so separate stacks share no line. Without a branch stacked on another, every row has lane 0
  alone (`alone`): the node of the branch, no line, its commits newest first.
  [@test] ../../src/lib/stack-lanes.test.ts (`keeps a straight chain on lane 0`, `continues the longest chain on the parent's lane and forks the others off it, shortest first`, `opens a lane per fork inside a side subtree, and frees it after the subtree`, `keeps the given order of children with chains of the same length: the oldest is the main line`)
  [@test] ../../src/stores/branch-table-rows.test.ts (`keeps a straight chain on one lane and forks a leaf sibling and a side chain off it`, `starts each stack on lane 0, so separate stacks share no line`, `puts every branch on one lane while no branch is stacked on another: its node says where it is on its way into master`)
- The review window of a stacked branch must show its stack from the baseline as segments: the branch on its parent
  first, Shift+click extends the part, "From Root" shows what its Safe Push lands and Patronus checks ([Branch Review](branch-review.spec.md#the-stack-and-the-commits)).
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
  heading the stack, its rows those of the table ([Stack Island](stack-island.spec.md)); the rules below are of a stack
  in the table, and an island collapses by the same `collapsedStacks`.
- The root row of a stack must head it, so a stack of 22 branches doesn't read as 22 rows. A stack is the tree of the
  branches built on a root; what a branch's ancestors are to it is "built on", and a Safe Push "lands … with it". After
  the root's name a chip sizes the stack, "22 branches", and the tooltip says the rest: "Stack of vm-lane-daemon · 22
  branches · 152 commits · 3 your step · 18 waiting · Collapse Stack ⌥←" (`StackChip`): where the branches stacked on
  the root are on their way into master. A click on it, ⌥← from any row of the stack
  (the focus moves to the root) and "Collapse Stack" of the branch menu collapse the stack into its root; the click,
  ⌥→ and "Expand Stack" expand it. A held key collapses once. A collapsed root has no line down from its node, a short
  line fading out below it instead (`lane-collapsed`), and its chip adds the strongest state the hidden rows hold, by the
  nodes of their rows (`summarizeStackState`, [Landing](../../docs/design/landing.md)), in the colour of the node, the
  user's moves first: "· 1 to fix" (`destructive`: a conflict, an error, a robot that failed on the branch as it is,
  changes requested), "· 2 your step" (`primary`), "· 3 ready" (`success`), then "· 4 landing" with the ring of the
  robot, "· 2 waiting" (muted: reviewers, the parent, a robot that holds it) and "· 1 in master" (`info`). They are the
  words of the landing overview and of the filter chips; a push is no state of its own. A robot of an earlier state,
  fixed since, makes no fix: the row offers a new Safe Push. Stacks are expanded by default; a collapsed one
  is remembered per repository (`collapsedStacks`, by root; a rename of the root keeps it, a stack on another branch or
  a merged root leaves the list), and the table keeps the lane column of the expanded stacks, so no name moves.
  Collapsing closes the inline form of a hidden row; a reveal of a hidden branch, or an inline form opened for one
  elsewhere, expands its stack. The header counters, the landing overview (a stack is one row there, its root's) and Your Turn count every branch.
  [@test] ../../src/stores/branch-table-rows.test.ts (`collapses a stack to its root and keeps the lane count of the expanded table`, `marks no root collapsed whose stack has no other row, and finds the root from any row of a stack`, `keeps only the collapsed roots that still head a stack`)
  [@test] ../../src/lib/stack-state.test.ts (`counts the rows of a stack by their nodes, and nothing of a branch without one`, `puts the user's moves first, a fix before a step before ready, then what lands, waits and is in master, in the colours of the nodes`)
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
  Expanded, an alert says "Built on master: login landed." (`info` for a landed parent, `warning` for one without
  commits, likely a typo, neutral for an unapplied one) with an "Unstack from login" link: the undoable change of the
  branch menu, its very item, the only restack, by the user. While it runs, the link is disabled with a spinner and
  says "Unstacking oauth…"; a second click runs nothing. It names no pull request base: every pull request goes into the baseline,
  and the push retargets an open one into the branch of login ([Pull requests](#pull-requests)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`, `should unstack once from the alert, its spinner on the button until the sync shows the branch`)
  [@test] ../../src/lib/stack.test.ts (`says why the branch isn't built on its parent`, `is passed only when the branch is built elsewhere`)
- A branch with a stack problem must show the label of the problem as its `destructive` status badge ("conflicting
  parents", "parent cycle", "its own parent", "case clash", "empty prefix level"; never "internal error"), its message
  on hover, and keep its commits in the row and the preview, the commits to reword marked "Error". Expanded, and in
  the preview in place of "Sync failed", a `destructive` alert says the message, no `bd` command, with a link per fix
  (`StackProblemAlert`): "Stack On ci-traces", "Unstack" ("Unstack from lane-b" of a branch with a parent), "Rename…",
  each the very item of the branch menu, with its spinner while it runs; in the preview a fix closes the preview
  first. A reword is the commit's "Reword…". The branch menu offers the same fixes enabled: "Stack On…" lists the
  parents of the fixes (the stack parent too), "Unstack" a branch the problem left without a parent; every other
  rewrite, Unapply, Review Changes and Safe Push say the reason ("publish-viewer has conflicting parents") instead of
  "The branch failed to sync"; a drag takes the fixes too (`stackOnDrop`, `unstackDrop`).
  [@test] ../../tests/e2e/stacked-branches/stack-problems.spec.ts (`should name conflicting parents, keep the commits and stack on a parent from the alert`, `should offer the fixes in the menu and block the other rewrites with the problem`, `should unstack a branch with conflicting parents from the menu`, `should cut a branch out of a parent cycle from its alert`, `should open the rename of a branch whose name differs from another only in case`, `should show the problem with its fixes in the preview, a fix closing the preview`)
  [@test] ../../src/lib/branch-drop.test.ts (`blocks the rewrites of a branch with a stack problem by the reason Rust gives, and runs the fixes it offers`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_rewrite_needs_commits_a_build_and_no_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest`)
- A blocked branch must show a muted `blocked by <parent>` status after a `warning` dot with the reason as its tooltip,
  a disabled push button (in the row and the preview), no share in a push of the stack (`domain_policy::push::push_plan`
  leaves a branch the last sync did not build out, `Kept::NotBuilt`), and, expanded, a `warning` alert with a link that
  reveals the parent.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)
- Where the stack graph isn't visible, a stacked branch must say what it is built on ("on auth", "on master" while its
  parent has no commits): the branch preview; the landing overview shows the stack as one row, its root's. The branch row says it only for a branch built
  elsewhere ("on master"); the graph shows the parent.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)
- A push of a stacked branch must count only its own commits as unpushed afterwards (`parentBranch`).

## Testing / Local Run

- `pnpm test:backend`: `crates/history-rewrite/src/stack_test.rs`, `stack_sync_test.rs`, `commit_grouper_test.rs`,
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
