---
name: Next Step
description: Requirements for the one model of what to do next with a branch (`domain_policy::next_step` in Rust for the app and bd, worded by the page's presenter `presentNextStep`), the first matching row of a table of its states (a sync, a failure, a block, a hold, landed, a robot that runs or merged, a branch outside what its Safe Push lands that its build needs (the landing needs Rust computes after a sync), landed by a merged Safe Push (its own, of a branch built on it or of the whole stack, by the hashes or by content, the commits added since the work left) until the update, a robot to restart, a pull request into another base or a draft, the approvals that count missing, waited for or approved, as Patronus judges them; a stacked branch by the rows of its own pull request), rendered by the Safe Push slot of the row, the "⋯" menu, S, the branch preview, the review window, the stack header and the start of a Safe Push with its popover (`domain_policy::land::decide`, presented by `presentLand`); the same table for the whole stack of a root from its stack branch (useStackNextStep), whose pull request a branch without its own points to, which lands only what merged Safe Pushes left of it; the readiness store that asks Patronus for it in the background, only for the branches a surface shows, and Check Approvals Again.
style: plain-1
targets:
  - src/presenters/next-step.ts
  - src/model/store.ts
  - src/lib/menu.ts
  - src/hooks/useBranchNextStep.ts
  - src/hooks/useStackNextStep.ts
  - src/lib/whole-stack.ts
  - src/lib/approval-news.ts
  - src/components/branch/WholeStackSlot.tsx
  - src/stores/review-readiness.ts
  - src/model/demand.ts
  - crates/repository-model/src/readiness_facts.rs
  - crates/remote-facts/src/scheduler_readiness.rs
  - crates/domain-policy/src/readiness.rs
  - crates/domain-policy/src/next_step.rs
  - crates/domain-policy/src/actions.rs
  - crates/domain-policy/src/landing_route.rs
  - crates/repository-model/src/action_view.rs
  - crates/sync-core/src/repo_facts.rs
  - crates/domain-policy/src/landing_node.rs
  - crates/domain-policy/src/landing_needs.rs
  - crates/domain-policy/src/land.rs
  - crates/repository-model/src/derive.rs
  - crates/bd-core/src/next_step_summary.rs
  - crates/domain-policy/src/landed.rs
  - crates/domain-policy/src/pull_request.rs
  - src/components/branch/NextStepSlot.tsx
  - src/presenters/landing-node.ts
  - src/hooks/useLandingNode.ts
  - src/components/branch/SafePushButton.tsx
  - src/stores/safe-push.ts
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useBranchRowKeys.ts
  - src/hooks/usePatronusReviewRequirement.ts
  - src/hooks/useReviewStatusRefresh.ts
  - src/components/branch/BranchPreviewDialog.tsx
  - src/components/branch/BranchPreviewActions.tsx
  - src/components/branch/StackIslandCard.tsx
  - src/components/branch/SafePushPopover.tsx
  - src/hooks/useSafePush.ts
  - src/presenters/land.ts
  - src/stores/request-review-popover.ts
  - src/presenters/review.ts
  - src/stores/pull-requests.ts
  - src/lib/review-window.ts
  - src/stores/patronus.ts
  - src/lib/patronus.ts
  - src/hooks/useLandedIn.ts
  - src/components/branch/LandingSpanLabel.tsx
  - src/components/branch/LandedChip.tsx
  - crates/sync-core/src/robot_landing.rs
  - crates/trial-check/src/landing_gate.rs
  - crates/domain-types/src/landing.rs
  - crates/repository-model/src/facts.rs
  - src/presenters/landing.ts
  - src/presenters/running-move.ts
  - crates/operations/src/land.rs
  - crates/operations/src/robot.rs
  - crates/operations/src/pull_request.rs
  - crates/domain-types/src/next_step.rs
  - src/presenters/reasons.ts
---

# Next Step

Status: Implemented
Date: 2026-10-17

## Purpose

Every surface used to decide on its own what a branch can do now, from its own share of the facts: the row offered
"Safe Push…" for a branch whose code owner was still asked, the form enabled Start, and the robot failed at once for
the approvals Patronus checks when it starts. One rule, `domain_policy::next_step` in Rust, now decides the next step
of a branch from all the facts (the branch, its robots, its pull request and the readiness Patronus answers,
[Patronus](../patronus/spec.md#code-owner-review)) for the app and `brd` alike; the session sends it in its view
(`nextSteps`, a kept part per subject the row reads with `useRowPart(name, "landing")`), the page words it
(`presentNextStep` of `src/presenters/next-step.ts`, which decides nothing), and every surface renders it: the slot shows
the step, every "Safe Push" is disabled with the same reason, no action is offered whose failure is known.

## Goals

- One answer to "what now" for a branch, the same in the row, the menu, the keys, the preview, the review window and
  the stack header.
- Never offer an action that Patronus would fail; say why instead, and hand over the action that helps.
- A step an action needs and the app can do is part of the action, never its reason to be disabled: a push, a wait for
  the sync. The action keeps its plain name ("Create Pull Request…"), the progress shows in its busy button. Only a
  consequence a person decides is told and asked: dropping someone's remote commits, dismissing approvals, canceling a
  running robot.
- A wait reads as a wait: the state is the label, and it opens the pull request it waits in.

## Non-goals

- Judging approvals in the app: Patronus is the only judge; the app renders its readiness.

## Concepts

### What the Safe Push needs

git merges a branch that uses a name only another branch adds, and the robot's build fails: the robot of 2026-10-22
ran `ui-perf-nightly`, built on `lane-run`, whose commit imports a class `stills-codec` adds, while the robot of
`stills-codec` ran beside it. The landing gate refuses such a start only after the press; the row says it before any
press.

### Facts without an answer

A start, a repository switch and a sync give the rows before the remote answers for them: Patronus (whether it lands
the repository, its robots, the view of what they hold and land the session derives), GitHub (the pull requests), the
problems of a failed robot (whether it failed for approvals) and the readiness of each row. Seen on 2026-10-24 on the
monorepo: the rows said "Safe Push" for 3 s, then "Awaiting Review" and "Create Pull Request…"; a click in those
seconds confirmed a start through the API whose approvals don't count.

The landing needs, measured on 2026-10-24, come 0.2 s after the rows on the monorepo with a warm cache and 0.7 to
1.2 s cold (9 branches), 0.1 to 0.2 s in the e2e fixtures, so the Safe Push waits for them as for the other facts.

## Requirements

### Requirement: The table: The next step is the first row that matches

The next step of a branch MUST be the first row of the table that matches. "Safe Push elsewhere" is the reason every
other surface MUST disable "Safe Push" with (`safePushBlocker`, the `safe-push` action Rust offers).

#### Scenario: The first row that matches decides the step
- **WHEN** the next step of a branch is decided
- **THEN** it is the first of these rows that matches, its slot as the row says
- **AND** every other surface offers "Safe Push" as "Safe Push elsewhere" says, disabled with its reason

  | # | State | Slot | Safe Push elsewhere |
  |---|---|---|---|
  | 0 | A fact the rows below decide on has no answer yet (`PendingFact`; [below](#facts-without-an-answer)): Patronus hasn't answered for the repository or its view of the robots it listed first isn't in, GitHub hasn't listed the pull requests, the check of what the branch needs to land is still running, Patronus hasn't told why the robot of the branch failed, the first answer of the readiness; never while a reason of row 1 or of the branch holds the start back, which it says instead. Or a read failed (`UnavailableFact`): the Safe Push CLI is not logged in, Patronus fails, Patronus didn't give why the robot failed | muted "Checking…" with its spinner, disabled, the fact on hover ("Waits for GitHub to list the pull requests"); after a failed read what helps: "Copy Login Command", "Refresh Patronus", or the restart disabled with the failure | disabled with the same words ("Waits for the answer of Patronus", "Waits for Patronus to check the approvals", "Run ./safePush.cmd once to log in to Patronus"); a Dry Run only while Patronus hasn't answered or can't be asked |
  | 1 | Syncing, failed to sync, no commits, blocked, held by a Safe Push, parked or built on a parked branch, landed, a robot runs, merged (by the robot, by the merged Safe Push of a branch built on it or of the whole stack, or the pull request of these commits; all of it, no commit added since) | nothing; the Status cell says it ("Landed with oauth", "Parked with auth"), a running robot first with what else holds the branch beside it ([Branch Row](../branch-row/spec.md#status-cell)), as `brd status` does (`next_step.status`) | disabled: "Wait for the sync to finish", "Parked: Unpark it to push", "Built on parked auth: Unpark auth to push", "Holds parked auth: land the stack in pieces" (the whole stack), "Landed in origin/master", "Landed in master with oauth", "Merged by Patronus", "Pull request #7 is merged", … (a running robot of an older commit: enabled, the start replaces it) |
  | 1′ | The build of what its Safe Push lands needs a commit of a branch outside it (`landingNeeds` of the repository view, [below](#what-the-safe-push-needs)), or the user declared that another branch lands first ([Declared Needs](../declared-needs/spec.md); before the waits of row 0) | "Needs stills-codec" (a commit not in a branch by its hash), "Needs docs", the reason on hover; a click runs the fix, "Stack lane-run on stills-codec", "Unstack ui from auth" | enabled: its popover says why before the start, with the fix and Start Anyway |
  | 1″ | Someone else pushed commits to its remote branch that it lacks (`othersBehind` of the remote status, [Outgoing](../../repository/outgoing/spec.md#remote-commits-of-others)): a push, also the one of a start or a restart, drops them | "Pick Remote Commits", the reason on hover ("The remote has 1 commit of someone else that auth doesn't: a push drops it"); a click opens the question of the push button, which lists the commits with the pick | enabled: its start asks before the push that drops them |
  | 2 | A robot failed or was canceled, not for approvals, and no running Safe Push of a branch it is built on (of the whole stack: of a branch of it) lands a part of it; while one does, the failure of the earlier piece waits for it: the rows below decide, the Safe Push disabled with "A Safe Push of auth is running: wait until it merges" | "Restart Safe Push" (Safe Push… where Patronus can't restart it) | enabled |
  | 3 | A robot failed for approvals, started from the open pull request into the target, readiness `approved` | "Restart Safe Push" | enabled |
  | 4 | removed: a stacked branch takes the rows of its own pull request into master, see below | — | — |
  | 5 | An open pull request into a virtual branch (`{prefix}/virtual/…`, `base` `virtual` of the open pull request the session reads, `domain_policy::pull_request::base_of`: of its parent, of another branch of the table, or of one that landed, was archived or deleted): every pull request of a virtual branch goes into the target | "Retarget #7 to master" ("Retarget #7" in the slot) | enabled: the start retargets #7 into master first, then starts from it, as `brd safe-push` (`PlannedEffect::Retarget` of `domain_policy::land::decide`) |
  | 5 | Merge route: an open pull request into the branch of the layer below it that GitHub lists in no stack (`layerNotLinked` of the merge, [Stacks of GitHub](../stacked-branches/spec.md#stacks-of-github)): its merge would land nothing | "Link GitHub Stack" ("Link Stack" in the slot): Link GitHub Stack (`Intent::LinkStack`) | its merge waits for the link |
  | 5′ | An open pull request into another base, which may be meant (a release branch; `base` `other`) | quiet "Open Pull Request", the reason on hover; "Retarget #7 to master" stays in "⋯" | disabled: "Approvals in #7 don't count: its base is release-252"; a start that counts no approval (without review, a profile that skips the review) starts, as `brd safe-push` (D3b) |
  | 6 | An open draft | quiet "Draft", opens it | disabled: "#7 is a draft: Patronus merges it once it is ready for review" |
  | 7 | `missing`, no open pull request (none, or a closed one) | "Create Pull Request…" (the code owners preselected; on the remote or not, its dialog pushes the branch; disabled only while the GitHub repository is unknown); a branch of a whole stack whose pull request is open: quiet "In #12", which opens it (below) | disabled: "Code owners must approve in a pull request: IJPL Code Platform" |
  | 8 | `missing`, a member of a missing group requested changes (and wasn't asked again since) | quiet "Changes Requested" in `destructive`, opens the pull request | disabled: "Ilya Korennoy requested changes on #1078" |
  | 9 | `missing`, a missing group asks fewer people than the approvals it needs (nobody, or one of a group that needs two) | "Request Review…", its popover ([Pull Requests](../pull-requests/spec.md#request-review)) | disabled: "Waits for the approval of Air Platform: nobody of it is requested on #1078" |
  | 10 | `missing`, someone of every missing group is asked | quiet "Awaiting Ilya Korennoy" ("Awaiting Review" in the slot, the people in its hover; "Awaiting 2 Reviews" for several), opens the pull request | disabled: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078" |
  | 8′ | `unknown`, `unavailable`, an open pull request into the target where a reviewer requested changes and wasn't asked again since (`changesRequestedBy`, bots left out) | quiet "Changes Requested" in `destructive`, opens the pull request | enabled: Patronus can't judge the approvals, the start checks them |
  | 10′ | `unknown`, `unavailable`, an open pull request into the target that waits for a requested reviewer (`readiness::waiting_reviewers`: asked and not approving since, bots left out) | quiet "Awaiting ikorennoy" ("Awaiting 2 Reviews"), its hover "Waits for ikorennoy on #1078 · Patronus can't tell yet whether the approvals there count: the start checks them", opens the pull request | enabled, as 8′ |
  | 11 | `approved`, `notRequired`; `unknown`, `unavailable` where GitHub tells no wait | "Safe Push" | enabled |

  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`, `the_cases_are_well_formed_and_name_every_row_of_the_next_step_table`)
  [@test] crates/domain-policy/src/next_step_test.rs (`every_row_with_every_fact_without_a_fresh_answer_never_answers_an_action_the_fresh_one_would_replace`)

#### Scenario: Row 0 waits for a fact without an answer, or says a read that failed
- **GIVEN** a fact the rows below decide on has no answer yet, or its read failed
- **WHEN** no reason of row 1 or of the branch holds the start back
- **THEN** the slot is a muted "Checking…", or what helps after a failed read
  [@test] crates/domain-policy/src/next_step_test.rs (`row_0_waits_for_a_fact_no_answer_told_yet_and_a_reason_of_the_subject_outranks_it`, `row_0_a_read_that_failed_decides_no_start_and_offers_what_helps`)
  [@test] src/presenters/next-step.test.ts (`row 0: words the fact a step waits for and a read that failed, with what helps: a login to copy, a refresh, a restart disabled`)

#### Scenario: Row 1 offers nothing and the Status cell says why
- **WHEN** the branch syncs, failed to sync, has no commits, is blocked, held, parked, landed, merged or its robot runs
- **THEN** the slot offers nothing and the Status cell says why
- **AND** both starts are disabled with the reason, but for a running robot of an older commit: its start is enabled and replaces it
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
  [@test] src/presenters/next-step.test.ts (`row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer nothing after a merge, and say so in the menu`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should offer no Safe Push of a branch whose pull request merged`)

#### Scenario: Row 1′ names the branch the build needs
- **WHEN** the build of what its Safe Push lands needs a branch outside it, or the user declared one that lands first
- **THEN** the slot names that branch and a click runs the fix
- **AND** the Safe Push stays enabled, its popover saying why before the start
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_needs_a_branch_outside_what_the_safe_push_lands_and_keeps_the_start`)
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the build needs and runs the fix, or opens the Safe Push`)

#### Scenario: Row 1″ takes in the commits of others on the remote
- **WHEN** someone else pushed commits to its remote branch that it lacks
- **THEN** the slot is "Pick Remote Commits", the reason on hover
- **AND** a start asks before the push that drops them
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_double_prime_takes_in_the_commits_of_others_on_the_remote_before_a_start`)
  [@test] src/presenters/next-step.test.ts (`row 1″: takes in the commits someone else pushed to the remote branch, which a push would drop`)
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should make the pick the next step of a branch on Space, before its Safe Push, and run it from the slot`)

#### Scenario: Rows 2 and 3 restart a failed robot
- **WHEN** a robot failed or was canceled, not for approvals, or failed for approvals with the readiness `approved`
- **THEN** the slot is "Restart Safe Push" (Safe Push… where Patronus can't restart it)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)
  [@test] src/presenters/next-step.test.ts (`rows 2 and 3: restarts the robot, or the pull request that collects the approvals, or a Safe Push where it can't restart`)

#### Scenario: Row 5 retargets a pull request into a virtual branch, row 5′ opens one into another base
- **WHEN** the open pull request goes into a virtual branch
- **THEN** the step is "Retarget #7 to master" and the start retargets it first
- **AND** an open pull request into another base that may be meant is a quiet "Open Pull Request", its retarget in "⋯"
  [@test] crates/domain-policy/src/next_step_test.rs (`row_5_retargets_a_pull_request_into_a_virtual_branch_and_5_prime_opens_one_into_another_base`)
  [@test] src/presenters/next-step.test.ts (`row 5: retargets a pull request into a virtual branch, short in its column; 5′ opens one into another base, the retarget in the menu`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should retarget a pull request into the branch of a parent that landed, whose approvals don't count`, `should retarget a pull request into the virtual branch of a branch in the table, as a parent's, whose approvals don't count`, `should retarget the pull request of a stacked branch into its parent's branch from its slot, whose approvals don't count`, `should open a pull request into another base that may be meant, and keep its retarget in the menu`)

#### Scenario: Row 6 opens a draft
- **WHEN** the open pull request is a draft
- **THEN** the step is a quiet "Draft" that opens it
- **AND** its Safe Push is disabled with why, a Dry Run runs
  [@test] crates/domain-policy/src/next_step_test.rs (`row_6_a_draft_is_a_step_that_opens_it_the_safe_push_waits_a_dry_run_runs`)
  [@test] src/presenters/next-step.test.ts (`row 6: a draft is a quiet step that opens it; its Safe Push says why, a Dry Run runs`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`)

#### Scenario: Row 7 creates the pull request
- **WHEN** the approvals are `missing` and no pull request is open
- **THEN** the step is "Create Pull Request…" with the code owners preselected, its dialog pushing the branch
- **AND** a branch of a whole stack whose pull request is open has a quiet "In #12"
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] src/presenters/next-step.test.ts (`row 7: Create Pull Request… naming the code owners, or In #12 of the whole stack, quietly`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer Create Pull Request… with the code owner when approvals count only in one`, `should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`)

#### Scenario: Rows 8 to 10 tell the missing approvals
- **WHEN** the approvals are `missing` in an open pull request
- **THEN** the step is "Changes Requested", Request Review… or the wait naming the people asked
- **AND** the Safe Push is disabled with the reason, also on S
  [@test] src/presenters/next-step.test.ts (`rows 8 to 10: changes requested, Request Review…, the wait naming the people asked`)
  [@test] src/lib/pull-request.test.ts (`names who requested changes and wasn't asked again since, bots left out`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

#### Scenario: Rows 8′ and 10′ tell the wait GitHub tells while Patronus can't judge
- **WHEN** the readiness is `unknown` or `unavailable` and a reviewer requested changes or is waited for in the open pull request
- **THEN** the step is a quiet "Changes Requested" or "Awaiting ikorennoy"
- **AND** the Safe Push stays enabled, the start checks the approvals
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_8_prime_and_10_prime_tell_the_wait_github_tells_while_patronus_cannot_judge`)
  [@test] src/presenters/next-step.test.ts (`rows 8′ and 10′: the wait GitHub tells while Patronus can't judge, the Safe Push enabled`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the reviewer GitHub asks while Patronus can't judge the approvals, keep Safe Push, and tell the approval`)

#### Scenario: Row 11 is the Safe Push
- **WHEN** the readiness is `approved` or `notRequired`, or `unknown` or `unavailable` where GitHub tells no wait
- **THEN** the step is "Safe Push", enabled
  [@test] crates/domain-policy/src/next_step_test.rs (`row_11_safe_push_and_the_start_without_review_while_github_is_signed_out`)
  [@test] src/presenters/next-step.test.ts (`row 11: Safe Push by the branches it lands, and what Patronus checks while GitHub lists no pull request`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer Safe Push once the code owner approved, name whose approval counts, and start it from the pull request with a click`, `should keep Safe Push while Patronus doesn't tell whose approvals count, and say so before it starts`)

### Requirement: The table: A Dry Run is never held back by approvals or a pull request

A Dry Run MUST NOT be held back by approvals, a draft, a pull request into another base or a robot that lands an
ancestor (`dryRunBlocker`).

#### Scenario: A draft holds the Safe Push back, not the Dry Run
- **GIVEN** the open pull request of the branch is a draft
- **WHEN** the user runs a Dry Run
- **THEN** it runs, while the Safe Push is disabled
  [@test] crates/domain-policy/src/next_step_test.rs (`row_6_a_draft_is_a_step_that_opens_it_the_safe_push_waits_a_dry_run_runs`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`)

#### Scenario: Missing approvals, another base or a robot of an ancestor
- **WHEN** the approvals are missing, the pull request goes into another base, or a robot lands an ancestor
- **THEN** the Dry Run is not held back by it

### Requirement: The table: Row 1 holds a branch a merged Safe Push landed whole

Row 1 MUST also hold a branch that a merged Safe Push (not a Dry Run) landed whole (`getLandedBy`, the `landedBy` of
the repository view, `StepFacts.landed` of `domain_policy::next_step`), under the same conditions under which the merge
closes their pull requests ([Patronus](../patronus/spec.md#merged)), and a robot first seen merged (after a restart, a
landing by `brd`).

#### Scenario: The branch the robot ran on, an ancestor it carried, every branch of a whole stack
- **WHEN** a Safe Push merged that ran on the branch itself, on a branch built on it that carried it as an ancestor, or on the stack branch of a stack (the Safe Push of the whole stack)
- **THEN** the branch, the ancestor and every branch of that stack are in row 1
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer its Safe Push no more`)

#### Scenario: A robot first seen merged
- **WHEN** the robot is first seen merged (after a restart, a landing by `brd`)
- **THEN** the branches it landed are in row 1 too, and the rest lands without them
  [@test] src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)

#### Scenario: A Dry Run lands nothing
- **WHEN** a Dry Run merged
- **THEN** it landed no branch
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_dry_run_lands_nothing`)

### Requirement: The table: What a merged Safe Push landed is told by the hashes, else by content

What a merged Safe Push landed MUST be decided by the hashes while the robot ran on a commit of its branch as the table
has it, or on the stack branch as it is; after a change below that commit it MUST be decided by content: the commits
the robot tested against each branch as the table has it (the repository view, asked once per state of the branches;
[Patronus](../patronus/spec.md#merged)).

#### Scenario: The robot ran on a commit of the branch as the table has it
- **WHEN** the robot ran on its tip, the same commits rebuilt on a newer baseline, or a commit the branch built new ones on since
- **THEN** the hashes tell what it landed
  [@test] crates/repository-model/src/patronus_view_test.rs (`new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`, `a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`)

#### Scenario: A change below the commit of the robot
- **WHEN** a commit below the robot's commit changed: a new commit or an amend of a branch it is built on, a reword, for the whole stack any change of a branch of it, which rebuilds the stack branch
- **THEN** the content tells what it landed: the commits the robot tested against each branch as the table has it
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_missing_source_lands_nothing`, `a_commit_added_below_the_robots_commit_is_the_only_work_left`)
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_reworded_commit_below_keeps_the_branches_landed`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

### Requirement: The table: The Status cell names who landed the branch until the update

Until the update archives the branch (the fetched baseline then says "Landed in origin/master" first), its Status
cell MUST name who landed it, "landed" with "Landed with oauth" ("Landed with the whole air stack"), its hover when it
merged and a click the robot in Patronus; the preview MUST show it too; the branch the robot ran on keeps its robot,
"Merged".

#### Scenario: The Status cell of a landed branch
- **GIVEN** a merged Safe Push of oauth landed the branch
- **WHEN** the update has not archived it yet
- **THEN** its Status cell says "landed" with "Landed with oauth", its hover when it merged
- **AND** a click opens the robot in Patronus, and the preview shows it too
  [@test] src/lib/patronus.test.ts (`names who landed a branch in its row: a branch built on it, the whole stack, its own robot with the work left`)

#### Scenario: The branch the robot ran on
- **WHEN** the branch is the one the merged robot ran on
- **THEN** it keeps its robot, "Merged"

### Requirement: The table: A branch a merged Safe Push landed offers no start and no push

The Safe Push and the Dry Run of a branch a merged Safe Push landed MUST be disabled with "Landed in master with oauth"
("Merged by Patronus" of the robot's own branch) on every surface, S says it, and its push MUST be held as by a running
robot ([Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: Every surface says why
- **WHEN** a merged Safe Push of oauth landed the branch
- **THEN** its Safe Push and Dry Run are disabled with "Landed in master with oauth" on every surface, S says it
- **AND** its push is held
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
  [@test] src/presenters/next-step.test.ts (`row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed`)
  [@test] src/presenters/reasons.test.ts (`says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should offer no Safe Push, Dry Run or push of the branches the whole stack landed, each saying why, the hover of their chip saying so`)

### Requirement: The table: A branch built on a landed one lands without it

A branch built on a landed one MUST land without it: "Safe Push" where it said "Safe Push · 2"
(`getLandingBranchCount`), its popover not naming it.

#### Scenario: The count of the branches it lands
- **GIVEN** a branch built on a branch a merged Safe Push landed
- **WHEN** its step is shown
- **THEN** it says "Safe Push" where it said "Safe Push · 2", and its popover doesn't name the landed one
  [@test] src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)

### Requirement: The table: What landed a branch is one rule, the first that holds

What landed a branch, and a whole stack, MUST be one rule in Rust for the app and `brd`
(`domain_policy::landed::landed_state`, the view's `landedStates`, [Patronus](../patronus/spec.md#merged)), the first
that holds: the fetched baseline has every commit of it (of a whole stack, of every branch); merged Safe Pushes landed
it (also a part of it, the commits added after it the work left); GitHub merged its pull request into the target with
the commits it has.

#### Scenario: The baseline first, then merged Safe Pushes, then its pull request
- **WHEN** more than one of them holds
- **THEN** the first that holds tells what landed the branch
  [@test] crates/domain-policy/src/landed_test.rs (`what_landed_a_branch_the_baseline_first_then_merged_safe_pushes_then_its_pull_request`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A whole stack
- **WHEN** every branch of a whole stack landed
- **THEN** the whole stack landed
  [@test] crates/domain-policy/src/landed_test.rs (`a_whole_stack_landed_when_every_branch_of_it_did`)

### Requirement: The table: A pull request lands the branch only merged with the commits it has

A merged pull request MUST land the branch only when its head is the branch's tip (the stack branch's), or its remote
branch, which has what the branch has (a branch only rebuilt on a newer baseline since), and it merged into the
target. A merged pull request of other commits is new work under the same name; a closed one lands nothing.

#### Scenario: Its head is the tip or the remote branch
- **WHEN** GitHub merged the pull request into the target, its head the branch's tip or its remote branch, which has what the branch has
- **THEN** it landed the branch
  [@test] crates/domain-policy/src/landed_test.rs (`a_pull_request_lands_the_branch_only_merged_with_the_commits_it_has`)

#### Scenario: Other commits, or a closed pull request
- **WHEN** the merged pull request has other commits, or the pull request was closed
- **THEN** it lands nothing: other commits are new work under the same name

#### Scenario: Merged into another branch than the target
- **WHEN** it merged into another branch than the target, a parent's virtual branch most of all (a child of a stack merged after its parent's pull request had landed: the target lacks its commits)
- **THEN** it lands nothing
- **AND** a layer of a stack of GitHub into the target keeps its old base once merged and lands (`domain_policy::pull_request::merges_into_target`)
  [@test] crates/domain-policy/src/landed_test.rs (`a_pull_request_merged_into_another_branch_lands_nothing`)

### Requirement: The table: A start of a branch all of which landed is refused

A start of a branch all of which landed MUST be refused at both sides (`brd safe-push`: `landed`).

#### Scenario: The start of a landed branch
- **WHEN** a Safe Push of a branch all of which landed starts, in the app or by `brd safe-push`
- **THEN** it is refused, `landed`
  [@test] crates/domain-policy/src/landed_test.rs (`the_reason_a_start_gives_of_what_landed_all_of_the_subject`)

### Requirement: The table: The step says why nothing of the branch is left to land

The step MUST say why nothing of the branch is left to land (`BranchNextStep.landed`, `formatLandedWhole`: the words of
the view's landed state, `formatLanded`; before the view of the robots, the robot of the row that merged), also while
another reason of row 1 (a sync) disables its start.

#### Scenario: The words of what landed it
- **WHEN** nothing of the branch is left to land, also while it syncs
- **THEN** the step says "Landed in origin/master", "Landed in master with oauth", "Merged by Patronus" or "Pull request #7 is merged"
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
  [@test] src/presenters/next-step.test.ts (`row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed`)
  [@test] src/presenters/reasons.test.ts (`words what landed a subject: the baseline, merged Safe Pushes, its own robot, its pull request`)

### Requirement: The table: Create Pull Request… of a landed branch is disabled with why

"Create Pull Request…" of the branch MUST read the reason that nothing is left to land: disabled with that reason,
nothing is left to review ([Pull Requests](../pull-requests/spec.md#creating-a-pull-request)).

#### Scenario: Nothing is left to review
- **WHEN** nothing of the branch is left to land
- **THEN** its "Create Pull Request…" is disabled with the reason of the step

### Requirement: The table: The commits added after the landed ones are the only work left

A landed branch with commits added after the landed ones MUST NOT be settled: those commits are the only work left of
it, its Status cell says so, and it takes the rows of a branch with work: its push, its Safe Push.

#### Scenario: A commit added after the landed ones
- **WHEN** a commit was added to a landed branch after the landed ones
- **THEN** its Status cell says "Landed with oauth · 1 commit after it" (of the robot's own branch "Landed · 1 commit after it"), its hover "1 commit added since, not in master yet"
- **AND** it takes the rows of a branch with work: its push, its Safe Push
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_commit_added_below_the_robots_commit_is_the_only_work_left`)
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_commit_added_below_the_robots_commit_is_the_only_work_left`, `a_commit_not_synced_yet_is_after_the_landed_ones`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

### Requirement: The table: A branch with a commit changed since is not landed

A branch with a commit changed since (an amend), or a new one below a tested one, MUST NOT be landed: what landed is
not what it is.

#### Scenario: An amend, or a new commit below a tested one
- **WHEN** a commit of a landed branch was amended, or a new one added below a tested one
- **THEN** the branch is not landed
  [@test] crates/sync-core/src/robot_landing_test.rs (`an_amended_commit_leaves_its_branch_not_landed`)

### Requirement: The table: Rows 8 to 10 count only the groups still short of approvals

Rows 8 to 10 MUST count only the groups still short of approvals: changes requested by a member of an approved group
don't block, as Patronus doesn't block on them.

#### Scenario: Changes requested by a member of an approved group
- **GIVEN** a group that approved and a member of it who requested changes
- **WHEN** the step is decided
- **THEN** those changes block nothing

### Requirement: The table: Work the remote lacks keeps the wait, Request Review… pushes it first

With work the remote lacks the push slot MUST show the push and the step MUST still show the wait; Request Review…
MUST push that work first, a step of the request, so the reviewers see the commits
([Pull Requests](../pull-requests/spec.md#request-review)).

#### Scenario: Request Review… of a branch with work the remote lacks
- **GIVEN** a branch with commits the remote lacks
- **WHEN** the user runs Request Review…
- **THEN** it pushes those commits first, then requests the review
  [@test] tests/e2e/next-step/next-step.spec.ts (`should push the commits the remote lacks as the first step of Request Review…`)

### Requirement: The table: A missing group nobody can approve disables Request Review…

A missing group nobody can approve (`noEligibleReviewer`) MUST disable Request Review…: "Nobody of IJPL Code Platform
can approve these changes".

#### Scenario: Nobody of the group can approve
- **WHEN** nobody of a missing group can approve the changes
- **THEN** Request Review… is disabled with "Nobody of IJPL Code Platform can approve these changes"

### Requirement: The table: Request Review… is one item in the slot and the branch menu

Request Review… MUST be the item of `getRequestReviewItem` ([Pull Requests](../pull-requests/spec.md#request-review)),
the same in the slot (row 9, its hover the reason of the step, then whom it asks) and the branch menu (every branch
with an open pull request, also while the slot shows another step: it opens the popover at the slot).

#### Scenario: From the branch menu
- **GIVEN** a branch with an open pull request whose slot shows another step
- **WHEN** the user chooses Request Review… in the branch menu
- **THEN** its popover opens at the slot
  [@test] tests/e2e/next-step/next-step.spec.ts (`should open Request Review… from the branch menu, at the row's next step`)

### Requirement: The table: Once the review is requested the step is the wait

The popover asks the code owners; once the request is made the step MUST be the wait: a suggested member of a missing
group whom the pull request asks counts as requested (`with_requested_on_pull_request` of `domain_policy::next_step`),
also while Patronus answers again with the people requested before. Someone asked who is no suggested member of the
group MUST count as asking nobody of it.

#### Scenario: The code owner asked from Request Review…
- **WHEN** the user asks the code owner nobody asked yet from Request Review…
- **THEN** the step is the wait for that code owner from the click on, also while Patronus answers with the people requested before
  [@test] tests/e2e/next-step/next-step.spec.ts (`should ask the code owner nobody asked yet from Request Review…, then wait for him from the click on`)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)

#### Scenario: A member asked again after changes
- **WHEN** a member who requested changes is asked again
- **THEN** the member is awaited
  [@test] crates/patronus/src/review_readiness_test.rs (`a_member_re_requested_after_changes_is_awaited`)

#### Scenario: Someone asked who is no suggested member
- **WHEN** the pull request asks someone who is no suggested member of the missing group
- **THEN** it asks nobody of that group

### Requirement: The table: A robot failed for approvals keeps the pull request step

A robot that failed for approvals (row 3) MUST keep the pull request step of before ("Open Pull Request", or "Create
Pull Request…" without one) while Patronus can't tell whether the approvals count (`unknown`, `unavailable`): a restart
may fail again; before the first answer the step waits for it (row 0).

#### Scenario: Patronus can't tell whether the approvals count
- **WHEN** a robot failed for approvals and the readiness is `unknown` or `unavailable`
- **THEN** the step is "Open Pull Request", or "Create Pull Request…" without one
- **AND** before the first answer the step waits for it (row 0)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)
  [@test] src/presenters/next-step.test.ts (`rows 2 and 3: restarts the robot, or the pull request that collects the approvals, or a Safe Push where it can't restart`)

#### Scenario: Approved, but started without the pull request
- **WHEN** a robot failed for approvals that started without the pull request, and the readiness is approved
- **THEN** the step is "Safe Push": a new start from it counts the approvals

### Requirement: The table: The restart of a robot failed for approvals waits for them

The restart of a robot that failed for approvals MUST be disabled with the reason of rows 7 to 10 while the approvals
are missing, on every surface that offers it: "⋯", the robot details and the landing overview (`getRobotItems`, as
Rust offers them).

#### Scenario: The approvals are missing
- **WHEN** the approvals of a robot that failed for them are missing
- **THEN** its restart is disabled with the reason of rows 7 to 10 in "⋯", the robot details and the landing overview
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)

### Requirement: The table: A stacked branch takes the rows of its own pull request

A stacked branch MUST have no row of its own (row 4 is removed, the numbers of the others stay): its Safe Push lands it
with what it is built on, in one robot ([Landing](../../../../docs/design/landing.md)), and its pull request goes into
master ([Pull Requests](../pull-requests/spec.md)), so it takes the rows of that pull request: its own approvals, its
own draft, its own wait.

#### Scenario: The wait of its own pull request
- **GIVEN** a stacked branch with a pull request of its own
- **WHEN** its pull request asks a code owner
- **THEN** its step waits for that code owner, not for its root
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner a stacked branch's own pull request asks, not for its root`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: One Safe Push lands it with what it is built on
- **WHEN** the user starts the Safe Push of a stacked branch
- **THEN** one robot lands it with what it is built on
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`)

### Requirement: The table: The reasons of its ancestors come through the rows that hold them

The reasons of the ancestors of a stacked branch MUST come through the rows that hold them already: a parent that
failed to build blocks it (row 1, `Blocked`), a Safe Push that lands the branch holds it (row 1), and a robot that
lands an ancestor is the reason of its start (`landing.start` of the view, `domain_policy::land::start_refusal`).

#### Scenario: A robot lands an ancestor
- **WHEN** a robot runs that lands an ancestor of the branch
- **THEN** every step's Safe Push is disabled with "A Safe Push of login is running: wait until it merges"
- **AND** a dry run may run
  [@test] crates/domain-policy/src/next_step_test.rs (`row_0_waits_for_a_fact_no_answer_told_yet_and_a_reason_of_the_subject_outranks_it`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should wait for the Safe Push of an ancestor, which lands its commits`)

#### Scenario: A parent that failed to build, a Safe Push that lands the branch
- **WHEN** a parent failed to build, or a Safe Push that lands the branch runs
- **THEN** the branch is in row 1: `Blocked`, or held

### Requirement: The table: The surfaces of a stacked branch ask its own start

The slot, "⋯", S, the preview and the review window of a stacked branch MUST ask its own start, its popover at its own
row ([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)); the header of a stack island shows the step
of the whole stack (below), its popover at the header ([Stack Island](../stack-island/spec.md#header)). The texts MUST
name the branch by `NextStepFacts.subject`, the branch's name.

#### Scenario: Every surface of the row starts its own Safe Push
- **WHEN** the user starts the Safe Push of a stacked branch from "⋯", Safe Push With Profile… of its menu, S, the review window, or the footer of its preview and S in it
- **THEN** its own start runs, its popover at its own row
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on from the ⋯ of its row and Safe Push With Profile… of its menu`, `should land a stacked branch with what it is built on from S`, `should land a stacked branch with what it is built on from the review window, at the row of the main window`, `should land a stacked branch with what it is built on from the footer of its preview and S in it, the popover at its row`)

#### Scenario: The header of a stack island
- **WHEN** a stack island shows its header
- **THEN** the header shows the step of the whole stack, its popover at the header

### Requirement: The table: An operation on the row disables the action of the slot

An operation on the row ([Running Operations](../../../../docs/design/operations.md)), as the view of the session has
it in every window (`RepositoryView.operations`: a rewrite of the branch or of its commits, the retarget of its pull
request; `useRowOperation`), MUST disable the action of the slot with its busy state ("Renaming auth to login…"), never
a quiet step: it only opens the pull request.

#### Scenario: A rename runs on the row
- **WHEN** an operation runs on the row, in any window of the repository
- **THEN** the action of the slot is disabled with its busy state ("Renaming auth to login…")
- **AND** a quiet step stays, as it only opens the pull request
  [@test] src/presenters/next-step.test.ts (`lays what runs on the row over the step: its own action runs, the others wait, never a quiet step nor another field of the pull request`)
  [@test] src/hooks/useBranchContextActions.test.ts (`says why a branch can't start a Safe Push in the words of its next step, and what runs on the row over it`)

### Requirement: The table: A step that writes another field of the pull request runs beside it

An operation on the row MUST NOT disable a step that writes another field of the pull request than the operation
does: a retarget (the base) during a review request (the reviewers) runs beside it, neither refuses the other.

#### Scenario: A retarget while a review is requested
- **WHEN** the user retargets a pull request while a review is requested in it
- **THEN** both reach GitHub, neither refuses the other
  [@test] tests/e2e/next-step/next-step.spec.ts (`should retarget a pull request while a review is requested in it: both reach GitHub, neither refuses the other`)
  [@test] src/presenters/next-step.test.ts (`lays what runs on the row over the step: its own action runs, the others wait, never a quiet step nor another field of the pull request`)

### Requirement: The table: The slot's own action that runs shows its spinner

The slot's own action that runs (a retarget, a restart of its robot) MUST show as running: disabled, its spinner in
place of its icon.

#### Scenario: The retarget of the slot runs
- **WHEN** the retarget or the restart the slot offers runs
- **THEN** the slot's action is disabled, its spinner in place of its icon
  [@test] src/presenters/next-step.test.ts (`lays what runs on the row over the step: its own action runs, the others wait, never a quiet step nor another field of the pull request`)

### Requirement: The table: While GitHub doesn't list the pull requests rows 5 to 10 don't apply

The pull requests are a fact of their own: while GitHub doesn't list them (not configured, signed out, SSO; before its
first answer the step is row 0), the readiness MUST count as `unavailable` and rows 5 to 10 MUST NOT apply: the table
never offers a Create Pull Request… for a branch that may have one; the start then holds nothing back either, and
confirms first.

#### Scenario: GitHub signed out
- **WHEN** GitHub doesn't list the pull requests of the repository
- **THEN** no Create Pull Request… is offered and the Safe Push holds nothing back
- **AND** its start confirms first: "GitHub doesn't list the pull requests of the repository: Patronus checks the approvals when the robot starts."
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)

### Requirement: The table: A repository only on Space never asks GitHub

A repository only on Space (no GitHub) MUST NOT ask for the pull requests; its Safe Push stays possible, its merge
requests are deprecated and the robot decides.

#### Scenario: No GitHub
- **WHEN** the repository is only on Space
- **THEN** nothing asks GitHub and its Safe Push stays possible, the robot deciding

### Requirement: The table: A repository not on Space has no slot

A repository not on Space MUST have no slot; Request Review… of an open pull request, which asks GitHub, still opens
from the menus, at an anchor of the row that takes no room ([Pull Requests](../pull-requests/spec.md#request-review)).

#### Scenario: Off Space
- **WHEN** the repository is not on Space
- **THEN** its rows have no slot and keep the reasons of the subject, with no notice
- **AND** Request Review… of an open pull request opens from the menus, at an anchor of the row that takes no room
  [@test] crates/domain-policy/src/next_step_test.rs (`a_repository_not_on_space_has_no_slot_and_keeps_the_reasons_of_the_subject`)
  [@test] src/presenters/next-step.test.ts (`has no slot off Space and keeps the reasons of the subject, with no notice`)

### Requirement: The table: The step and the node are one rule in Rust, derived by the session

The table MUST be one rule in Rust for the app and `brd` (`domain_policy::next_step`, over `StepFacts`: the subject as
the table and the robots have it, each fact of the remote as it stands), and so MUST the node of the row
(`domain_policy::landing_node`): the session derives the step and the node of every row and of every whole stack that
forks with each patch of its view (`RepositoryView.nextSteps`, `repository_model::derive::next_steps`).

#### Scenario: Each patch of the view
- **WHEN** the session sends a patch of its view
- **THEN** it carries the next step and the node of each subject, and a patch only those that changed
- **AND** a sync of a branch is row 1 meanwhile
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`, `row_0_waits_for_a_fact_no_answer_told_yet_and_a_reason_of_the_subject_outranks_it`, `row_0_a_read_that_failed_decides_no_start_and_offers_what_helps`, `row_1_prime_needs_a_branch_outside_what_the_safe_push_lands_and_keeps_the_start`, `rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`, `row_5_retargets_a_pull_request_into_a_virtual_branch_and_5_prime_opens_one_into_another_base`, `row_6_a_draft_is_a_step_that_opens_it_the_safe_push_waits_a_dry_run_runs`, `rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`, `rows_8_prime_and_10_prime_tell_the_wait_github_tells_while_patronus_cannot_judge`, `row_11_safe_push_and_the_start_without_review_while_github_is_signed_out`, `a_stale_readiness_keeps_its_step_and_its_action_waits_for_the_new_answer`, `a_repository_not_on_space_has_no_slot_and_keeps_the_reasons_of_the_subject`, `the_whole_stack_takes_the_rows_of_its_stack_branch`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`is_ready_to_land_once_patronus_judged_the_approvals_else_the_start_is_the_users_step`, `waits_for_others_the_reviewers_a_base_that_may_be_meant_and_is_the_users_step_otherwise`, `off_space_a_branch_lands_through_its_pull_request`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_carries_the_next_step_and_the_node_of_each_subject_and_a_patch_only_those_that_changed`)

#### Scenario: `brd` prints the step of the app's row
- **WHEN** `brd status` or `brd safe-push --dry-run` runs
- **THEN** it prints the step of what it read ([Agent CLI](../../cli/agent-cli/spec.md#brd-status)), the step of the app's row
  [@test] crates/bd-remote/src/landed_test.rs (`status_gives_each_branch_and_whole_stack_the_next_step_of_the_apps_row`)
  [@test] crates/bd-remote/src/safe_push_test.rs (`check_names_the_profiles_and_the_review_and_starts_nothing`)

### Requirement: The table: A step carries the facts its words need, never the words

A step MUST carry the facts its words need, never the words: its row, its kind with the pull request and the groups it
names, the blockers of the Safe Push and the Dry Run (a reason of the rules, the sync, or the step's own reason),
whether it waits for the readiness or stands on a stale answer of it, what landed all of the subject, the retarget of
"⋯" and the Safe Push without review offered beside.

#### Scenario: The page words the facts of the step
- **WHEN** the page renders a step
- **THEN** it words the facts the step carries, and decides nothing

### Requirement: The table: A fact without a fresh answer never answers a step a later one turns

A fact without a fresh answer MUST NOT answer a step a later answer would turn into another: one that loads or nobody
asks waits (row 0), a stale one keeps the step of the answer before (a stale readiness holds the action), a read that
failed is an answer that says so (what helps for Patronus, never missing approvals for the pull requests or the
readiness, the gate of the start for the landing needs).

#### Scenario: Every row with every fact without a fresh answer
- **WHEN** a fact of any row loads, is asked by nobody, is stale or failed
- **THEN** the step never is an action the fresh answer would replace
  [@test] crates/domain-policy/src/next_step_test.rs (`every_row_with_every_fact_without_a_fresh_answer_never_answers_an_action_the_fresh_one_would_replace`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`waits_for_the_first_answers_as_the_step_does_and_says_a_read_that_failed`)

### Requirement: The table: What runs on the row is the session's

What runs on the row (an operation, a start, a push, a move) MUST be the session's (`RepositoryView.operations`), which
the step presents.

#### Scenario: A move of the row
- **WHEN** an operation, a start, a push or a move runs on the row
- **THEN** the step and the node present it as the session's view has it, the node of the subject staying through a move
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move`)

### Requirement: The table: The derivation of the rows stays within its budget

The derivation of the 141 rows of the `perf:table` template, twice per flush, MUST stay within 20 ms in a debug build.

#### Scenario: The large table
- **WHEN** a flush derives the next steps of the 141 rows of the `perf:table` template twice
- **THEN** it takes 20 ms at most in a debug build
  [@test] crates/repository-model/src/derive_budget_test.rs (`a_flush_derives_the_next_steps_of_the_141_rows_of_the_large_table_within_its_budget`)

### Requirement: What the Safe Push needs: The session computes the landing needs after a sync

After a sync the session of the repository MUST compute, for every branch, the refusals the gate of its start would
give (`landingNeeds` of `BranchPatronus` in the Patronus part of its view, `trial_check::landing_gate::landing_needs`
through its host, the same rule as the gate): one trial check of every branch, its dangling references and missing
declarations, less what merged Safe Pushes landed. The row MUST say it before any press.

#### Scenario: A branch uses a name only a branch outside it adds
- **GIVEN** `lane-run`, whose commit imports a class `stills-codec` adds
- **WHEN** the sync ends
- **THEN** the landing needs of `lane-run` name `stills-codec`, which the gate would refuse only after the press ([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run))
  [@test] crates/trial-check/src/landing_gate_test.rs (`a_branch_that_uses_a_name_of_a_branch_outside_it_needs_that_branch_and_so_does_a_branch_built_on_it`, `the_needs_of_a_branch_are_the_refusals_of_its_start`, `a_landed_declaration_is_needed_by_nobody`, `a_running_safe_push_of_the_branch_that_adds_the_name_is_named_in_the_reason`)
  [@test] crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`)
  [@test] crates/trial-check/src/process_budget_test.rs (`the_landing_needs_of_every_branch`)

#### Scenario: A branch built on one that needs a branch
- **WHEN** a branch is built on one that needs a branch
- **THEN** it needs that branch too: its Safe Push lands that one

### Requirement: What the Safe Push needs: The needs are a fact with its inputs

The landing needs MUST be a fact with its inputs (the baseline, the tips of the table, the commits the view names
landed): unknown until computed, never "nothing", one computation at a time, not computed again while the inputs
stay. They come in a patch of the view (its `landingNeeds` state `loading` until then), the step waiting meanwhile
(row 0, [below](#facts-without-an-answer)); a failed computation is `unavailable` for its inputs, and no step waits
for it.

#### Scenario: One computation at a time
- **WHEN** the inputs change while a computation runs
- **THEN** the newest wanted inputs are computed next, and the same inputs never again
  [@test] crates/repository-model/src/facts_test.rs (`the_landing_needs_are_computed_one_at_a_time_the_newest_wanted_next`)

#### Scenario: A failed computation
- **WHEN** the computation of the needs fails
- **THEN** they are `unavailable` for its inputs, and no step waits for them

### Requirement: What the Safe Push needs: Row 1′ names the branch the build needs and runs the fix

Row 1′ MUST map the needs to the step, no rule of its own (`domain_policy::next_step`): "Needs stills-codec", its
hover the reason in the words of the gate's toast, and a click the fix: the restack of the root of what lands onto
that branch (`LandingFix`, `formatLandingFix`); "⋯" MUST offer the same fix first ("Stack lane-run on stills-codec").
A fix no action runs MUST open the Safe Push popover.

#### Scenario: The step, its hover and its fix
- **WHEN** `lane-run` needs `stills-codec`
- **THEN** the step is "Needs stills-codec", its hover the reason in the words of the gate's toast ("“Show each scenario” of lane-run uses Stills (lane/src/BuildLog.kt:3), which “Name lossy stills” of stills-codec adds: git merges it, but the build of lane-run fails. Stack lane-run on stills-codec to land it too."), and a click the fix: the restack of the root of what lands onto that branch (`LandingFix`, `formatLandingFix`); "⋯" offers the same fix first ("Stack lane-run on stills-codec")
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_needs_a_branch_outside_what_the_safe_push_lands_and_keeps_the_start`)
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the build needs and runs the fix, or opens the Safe Push`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say on the row which branch its Safe Push needs before any press, and stack it on that branch from the slot`, `should stack the branch on the branch its Safe Push needs from the menu of a branch built on it`)

#### Scenario: A fix no action runs
- **WHEN** the fix is the move of an unassigned commit, or a restack that would close a cycle
- **THEN** a click opens the Safe Push popover

### Requirement: What the Safe Push needs: Row 1 comes first, the restart and the approvals after

Row 1 (a running robot, a sync) MUST come before the landing needs; the restart of a failed robot and the approvals
MUST come after them: a restart would fail on the same build.

#### Scenario: A failed robot that needs a branch
- **WHEN** a branch whose robot failed needs a branch outside it
- **THEN** its step is the need, not the restart

### Requirement: What the Safe Push needs: The Safe Push stays enabled and its popover says why

The Safe Push of a branch with landing needs MUST stay enabled: its popover shows the reason before the start, with the
fix and Start Anyway ([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)). While the branch that adds
the name is landing in a running Safe Push, the reason MUST say so.

#### Scenario: The branch that adds the name is landing
- **WHEN** `stills-codec` is landing in a running Safe Push
- **THEN** the reason says "stills-codec is landing in its Safe Push, which tests it without this start: a start beside it fails too."
  [@test] src/presenters/landing.test.ts (`says that the branch the start needs is landing in a running Safe Push, which tests it without this start`)

### Requirement: What the Safe Push needs: The whole stack has no landing needs of its own

The whole stack MUST have no landing needs of its own: the gate of its start checks it.

#### Scenario: The step of the whole stack
- **WHEN** the step of the whole stack is decided
- **THEN** no landing need of its own decides it, and the gate of its start checks what it lands

### Requirement: What the Safe Push needs: A declared order is a need read of the repository

The order the user declares (`brd need`) MUST be a need too, read of the repository rather than computed, so it needs
no answer and comes before the waits of row 0; the whole stack MUST have the declared needs of every branch it lands
([Declared Needs](../declared-needs/spec.md#every-route)).

#### Scenario: A declared need
- **WHEN** the user declared with `brd need` that another branch lands first
- **THEN** the step is the need before any wait of row 0
- **AND** the whole stack has the declared needs of every branch it lands

### Requirement: What the Safe Push needs: The decision of the start reads the same needs

The decision of the start MUST read the same needs (`domain_policy::land::decide` over the view, `RepositoryView.lands`):
each a blocker of the landing gate that only Start Anyway lifts (`LandingAllowances`), the popover's reason and fix;
needs not computed yet are a wait ("Checking what lane-run needs to land…"), a failed computation none.

#### Scenario: The popover before the start
- **WHEN** the build of what the start lands needs a branch outside it
- **THEN** the popover opens with the reason, the fix and Start Anyway before the start
  [@test] crates/domain-policy/src/land_test.rs (`the_landing_gate_refuses_unless_allowed_then_a_conflict_with_the_target`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_decides_the_default_starts_of_each_branch_and_a_read_answers_another_intent`)
  [@test] src/presenters/land.test.ts (`opens the popover when the build of what it lands needs a branch outside it: the reason, the fix and Start Anyway before the start`)

#### Scenario: Needs not computed yet
- **WHEN** the needs are not computed yet
- **THEN** the start waits: "Checking what lane-run needs to land…"

### Requirement: Facts without an answer: The step decides only on answers

The step MUST decide only on answers: row 0 waits for the first fact the rows below would read without one
(`PendingFact`), never the step a later answer turns into another: an unknown pull request is no "no pull request", an
unanswered readiness no "approved". A reason of the branch (row 1, a robot of a branch below, a block, a parked
branch, an operation on the row) MUST outrank it; so does a known landing need (row 1′).

#### Scenario: The facts row 0 waits for
- **WHEN** a start, a repository switch or a sync gives the rows before the remote answers for them
- **THEN** row 0 waits while Patronus hasn't answered or its view of the robots it listed first isn't in (`isPatronusPending`), GitHub hasn't answered the listing (`isPullRequestsLoading`), the problems of the failed robot of the branch are still to come (`isRobotProblemsPending`), or the first answer of the readiness of the branch as it is hasn't come
  [@test] crates/domain-policy/src/next_step_test.rs (`row_0_waits_for_a_fact_no_answer_told_yet_and_a_reason_of_the_subject_outranks_it`, `every_row_with_every_fact_without_a_fresh_answer_never_answers_an_action_the_fresh_one_would_replace`)
  [@test] src/presenters/next-step.test.ts (`row 0: words the fact a step waits for and a read that failed, with what helps: a login to copy, a refresh, a restart disabled`, `keeps a stale answer, its action waiting for the new one, and a start reads only a fresh one`)
  [@test] src/stores/patronus.test.ts (`waits for Patronus until the view of the robots it listed first is in, then never again for a change of them`, `takes a failure of Patronus as its answer, and a list of no robot as having its view`, `waits for why a failed robot failed until its problems come, and no longer once Patronus didn't give them`)
  [@test] src/stores/pull-requests.test.ts (`is loading until GitHub's first answer, which a failure is too: no pull request is known meanwhile`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should wait with Checking… for the pull requests and the approvals, S and the menu saying why, then start the Safe Push they allow`)

#### Scenario: A start waits or confirms, never refuses
- **WHEN** a start is decided while a fact has no answer
- **THEN** it waits or confirms, and never refuses for it
  [@test] crates/domain-policy/src/land_test.rs (`every_fact_without_an_answer_waits_or_confirms_and_never_refuses`)
  [@test] src/presenters/land.test.ts (`waits for the facts Rust waits for, each saying what it waits for, and refuses with why a read failed`)

### Requirement: Facts without an answer: The Safe Push waits for the landing needs

The landing needs MUST be such a fact too (`Needs::Computing` of `domain_policy::next_step`: the session's check after
the sync hasn't answered for the view the rows read, its `landingNeeds` `loading`; "Checking what JIRA-456 needs to
land…"), so the Safe Push waits for them as for the other facts instead of offering a start whose gate may refuse it
after the press.

#### Scenario: The needs are still computed
- **WHEN** the check of what the branch needs to land hasn't answered
- **THEN** the step, S and the popover wait for the answer: "Checking what JIRA-456 needs to land…"
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should check what a branch needs to land before it offers its Safe Push, S and the popover waiting for the answer`)

### Requirement: Facts without an answer: Every surface waits the same way

Every surface MUST wait the same way: the slot shows a muted "Checking…" with its spinner, disabled, the fact on hover;
"⋯" keeps Safe Push and "Patronus ›" disabled with the same words, and so do the footer of the preview and the title
bar of the review window, from the first view of a repository Patronus knows (`LandingRemote::patronus`) until it
answers; where it doesn't know the repository none is shown (`PatronusStanding::of_remote`).

#### Scenario: The slot, the menu, the preview and the review window wait
- **WHEN** a fact the step reads has no answer yet
- **THEN** the slot shows a muted "Checking…" with its spinner, its hover "Waits for GitHub to list the pull requests", "Waits for the answer of Patronus", "Checking what JIRA-456 needs to land…", "Waits for Patronus to tell why the Safe Push failed" or "Waits for Patronus to check the approvals"
- **AND** "⋯", the footer of the preview and the title bar of the review window keep the starts disabled with the same words
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should wait with Checking… for the pull requests and the approvals, S and the menu saying why, then start the Safe Push they allow`)
  [@test] src/hooks/useBranchContextActions.test.ts (`says why a branch can't start a Safe Push in the words of its next step, and what runs on the row over it`, `offers Safe Push disabled until Patronus answers, and Create Pull Request… until GitHub lists the pull requests, as the review window does`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)

### Requirement: Facts without an answer: Create Pull Request… waits for the listing

"Create Pull Request…" of "⋯" MUST be there from the first view wherever git names the GitHub repository of the
branches (`LandingRemote::github`), disabled with "Waits for GitHub to list the pull requests" while the branch may
have one; once GitHub lists a live one, "Open Pull Request #7" takes its place. Off GitHub it is never shown. Create
Stack Pull Request… of a whole stack waits for the listing the same way.

#### Scenario: Before GitHub lists the pull requests
- **WHEN** GitHub hasn't listed the pull requests yet
- **THEN** "Create Pull Request…" of "⋯" is disabled with "Waits for GitHub to list the pull requests"
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should never offer Create Pull Request… before GitHub lists the pull requests, say a failed listing, and keep the last listing through a failed refresh`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

#### Scenario: GitHub lists an open pull request of the branch
- **WHEN** "⋯" shows "Create Pull Request…" waiting, and GitHub lists the open #7 of the branch
- **THEN** "Open Pull Request #7" is in the place of "Create Pull Request…", and the items above it stay in their places
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge, Create Pull Request… and Request Review… in their places while GitHub lists, the open pull request in the place of Create @webkit`)

### Requirement: Facts without an answer: The places of the menu stand from the first view

Whether the menu of a branch shows its starts and its pull request MUST be decided on what git tells at the first
view: whether Patronus knows the repository (`LandingRemote::patronus`) and the GitHub repository of the branches
(`LandingRemote::github`). An answer of Patronus or GitHub MUST only enable such an item or say why it is disabled: a
shown item never leaves, so the item under a resting pointer stays.

#### Scenario: A repository on Space
- **WHEN** `origin` is on Space and Patronus hasn't answered
- **THEN** "⋯" shows Safe Push and "Patronus ›", Safe Push disabled with "Waits for the answer of Patronus"
- **AND** after the answers the same items are in the same places, Safe Push enabled, and the item under the pointer is the one it rested on
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Safe Push in its place while Patronus answers, disabled until it does, and the item under the pointer where it was @webkit`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)

#### Scenario: The GitHub repository origin pushes to
- **WHEN** the route is the merge of the pull request, `origin` pushes to the GitHub repository of the branches, and GitHub hasn't answered
- **THEN** "⋯" shows "Merge", disabled with "Waits for GitHub to tell what you may do in the repository", "Create Pull Request…" and "Request Review…", disabled with "Waits for GitHub to list the pull requests"
- **AND** once GitHub lists #7, "Merge #7" is in the place of "Merge", "Open Pull Request #7" in the place of "Create Pull Request…", and "Request Review…" is enabled in its place
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge, Create Pull Request… and Request Review… in their places while GitHub lists, the open pull request in the place of Create @webkit`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)

#### Scenario: A mirror origin doesn't push to
- **WHEN** the branches end up in a GitHub repository `origin` doesn't push to (`origin` a path, `branchdeck.githubRepository` set)
- **THEN** "⋯" never shows "Merge", before GitHub answers or after
- **AND** "Create Pull Request…" waits for the listing, then is enabled in its place, and "Request Review…" says "JIRA-456 has no open pull request: Create Pull Request… asks for its review"
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should never offer Merge on a mirror origin doesn't push to, and keep Create Pull Request… in its place while GitHub lists`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A local remote
- **WHEN** `origin` is a path and nothing names a GitHub repository
- **THEN** "⋯" shows no start and no pull request from the first view, and none after the answers
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should offer no start and no pull request on a local remote from the first view`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Facts without an answer: S and the node of the row wait too

While the step waits for a fact, S MUST say it in an `info` notice ("Safe Push of JIRA-456 waits for GitHub"), and the
node of the row MUST wait too, the header counting no move of it
([Landing Overview](../../repository/landing-overview/spec.md#indicator)).

#### Scenario: S on a row that waits for GitHub
- **WHEN** the user presses S on a row whose step waits for GitHub to list the pull requests
- **THEN** an `info` notice says "Safe Push of JIRA-456 waits for GitHub"
- **AND** the node of the row waits, and the header counts no move of it
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should wait with Checking… for the pull requests and the approvals, S and the menu saying why, then start the Safe Push they allow`)

### Requirement: Facts without an answer: A Dry Run waits only for Patronus

A Dry Run MUST wait only for Patronus: approvals and pull requests never hold it back. The readiness store asks for
the answer the step waits for (`needsReadiness`).

#### Scenario: The pull requests and the readiness have no answer
- **WHEN** only the pull requests or the readiness have no answer yet
- **THEN** the Dry Run is not held back

### Requirement: Facts without an answer: The step of a start reads the facts of the start

The step of a start MUST read the facts of the start: it waits for a readiness without an answer and for GitHub's
first answer itself (the waits of `domain_policy::land::decide`, never `unlisted` before that answer), so neither holds
it back there.

#### Scenario: A start before GitHub's first answer
- **WHEN** a start is decided before GitHub's first answer
- **THEN** its decision waits for it, never `unlisted`

### Requirement: Facts without an answer: A listing GitHub can't give leaves the pull requests unknown

A failed answer MUST be an answer, never "no": a listing GitHub can't give (signed out, SSO, no access, or a failure
before any listing; `getPullRequestListing` `unlisted`, the step's `unlisted`, `formatAvailabilityHint`) leaves the
pull requests unknown, so rows 5 to 10 don't apply and the Safe Push stays, its hover saying it, the node the same, and
its start confirms first.

#### Scenario: GitHub offline
- **WHEN** GitHub can't give the listing
- **THEN** the Safe Push stays, its hover "Lands JIRA-456 into master once the checks pass · GitHub: offline: Patronus checks the approvals when the robot starts", the node the same
- **AND** its start confirms first
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should never offer Create Pull Request… before GitHub lists the pull requests, say a failed listing, and keep the last listing through a failed refresh`)

### Requirement: Facts without an answer: A failed refresh keeps the last listing

A refresh GitHub fails for a while after a listing (an error, offline, the rate limit) MUST keep that listing
(`listed`): the steps read its pull requests as of then, so a failed refresh never turns a wait back into a Safe Push.

#### Scenario: A refresh fails after a listing
- **WHEN** a refresh of GitHub fails for a while after a listing
- **THEN** the steps read the pull requests of that listing, a wait stays a wait
- **AND** once the token is gone, the listing is not kept
  [@test] src/stores/pull-requests.test.ts (`keeps the last listing through a refresh GitHub failed for a while, never once its token is gone`)

### Requirement: Facts without an answer: A view Patronus didn't give leaves the step to the rows below

A view Patronus didn't give MUST leave the step to the rows below.

#### Scenario: No view of Patronus
- **WHEN** Patronus gave no view
- **THEN** the rows below decide the step

### Requirement: Facts without an answer: A read of Patronus that failed decides no start

A read of Patronus that failed MUST decide no start (`UnavailableFact`, `StepKind::Unavailable` of
`domain_policy::next_step`), after row 1 and the reasons of the branch; the step MUST be what helps. The GitHub mirror
is unknown then too, so no start goes as if the repository were only on Space.

#### Scenario: The Safe Push CLI is not logged in
- **WHEN** the Safe Push CLI is not logged in (`notLoggedIn`)
- **THEN** the step is "Copy Login Command", the command of the hint for a terminal ("Run ./safePush.cmd once to log in to Patronus: the command goes to the clipboard, for a terminal")
  [@test] crates/domain-policy/src/next_step_test.rs (`row_0_a_read_that_failed_decides_no_start_and_offers_what_helps`)
  [@test] src/presenters/next-step.test.ts (`row 0: words the fact a step waits for and a read that failed, with what helps: a login to copy, a refresh, a restart disabled`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should offer the login instead of a Safe Push while the Safe Push CLI is not logged in, and a refresh while Patronus fails`)

#### Scenario: Patronus fails before it listed the robots once
- **WHEN** Patronus fails (`error`) before it listed the robots once
- **THEN** the step is "Refresh Patronus", which asks it again

#### Scenario: Safe Push and Dry Run of the menu, and S
- **WHEN** a read of Patronus failed
- **THEN** Safe Push and Dry Run of "⋯" are disabled with the hint
- **AND** S says it in an `info` notice ("Safe Push of JIRA-456 can't start") whose action is the step's

### Requirement: Facts without an answer: An error after a list leaves the steps on the robots it listed

An error of Patronus after a list MUST leave the steps on the robots it listed (`Stale`, `PatronusStanding`), as a
failed refresh of GitHub keeps the last listing; the landing overview says the error.

#### Scenario: A failed refresh of Patronus
- **WHEN** a refresh of Patronus fails after it listed the robots
- **THEN** the steps stand on the robots it listed, and the landing overview says the error

### Requirement: Facts without an answer: A view of the robots that failed is no "nothing holds it"

A view of the robots the session couldn't derive while no earlier one stands for it (`unavailable`, the failure of
`PatronusStanding`) MUST NOT be "nothing holds it": "Refresh Patronus", "Patronus didn't tell what its robots hold and
land (…)". Over an earlier view the steps stand on that one (`stale` with why), its holds, landing ancestors and needs
stale until the next answer.

#### Scenario: No view stands for the robots
- **WHEN** the session couldn't derive the view of the robots and no earlier one stands for it
- **THEN** the step is "Refresh Patronus", saying "Patronus didn't tell what its robots hold and land (…)"
  [@test] src/stores/patronus.test.ts (`keeps the last view through a failed derivation of it, and says the failure while no view stands for the robots`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should offer no start while Patronus gave no view of its robots, and stand on what it listed through a failed refresh`)

#### Scenario: An earlier view stands
- **WHEN** the derivation fails over an earlier view
- **THEN** the steps stand on that one, `stale` with why

### Requirement: Facts without an answer: A failed view is derived again only after a new input

The session MUST derive a view of the robots that failed again after the next answer of Patronus (the refresh asks
one), a sync or a push, never by itself.

#### Scenario: The next answer of Patronus
- **WHEN** Patronus answers again after a failed derivation
- **THEN** the session derives the view again
  [@test] crates/repository-model/src/session_test.rs (`a_view_that_failed_is_derived_again_after_the_next_answer_of_patronus`)

### Requirement: Facts without an answer: Problems not given keep the restart disabled

Problems Patronus didn't give for the failed robot of a branch or of a whole stack (`problemsFailed`), like problems
still to come, MUST keep the restart and the prompts for an agent disabled with the reason on every surface of the
robot (the menu, the landing overview, the details of its chip; `robotProblemsReason`). Check Approvals Again asks for
them again (`askRobotProblemsAgain`).

#### Scenario: The problems are still to come, or failed
- **WHEN** the problems of a failed robot are still to come, or Patronus didn't give them
- **THEN** the restart and the prompts are disabled, the details of its chip saying "Waits for Patronus to tell why the Safe Push failed" or "Patronus didn't tell why the Safe Push failed (…): Check Approvals Again asks again"
- **AND** the landing overview shows the branch waiting with its step while they are to come, never a robot to fix by a restart
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should restart a failed Safe Push and copy its prompt only once Patronus tells why it failed`, `should say a failed answer of why a Safe Push failed, its restart disabled until Check Approvals Again asks again`)

#### Scenario: Check Approvals Again asks for them again
- **WHEN** the user runs Check Approvals Again
- **THEN** the problems of the failed robot are asked again, and the readiness with recheck
  [@test] src/stores/review-readiness.test.ts (`asks again with recheck after the pull requests from GitHub, the answer stale meanwhile`)

### Requirement: Facts without an answer: Signed out of GitHub, the start without review stays beside

While GitHub can't list the pull requests (signed out, failing; the step's `unlisted`), row 7 may apply: no pull
request is known and the approvals may be missing, so the Safe Push of row 11 MUST keep the start without review beside
it (`withoutReview`, "Safe Push Without Review…" of the pull request dialog), its blocker the reason of the branch.

#### Scenario: GitHub signed out
- **WHEN** GitHub is signed out
- **THEN** the step is Safe Push, and the pull request dialog offers "Safe Push Without Review…"
  [@test] crates/domain-policy/src/next_step_test.rs (`row_11_safe_push_and_the_start_without_review_while_github_is_signed_out`)
  [@test] src/presenters/next-step.test.ts (`row 11: Safe Push by the branches it lands, and what Patronus checks while GitHub lists no pull request`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should leave the Safe Push without review to the user while GitHub is signed out, from the pull request dialog`)

#### Scenario: The review window asks for it
- **WHEN** the review window asks for the start without review while the main window has none
- **THEN** it is refused with the step's reason (what it waits for), else "The approvals of auth don't wait for a pull request now"

### Requirement: Facts without an answer: A stale readiness keeps its step, its action waits

A stale answer of the readiness (the branch moved on: a new tip, a new review, Check Approvals Again) MUST keep its
step while the new one loads, so the row doesn't flicker, but its action MUST wait for the new answer, which may decide
another step.

#### Scenario: A new tip while the step is Safe Push
- **WHEN** the readiness of the branch is stale
- **THEN** Safe Push, Restart, Create Pull Request… and Request Review… are disabled with "Waits for Patronus to check the approvals again", S says it
- **AND** a quiet step only opens the pull request and stays
  [@test] crates/domain-policy/src/next_step_test.rs (`a_stale_readiness_keeps_its_step_and_its_action_waits_for_the_new_answer`)
  [@test] src/presenters/next-step.test.ts (`keeps a stale answer, its action waiting for the new one, and a start reads only a fresh one`)

### Requirement: Facts without an answer: A value of the cached frame is a value, never a wait

A value of the cached frame of an earlier run (`FactState::Cached`: the robots, the pull requests, the view of the
robots, the landing needs, the readiness the frame saved, which no answer of this run replaced yet) MUST be a value,
never a wait: every row decides over it the step and the node the same answers would decide fresh, so the frame shows
its steps from its first paint.

#### Scenario: The first paint of the frame
- **WHEN** the window shows the cached frame of an earlier run
- **THEN** every row shows the step the same answers would decide fresh
- **AND** a readiness of the frame is a stale answer, its step kept and its action waiting for the fresh one
  [@test] crates/domain-policy/src/next_step_test.rs (`every_row_over_the_values_of_the_cached_frame_decides_the_step_of_the_fresh_answers`)

### Requirement: Facts without an answer: A start reads only fresh answers

A start MUST read only fresh answers: its decision waits for each value of the cached frame as it waits while the fact
loads (`domain_policy::land::standing`), and every remote action of the frame waits for the first sync
([Branch Row](../branch-row/spec.md#actions-cell)).

#### Scenario: A start over the cached frame
- **WHEN** a start is decided over a value of the cached frame
- **THEN** it waits for the fresh answer
  [@test] crates/domain-policy/src/land_test.rs (`every_fact_without_an_answer_waits_or_confirms_and_never_refuses`)

### Requirement: Facts without an answer: The first fresh answer after the frame is no news

The first fresh answer after a value of the frame MUST be no news (`domain_policy::news::tells_after`): a robot that
finished while the app was closed shows, nothing tells it.

#### Scenario: A robot finished while the app was closed
- **WHEN** the first fresh answer shows that a robot of the frame finished
- **THEN** the row shows it, and no news tells it
  [@test] crates/domain-policy/src/news_test.rs (`an_answer_after_the_value_of_the_cached_frame_is_no_news`)
  [@test] crates/repository-model/src/facts_test.rs (`a_robot_of_the_cached_frame_that_the_first_fresh_answer_shows_finished_is_no_news`)

### Requirement: The whole stack: The whole stack has the same table over its stack branch

The whole stack of a root ([Whole Stack](../whole-stack/spec.md)) MUST have the same table, its facts those of its
stack branch `{prefix}/stack/<root>` while the stack forks (`next_steps` of `repository_model::derive` under
`stack:<root>`, `getStackNextStep`, `useStackNextStep`); a branch of it that syncs is its sync. Its texts MUST name it
"the whole air stack" (`subject`).

#### Scenario: The facts of the whole stack
- **WHEN** the step of the whole stack is decided
- **THEN** it reads the robot of the whole stack, the robot of a branch of it that lands a part of it (the start waits for it, a dry run doesn't), its own pull request (never retargeted: it goes into the target by construction), the readiness of the whole stack (`stack:<root>`, below) and its stack branch's commits
- **AND** its texts say "Safe Push of the whole air stack waits for Ilya Korennoy"
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A straight chain
- **WHEN** the stack is a straight chain, which has no stack branch
- **THEN** its whole stack is the step of its last branch, which waits for the robot of a branch of it as the stack branch does (row 2)

### Requirement: The whole stack: A stack branch the sync didn't build disables its starts

A stack branch the sync didn't build MUST disable its Safe Push and its Dry Run with the reason
(`WholeStackView.forking.notBuilt`, `domain_policy::reasons::stack_not_built`).

#### Scenario: The stack branch conflicts
- **WHEN** the stack branch conflicts at 0caf9066 in config.js
- **THEN** its Safe Push and Dry Run are disabled with "The stack branch conflicts at 0caf9066 in config.js: the stack lands in pieces"

### Requirement: The whole stack: Row 7 of the whole stack creates its pull request

Row 7 of the whole stack MUST offer "Create Pull Request of the Stack…" ("Create Pull Request…" in the slot of the
island header and the landing overview), the code owners of the whole stack preselected.

#### Scenario: The header creates the pull request
- **WHEN** the user creates the pull request of the whole stack from its header
- **THEN** it asks the code owners of the whole stack, as the step of a branch does, the move made from the click
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does, the move made from the click`)

### Requirement: The whole stack: Its Safe Push lands every branch of it

The Safe Push of the whole stack MUST land every branch of it, and say so: the count in its label, the branches in
its hover, the main line first, then the side branches.

#### Scenario: A stack of 7 branches
- **WHEN** the whole stack has 7 branches
- **THEN** its step is "Safe Push · 7", its hover "Lands 7 branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and side-test"

### Requirement: The whole stack: It lands only what merged Safe Pushes left of it

The step of the whole stack MUST know which branches of it merged Safe Pushes landed already (`StackLanding.rest`,
`domain_policy::stack::whole_stack_rest`, the same `getLandedBy` as the rows): a piece that landed its main line
leaves the rest, which its Safe Push lands. The stack branch still carries the landed commits (Patronus finds them in
the target), so the start MUST name and count only the rest.

#### Scenario: A piece landed its main line
- **WHEN** a piece landed air, bazel and deploy, and side is left
- **THEN** the step is "Safe Push", its hover and popover "Lands 1 branch · 2 commits into master in one Safe Push: side; air, bazel and deploy landed with deploy"
- **AND** a branch landed with commits after it counts those
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should land only what is left of the whole stack after a piece landed its main line, and offer nothing once every branch of it landed`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: Every branch of it landed
- **WHEN** every branch of it landed, in pieces or by its own robot
- **THEN** nothing is left: row 1, Safe Push Whole Stack and Dry Run Whole Stack disabled with "Landed in master with deploy and side" ("Merged by Patronus" of its own robot)

### Requirement: The whole stack: The approvals of its pull request count for its Safe Push only

Approvals MUST count only in the pull request whose comment started the robot, so those of the whole stack count for
its Safe Push only. While its pull request is open, row 7 of a branch of it without a pull request of its own MUST be a
quiet "In #12" that opens it; the Safe Push of the branch itself still needs approvals of its own, as S says.

#### Scenario: A branch without its own pull request
- **GIVEN** #12, the open pull request of the whole air stack
- **WHEN** a branch of it has no pull request of its own
- **THEN** its step is a quiet "In #12", the reason on hover ("Approvals in #12, the pull request of the whole air stack, count for its Safe Push only")
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] src/presenters/next-step.test.ts (`row 7: Create Pull Request… naming the code owners, or In #12 of the whole stack, quietly`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: The popover of a piece of the stack
- **WHEN** the user starts the Safe Push of a piece of the stack while #12 is open
- **THEN** the popover says first "#12, the pull request of the whole air stack, is open: its approvals count only for the Safe Push of the whole stack, not for this Safe Push of deploy." (a confirm reason, `wholeStack`)
  [@test] src/presenters/land.test.ts (`opens the popover for a key and for each confirmation of the decision: a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open`)

### Requirement: A push into the target: The route of a push takes its own table

A repository that lands by a push of the copy into the target (`LandingRoute::Push`,
[Landing Routes](../landing-routes/spec.md#push-to-master)) MUST take the table of its route, on Space or off it,
whatever Patronus answers: row 1 while the Status cell says why nothing lands now, row 0 while the gate of the route
has no answer (`permission`, `targetRules`; a reason of the subject outranks it) and then while the pull requests or
the landing needs have none, 1′ and 1″, then its rows.

#### Scenario: The rows of the route
- **WHEN** the step of a branch of a repository that lands by a push is decided
- **THEN** row 1 comes first while the Status cell says why nothing lands now (a sync, a failure, no commits, blocked, held, parked, landed, its robot running), then row 0, 1′ and 1″, then the first of these rows that matches

  | # | State | Slot | Start elsewhere |
  |---|---|---|---|
  | push′ | The fetched baseline has commits the fork point of the checked out branch lacks, which the copy is built on (`baselineAhead`, the gate of the route), unless a landing in this session holds the same target ref | "Update Project" (⌘T's update), its node the user's step | Push to master disabled: "origin/master has new commits: Update Project rebuilds the copy on the new baseline"; while that landing is in flight, Push to master stays enabled and queues behind it |
  | push | Nothing of the table above holds it | "Push to master" ("Push to master · 3" for what it is built on), its node ready | disabled with the refusal of the gate ("You have no write access to acme/monorepo"), else enabled: its popover confirms what CI did not check and what the gate asks |

  [@test] crates/domain-policy/src/next_step_test.rs (`a_push_into_the_target_is_the_step_of_its_route_and_its_gate_holds_it_back`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`a_push_into_the_target_is_ready_and_a_refusal_of_its_gate_waits`)
  [@test] src/presenters/next-step.test.ts (`row push: Push to master by the branches it lands, its start disabled with the refusal of the gate of the route`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: No write access
- **WHEN** the token has no write access to the repository
- **THEN** Push to master is disabled with the refusal of the gate ("You have no write access to acme/monorepo")
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should disable Push to master with the reason when the token has no write access to the repository`)

### Requirement: A push into the target: A Dry Run is never offered on the route of a push

A Dry Run is a robot of Patronus: it MUST NOT be offered on this route (`dryRunNeedsPatronus`). The whole stack of a
root that forks MUST take the same rows in its header.

#### Scenario: The menus of a branch on the route of a push
- **WHEN** the repository lands by a push
- **THEN** no Dry Run is offered, and the header of the whole stack of a root that forks takes the same rows

### Requirement: A push into the target: The slot starts the push as Safe Push does

The slot MUST be the start of the row as "Safe Push" is (`SafePushButton`: its steps while it runs, "Pushing 45%"): a
click opens its popover at the slot when it asks something, S opens it always, Enter pushes; the menus, the preview,
the review window and the header of an island start the same ([Landing Routes](../landing-routes/spec.md#the-page)).
The label MUST name the target.

#### Scenario: A push from the slot
- **WHEN** the user clicks Push to master in the slot of the row
- **THEN** the popover asks what CI did not pass, the push lands the copy, and the project updates after it
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push the copy into master from the slot of the row, the popover asking what CI did not pass, and update after it`)

#### Scenario: S on the row
- **WHEN** the user presses S on the row
- **THEN** the popover of Push to master opens
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should open the popover of Push to master with S on the row`)

#### Scenario: The label names the target
- **WHEN** the baseline is `origin/main`
- **THEN** the label is "Push to main"

### Requirement: A push into the target: A fetched baseline ahead of the copy asks for Update Project

When the fetched baseline has commits the copy is not built on (row push′), the step MUST be Update Project, which
builds the copy anew on them, and the push MUST wait for it on every surface: its push would be no fast-forward, which
the server refuses after a round trip. `brd land` MUST refuse it as `baseline_moved` with `brd update` next, pushing
nothing.

#### Scenario: origin/master has new commits
- **WHEN** the fetched baseline has commits the fork point of the checked out branch lacks
- **THEN** the step is Update Project, and Push to master waits for it with why
- **AND** after the update the push runs
  [@test] crates/domain-policy/src/next_step_test.rs (`a_fetched_baseline_ahead_of_the_copy_offers_update_project_before_the_push`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_fetched_baseline_ahead_of_the_fork_point_refuses_the_push_until_the_update`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`a_fetched_baseline_ahead_of_the_copy_makes_the_update_the_users_step`)
  [@test] src/presenters/next-step.test.ts (`row push′: updates the project, whose fetched baseline has commits the copy is not built on, and Push to master says why it waits`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should offer Update Project while the fetched master has commits the branch is not built on, and push after it`)

### Requirement: A push into the target: A landing in flight on the same target queues the push

While another landing in this session holds that target ref, the push gate MUST stay open (or keep its confirmations)
and the action MUST remain Push to master, so it queues instead of refusing; a target moved outside the session still
takes the Update Project step.

#### Scenario: A second push to master
- **WHEN** the user pushes a second branch to master while the first landing's update and sync run
- **THEN** the step stays Push to master and the second push queues until they finish
  [@test] crates/domain-policy/src/next_step_test.rs (`a_push_to_master_stays_the_next_step_while_another_landing_is_in_flight`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should queue a second push to master until the first landing's update and sync finish`)

#### Scenario: A target moved outside the session
- **WHEN** the target moved outside the session
- **THEN** the step is Update Project

### Requirement: A merge of the pull request: The route of a merge takes its own table

A repository that lands by a merge of the pull request on GitHub (`LandingRoute::PullRequest`,
[Landing Routes](../landing-routes/spec.md#merge)) MUST take the table of its route while no robot of Patronus lands
it (Patronus answers that the repository is not its) and a merge there lands into its target; the gate refuses
neither a mirror nor a repository off GitHub: there the rows of the Patronus table stay, the slot hidden.

#### Scenario: The rows of the route
- **WHEN** the step of a branch of a repository that lands by a merge is decided
- **THEN** rows 1, 0, 1′ and 1″ come first as a push's (row 0 also while the listing of the pull requests has no answer, and while GitHub has not computed whether it can merge the pull request, `mergeState`: `UNKNOWN` is asked again, never "no")
- **AND** then the first of these rows that matches by what GitHub tells of the open pull request of the subject into the target (`land_direct::merge_of`)

  | # | State | Slot | Start elsewhere |
  |---|---|---|---|
  | pr | No open pull request into the target | "Create Pull Request…", its node the user's step | Merge disabled: "auth has no open pull request: create one, GitHub merges it" |
  | merge | GitHub can merge it (`CLEAN`, `HAS_HOOKS`, `UNSTABLE` with a confirmation); its required checks still run (`BLOCKED` by them) where the repository allows auto-merge; it shows another commit than the copy, which a push restarts the checks of | "Merge #7", or "Auto-Merge #7" (the push first when it shows another commit), its node ready | disabled with the refusal of the gate ("You have no write access to acme/monorepo"), else enabled: its popover names the method and what it asks |
  | merge′ | Its auto-merge is enabled | "Merges when checks pass", Cancel Auto-Merge; its node landing | — |
  | merge″ | What holds the merge back: a draft, changes requested, approvals the base requires, required checks that fail, or still run without auto-merge, a head behind its base (`BEHIND`), a conflict (`DIRTY`), a pull request that shows another commit without auto-merge, what GitHub doesn't name | its reason; the node a fix (a conflict, changes, failing checks), the user's step (a head behind, a branch to push, a draft) or a wait (approvals, running checks) | Merge disabled with the reason |

  [@test] crates/domain-policy/src/next_step_test.rs (`a_merge_of_the_pull_request_is_the_step_of_its_route_by_what_github_tells`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`a_merge_of_the_pull_request_is_ready_landing_a_fix_or_a_wait_by_what_holds_it_back`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_merge_of_the_pull_request_offers_its_merge_and_cancel_auto_merge_while_github_merges_it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A pull request into a virtual branch or another base
- **WHEN** the open pull request goes into a virtual branch or another base
- **THEN** it takes rows 5 and 5′ of the table above, the merge disabled with the base

### Requirement: A merge of the pull request: A Dry Run is never offered on the route of a merge

A Dry Run is a robot of Patronus: it MUST NOT be offered on this route (`dryRunNeedsPatronus`). The whole stack of a
root that forks MUST take the same rows in its header, of the pull request of its stack branch.

#### Scenario: The menus of a branch on the route of a merge
- **WHEN** the repository lands by a merge of the pull request
- **THEN** no Dry Run is offered, and the header of the whole stack of a root that forks takes the rows of the pull request of its stack branch

### Requirement: The surfaces: The slot renders the step and hosts the popover of the start

The slot (`NextStepSlot`, the step column of the Actions cell, [Branch columns](../../../../docs/design/ui-theme.md#branch-columns))
MUST render the step and host the popover of the branch's start (`SafePushPopover`, anchored to the slot).

#### Scenario: The step in the slot
- **WHEN** a branch row shows
- **THEN** its slot renders the step, and the popover of its start opens anchored to the slot

### Requirement: The surfaces: The menu offers the starts disabled with the reason of the step

"⋯" MUST have "Safe Push" (S) first and in its "Patronus ›" "Safe Push With Profile…", "Dry Run" and "Check Approvals
Again" (`check-approvals`, always enabled), each disabled with the reason of the step, and "Retarget #7 to master" for
any open pull request into another base.

#### Scenario: The menu of a branch that waits
- **WHEN** the user opens "⋯" of a branch whose step holds the Safe Push back
- **THEN** "Safe Push", "Safe Push With Profile…" and "Dry Run" are disabled with the reason of the step, "Check Approvals Again" enabled
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

#### Scenario: A pull request into another base
- **WHEN** the open pull request goes into another base
- **THEN** "⋯" offers "Retarget #7 to master"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the retarget of a pull request into the parent's branch as the next step, whose approvals don't count`)

### Requirement: The surfaces: S opens the popover, or says why in a notice

S on the focused row MUST open the popover, or, disabled, say why in an `info` toast (a precondition, `notice`): a
short title naming the branch, the reason as the description, with the action that helps. A click that waited for an
answer of the start (the profiles, a readiness the branch moved past meanwhile) MUST say the same notice when the
answer is the step's reason.

#### Scenario: The notice of S
- **WHEN** the user presses S on a row whose Safe Push is disabled
- **THEN** the notice says "Safe Push of auth waits for Ilya Korennoy" / "IJPL Code Platform · requested on #1078", "Safe Push of auth waits for 2 reviews", "#7 of auth is a draft" or "Safe Push of auth can't start" / "Landed in origin/master"
- **AND** its action is "Open Pull Request" while the step waits in it, the step's own action while it is the user's ("Create Pull Request…", "Create Pull Request of the Stack…" for the whole stack, row 7; "Request Review…", row 9; "Retarget #7 to master", row 5), else "Open in Patronus" of the robot that runs on the branch or lands it ("A Safe Push of the last commit is running")
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on S why the whole stack can't start without its pull request, and create it from the notice`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should open Request Review… from the notice of S while nobody of a group is asked`)
  [@test] src/presenters/next-step.test.ts (`words the notice of S as a short title naming the subject, a reason of the subject first`)

### Requirement: The surfaces: The preview, the review window and the stack header read the same blocker

The preview footer, the review window (`getBranchReviewItems`), S in the preview and the stack header MUST read the
same blocker as the row. Your Turn is not a branch row: its Safe Push of a reviewed pull request has its own check
([Review Requests](../../repository/review-requests/spec.md)).

#### Scenario: S in the preview
- **WHEN** the user presses S in the branch preview
- **THEN** the Safe Push popover opens, and its footer button starts it
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)

### Requirement: The surfaces: The node of the row renders the step as where the branch is

The node of the row, at the left edge of the name cell, MUST render the step too, as where the branch is on its way
into master ([Branch Row](../branch-row/spec.md#tree), `getLandingNode` over the same facts): ready to land, the
user's step first, a fix, landing now, a wait, in master.

#### Scenario: What the slot can't say
- **WHEN** the slot is empty (row 1: a robot runs, the branch is held, landed, blocked), or the repository is not on Space, which has no slot
- **THEN** the node says why, or the pull request it lands through
- **AND** the header of a stack island has the node of the whole stack
  [@test] crates/domain-policy/src/landing_node_test.rs (`is_ready_to_land_once_patronus_judged_the_approvals_else_the_start_is_the_users_step`, `waits_for_others_the_reviewers_a_base_that_may_be_meant_and_is_the_users_step_otherwise`)
  [@test] src/presenters/landing-node.test.ts (`words where a start lands it: ready with the branches it lands, the user's step while Patronus checks the approvals at the start`)

### Requirement: The surfaces: A quiet step is the state as its label

A quiet step MUST be the state as its label (the [UX Writing](../../../../docs/contributing/ux-writing.md#actions)
exception): a ghost `xs` button in `text-muted-foreground` (`destructive` for Changes Requested), the `ClockIcon`
before "Awaiting …"; the label truncates to the slot. The pull request chip keeps its own state: the row doesn't say
"awaiting" twice.

#### Scenario: The hover of a quiet step
- **WHEN** the pointer rests on a quiet step
- **THEN** the hover gives the whole text, a line per missing group ("Awaiting Ilya Korennoy · IJPL Code Platform", "1 more approval · IJPL Code Platform" for a group that has an approval and needs more), then "Open Pull Request #1078" with O
- **AND** O opens the pull request on the focused row too

### Requirement: The surfaces: The start renders the step with the readiness as it is

The start of a Safe Push MUST render the step too (`presentLand` of Rust's decision,
[Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)): it reads the next step with the readiness of the
branch as it is (never a stale one: the start waits for the new answer, the `waits` of its decision), so a refusal
says the reason of the step, and missing approvals refuse it unless the chosen profile is known to skip the review
(`skipsReview`).

#### Scenario: Missing approvals at the start
- **WHEN** the approvals turn out missing at the start
- **THEN** the popover says why and the start is refused
  [@test] tests/e2e/next-step/next-step.spec.ts (`should say in the popover that Patronus doesn't tell whose approvals count, and refuse the start once they are missing`)

#### Scenario: A profile that skips the review
- **WHEN** the options of the last robot's profile said it skips the review
- **THEN** the start with that profile starts
  [@test] tests/e2e/next-step/next-step.spec.ts (`should start with the profile of the last robot when its options said it skips the review`)

### Requirement: The surfaces: A Safe Push without review only from the pull request dialog

Row 7 MUST also offer a [Safe Push without review](../patronus/spec.md#safe-push-without-review) in the "Create pull
request" dialog (`BranchNextStep.withoutReview`), never on the Safe Push surfaces, which stay disabled.

#### Scenario: Row 7
- **WHEN** the step is row 7
- **THEN** the "Create pull request" dialog offers the Safe Push without review, and the Safe Push surfaces stay disabled

### Requirement: The surfaces: A push that dismisses the approvals refuses the start

A push that the rules of the repository make dismiss the approvals Patronus counts (`approved`) would start the robot
without them: the start MUST be refused with "The push dismisses the approvals of #7: push first, then ask for the
review again", and the push alone offered ("Push Branch").

#### Scenario: The rules dismiss approvals on a push
- **WHEN** the start of an approved branch needs a push that dismisses its approvals
- **THEN** the start is refused with "The push dismisses the approvals of #7: push first, then ask for the review again" and "Push Branch" is offered

### Requirement: The readiness store: The session asks the readiness, the store applies its answers

The session of the repository MUST ask the readiness (`remote_facts::scheduler`, `repository_model::readiness_facts`;
`facts.readiness` of its view, by subject); `src/stores/review-readiness.ts` only applies its answers, for the row, the
start, the "Create pull request" dialog (its code owners) and the stack header.

#### Scenario: A window needs the readiness
- **WHEN** a window needs the readiness of a subject
- **THEN** the session asks the fake Patronus for it, and the store applies the answer
  [@test] crates/test-server/src/review_readiness_test.rs (`the_session_asks_the_readiness_a_window_needs_from_the_fake_patronus`)

### Requirement: The readiness store: The window tells what to ask, needed and visible

The window MUST tell what to ask (`src/model/demand.ts`): the subjects a start (a click that waits, its popover) or
the dialog needs now (`demandReviewReadiness`, `needed`), and of the subjects a surface shows (`showReviewReadiness`,
through `useBranchNextStep`) those whose next step depends on the readiness (`needsReadiness`), as visible branches
and stacks.

#### Scenario: Needed and visible subjects
- **WHEN** a surface needs the readiness now, or shows a subject whose step depends on it
- **THEN** the window sends the first as needed and the second as visible
  [@test] src/model/demand.test.ts (`sends the readiness a surface needs now as needed, and the subjects whose step a surface shows as visible`)

#### Scenario: The steps that depend on the readiness
- **WHEN** the subject is on GitHub with the pull requests listed, and past rows 1 to 6
- **THEN** its step depends on the readiness, pushed or not, as Patronus judges the local commits, so an unpushed branch's step is never an answer left from before; a stacked branch too, for its own pull request
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)

#### Scenario: The visible subjects
- **WHEN** the window tells the subjects it shows
- **THEN** they are a row on the screen, not one a collapsed stack, side stack or island hides (it is not rendered), and of the root of a stack that forks its whole stack too, so a small stack without an island knows the approvals of its whole stack before a click
- **AND** the header of an island (its whole stack, below, or the last branch of a straight chain, also while collapsed), the preview and the review window (its chain, or `stack:<root>` of the review of the whole stack)

#### Scenario: The root of a small stack
- **WHEN** the row of the root of a small stack without an island shows
- **THEN** the approvals of its whole stack are asked before a click, and its pull request offered where they are missing
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask for the approvals of the whole stack of a small stack while its root's row shows, and offer its pull request where they are missing`)

### Requirement: The readiness store: The session asks only the subjects whose step waits for it

The session MUST ask only those of the shown subjects whose own next step waits for the readiness
(`domain_policy::readiness::wanted` over its next steps, `repository_model::derive::wanted_readiness`), so a shown
subject past which the step doesn't read the readiness (a sync, a robot that runs, a retarget, a draft, the pull
requests not listed yet) costs Patronus nothing.

#### Scenario: A shown subject whose step doesn't read the readiness
- **WHEN** a shown subject syncs, has a running robot, a retarget, a draft, or its pull requests are not listed yet
- **THEN** the session doesn't ask its readiness
  [@test] crates/domain-policy/src/readiness_test.rs (`the_session_asks_the_readiness_of_the_shown_subjects_whose_step_waits_for_it`)
  [@test] crates/repository-model/src/readiness_facts_test.rs (`a_shown_subject_is_asked_only_while_its_next_step_waits_for_the_readiness`)

### Requirement: The readiness store: The ask of a subject is what its row shows

The ask of a subject MUST be what its row shows: the tip (the copy the sync built), and the start of the robot: the
open pull request into the target with its reviewers and author while Patronus names the GitHub repository, else
through the API (`domain_policy::pull_request::start_pull_request`; the page's own `readinessInputOf` follows the same
rule).

#### Scenario: The tip and the start
- **WHEN** the session asks the readiness of a branch
- **THEN** it asks the tip a window shows and the pull request its robot starts from
  [@test] crates/repository-model/src/readiness_facts_test.rs (`the_ask_is_the_tip_a_window_shows_and_the_pull_request_its_robot_starts_from`)
  [@test] crates/domain-policy/src/pull_request_test.rs (`a_safe_push_starts_from_the_open_pull_request_into_the_target_or_a_virtual_branch_while_patronus_names_the_github_repository`)

### Requirement: The readiness store: The whole stack is one more subject

The whole stack MUST be one more subject, `stack:<root>` (`stackReadinessKey`, a name no branch has): asked as its
stack branch (`ReviewReadinessBranch.source`) with the pull request of the whole stack, only while the sync built the
stack branch (another state has nothing its Safe Push would land).

#### Scenario: The stack branch is not built
- **WHEN** the sync didn't build the stack branch
- **THEN** the readiness of `stack:<root>` is not asked

### Requirement: The readiness store: A demand and a recheck go at once, the rest is a queue

What a surface needs and the rechecks of Check Approvals Again MUST go at once, each in a call of its own beside
whatever is in flight: a click waits for no pre-check but its own. The rest MUST be a queue: the kept answers of all of
it first, in one read (the object cache keeps them by what Patronus decides on), then the shown subjects before the
hidden ones, at most 4 pre-checks at a time, in table order, the next as one answers.

#### Scenario: A demanded branch while the batch is in flight
- **WHEN** a surface needs a branch while the queue's batch is in flight
- **THEN** it is asked at once in its own call
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_a_demanded_branch_at_once_in_its_own_call_while_the_batch_is_in_flight`)

#### Scenario: The order of the queue
- **WHEN** the queue asks
- **THEN** the kept answers come from the cache first, then the shown rows before the hidden ones, at most four in parallel
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_the_shown_rows_before_the_hidden_awaiting_ones_at_most_four_in_parallel`, `answers_the_cache_before_the_queue`)

#### Scenario: The kept answers
- **WHEN** the cache keeps the answer of the same inputs
- **THEN** it comes from the cache without asking Patronus
  [@test] crates/remote-facts/src/review_readiness_test.rs (`the_kept_answers_come_from_the_cache_without_asking_patronus`)

### Requirement: The readiness store: A hidden subject is asked once it shows

A hidden subject MUST be asked once it shows; its answer stays meanwhile, marked stale when it shows with another
input. One exception: a hidden subject whose last answer waits for an approval in the pull request of the same tip and
start MUST still be asked when a review of it changes, so its approval is still news.

#### Scenario: A hidden subject and a new review
- **WHEN** a review changes in the pull request of a hidden subject whose last answer waits for an approval
- **THEN** it is asked, and a hidden subject is asked only for new reviews
  [@test] crates/repository-model/src/readiness_facts_test.rs (`new_inputs_make_the_readiness_stale_and_a_hidden_subject_is_asked_only_for_new_reviews`)
  [@test] crates/domain-policy/src/refresh_test.rs (`a_demanded_readiness_is_asked_at_once_then_the_shown_ones_then_the_hidden_that_wait_for_an_approval`)

### Requirement: The readiness store: Another repository waits for none of the calls of the one before

Another repository MUST be another session, which waits for none of the calls of the one before.

#### Scenario: A repository change during a batch
- **WHEN** the user switches to another repository during a batch
- **THEN** its session does not wait for the batch
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_repository_change_during_a_batch_does_not_wait_for_it`)

### Requirement: The readiness store: Nothing is asked while Patronus doesn't answer

While Patronus doesn't answer for the repository (not logged in, failing), nothing MUST be asked: the subjects a window
asks for are `unavailable` with why, so a click never waits for an answer that can't come.

#### Scenario: Patronus not logged in
- **WHEN** Patronus doesn't answer for the repository
- **THEN** a subject the window needs is `unavailable` with why
  [@test] crates/repository-model/src/readiness_facts_test.rs (`a_needed_subject_is_unavailable_while_patronus_does_not_answer_for_the_repository`)

### Requirement: The readiness store: A sync holds back no ask of the readiness

A sync MUST hold back no ask of the readiness: a demand told while it runs is asked at once, and its end asks a
subject again only for new inputs (a tip it moved).

#### Scenario: A demand during a sync
- **WHEN** a window demands the readiness while a sync runs
- **THEN** it is asked at once, and the end of the sync asks it once more only for new inputs
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_demand_during_a_sync_is_asked_at_once_and_the_end_of_the_sync_asks_it_once_more_only_for_new_inputs`)

### Requirement: The readiness store: An answer is keyed by its input

An answer MUST be keyed by its input: the tip, the start and the reviews. A sync onto a new baseline that keeps them
asks nothing; a new tip or a new review asks once, its readiness stale in the patch that shows it. The answer of the
old input stays, marked stale, until the new one arrives, so the row doesn't flicker.

#### Scenario: A new tip
- **WHEN** the tip of a branch moves
- **THEN** the readiness is asked once, stale in the patch of the new tip, and the old answer stays until the new one arrives
  [@test] crates/domain-policy/src/refresh_test.rs (`the_readiness_is_asked_for_new_inputs_a_recheck_and_five_minutes_after_patronus_failed_never_for_the_same_inputs`)
  [@test] crates/repository-model/src/session_test.rs (`the_readiness_a_window_needs_is_in_its_view_and_stale_in_the_patch_of_a_new_tip`)
  [@test] src/stores/review-readiness.test.ts (`shows the readiness the session answered for the input the branch has, and none while it loads`, `keeps the answer of the old input, stale, while a new review loads: the row doesn't flicker`)

### Requirement: The readiness store: A start never acts on a stale answer

A start MUST NOT act on a stale answer, nor on one whose input the branch moved past before the session's patch of it
(`freshReadiness` compares the key).

#### Scenario: The branch moved past the input
- **WHEN** the branch moved past the input of the answer before the session's patch of it
- **THEN** the answer is not fresh, and the start waits for the new one
  [@test] src/stores/review-readiness.test.ts (`is not fresh for an input the branch moved past before the session's patch of it`)
  [@test] crates/domain-policy/src/next_step_test.rs (`a_stale_readiness_keeps_its_step_and_its_action_waits_for_the_new_answer`)
  [@test] src/presenters/next-step.test.ts (`keeps a stale answer, its action waiting for the new one, and a start reads only a fresh one`)

### Requirement: The readiness store: An answer Patronus didn't give is unavailable, asked again later

An answer Patronus didn't give MUST be `unavailable`, asked again 5 minutes later on the clock of the session; a read
that failed after an answer is `unavailable` too, never a stale answer the start would wait on.

#### Scenario: Patronus didn't answer
- **WHEN** Patronus didn't give an answer
- **THEN** the readiness is `unavailable` and asked again 5 minutes later
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_again_five_minutes_after_patronus_did_not_answer`)
  [@test] src/stores/review-readiness.test.ts (`is unavailable when the session can't ask Patronus, and when its read failed after an answer`)

### Requirement: The readiness store: Check Approvals Again asks again with recheck

"Check Approvals Again" MUST refresh the pull requests from GitHub, then ask the session to ask Patronus again with
`recheck` (`refresh_facts { readiness, manual, subject }`: new inputs, the answer stale from the press on).

#### Scenario: The press of Check Approvals Again
- **WHEN** the user runs Check Approvals Again
- **THEN** the pull requests come from GitHub first, then Patronus is asked with recheck, the answer stale meanwhile
  [@test] src/stores/review-readiness.test.ts (`asks again with recheck after the pull requests from GitHub, the answer stale meanwhile`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should ask Patronus again from Check Approvals Again, after the pull request from GitHub`)

### Requirement: The readiness store: The recheck is the last word

An answer of an ask that started before the recheck (the queue's) MUST NOT replace the recheck's, and its pre-check
MUST replace no answer the cache kept since it was asked: the recheck is the last word.

#### Scenario: The batch answers after the recheck
- **WHEN** the batch asked before the recheck answers later
- **THEN** the answer of the recheck stays
  [@test] crates/remote-facts/src/scheduler_test.rs (`keeps_the_answer_of_a_recheck_when_the_batch_asked_before_it_answers_later`)
  [@test] crates/repository-model/src/readiness_facts_test.rs (`the_answer_of_an_ask_started_before_a_recheck_never_replaces_the_rechecks`)
  [@test] crates/remote-facts/src/review_readiness_test.rs (`a_background_answer_after_a_recheck_beside_it_keeps_the_answer_of_the_recheck`)

### Requirement: The readiness store: A row that awaits an approval asks GitHub on a window focus

While a row waits for an approval (Awaiting), a window focus MUST ask GitHub for the pull requests at once, not after
the usual 2 minutes: the user comes back from asking for the review; and each poll of the review requests MUST ask for
them too (also while the header of a whole stack waits).

#### Scenario: The window gets the focus
- **WHEN** the window gets the focus while a row waits for an approval
- **THEN** the pull requests are asked from GitHub at once

### Requirement: The readiness store: The approval that ends the wait is news

The approval that ends the wait MUST be news of the session with "Safe Push" ([Review
Requests](../../repository/review-requests/spec.md#approval-news)); in the pull request of the whole stack, "Ilya
Korennoy approved the whole air stack" with "Safe Push Whole Stack".

#### Scenario: An approval in the pull request of the whole stack
- **WHEN** Ilya Korennoy approves the pull request of the whole air stack
- **THEN** the news says "Ilya Korennoy approved the whole air stack" with "Safe Push Whole Stack"
  [@test] src/lib/approval-news.test.ts (`names the whole stack for the approvals of its pull request`)

## User Experience

- The slot is as wide as the widest step of the view, at most 10rem, its step a ghost `xs` button with its
  `compactLabel` ("Retarget #7", "Awaiting Review", "Create Pull Request…" of the whole stack), every label of a step
  within the 10rem; a start that runs keeps the width the step had at rest ("Checking…", "Waiting…" for another holder
  of the repository lock, the spinner and "45%" of its push, "Starting…"; "Pushing 45%" and the branches it pushes in
  its name and hover), so nothing moves under the pointer ([UX Writing](../../../../docs/contributing/ux-writing.md#actions)).
  The step of a start is the step of its operation in the view (`RepositoryView.operations`, `useLandRun`): every
  window of the repository shows it; the percent and whom a wait is for come on the channel of the window that asked.
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`fits every label of a step in its column, so none truncates`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
  [@test] crates/domain-policy/src/next_step_test.rs (`row_5_retargets_a_pull_request_into_a_virtual_branch_and_5_prime_opens_one_into_another_base`, `rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] src/presenters/next-step.test.ts (`row 5: retargets a pull request into a virtual branch, short in its column; 5′ opens one into another base, the retarget in the menu`, `rows 8 to 10: changes requested, Request Review…, the wait naming the people asked`)
- Disabled, a step keeps the focus and the hover
  (`focusableWhenDisabled`), dimmed to `opacity-50`. A quiet step is muted, Changes Requested `text-destructive`.
- Wording follows [UX Writing](../../../../docs/contributing/ux-writing.md): Title Case for the steps, sentence case for the
  reasons, people by their full name (the login without one).

## Testing / Local Run

- `pnpm test:unit`: `src/presenters/next-step.test.ts`, `src/presenters/landing-node.test.ts`, `src/stores/review-readiness.test.ts`; `cargo nextest run -p
  remote-facts -p repository-model -p domain-policy -E 'test(/readiness/)'` for the queue of the session.
- `pnpm e2e`: `tests/e2e/next-step` (`TestRepositoryBuilder.setReviewAnswers`, the answers of the fake Patronus's
  pre-check `AWAITING_CODE_OWNER`, `APPROVED_BY_CODE_OWNER`, `NOT_EVALUATED`, `NO_REVIEW_NEEDED` of
  `tests/e2e/helpers/patronus-helpers.ts`, judged by the real core); a start from the pull request asserts the
  `/patronus merge` comment the fake GitHub got, a review request the `requested_reviewers` request.

## References

- [Patronus](../patronus/spec.md): the robot of a row, the code-owner review, the start of a Safe Push
- [Branch Row](../branch-row/spec.md#actions-cell): the Actions cell
- [Pull Requests](../pull-requests/spec.md): the reviewers and the author the readiness takes
- [Stack Island](../stack-island/spec.md#header): the Safe Push of the whole stack in its header
