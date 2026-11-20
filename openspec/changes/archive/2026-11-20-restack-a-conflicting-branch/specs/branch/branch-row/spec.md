## MODIFIED Requirements

### Requirement: Branch menu: The menu of a branch that can't change keeps every item

The menu of a branch that can't change now (no commits, a sync error) MUST keep every item: the rewrites are disabled
with the reason, as Rust decides them, except the fixes of a stack problem, which run while it blocks the rest
([Stacked Branches](../stacked-branches/spec.md#ui)); what rewrites nothing on the branch stays as it is. A merge
conflict is a grouping the copies don't apply in, so it holds back only Unapply among the rewrites, which takes the
built virtual branch: the rewrites of the original commits are its way out.

#### Scenario: The rewrites of a branch that can't change
- **WHEN** the menu of a branch without commits or with a sync error opens
- **THEN** the rewrites (Rename…, Move
  to…, Ungroup, Stack On…, Unstack, Add Issue Reference…, Amend Changes…, Unapply…, Squash Fixups…) are disabled with
  the reason ("The branch failed to sync", the reason of a stack problem, "The branch has no commits")
- **AND** the fixes of a stack problem run while it blocks the rest
  [@test] crates/domain-policy/src/actions_test.rs (`a_rewrite_needs_commits_and_no_error_of_the_sync_never_a_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest`)

#### Scenario: The rewrites of a branch whose sync conflicts
- **WHEN** the menu of a branch with a merge conflict opens
- **THEN** Rename…, Move to…, Ungroup, Stack On…, Unstack, Add Issue Reference…, Amend Changes… and Squash Fixups… are
  enabled, and the sync each asks builds the branch again
- **AND** Unapply…, Review Changes and Push Branch are disabled with "The branch failed to sync"
  [@test] crates/domain-policy/src/actions_test.rs (`a_merge_conflict_holds_back_only_what_reads_the_build_and_the_rewrites_of_the_original_commits_run`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a branch whose sync conflicts on the branch it needs from the menu and by a drop on its Stack on chip, the conflict gone`)

#### Scenario: What rewrites nothing on the branch
- **WHEN** the menu of such a branch opens
- **THEN** Review Changes, the pull request and its review request, Push Branch and Push with Children (they wait for the sync themselves), Safe Push, Safe Push With Profile… and Dry Run (disabled with the reasons of the [next step](../next-step/spec.md)), the robot's actions stay as they are
  [@test] crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)
