---
name: Whole Stack
description: Requirements for the whole stack of stacked branches that fork, as one more virtual branch `{prefix}/stack/<root>` that the sync builds of all their commits in the order of the history, without notes or branch events, remembered by its commits, its state (built, blocked, conflict, stale) and remote status reported per sync, deleted once the stack no longer forks unless it is on the remote, never pushed unless built, and landed in one robot by `bd safe-push --whole-stack` from its own pull request, the robots of the stack and of its branches waiting for each other; in the app the whole stack is an object of its own, "the whole air stack": its robot (polled, told, holding every branch, its prompt with the commits of every branch), its pull request (created, opened, closed by content after its merge, one that no longer matches it asked before it is closed), its review and one list of its actions in the island's menu and the root's; and its Safe Push and Dry Run in the app, from the island's header (its slot, its "⋯", S) or the root's menu of a small stack, its popover saying what lands, only its stack branch pushed, its readiness and approval news of its own pull request, "In #12" for a branch without one, a stack branch not built disabling it with the way to land it in pieces.
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
  - ../../src/lib/whole-stack.ts
  - ../../src/hooks/useWholeStackActions.ts
  - ../../src/stores/patronus.ts
  - ../../src/stores/pull-requests.ts
  - ../../src/stores/landed-stacks.ts
  - ../../src/lib/landed-stack.ts
  - ../../src/lib/patronus-actions.ts
  - ../../src/lib/patronus-failing.ts
  - ../../src/lib/robot-overview.ts
  - ../../src/lib/robot-prompt.ts
  - ../../src/lib/robot-prompt-copy.ts
  - ../../src/lib/git/stack-push.ts
  - ../../src/lib/create-pull-request-context.ts
  - ../../src/hooks/useCreatePullRequestContext.ts
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../src/components/branch/InlineCloseStackPullRequest.tsx
  - ../../src/components/branch/OrphanedStackPullRequest.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/lib/stack-state.ts
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/repository/RobotsIndicator.tsx
  - ../../src/lib/branch-review-window.ts
  - ../../src/hooks/useStackNextStep.ts
  - ../../src/hooks/useSafePush.ts
  - ../../src/lib/safe-push-start.ts
  - ../../src/lib/safe-push-decision.ts
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

### The whole stack in the app

- The whole stack is named "the whole air stack" in a sentence and "Whole air stack" at its start, as `bd` names it
  (`formatWholeStack`, `formatRobotSubject`), wherever the app names the robot or the pull request of a stack branch.
  [@test] ../../src/lib/whole-stack.test.ts (`is named as bd names it`)
  [@test] ../../src/lib/patronus.test.ts (`is named the whole stack, at the start of a sentence too`)
- The robot of the whole stack must be the root's stack's (`findStackRobot`: started on its stack branch, or a Safe
  Push of a temporary ref on the tip of the stack branch), polled while it runs, its problems and its failing read, as
  the robot of a branch is; it is told when it merged ("Whole air stack merged into master", in the toast, Activity and
  the OS notification), failed ("Safe Push of the whole air stack failed", its pull request the step when it needs
  review) or will fail, listed in the robots indicator as "Whole air stack", a click showing the root, and counted in
  the header of its island as the root's ("1 running" with its ring, [Stack Island](stack-island.spec.md#header)).
  [@test] ../../src/lib/patronus.test.ts (`belongs to the root of its stack by its stack branch, or as a robot of a temporary ref by the tips of the stack branch`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should count the robot of the whole stack in the header of its island, a click on it showing the root`)
  [@test] ../../src/stores/patronus.test.ts (`closes the pull requests of the branches the whole stack landed`)
  [@test] ../../src/lib/robot-overview.test.ts (`lists the robot of the whole stack as the whole stack after the branches, on its stack branch as it is, a click showing the root`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should show the robot of the whole stack on the rows of a small stack, hold their pushes and cancel it from the root's menu`)
- While it runs and pushes into the target, it must hold every branch of the stack (`PushHold.wholeStack`, by the
  source ref: its stack branch, not by a branch's revision) and land them: their rows say "lands with the whole air
  stack", their push and their Safe Push wait ("A Safe Push of the whole air stack is running: it lands auth with it");
  the robot of a branch of the stack holds the whole stack's restart and start in turn ("A Safe Push of auth is
  running: wait until it merges", `findLandingMember`). A dry run holds nothing.
  [@test] ../../src/stores/patronus.test.ts (`holds every branch the whole-stack Safe Push lands`, `holds nothing for a dry run of the whole stack`)
  [@test] ../../src/lib/patronus.test.ts (`lands every branch of the stack, and waits for the robot of a branch of it, as that one waits for it`)
- Its actions are the actions of a robot (`getRobotActions`): the restart works on the stack branch, and pushes it first
  while it has work its remote lacks (`pushWholeStack`, a running robot canceled first), only while the sync built it;
  cancel, the prompts, Patronus, its pull request.
  [@test] ../../src/lib/patronus.test.ts (`restarts on its stack branch, pushed first while the stack branch has work the remote lacks`)
- The prompt for an agent of its failure must name the stack branch and the commits of every branch of the stack, each
  with its branch, by identity (`stack_robot_commit_map`: no mapping notes), a commit of a member the stack branch
  lacks said as not synced; `bd robot --whole-stack` prints the same map.
  [@test] ../../crates/sync-core/src/robot_commits_test.rs (`a_robot_of_the_whole_stack_lists_the_commits_of_every_member_with_its_branch`)
  [@test] ../../src/lib/robot-prompt.test.ts (`names the stack branch and every branch of the whole stack for its robot: it built them all`)
  [@test] ../../crates/bd-cli/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)
- When it merged, the open pull requests of every branch of the stack must be closed with a comment ("Landed in
  master with #12, the Safe Push of the whole air stack (user/stack/air), which holds this branch."), and the toast
  says it ("Safe Push · closes the pull requests of 6 branches: they landed with the whole stack"); only while it ran
  on the stack branch as it is, and only a pull request that holds nothing its branch lacks: its head is the branch's
  tip, or the remote branch has no commit the branch lacks ([Patronus](patronus.spec.md#merged), the same for a piece).
  [@test] ../../src/lib/landed-stack.test.ts (`closes the pull requests of every branch of the stack the robot of the whole stack landed, not the one it started from`)
  [@test] ../../src/stores/patronus.test.ts (`closes the pull requests of the branches the whole stack landed`)
- One builder must list the actions of the whole stack (`getWholeStackMenuItems`), which the "⋯" of the stack island
  renders and the root's menu under "Whole Stack ›" (so the root's preview too), for a small stack without an island
  as for an island: "Safe Push Whole Stack" and "Dry Run Whole Stack" ([below](#the-safe-push-of-the-whole-stack-in-the-app)),
  "Open Pull Request #12" of its pull request, else "Create Pull Request of the Stack…" (disabled
  with the reason while the sync didn't build the stack branch: `getStackBranchBlocker`), Push with Children (the
  island's; the root's menu has it beside its push), "Review Whole Stack" ([Branch Review](branch-review.spec.md#opening))
  and the actions of its robot.
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`, `should review the whole stack in the window of its stack branch, from the island's menu and the root's menu`)
- "Create Pull Request of the Stack…" must open the "Create pull request" dialog of the stack branch into the baseline
  ("Whole air stack → master", "Holds the commits of every branch of the stack: air, bazel, bridge and 4 more: 7
  branches · 14 commits"), its text from `stack_pull_request_text` (`get_new_stack_pull_request`: the descriptions in
  one `git config`, the subjects as the rows show them); Create pushes the stack branch alone (`push_branches` with
  `wholeStack`, leased on its remote head), then creates the pull request of its head. It asks the code owners of the
  whole stack to review, as the dialog of a branch asks the branch's: those Patronus suggests for the stack branch (the
  readiness of `stack:<root>`), preselected, while the sync built it.
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`, `should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`the_text_of_a_new_pull_request_of_the_whole_stack`)
  [@test] ../../crates/sync-core/src/push_test.rs (`the_whole_stack_pushes_its_stack_branch`)
- A pull request of the whole stack that no longer matches it (a Safe Push of a branch landed the root in a piece, as
  the app saw the merge; or the stack no longer forks and its stack branch stays for its remote branch) must never be
  closed by itself, and its stack branch stays: the merge toast, the root's preview, the island's header and the
  description of its items say "#12 no longer matches the air stack: auth landed", and "Close Pull Request #12…" asks in
  the root's row before it closes it with a comment that says why.
  [@test] ../../src/lib/whole-stack.test.ts (`has a pull request that no longer matches it once a piece landed the root or the stack no longer forks, an open one only`)
  [@test] ../../src/stores/patronus.test.ts (`says that a piece that landed the root leaves the pull request of the whole stack, never closed by itself`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say that the pull request of the whole stack no longer matches it once a piece landed the root, and close it only when asked`)

### The Safe Push of the whole stack in the app

- The app must start the Safe Push and the Dry Run of the whole stack as the start of a branch is
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)), its subject the key `stack:<root>`
  (`requestSafePush`, `readSafePush`; the decision of `getSafePushDecision`): its facts are the stack branch, the robot
  of the whole stack and the profile of its last one, its pull request and the readiness of the whole stack. It lands
  more than a row, so it always opens its popover ("Safe Push of the whole air stack"): what lands ("Lands 7 branches ·
  14 commits into master in one Safe Push: air, bazel, … side-fix and side-test.", the main line first, then the side
  branches, `getWholeStackMembers`), "Pushes only its stack branch user-name/stack/air: the branches of the stack stay
  as they are on the remote.", with its pull request open "Approvals count in #12, the pull request of the whole
  stack.", the profile, the review, the robot of the whole stack it cancels; Dry Run, Cancel and the start (Enter).
  The start pushes the stack branch alone while its remote lacks it (`pushWholeStack`, leased, a robot it replaces
  canceled first), then starts the robot from it (`start_patronus_robot` with `stack`), or comments on its pull request.
  A straight chain, which has no stack branch, starts its last branch.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)
- It must be asked from the header of an island (its slot, "Safe Push Whole Stack" and "Dry Run Whole Stack" of its
  "⋯", S on the focused header), the popover at the header's slot ([Stack Island](stack-island.spec.md#header)), and
  from "Whole Stack ›" of the root's menu, the popover at the header of the root's island, else at the root's row (a
  small stack, `getWholeStackStartItems`).
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)
- Its next step must be the table of [Next Step](next-step.spec.md#the-whole-stack) on its facts: disabled with the
  reason while the sync didn't build its stack branch ("The stack branch conflicts at 0caf9066 in config.js: the
  stack lands in pieces", a Dry Run too), while a robot of a branch of it runs, while its approvals are missing in its
  own pull request. A stack branch not built lands the stack in pieces: the "⋯" of the island offers the Safe Push of
  the last branch of its main line, and so do the notice of S and the popover ("Land it in pieces").
  [@test] ../../src/hooks/useStackNextStep.test.ts (`is disabled with the reason of a stack branch the sync didn't build, for a Dry Run too: the stack lands in pieces`, `waits for the robot of a branch of the stack, which lands a part of it; a Dry Run doesn't`)
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
  `patronus_start_test.rs`, `stack_pull_request_test.rs`; `crates/bd-cli/src/status_test.rs`, `safe_push_test.rs`,
  `push_test.rs`, `robot_test.rs`.
- `pnpm test:unit`: `src/stores/branch-sync.test.ts`, `src/lib/patronus.test.ts`, `src/lib/whole-stack.test.ts`,
  `src/lib/landed-stack.test.ts`, `src/stores/patronus.test.ts`, `src/hooks/useStackNextStep.test.ts`.
- `pnpm e2e tests/e2e/stack-island`: the whole stack in the app (templates `stack_island`, `stack_side_conflict`);
  `tests/e2e/stacked-branches/stacked-branches.spec.ts` for a small stack.
- `pnpm perf:monorepo`: the full and the unchanged `bd sync` of the pinned monorepo clone, one stack that forks.

## References

- [Landing Work into Master](../../docs/design/landing.md): why the whole stack is one more virtual branch
- [Git Operations](../../docs/design/git-operations.md): "Stack branch", the build and the ref rules
- [Stacked Branches](stacked-branches.spec.md): how branches stack
- [Stack Island](stack-island.spec.md): the header the whole stack's Safe Push is in
- [Next Step](next-step.spec.md#the-whole-stack): the step of the whole stack
