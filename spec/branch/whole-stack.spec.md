---
name: Whole Stack
description: Requirements for the whole stack of stacked branches that fork, as one more virtual branch `{prefix}/stack/<root>` that the sync builds of all their commits in the order of the history, without notes or branch events, remembered by its commits, its state (built, blocked, conflict, stale) and remote status reported per sync, deleted once the stack no longer forks unless it is on the remote, never pushed unless built, taken along by Push with Children of the root once it is on the remote or its pull request is open, and landed in one robot by `bd safe-push --whole-stack` from its own pull request, the robots of the stack and of its branches waiting for each other; in the app the whole stack is an object of its own, "the whole air stack": its robot (polled, told, holding every branch, its branches "Landed with the whole air stack" after its merge until the update, its prompt with the commits of every branch), its pull request (created, opened, closed by content after its merge, one that no longer matches it asked before it is closed), its review and one list of its actions in the island's menu and the root's; and its Safe Push and Dry Run in the app, from the island's header (its slot, its "⋯", S) or the root's menu of a small stack, its popover saying what lands, only its stack branch pushed, its readiness and approval news of its own pull request, "In #12" for a branch without one, a stack branch not built disabling it with the way to land it in pieces.
style: plain-1
targets:
  - ../../crates/sync-core/src/stack_branch.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/branch_processor.rs
  - ../../crates/domain-types/src/stack.rs
  - ../../crates/domain-types/src/lib.rs
  - ../../crates/domain-types/src/branch_names.rs
  - ../../crates/git-ops/src/tree_id_cache.rs
  - ../../crates/object-cache/src/stack_branch.rs
  - ../../crates/branch-integration/src/common.rs
  - ../../crates/bd-core/src/stack_summary.rs
  - ../../crates/domain-policy/src/stack.rs
  - ../../crates/domain-policy/src/push.rs
  - ../../crates/repository-model/src/derive.rs
  - ../../crates/bd-remote/src/status.rs
  - ../../crates/bd-core/src/sync_command.rs
  - ../../crates/bd-remote/src/safe_push_command.rs
  - ../../crates/bd-remote/src/push_command.rs
  - ../../crates/bd-remote/src/push_refusals.rs
  - ../../crates/domain-policy/src/holds.rs
  - ../../crates/domain-policy/src/land.rs
  - ../../crates/domain-policy/src/actions.rs
  - ../../crates/bd-remote/src/robot_command.rs
  - ../../crates/patronus/src/service.rs
  - ../../crates/sync-core/src/patronus_start.rs
  - ../../crates/domain-policy/src/robot_match.rs
  - ../../crates/domain-policy/src/attention.rs
  - ../../crates/remote-facts/src/pull_requests.rs
  - ../../crates/remote-facts/src/pull_request_create.rs
  - ../../crates/remote-facts/src/review_readiness.rs
  - ../../crates/remote-facts/src/stack_pull_request.rs
  - ../../crates/domain-types/src/patronus.rs
  - ../../crates/domain-types/src/pull_request.rs
  - ../../src-tauri/src/commands/patronus.rs
  - ../../src/stores/branch-sync.ts
  - ../../src/stores/repository.ts
  - ../../src/lib/patronus.ts
  - ../../src/lib/whole-stack.ts
  - ../../src/hooks/useWholeStackActions.ts
  - ../../src/stores/patronus.ts
  - ../../src/stores/pull-requests.ts
  - ../../src/stores/landed-stacks.ts
  - ../../crates/operations/src/close.rs
  - ../../crates/domain-policy/src/landed.rs
  - ../../src/lib/robot-run.ts
  - ../../src/presenters/actions.ts
  - ../../src/lib/patronus-failing.ts
  - ../../src/lib/robot-overview.ts
  - ../../src/lib/robot-prompt.ts
  - ../../src/lib/robot-prompt-copy.ts
  - ../../src/lib/push-run.ts
  - ../../src/lib/git/push-toast.tsx
  - ../../src/lib/stack.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/presenters/push-button.ts
  - ../../src/hooks/useBranchPushes.ts
  - ../../src/lib/pull-request-dialog.ts
  - ../../src/lib/pull-request-run.ts
  - ../../src/hooks/useCreatePullRequestContext.ts
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../src/components/branch/InlineCloseStackPullRequest.tsx
  - ../../src/components/branch/OrphanedStackPullRequest.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/lib/stack-state.ts
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/branch/LandedChip.tsx
  - ../../src/components/repository/RobotRows.tsx
  - ../../src/lib/branch-review-window.ts
  - ../../src/hooks/useStackNextStep.ts
  - ../../crates/domain-policy/src/next_step.rs
  - ../../crates/domain-policy/src/landing_node.rs
  - ../../src/presenters/next-step.ts
  - ../../src/hooks/useLandingNode.ts
  - ../../crates/sync-core/src/robot_landing.rs
  - ../../src/hooks/useBranchNextStep.ts
  - ../../src/hooks/useSafePush.ts
  - ../../crates/operations/src/land_execute.rs
  - ../../crates/operations/src/land_session.rs
  - ../../src/presenters/land.ts
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/components/branch/WholeStackSlot.tsx
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/RequestReviewForm.tsx
  - ../../src/stores/review-readiness.ts
  - ../../src/lib/approval-news.ts
  - ../../crates/test-utils/src/repo_template.rs
  - ../../crates/sync-core/src/push.rs
  - ../../crates/sync-core/src/robot_commits.rs
  - ../../crates/sync-core/src/branch_review.rs
  - ../../src-tauri/src/commands/pull_requests.rs
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/BranchActions.tsx
---

# Whole Stack

Status: Implemented
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
- Mapping notes of the stack branch's copies, or a row of it among the branches: the commit map of its robot matches
  the commits by their identity.
- A Safe Push of the whole stack without review: a profile that skips the code-owner review is a choice of the
  popover, but the "Create pull request" dialog of the stack offers no start without review.
- Closing a pull request of the whole stack by itself: one that no longer matches the stack is the user's to close.

## Requirements

### The stack branch

- With stacked branches on, every stack that forks (a branch of it has two children or more in it) must also be built
  as the virtual branch `{prefix}/stack/<root>` (`to_stack_branch_name`), stacks inside an issue family included; a
  straight chain has none. The stack is the root and the branches built on it (`effective_base`, the parent that
  blocks a blocked branch, the declared parent of a branch with a stack problem); a branch whose parents lead into a
  cycle is in no stack.
  [@test] ../../crates/sync-core/src/stack_branch_test.rs (`a_stack_forks_where_a_branch_has_two_children_and_takes_its_commits_in_history_order`, `a_fork_below_the_root_and_two_stacks`, `a_straight_chain_and_a_lone_branch_have_none`, `a_member_with_a_problem_blocks_the_stack_and_a_cycle_is_in_none`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_straight_stack_has_none`)
  [@test] ../../crates/domain-types/src/branch_names_test.rs (`the_stack_branch_of_a_root_is_in_its_own_namespace`)
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
- Its remote status must count the commits of the most frequent author of the commits of all its members as the
  user's, of a tie the author seen first in history; the status a push of it leaves counts the same ones, whoever
  authored its root.
  [@test] ../../crates/sync-core/src/stack_branch_test.rs (`the_email_of_a_stack_branch_is_its_most_frequent_author`)
  [@test] ../../crates/operations/src/push_test.rs (`a_push_of_a_stack_branch_counts_the_commits_of_its_members_author_as_the_sync_does`)
- A stack branch whose stack no longer forks (or whose root is gone, or with stacked branches off) must be deleted in
  one transaction with compare-and-swap, but only while it has no remote branch and no worktree has it checked out;
  one with a remote branch stays, `Stale` without members, for the user to decide on.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_stack_that_no_longer_forks_loses_its_stack_branch_unless_it_is_on_the_remote`)
- The archive step and the integration detection must leave the stack branches out: never archived, and a stack tip
  never names the detection note of an archived branch.
  [@test] ../../crates/sync-core/src/detection_archive_step_test.rs (`branches_are_classified_by_their_namespace`)
- `bd sync` and `bd status` must list them as `stacks`: `root`, `branch`, `state` (`built`, `blocked`, `conflict`,
  `stale`), `tip`, `member` and `reason` of a block, `conflict`, `error`, `commits`, `members`, and with `bd sync` the
  `remote` summary. `bd status` reads the state without a sync and without a git process: blocked by the placement,
  else the remembered build of the current commits when the ref is at it, else stale.
  [@test] ../../crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_status_of_a_forking_stack`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)
- The app must keep the stack branches of the last sync by root (`useStackBranch`, `getStackBranch`; the full name by
  `getFullStackBranchName`): a grouping keeps the ones it names as they were until their update and drops the others
  (the session's table in Rust, whose patches the window applies by root).
  [@test] ../../crates/repository-model/src/table_test.rs (`the_grouping_keeps_the_announced_stack_branches_and_the_baseline_status_fills_its_section`)
  [@test] ../../src/model/store.test.ts (`keeps the stack branches the patches report until one changes or removes them`)

### The Safe Push of the whole stack

- The whole stack of a branch must be the stack of its root by declared parent, for the app and `bd` alike
  (`domain_policy::stack::whole_stack_of`, D8, the owner's decision: the stack as the user declared it; the parent that
  blocks a branch is a detail of its build): the stack branch of the root while the stack forks, else the straight
  chain from the root, which its last branch that is not parked lands (a parked one at its end stays local). A blocked
  branch whose declared parent has no commits heads a whole stack of its own, never the chain of the branch that blocks
  it; a fork no stack branch is known of yet waits for the sync that builds it (`bd` refuses it as `stack_not_built`).
  The view places the whole stack on the structure of its root (`BranchStructure.wholeStack`), and what its Safe Push
  lands, the branches as a sentence names them (the main line first) less those merged Safe Pushes landed whole, on its
  landing (`StackLanding.rest`, `whole_stack_rest`).
  [@test] ../../crates/domain-policy/src/stack_test.rs (`the_whole_stack_of_a_branch_is_the_stack_of_its_declared_root`, `the_whole_stack_of_a_blocked_branch_follows_its_declared_parent_not_what_blocks_it`, `the_members_of_a_whole_stack_name_its_main_line_first`, `the_rest_of_a_whole_stack_is_what_merged_safe_pushes_left_of_it`)
  [@test] ../../crates/repository-model/src/derive_test.rs (`the_root_of_a_stack_carries_its_whole_stack`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)
- `bd safe-push <branch> --whole-stack` must land the whole stack of the branch in one robot from its stack branch,
  the only branch it pushes; a straight chain from its last branch; refused unless the stack branch is `built`
  (`conflict`, `stack_not_built`, exit 1), and both ways while a robot of the stack or of a branch of it runs
  ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)). Push with Children of the root, the app's and `bd push <root>
  --with-children`, must take the stack branch along after the branches once it is on the remote or its pull request
  is open, only when the sync built it, so its pull request follows the branches
  ([Stacked Branches](stacked-branches.spec.md#pull-requests)), by one rule (`domain_policy::push::stack_branch_along`:
  not while its push is held, nor while its remote has commits it lacks, which `bd` pushes over only with `--force`);
  `bd robot --whole-stack` must print its robot.
  [@test] ../../crates/domain-policy/src/push_test.rs (`the_stack_branch_goes_along_once_published_built_not_held_and_behind`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`, `the_whole_stack_of_a_straight_chain_lands_from_its_last_branch`, `a_whole_stack_that_conflicts_is_refused_with_the_conflict`, `push_with_children_takes_the_stack_branch_along_while_its_pull_request_is_open`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`with_children_takes_the_stack_branch_along_once_it_is_on_the_remote`)
  [@test] ../../crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
- A robot started on a stack branch must name the root of its stack (`PatronusRobot::stack_root`) and no branch, and
  must belong to no branch, not even by its revision, in `bd` and the session's view, whose `robotId` the app reads
  (`RobotIndex::branch_robot` of `domain_policy::robot_match`); `RobotIndex::stack_robot` finds the robot of the whole
  stack.
  [@test] ../../crates/patronus/src/service_test.rs (`a_robot_of_the_stack_branch_names_the_root_of_its_stack_and_no_branch`)
  [@test] ../../crates/domain-policy/src/robot_match_test.rs (`the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`the_robot_of_the_whole_stack_lands_every_branch_of_it`)
  [@test] ../../src/stores/patronus.test.ts (`holds every branch the whole-stack Safe Push lands`)
- The pull request of a stack branch must be the stack's, by its root (`PullRequestsResult::stack_pull_requests`),
  never a branch's; one that GitHub says exists must be found by its head.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`the_pull_request_of_a_stack_branch_is_the_stacks_by_its_root`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_pull_request_of_the_whole_stack_is_its_stack_branchs_and_the_start_comments_on_it`)
- Its text must be the stack branch's own description, else the root's title with the count of the branches built on
  it and a section per branch, its description or the subjects of its commits, from what the sync read, cut to the
  65,536 characters of a body and the 256 of a title GitHub takes, naming the branches left out.
  [@test] ../../crates/remote-facts/src/stack_pull_request_test.rs (`the_pull_request_of_the_whole_stack_names_each_member_by_its_description_else_its_commits`, `a_text_too_long_for_github_is_cut_and_names_the_members_left_out`)
- The readiness of the whole stack must ask Patronus about its stack branch (`ReviewReadinessBranch::source`, a stack
  branch of the prefix, else refused), the answer under the key the caller gives; the start of the app is the
  `Land` of the whole stack (`Subject::WholeStack`), whose robot starts from the stack branch.
  [@test] ../../crates/remote-facts/src/review_readiness_test.rs (`the_whole_stack_is_asked_as_its_stack_branch_under_the_key_of_the_caller`)

### The whole stack in the app

- The whole stack is named "the whole air stack" in a sentence and "Whole air stack" at its start, as `bd` names it
  (`formatWholeStack`, `formatRobotSubject`), wherever the app names the robot or the pull request of a stack branch.
  [@test] ../../src/lib/whole-stack.test.ts (`is named as bd names it`)
  [@test] ../../src/lib/patronus.test.ts (`is named the whole stack, at the start of a sentence too`)
- The robot of the whole stack must be the root's stack's (`robotId` of its stack in the session's view,
  `RobotIndex::stack_robot`: started on its stack branch, or a Safe Push of a temporary ref on the tip of the stack
  branch), polled while it runs, its problems and its failing read, as
  the robot of a branch is; it is told when it merged ("Whole air stack merged into master", in the toast, Activity and
  the OS notification), failed ("Safe Push of the whole air stack failed", its pull request the step when it needs
  review) or will fail, listed in the landing overview as "Whole air stack", a click showing the root, and counted in
  the header of its island as the root's ("1 running" with its ring, [Stack Island](stack-island.spec.md#header)).
  Its failure asks for the user (the landing overview's attention, "1 failed" of the island, the Dock badge) only on a
  stack branch the sync built (`domain_policy::attention::stack_attention`, the view's `attention` of `stack:<root>`):
  on one not rebuilt (`Stale`, kept for its remote branch once the stack no longer forks; `Blocked`; `Conflict`) its
  restart is refused and the stack lands in pieces, so the overview lists it as finished, and the island and the badge
  count nothing for it.
  [@test] ../../tests/e2e/patronus/dock.spec.ts (`should count the failed robot of the whole stack only while the sync built its stack branch`)
  [@test] ../../crates/domain-policy/src/robot_match_test.rs (`the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should count the robot of the whole stack in the header of its island, a click on it showing the root`)
  [@test] ../../src/stores/patronus.test.ts (`closes the pull requests of the branches the whole stack landed`)
  [@test] ../../crates/domain-policy/src/attention_test.rs (`the_failed_robot_of_a_whole_stack_needs_the_user_only_on_a_built_stack_branch_no_robot_of_a_branch_lands`)
  [@test] ../../src/lib/robot-overview.test.ts (`lists the robot of the whole stack as the whole stack after the branches, a click showing the root, its failure as the move the view names, any other as finished`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should show the robot of the whole stack on the rows of a small stack, hold their pushes and cancel it from the root's menu`)
- While it runs and pushes into the target, it must hold every branch of the stack its commit has (`PushHold.wholeStack`,
  by the source ref: its stack branch, not by a branch's revision; after a change of the stack the content tells,
  [Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)) and land them: their rows say "lands with the
  whole air stack", their push and their Safe Push wait ("A Safe Push of the whole air stack is running: it lands 1
  commit of auth with it");
  the robot of a branch of the stack holds the whole stack's restart and start in turn ("A Safe Push of auth is
  running: wait until it merges", the `landingMember` of the view; the start refusal is the rule of Rust,
  `domain_policy::land::start_refusal`, the view's `landing.start` of the stack, after a stack branch the sync did not
  build and the robot on its commit). A dry run holds nothing.
  [@test] ../../src/stores/patronus.test.ts (`holds every branch the whole-stack Safe Push lands`, `holds nothing for a dry run of the whole stack`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_whole_stack_waits_for_its_build_its_last_commit_then_a_member_that_lands_a_part`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`the_robot_of_the_whole_stack_lands_every_branch_of_it`)
- Its actions are the actions of a robot (`getRobotActionItems`, as Rust offers them): the restart works on the stack branch, and pushes it first
  while it has work its remote lacks (the push step of `RestartRobot`, a running robot canceled first), only while the sync built it;
  cancel, the prompts, Patronus, its pull request. Once merged Safe Pushes landed every branch of the stack, its restart
  is refused, with or without the push, of a Dry Run too, with the reason its Safe Push Whole Stack says ("Landed in
  master with deploy and side-test", `getStackLandedReason`), on every surface that lists the actions (the "⋯" of the
  island, "Whole Stack ›" of the menus of its branches); the header offers no step, and the landing overview lists the stack in
  master without its robot.
  [@test] ../../src/presenters/robot-actions.test.ts (`names what a restart of a robot of a branch changed since does: it cancels a running one, then pushes`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should run the restart of the robot of the whole stack once while its push runs: one push, one restart, one toast`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should refuse the restart of the failed robot of the whole stack once Safe Pushes of its branches landed every branch of it, on every surface that offers it`)
- A push of the stack branch alone (the push step of the restart, of Request Review… of its pull request, of the
  creation of its pull request and of the start of its Safe Push; the "Push" of a refused start of the whole stack, the
  intent `Push` of the subject `wholeStack`, `pushWholeStack`) must show where the pushes of the stack show, one indicator: the push of
  the header of its island ([Stack Island](stack-island.spec.md#header)), else the push button of the root's row of a
  small stack, "Pushing the whole stack 45%" with the line of git on hover, the branches of the stack not busy; the
  start of its Safe Push in its own button (the island's header, the root's slot of a small stack: the spinner and
  "45%", named "Pushing 45%"), the
  dialog of its pull request in its Create button ("Pushing 45%"); one hook decides which (`useStackBranchPushAlone`). One push of it at a time: a second request while one
  runs (another action's, or Push with Children that takes it along) waits for it in the registry of the session (its
  ref), no push of its own, one indicator; the same request joins it; then the request pushes only what the remote
  still lacks (what a sync rebuilt since; the stack branch alone after a Push with Children that failed for a
  branch), nothing when the remote has it all. A second restart of the same robot is the same request, one operation of
  the robot whose item runs meanwhile: one push, one restart, one toast. Push with Children waits for a push of it alone and takes it along only while the remote still
  lacks it. Once merged Safe Pushes landed every branch of the stack, it carries only commits that are in the target:
  its push is refused ("Cannot push the whole air stack", "Landed in master with deploy and side-test: nothing of it is
  left to push"), Push with Children leaves it, and "Create Pull Request of the Stack…" is disabled with the reason
  ([Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)).
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`, `should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it, the stack waiting from the click`, `should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should run the restart of the robot of the whole stack once while its push runs: one push, one restart, one toast`)
  [@test] ../../crates/operations/src/push_test.rs (`the_stack_branch_of_a_whole_stack_is_pushed_alone_and_goes_along_once_on_the_remote`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
- The prompt for an agent of its failure must name the stack branch and the commits of every branch of the stack, each
  with its branch, by identity (`stack_robot_commit_map`: no mapping notes), a commit of a member the stack branch
  lacks said as not synced; `bd robot --whole-stack` prints the same map.
  [@test] ../../crates/sync-core/src/robot_commits_test.rs (`a_robot_of_the_whole_stack_lists_the_commits_of_every_member_with_its_branch`)
  [@test] ../../src/lib/robot-prompt.test.ts (`names the stack branch and every branch of the whole stack for its robot: it built them all`)
  [@test] ../../crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)
- When it merged, the open pull requests of every branch of the stack must be closed with a comment ("Landed in
  master with #12, the Safe Push of the whole air stack (user/stack/air), which holds this branch."), and the toast
  says it ("Safe Push · closes the pull requests of 6 branches: they landed with the whole stack"); while it ran on the
  stack branch as it is, else by the commits it tested against each branch (a change of a branch since rebuilt the
  stack branch: the repository view by content, which the news of the merge waits for), and only a pull request that
  holds nothing its branch lacks: its head is the branch's tip, or the remote branch has no commit the branch lacks;
  never the pull request the robot started from (`domain_policy::landed::pull_requests_to_close`,
  [Patronus](patronus.spec.md#merged), the same for a piece).
  [@test] ../../crates/domain-policy/src/landed_test.rs (`leaves_the_pull_request_the_robot_started_from_to_patronus`)
  [@test] ../../src/stores/patronus.test.ts (`closes the pull requests of the branches the whole stack landed`)
- Until the update archives them, every branch of the stack it landed (under the same conditions, also for a robot
  first seen merged) must say so in its row, "Landed with the whole air stack" ([Patronus](patronus.spec.md#merged)),
  and offer no Safe Push, no Dry Run and no push: "Landed in master with the whole air stack"
  ([Next Step](next-step.spec.md#the-table), row 1); the "to push" counts of the header, the island and the filter chip
  leave them out. Any change of a branch of the stack rebuilds the stack branch, so no hash is the robot's any more:
  what it landed is then read by content ([Patronus](patronus.spec.md#merged)), a commit added to a branch since the
  only work left of it ("Landed with the whole air stack · 1 commit after it").
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`the_robot_of_the_whole_stack_landed_every_branch_of_it`)
  [@test] ../../src/lib/patronus.test.ts (`names who landed a branch in its row and why it starts nothing: a branch built on it, the whole stack, its own robot with the work left`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`)
  [@test] ../../src/presenters/reasons.test.ts (`says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer no Safe Push or push of them`)
- One builder must list the actions of the whole stack (`getWholeStackMenuItems`), which the "⋯" of the stack island
  renders, the menu of every branch of the stack under "Whole Stack ›", the root's whole stack (so their previews
  too), and the title bar and "⋯" of the review of the whole stack ([Branch Review](branch-review.spec.md#title-bar-footer-and-actions)): every branch is a piece of it, for a small stack without an island as for an island: "Safe Push Whole Stack" and "Dry Run Whole Stack" ([below](#the-safe-push-of-the-whole-stack-in-the-app)),
  "Open Pull Request #12" of its pull request and, while it is open and matches the stack, "Request Review…" of it (the
  step of the whole stack when it is the one, else `getRequestReviewItem` with the code owners of the readiness of
  `stack:<root>`; its popover at the header's slot of the island, else at an anchor of the root's slot, also from the
  root's preview, which closes; its stack branch pushed first), else "Create Pull Request of the Stack…" (disabled
  with the reason while the sync didn't build the stack branch, `WholeStackView.forking.notBuilt`, and once every branch of the
  stack landed, `getStackLandedReason`), Push with Children (the
  island's; the root's menu has it beside its push), "Review Whole Stack" ([Branch Review](branch-review.spec.md#opening))
  and the actions of its robot. An open "⋯" of the island follows what the list reads: the pull requests that load
  after the sync change its items while it is open (`getStackIslandMenuNow`).
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should follow the pull requests in the island's ⋯ while it is open: they load after the sync`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`is the submenu of the root's whole stack on every branch of the stack, none on a branch of no stack`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`, `should review the whole stack in the window of its stack branch, from the island's menu, the root's menu and the menu of a branch of it`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should ask for a review of the pull request of a small stack from Whole Stack › of the root's menu, its row's ⋯ and its preview, the popover at the root's row`, `should ask for a review of the pull request of the whole stack from the island's ⋯ and the root's menu, the popover at the header`, `should open Request Review… of the whole stack at the header while its Safe Push runs and the header offers no step`)
- "Create Pull Request of the Stack…" must open the "Create pull request" dialog of the stack branch into the baseline
  ("Whole air stack → master", "Holds the commits of every branch of the stack: air, bazel, bridge and 4 more: 7
  branches · 14 commits"), its text from `stack_pull_request_text` (`get_new_stack_pull_request`: the descriptions in
  one `git config`, the subjects as the rows show them); Create pushes the stack branch alone (the push step of the
  intent `CreatePullRequest`, leased on its remote head), then creates the pull request of its head. It asks the code owners of the
  whole stack to review, as the dialog of a branch asks the branch's: those Patronus suggests for the stack branch (the
  readiness of `stack:<root>`), preselected, while the sync built it.
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`, `should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does, the move made from the click`)
  [@test] ../../crates/remote-facts/src/process_budget_test.rs (`the_text_of_a_new_pull_request_of_the_whole_stack`)
  [@test] ../../crates/sync-core/src/push_test.rs (`the_whole_stack_pushes_its_stack_branch`)
- A pull request of the whole stack that no longer matches it (a Safe Push of a branch landed the root in a piece, as
  the app saw the merge; or the stack no longer forks and its stack branch stays for its remote branch) must never be
  closed by itself, and its stack branch stays: the merge toast, the root's preview, the island's header and the
  description of its items say "#12 no longer matches the air stack: auth landed", and "Close Pull Request #12…" (the
  preview's notice, the island's header, "⋯") asks in the root's row before it closes it with a comment that says why.
  A new pull request of the whole stack matches it again: the landed root is forgotten in the repository it was created
  in, also after a switch during its push, never the root of the same name in another one.
  [@test] ../../src/lib/whole-stack.test.ts (`has a pull request that no longer matches it once a piece landed the root or the stack no longer forks, an open one only`)
  [@test] ../../src/stores/landed-stacks.test.ts (`forgets the root in the repository the pull request was created in, also after a switch`)
  [@test] ../../src/stores/patronus.test.ts (`says that a piece that landed the root leaves the pull request of the whole stack, never closed by itself`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say that the pull request of the whole stack no longer matches it once a piece landed the root, and close it only when asked`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should offer the close of the pull request of the whole stack in the island header once it no longer matches`)

### The Safe Push of the whole stack in the app

- The app must start the Safe Push and the Dry Run of the whole stack as the start of a branch is
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)), its subject the key `stack:<root>`
  (`requestSafePush`, `readSafePush`; the decision of `domain_policy::land::decide`, `RepositoryView.lands` under the key): its facts are the stack branch, the robot
  of the whole stack and the profile of its last one, its pull request and the readiness of the whole stack. It lands
  more than a row, so it always opens its popover ("Safe Push of the whole air stack"): what lands ("Lands 7 branches ·
  14 commits into master in one Safe Push: air, bazel, … side-fix and side-test.", the main line first, then the side
  branches, `whole_stack_members`), "Pushes only its stack branch user-name/stack/air.", and what stays only of the
  branches on the remote ("…: the branches of the stack stay as they are on the remote.", "…: air and bazel stay as
  they are on the remote.", `describeStackBranchPush`), with its pull request open and its approvals needed
  "Approvals count in #12, the pull request of the whole stack." (never beside "No code-owner review needed."), the
  profile, the review, the robot of the whole stack it cancels; Dry Run, Cancel and the start (Enter).
  The start (`execute` of the `Land` of the whole stack) checks its landing gate first (the landing needs of the view
  are of branches: the engine runs the gate of the whole stack, `operations::land_session`), pushes the stack branch
  alone while its remote lacks it (leased, a robot it replaces canceled first), then starts the robot from it, or
  comments on its pull request. A straight chain, which has no stack branch, starts its last branch.
  [@test] ../../tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say in the popover of the whole stack only what holds: the branches on the remote stay, no approvals where none are needed`, `should say in the popover of the whole stack that its approvals count in its pull request while Patronus judges them`)
  [@test] ../../src/lib/whole-stack.test.ts (`pushes only its stack branch for its Safe Push, and says that its branches stay only of those on the remote`)
- It must be asked from the header of an island (its slot, "Safe Push Whole Stack" and "Dry Run Whole Stack" of its
  "⋯", S on the focused header), the popover at the header's slot ([Stack Island](stack-island.spec.md#header)),
  from "Whole Stack ›" of the root's menu, and from the Safe Push popover of the root and of any branch of a stack that
  forks ("Land the whole stack: Safe Push Whole Stack", [Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)),
  the popover at the header of the root's island, else at the root's row (a small stack, `getWholeStackStartItems`).
  Its steps must show where its popover opens, as the start of a branch shows them in its slot: "Checking…" while the
  facts load, the spinner and "45%" of its push, "Starting…", in the button of the header's slot, else in the root's slot of a small stack,
  in place of the root's own step while the start runs (`NextStepSlot`); the push button of the root's row stays the
  root's own meanwhile (`useStackBranchPushAlone`).
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`, `should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover`)
- After merged Safe Pushes landed branches of it (a piece that landed its main line, `getLandedBy`), it must land only
  the rest and say so: its label counts the rest ("Safe Push" for one branch left), its hover, menu item and popover
  "Lands 1 branch · 1 commit into master in one Safe Push: session; auth, login and oauth landed with oauth"
  (`StackLanding.rest`, `describeWholeStackLanding`); the stack branch still carries the landed commits, which Patronus
  finds in the target. Once every branch of it landed, nothing is left: Safe Push Whole Stack and Dry Run Whole Stack
  disabled with "Landed in master with oauth and the whole auth stack" ([Next Step](next-step.spec.md#the-whole-stack)).
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should land only what is left of the whole stack after a piece landed its main line, and offer nothing once every branch of it landed`)
- Its next step must be the table of [Next Step](next-step.spec.md#the-whole-stack) on its facts, as Rust decides it
  for its stack branch (`nextSteps` of the view under `stack:<root>`, its node beside it; the page words both,
  `useStackNextStep`, `useStackLandingNode`): disabled with the
  reason while the sync didn't build its stack branch ("The stack branch conflicts at 0caf9066 in config.js: the
  stack lands in pieces", a Dry Run too), while a robot of a branch of it runs, while its approvals are missing in its
  own pull request. Its readiness is asked while the header of its island or its root's row shows
  ([Next Step](next-step.spec.md#the-readiness-store)), so a small stack's menu and popovers know the missing approvals
  before a click: "Safe Push Whole Stack" disabled with "Code owners must approve in a pull request: IJPL Code
  Platform" beside "Create Pull Request of the Stack…", and before the answer with "Waits for Patronus to check the
  approvals" ([Facts without an answer](next-step.spec.md#facts-without-an-answer)); S on the header says it in the
  notice, whose action is "Create Pull Request of the Stack…". A stack branch not built lands the stack in pieces: the "⋯" of the island offers the Safe Push of
  the last branch of its main line, and so do the notice of S and the popover ("Land it in pieces").
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask for the approvals of the whole stack of a small stack while its root's row shows, and offer its pull request where they are missing`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say on S why the whole stack can't start without its pull request, and create it from the notice`)
  [@test] ../../src/stores/branch-sync.test.ts (`is the stack that forks and holds the branch, else the straight chain the branch heads`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)
- Its approvals must count in its own pull request only: the island's header says "Approvals count in #12, the pull
  request of the whole stack", apart from the pull requests of the branches; a branch of it without a pull request of
  its own, whose approvals are missing, shows a quiet "In #12" that opens it; the popover of a piece of the stack says
  that the approvals of #12 don't count for it. Request Review… of its pull request pushes its stack branch first.
  Approvals that arrive in it are news: "Ilya Korennoy approved the whole air stack" with "Safe Push Whole Stack".
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it`)
  [@test] ../../src/lib/approval-news.test.ts (`names the whole stack for the approvals of its pull request`)
  [@test] ../../src/lib/whole-stack.test.ts (`has the readiness of its stack branch under a key no branch has, and its approvals only in its own pull request`, `lands its branches in one robot, the side branches named: they are what only it lands`)
- When its robot merged, the pull requests of its branches must be closed (above), in the app as the user sees it.
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should close the pull requests of every branch of the whole stack once its Safe Push merged`)

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
- `pnpm perf:monorepo`: the full and the unchanged `bd sync` of the pinned monorepo clone, one stack that forks.

## References

- [Landing Work into Master](../../docs/design/landing.md): why the whole stack is one more virtual branch
- [Git Operations](../../docs/design/git-operations.md): "Stack branch", the build and the ref rules
- [Stacked Branches](stacked-branches.spec.md): how branches stack
- [Stack Island](stack-island.spec.md): the header the whole stack's Safe Push is in
- [Next Step](next-step.spec.md#the-whole-stack): the step of the whole stack
