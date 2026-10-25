---
name: Next Step
description: Requirements for the one model of what to do next with a branch (getBranchNextStep), the first matching row of a table of its states (a sync, a failure, a block, a hold, landed, a robot that runs or merged, a branch outside what its Safe Push lands that its build needs (the landing needs Rust computes after a sync), landed by a merged Safe Push (its own, of a branch built on it or of the whole stack, by the hashes or by content, the commits added since the work left) until the update, a robot to restart, a pull request into another base or a draft, the approvals that count missing, waited for or approved, as Patronus judges them; a stacked branch by the rows of its own pull request), rendered by the Safe Push slot of the row, the "⋯" menu, S, the branch preview, the review window, the stack header and the start of a Safe Push with its popover (getSafePushDecision); the same table for the whole stack of a root from its stack branch (useStackNextStep), whose pull request a branch without its own points to, which lands only what merged Safe Pushes left of it; the readiness store that asks Patronus for it in the background, only for the branches a surface shows, and Check Approvals Again.
style: plain-1
targets:
  - ../../src/lib/branch-next-step.ts
  - ../../src/lib/menu.ts
  - ../../src/hooks/useBranchNextStep.ts
  - ../../src/hooks/useStackNextStep.ts
  - ../../src/lib/whole-stack.ts
  - ../../src/lib/approval-news.ts
  - ../../src/components/branch/WholeStackSlot.tsx
  - ../../src/stores/review-readiness.ts
  - ../../src/model/demand.ts
  - ../../crates/repository-model/src/readiness_facts.rs
  - ../../crates/remote-facts/src/scheduler_readiness.rs
  - ../../crates/domain-policy/src/readiness.rs
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/lib/landing-node.ts
  - ../../src/hooks/useLandingNode.ts
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/hooks/usePatronusReviewRequirement.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/branch/BranchPreviewActions.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/hooks/useSafePush.ts
  - ../../src/lib/safe-push-decision.ts
  - ../../src/stores/request-review-popover.ts
  - ../../src/lib/request-review.ts
  - ../../src/stores/pull-requests.ts
  - ../../src/lib/branch-review-window.ts
  - ../../src/stores/patronus.ts
  - ../../src/lib/patronus.ts
  - ../../src/hooks/useLandedIn.ts
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/branch/LandedChip.tsx
  - ../../crates/sync-core/src/robot_landing.rs
  - ../../crates/trial-check/src/landing_gate.rs
  - ../../crates/domain-types/src/landing.rs
  - ../../crates/repository-model/src/facts.rs
  - ../../src/lib/landing-gate.ts
---

# Next Step

Status: Implemented
Date: 2026-10-17

## Summary

Every surface used to decide on its own what a branch can do now, from its own share of the facts: the row offered
"Safe Push…" for a branch whose code owner was still asked, the form enabled Start, and the robot failed at once for
the approvals Patronus checks when it starts. One pure function, `getBranchNextStep`, now decides the next step of a
branch from all the facts (the branch, its robots, its pull request and the readiness Patronus answers,
[Patronus](patronus.spec.md#code-owner-review)), and every surface renders it: the slot shows the step, every
"Safe Push" is disabled with the same reason, no action is offered whose failure is known.

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

## Requirements

### The table

- The next step is the first row that matches. "Safe Push elsewhere" is the reason every other surface disables
  "Safe Push" with (`safePushBlocker`, `getSafePushBlocker`); a Dry Run is never held back by approvals, a draft, a
  pull request into another base or a robot that lands an ancestor (`dryRunBlocker`).

  | # | State | Slot | Safe Push elsewhere |
  |---|---|---|---|
  | 0 | A fact the rows below decide on has no answer yet (`PendingFact`; [below](#facts-without-an-answer)): Patronus hasn't answered for the repository or its view of the robots it listed first isn't in, GitHub hasn't listed the pull requests, the check of what the branch needs to land is still running, Patronus hasn't told why the robot of the branch failed, the first answer of the readiness; never while a reason of row 1 or of the branch holds the start back, which it says instead. Or a read failed (`UnavailableFact`): the Safe Push CLI is not logged in, Patronus fails, Patronus didn't give why the robot failed | muted "Checking…" with its spinner, disabled, the fact on hover ("Waits for GitHub to list the pull requests"); after a failed read what helps: "Copy Login Command", "Refresh Patronus", or the restart disabled with the failure | disabled with the same words ("Waits for the answer of Patronus", "Waits for Patronus to check the approvals", "Run ./safePush.cmd once to log in to Patronus"); a Dry Run only while Patronus hasn't answered or can't be asked |
  | 1 | Syncing, failed to sync, no commits, blocked, held by a Safe Push, parked or built on a parked branch, landed, a robot runs, merged (by the robot, by the merged Safe Push of a branch built on it or of the whole stack, or the pull request of these commits; all of it, no commit added since) | nothing; the Status cell says it ("Landed with oauth", "Parked with auth") | disabled: "Wait for the sync to finish", "Parked: Unpark it to push", "Built on parked auth: Unpark auth to push", "Holds parked auth: land the stack in pieces" (the whole stack), "Landed in origin/master", "Landed in master with oauth", "Merged by Patronus", "Pull request #7 is merged", … (a running robot of an older commit: enabled, the start replaces it) |
  | 1′ | The build of what its Safe Push lands needs a commit of a branch outside it (`landingNeeds` of the repository view, [below](#what-the-safe-push-needs)) | "Needs stills-codec" (a commit not in a branch by its hash), the reason on hover; a click runs the fix, "Stack lane-run on stills-codec" | enabled: its popover says why before the start, with the fix and Start Anyway |
  | 2 | A robot failed or was canceled, not for approvals, and no running Safe Push of a branch it is built on (of the whole stack: of a branch of it) lands a part of it; while one does, the failure of the earlier piece waits for it: the rows below decide, the Safe Push disabled with "A Safe Push of auth is running: wait until it merges" | "Restart Safe Push" (Safe Push… where Patronus can't restart it) | enabled |
  | 3 | A robot failed for approvals, started from the open pull request into the target, readiness `approved` | "Restart Safe Push" | enabled |
  | 4 | removed: a stacked branch takes the rows of its own pull request into master, see below | — | — |
  | 5 | An open pull request into a virtual branch (`{prefix}/virtual/…`, `isVirtualBranchBase`: of its parent, of another branch of the table, or of one that landed, was archived or deleted): every pull request of a virtual branch goes into the target | "Retarget #7 to master" ("Retarget #7" in the slot) | disabled: "Approvals in #7 don't count: its base is user/virtual/auth" |
  | 5′ | An open pull request into another base, which may be meant (a release branch) | quiet "Open Pull Request", the reason on hover; "Retarget #7 to master" stays in "⋯" | disabled: "Approvals in #7 don't count: its base is release-252" |
  | 6 | An open draft | quiet "Draft", opens it | disabled: "#7 is a draft: Patronus merges it once it is ready for review" |
  | 7 | `missing`, no open pull request (none, or a closed one) | "Create Pull Request…" (the code owners preselected; on the remote or not, its dialog pushes the branch; disabled only while the GitHub repository is unknown); a branch of a whole stack whose pull request is open: quiet "In #12", which opens it (below) | disabled: "Code owners must approve in a pull request: IJPL Code Platform" |
  | 8 | `missing`, a member of a missing group requested changes (and wasn't asked again since) | quiet "Changes Requested" in `destructive`, opens the pull request | disabled: "Ilya Korennoy requested changes on #1078" |
  | 9 | `missing`, a missing group asks fewer people than the approvals it needs (nobody, or one of a group that needs two) | "Request Review…", its popover ([Pull Requests](pull-requests.spec.md#request-review)) | disabled: "Waits for the approval of Air Platform: nobody of it is requested on #1078" |
  | 10 | `missing`, someone of every missing group is asked | quiet "Awaiting Ilya Korennoy" ("Awaiting Review" in the slot, the people in its hover; "Awaiting 2 Reviews" for several), opens the pull request | disabled: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078" |
  | 8′ | `unknown`, `unavailable`, an open pull request into the target where a reviewer requested changes and wasn't asked again since (`changesRequestedBy`, bots left out) | quiet "Changes Requested" in `destructive`, opens the pull request | enabled: Patronus can't judge the approvals, the start checks them |
  | 10′ | `unknown`, `unavailable`, an open pull request into the target that waits for a requested reviewer (`waitingReviewers`: asked and not approving since, bots left out) | quiet "Awaiting ikorennoy" ("Awaiting 2 Reviews"), its hover "Waits for ikorennoy on #1078 · Patronus can't tell yet whether the approvals there count: the start checks them", opens the pull request | enabled, as 8′ |
  | 11 | `approved`, `notRequired`; `unknown`, `unavailable` where GitHub tells no wait | "Safe Push" | enabled |

  [@test] ../../src/lib/branch-next-step.test.ts (`row 1: offers nothing while the Status cell says why, and disables Safe Push and Dry Run with the reason`, `row 1: a branch a merged Safe Push of a branch built on it or of the whole stack landed offers nothing, its Safe Push and Dry Run say who landed it`, `row 1: a merged pull request of other commits (new work under the same name) blocks nothing`, `row 2: restarts a failed or canceled robot, or offers Safe Push… where Patronus can't restart it`, `row 2: no restart while a robot of a branch below lands a part of it, the steps of its pull request stay`, `row 3: restarts a robot that failed for approvals only from the pull request they now count in`, `a stacked branch takes the steps of its own pull request into master`, `a stacked branch waits for the reasons of its ancestors`, `row 5: retargets an open pull request into a virtual branch, of its parent or of one that landed, whose approvals don't count`, `row 5: opens a pull request into another base that may be meant, the retarget only in the menu`, `row 6: a draft is a quiet step that opens it; Patronus merges it once it is ready, a dry run runs`, `row 7: missing approvals without an open pull request: Create Pull Request…, the code owners named`, `row 7: a branch of a whole stack whose pull request is open points to it, quietly: its approvals count for the Safe Push of the whole stack`, `row 8: changes requested by a member of a missing group: a destructive quiet step that opens the pull request`, `row 9: a missing group with nobody asked: Request Review…, disabled only with the item's own reason`, `row 10: every missing group has someone asked: the quiet wait names them and opens the pull request`, `rows 8′ and 10′: while Patronus can't judge the approvals, the wait GitHub tells, the Safe Push still enabled`, `row 11: Safe Push… for every other readiness`, `row 0: waits with Checking… for a fact no answer told yet, never a step a later answer turns into another`, `row 0: a reason of the branch outranks a fact no answer told yet, and a start waits for the readiness itself`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should wait for the reviewer GitHub asks while Patronus can't judge the approvals, keep Safe Push, and tell the approval`)
  [@test] ../../src/lib/pull-request.test.ts (`waits for the reviewers asked and not approving since, bots left out; a re-requested approval waits for nobody`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`, `should offer Safe Push once the code owner approved, name whose approval counts, and start it from the pull request with a click`, `should keep Safe Push while Patronus doesn't tell whose approvals count, and say so before it starts`, `should offer Create Pull Request… with the code owner when approvals count only in one`, `should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`, `should retarget a pull request into the branch of a parent that landed, whose approvals don't count`, `should retarget a pull request into the virtual branch of a branch in the table, as a parent's, whose approvals don't count`, `should retarget the pull request of a stacked branch into its parent's branch from its slot, whose approvals don't count`, `should open a pull request into another base that may be meant, and keep its retarget in the menu`, `should offer nothing after a merge, and say so in the menu`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`, `should offer no Safe Push of a branch whose pull request merged`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the next step row cases answer as the app answers today`, `cover every row of the next step table`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_as_bd_answers_today`)
- Row 1 also holds a branch that a merged Safe Push (not a Dry Run) landed whole (`getLandedBy`, the `landedBy` of the repository view,
  `NextStepFacts.landedWith`): the branch it ran on itself, an ancestor the Safe Push of a branch built on it carried,
  and every branch of a stack whose Safe Push of the whole stack ran on the stack branch, the same conditions under
  which the merge closes their pull requests ([Patronus](patronus.spec.md#merged)). Also a robot first seen merged
  (after a restart, a landing by `bd`). What it landed is decided by the hashes while the robot ran on a commit of its
  branch as the table has it (its tip, the same commits rebuilt on a newer baseline, or a commit the branch built new
  ones on since) or on the stack branch as it is; after a change below that commit (a new commit or an amend of a
  branch it is built on, a reword, for the whole stack any change of a branch of it, which rebuilds the stack branch)
  by content: the commits the robot tested against each branch as the table has it (the repository view, asked once
  per state of the branches; [Patronus](patronus.spec.md#merged)). Until the update archives
  the branch (the fetched baseline then says "Landed in origin/master" first), its Status cell names who landed it,
  "landed" with "Landed with oauth" ("Landed with the whole air stack"), its hover when it merged and a click the robot
  in Patronus; the preview shows it too; the branch the robot ran on keeps its robot, "Merged". Its Safe Push and Dry
  Run are disabled with "Landed in master with oauth" ("Merged by Patronus" of the robot's own branch) on every
  surface, S says it, and its push is held as by a running robot
  ([Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)). A branch built on a landed one lands
  without it: "Safe Push" where it said "Safe Push · 2" (`getLandingBranchCount`), its popover not naming it.
  [@test] ../../src/lib/branch-next-step.test.ts (`row 1: a branch a merged Safe Push of a branch built on it or of the whole stack landed offers nothing, its Safe Push and Dry Run say who landed it`)
  [@test] ../../src/lib/patronus.test.ts (`names who landed a branch in its row and why it starts nothing: a branch built on it, the whole stack, its own robot with the work left`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`, `new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`, `a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`, `a_dry_run_lands_nothing`)
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`a_missing_source_lands_nothing`, `a_commit_added_below_the_robots_commit_is_the_only_work_left`)
  [@test] ../../src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`, `should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer no Safe Push or push of them`)
- The step says why nothing of the branch is left to land (`BranchNextStep.landed`, `formatLandedState`), also while
  another reason of row 1 (a sync) disables its start: "Landed in origin/master", "Landed in master with oauth",
  "Merged by Patronus", "Pull request #7 is merged". "Create Pull Request…" of the branch reads it: disabled with that
  reason, nothing is left to review ([Pull Requests](pull-requests.spec.md#creating-a-pull-request)).
  [@test] ../../src/lib/branch-next-step.test.ts (`says why nothing of a branch is left to land, what disables its Create Pull Request…, also behind another reason of its start`)
- A landed branch with commits added after the landed ones is not settled: those commits are the only work left of
  it, its Status cell says so ("Landed with oauth · 1 commit after it", of the robot's own branch "Landed · 1 commit
  after it"; its hover "1 commit added since, not in master yet"), and it takes the rows of a branch with work: its
  push, its Safe Push. A branch with a commit changed since (an amend), or a new one below a tested one, is not landed:
  what landed is not what it is.
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`a_commit_added_below_the_robots_commit_is_the_only_work_left`)
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`a_commit_added_below_the_robots_commit_is_the_only_work_left`, `a_reworded_commit_below_keeps_the_branches_landed`, `an_amended_commit_leaves_its_branch_not_landed`, `a_commit_not_synced_yet_is_after_the_landed_ones`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)
- Rows 8 to 10 count only the groups still short of approvals: changes requested by a member of an approved group
  don't block, as Patronus doesn't block on them. With work the remote lacks the push slot shows the push and the step
  still shows the wait; Request Review… pushes that work first, a step of the request, so the reviewers see the
  commits ([Pull Requests](pull-requests.spec.md#request-review)). A missing
  group nobody can approve (`noEligibleReviewer`) disables Request Review…: "Nobody of IJPL Code Platform can approve
  these changes".
- Request Review… is the item of `getRequestReviewItem` ([Pull Requests](pull-requests.spec.md#request-review)), the
  same in the slot (row 9, its hover the reason of the step, then whom it asks), the branch menu (every branch with an
  open pull request, also while the slot shows another step: it opens the popover at the slot). The popover asks the code owners; once the request is made the step is the wait: a suggested
  member of a missing group whom the pull request asks counts as requested (`withRequestedOnPullRequest`), also while
  Patronus answers again with the people requested before. Someone asked who is no suggested member of the group asks
  nobody of it.
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should ask the code owner nobody asked yet from Request Review…, then wait for him from the click on`, `should open Request Review… from the branch menu, at the row's next step`)
  [@test] ../../src/lib/branch-next-step.test.ts (`row 8: changes requested in an approved group don't block, as Patronus doesn't`, `row 9: a group that needs two approvals and asks one person still needs a request`, `row 10 right after Request Review…: a suggested member the pull request waits for is requested, while Patronus answers again`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should push the commits the remote lacks as the first step of Request Review…`)
  [@test] ../../crates/patronus/src/review_readiness_test.rs (`a_member_re_requested_after_changes_is_awaited`)
- Row 3: a robot that failed for approvals keeps the pull request step of before ("Open Pull Request", or "Create Pull
  Request…" without one) while Patronus can't tell whether the approvals count (`unknown`, `unavailable`): a restart may
  fail again; before the first answer the step waits for it (row 0). Approved, but started without the pull request: "Safe Push", a new start from it counts the approvals. The
  restart of such a robot is disabled with the reason of rows 7 to 10 while the approvals are missing, on every surface
  that offers it: "⋯", the robot details and the landing overview (`getGatedRobotActions`).
  [@test] ../../src/lib/branch-next-step.test.ts (`row 3: keeps the pull request step while nobody can tell whether the approvals count`)
- A stacked branch has no row of its own (row 4 is removed, the numbers of the others stay): its Safe Push lands it with
  what it is built on, in one robot ([Landing](../../docs/design/landing.md)), and its pull request goes into master
  ([Pull Requests](pull-requests.spec.md)), so it takes the rows of that pull request: its own approvals, its own draft,
  its own wait. The reasons of its ancestors come through the rows that hold them already: a parent that failed to
  build blocks it (row 1, `Blocked`), a Safe Push that lands the branch holds it (row 1), and a robot that lands an
  ancestor is the reason of `canStartRobot`, which disables every step's Safe Push ("A Safe Push of login is running:
  wait until it merges"; a dry run may run). The slot, "⋯", S, the preview and the review window ask its own start,
  its popover at its own row ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)). The header of a stack
  island shows the step of the whole stack (below), its popover at the header ([Stack Island](stack-island.spec.md#header)).
  The texts name the branch by `NextStepFacts.subject`, the branch's name.
  [@test] ../../src/lib/branch-next-step.test.ts (`a stacked branch takes the steps of its own pull request into master`, `a stacked branch waits for the reasons of its ancestors`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner a stacked branch's own pull request asks, not for its root`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`, `should wait for the Safe Push of an ancestor, which lands its commits`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the stack and whole stack cases answer as the app answers today`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_as_bd_answers_today`)
- An operation on the row ([Running Operations](../../docs/design/operations.md)) disables the action of the slot with
  its busy state ("Renaming auth to login…"), never a quiet step: it only opens the pull request, nor a step that
  writes another field of the pull request than the operation does: a retarget (the base) during a review request (the
  reviewers) runs beside it, neither refuses the other. The slot's own action that runs (a retarget, a restart of its
  robot) is running: disabled, its spinner in place of its icon.
  [@test] ../../src/lib/branch-next-step.test.ts (`words the notice of S as a short title naming the branch, the reason below; a reason of the branch first`)
  [@test] ../../src/lib/branch-next-step.test.ts (`disables the action of the slot while an operation on the row runs, never the quiet steps nor a write of another field of its pull request; its own runs`, `disables the Safe Push of a branch a robot of its ancestor lands, its slot says why`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should retarget a pull request while a review is requested in it: both reach GitHub, neither refuses the other`)
- The pull requests are a fact of their own: while GitHub doesn't list them (not configured, signed out, SSO; before
  its first answer the step is row 0), the readiness counts as `unavailable` and rows 5 to 10 don't apply: the table never offers a Create Pull Request… for
  a branch that may have one; the start then holds nothing back either, and confirms first: "GitHub doesn't list the
  pull requests of the repository: Patronus checks the approvals when the robot starts." A repository only on
  Space (no GitHub) never asks; its Safe Push stays possible, its
  merge requests are deprecated and the robot decides. A repository not on Space has no slot; Request Review… of an
  open pull request, which asks GitHub, still opens from the menus, at an anchor of the row that takes no room
  ([Pull Requests](pull-requests.spec.md#request-review)).
  [@test] ../../src/lib/branch-next-step.test.ts (`treats the readiness as unavailable without the pull requests: never a duplicate Create Pull Request…`, `has no slot in a repository that is not on Space, and keeps the reasons of the branch`)

### What the Safe Push needs

- git merges a branch that uses a name only another branch adds, and the robot's build fails: the robot of
  2026-10-22 ran `ui-perf-nightly`, built on `lane-run`, whose commit imports a class `stills-codec` adds, while the
  robot of `stills-codec` ran beside it. The landing gate refuses such a start only after the press
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)); the row says it before any press. After a sync
  the session of the repository computes, for every branch, the refusals the gate of its start would give
  (`landingNeeds` of `BranchPatronus` in the Patronus part of its view, `trial_check::landing_gate::landing_needs`
  through its host, the same rule as the gate): one trial check of every branch, its dangling references and missing
  declarations, less what merged Safe Pushes landed. A branch built on one that needs a branch needs it too: its Safe
  Push lands that one. The needs are a fact with its inputs (the baseline, the tips of the table, the commits the view
  names landed): unknown until computed, never "nothing", one computation at a time, not computed again while the
  inputs stay; they come in a patch of the view (its `landingNeeds` state `loading` until then), the step waiting
  meanwhile (row 0, [below](#facts-without-an-answer)); a failed computation is `unavailable` for its inputs, and no
  step waits for it. Row 1′ maps them to the step, no rule of its own (`getBranchNextStep`): "Needs stills-codec", its hover the
  reason in the words of the gate's toast ("“Show each scenario” of lane-run uses Stills (lane/src/BuildLog.kt:3),
  which “Name lossy stills” of stills-codec adds: git merges it, but the build of lane-run fails. Stack lane-run on
  stills-codec to land it too."), and a click the fix: the restack of the root of what lands onto that branch
  (`LandingFix`, `formatLandingFix`); "⋯" offers the same fix first ("Stack lane-run on stills-codec"). A fix no action
  runs (the move of an unassigned commit, a restack that would close a cycle) opens the Safe Push popover. Row 1 comes
  first (a running robot, a sync); the restart of a failed robot and the approvals come after: a restart would fail on
  the same build. While the branch that adds the name is landing in a running Safe Push, the reason says so: "stills-codec is
  landing in its Safe Push, which tests it without this start: a start beside it fails too." The Safe Push stays
  enabled: its popover shows the reason before the start, with the fix and Start Anyway
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)). The whole stack has no landing needs of its own:
  the gate of its start checks it.
  [@test] ../../src/lib/branch-next-step.test.ts (`row 1′: a branch whose Safe Push needs a branch outside what it lands says so and restacks; unknown needs change nothing`)
  [@test] ../../tests/e2e/next-step/landing-needs.spec.ts (`should say on the row which branch its Safe Push needs before any press, and stack it on that branch from the slot`, `should stack the branch on the branch its Safe Push needs from the menu of a branch built on it`)
  [@test] ../../src/stores/patronus.test.ts (`waits for the landing needs Rust computes after the view, unknown until then, and never for a computation that failed`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`the_landing_needs_are_computed_one_at_a_time_the_newest_wanted_next`)
  [@test] ../../crates/trial-check/src/landing_gate_test.rs (`a_branch_that_uses_a_name_of_a_branch_outside_it_needs_that_branch_and_so_does_a_branch_built_on_it`, `the_needs_of_a_branch_are_the_refusals_of_its_start`, `a_landed_declaration_is_needed_by_nobody`, `a_running_safe_push_of_the_branch_that_adds_the_name_is_named_in_the_reason`)
  [@test] ../../crates/trial-check/src/process_budget_test.rs (`the_landing_needs_of_every_branch`)
  [@test] ../../src/lib/landing-gate.test.ts (`says that the branch the start needs is landing in a running Safe Push, which tests it without this start`)

### Facts without an answer

- A start, a repository switch and a sync give the rows before the remote answers for them: Patronus (whether it
  lands the repository, its robots, the view of what they hold and land the session derives), GitHub (the pull requests), the
  problems of a failed robot (whether it failed for approvals) and the readiness of each row. Seen on 2026-10-24 on
  the monorepo: the rows said "Safe Push" for 3 s, then "Awaiting Review" and "Create Pull Request…"; a click in those
  seconds confirmed a start through the API whose approvals don't count. The step decides only on answers: row 0
  waits for the first fact the rows below would read without one (`PendingFact`: `isPatronusPending`, Patronus hasn't
  answered or its view of the robots it listed first isn't in; `isPullRequestsLoading`, GitHub hasn't answered the
  listing; `isRobotProblemsPending`, the problems of the failed robot of the branch are still to come; the first
  answer of the readiness of the branch as it is), never the step a later answer turns into another: an unknown pull
  request is no "no pull request", an unanswered readiness no "approved". A reason of the branch (row 1, a robot of a
  branch below, a block, a parked branch, an operation on the row) outranks it; so does a known landing need (row
  1′). The landing needs are such a fact too (`isLandingNeedsPending`: the session's check after the sync hasn't
  answered for the view the rows read, its `landingNeeds` `loading`; "Checking what JIRA-456 needs to land…"): measured
  on 2026-10-24, they come 0.2 s after the rows on the monorepo with a warm cache and 0.7 to 1.2 s cold (9 branches),
  0.1 to 0.2 s in the e2e fixtures, so the Safe Push waits for them as for the other facts instead of offering a start
  whose gate may refuse it after the press.
  [@test] ../../src/lib/branch-next-step.test.ts (`row 0: waits with Checking… for a fact no answer told yet, never a step a later answer turns into another`, `row 0: a reason of the branch outranks a fact no answer told yet, and a start waits for the readiness itself`)
  [@test] ../../tests/e2e/next-step/landing-needs.spec.ts (`should check what a branch needs to land before it offers its Safe Push, S and the popover waiting for the answer`)
  [@test] ../../src/stores/patronus.test.ts (`waits for Patronus until the view of the robots it listed first is in, then never again for a change of them`, `takes a failure of Patronus as its answer, and a list of no robot as having its view`, `waits for why a failed robot failed until its problems come, and no longer once Patronus didn't give them`)
  [@test] ../../src/stores/pull-requests.test.ts (`is loading until GitHub's first answer, which a failure is too: no pull request is known meanwhile`)
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should wait with Checking… for the pull requests and the approvals, S and the menu saying why, then start the Safe Push they allow`)
- Every surface waits the same way: the slot shows a muted "Checking…" with its spinner, disabled, the fact on hover
  ("Waits for GitHub to list the pull requests", "Waits for the answer of Patronus", "Checking what JIRA-456 needs to
  land…", "Waits for Patronus to tell why the Safe Push failed", "Waits for Patronus to check the approvals"); "⋯" keeps Safe Push and "Patronus ›" disabled
  with the same words, and so do the footer of the preview and the title bar of the review window, also before
  Patronus answers whether it lands the repository (hidden only once it says it doesn't, `isPatronusOffered`); "Create Pull Request…" of "⋯" is disabled with "Waits for GitHub to list the pull requests" while the
  branch may have one; S says it in an `info` notice ("Safe Push of JIRA-456 waits for GitHub"); the node of the row
  waits too and the header counts no move of it ([Landing Overview](../repository/landing-overview.spec.md#indicator)).
  A Dry Run waits only for Patronus: approvals and pull requests never hold it back. The readiness store asks for the
  answer the step waits for (`needsReadiness`). The step of a start reads the facts of the start: it waits for a
  readiness without an answer and for GitHub's first answer itself (`getSafePushDecision`, its review `loading`, never
  `unlisted` before that answer), so neither holds it back there.
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should wait with Checking… for the pull requests and the approvals, S and the menu saying why, then start the Safe Push they allow`, `should never offer Create Pull Request… before GitHub lists the pull requests, say a failed listing, and keep the last listing through a failed refresh`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`says why a branch can't start a Safe Push, in the words of every surface that offers it`, `offers Safe Push disabled until Patronus answers, and Create Pull Request… until GitHub lists the pull requests`)
- A failed answer is an answer, never "no": a listing GitHub can't give (signed out, SSO, no access, or a failure
  before any listing; `getPullRequestListing` `unlisted`, `pullRequestsUnlisted`, `formatAvailabilityHint`) leaves the
  pull requests unknown, so rows 5 to 10 don't apply and the Safe Push stays, its hover saying it ("Lands JIRA-456 into
  master once the checks pass · GitHub: offline: Patronus checks the approvals when the robot starts", the node the
  same), and its start confirms first. A refresh GitHub fails for a while after a listing (an error, offline, the rate
  limit) keeps that listing (`listed`): the steps read its pull requests as of then, so a failed refresh never turns a
  wait back into a Safe Push. A view Patronus didn't give leaves the step to the rows below.
  [@test] ../../src/stores/pull-requests.test.ts (`keeps the last listing through a refresh GitHub failed for a while, never once its token is gone`)
  [@test] ../../src/lib/branch-next-step.test.ts (`treats the readiness as unavailable without the pull requests: never a duplicate Create Pull Request…`)
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should never offer Create Pull Request… before GitHub lists the pull requests, say a failed listing, and keep the last listing through a failed refresh`)
- A read of Patronus that failed decides no start (`UnavailableFact`, `unavailableFactOf`), after row 1 and the
  reasons of the branch: the Safe Push CLI not logged in (`notLoggedIn`) makes the step "Copy Login Command", the
  command of the hint for a terminal ("Run ./safePush.cmd once to log in to Patronus: the command goes to the
  clipboard, for a terminal"); Patronus failing (`error`) before it listed the robots once makes it "Refresh
  Patronus", which asks it again; the GitHub mirror is unknown then too, so no start goes as if the repository were
  only on Space. An error after a list leaves the steps on the robots it listed (`hasPatronusListed`), as a failed
  refresh of GitHub keeps the last listing; the landing overview says the error. A view of the robots the session
  couldn't derive while no earlier one stands for it (`unavailable`, `getPatronusViewFailure`) is no "nothing holds
  it": "Refresh Patronus", "Patronus didn't tell what its robots hold and land (…)"; over an earlier view the steps
  stand on that one (`stale` with why), its holds, landing ancestors and needs stale until the next answer, and the
  session derives it again after the next answer of Patronus (the refresh asks one), a sync or a push, never by itself. Safe Push and Dry Run of "⋯" are disabled with the hint,
  S says it in an `info` notice ("Safe Push of JIRA-456 can't start") whose action is the step's. Problems Patronus
  didn't give for the failed robot of a branch or of a whole stack (`problemsFailed`), like problems still to come,
  keep the restart and the prompts for an agent disabled with the reason on every surface of the robot (the menu, the
  landing overview, the details of its chip, which say it: "Waits for Patronus to tell why the Safe Push failed",
  "Patronus didn't tell why the Safe Push failed (…): Check Approvals Again asks again"; `robotProblemsReason`); the
  landing overview shows the branch waiting with its step while they are to come, never a robot to fix by a restart.
  Check Approvals Again asks for them again (`askRobotProblemsAgain`).
  [@test] ../../src/lib/branch-next-step.test.ts (`row 0: a read that failed decides no start: a login to copy, a refresh, a restart disabled while why the robot failed is unknown`)
  [@test] ../../src/stores/review-readiness.test.ts (`asks again with recheck after the pull requests from GitHub, the answer stale meanwhile`)
  [@test] ../../src/stores/patronus.test.ts (`keeps the last view through a failed derivation of it, and says the failure while no view stands for the robots`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_view_that_failed_is_derived_again_after_the_next_answer_of_patronus`)
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should offer the login instead of a Safe Push while the Safe Push CLI is not logged in, and a refresh while Patronus fails`, `should offer no start while Patronus gave no view of its robots, and stand on what it listed through a failed refresh`, `should restart a failed Safe Push and copy its prompt only once Patronus tells why it failed, and say a failed answer`)
- While GitHub can't list the pull requests (signed out, failing; `pullRequestsUnlisted`), row 7 may apply: no pull
  request is known and the approvals may be missing, so the Safe Push of row 11 keeps the start without review beside
  it (`withoutReview`, "Safe Push Without Review…" of the pull request dialog), its blocker the reason of the branch.
  The review window that asks for it while the main window has none is refused with the step's reason (what it waits
  for), else "The approvals of auth don't wait for a pull request now".
  [@test] ../../src/lib/branch-next-step.test.ts (`leaves the Safe Push without review to the user while GitHub is signed out or failing: row 7 may apply`)
  [@test] ../../tests/e2e/next-step/facts-without-answer.spec.ts (`should leave the Safe Push without review to the user while GitHub is signed out, from the pull request dialog`)
- A stale answer of the readiness (the branch moved on: a new tip, a new review, Check Approvals Again) keeps its step
  while the new one loads, so the row doesn't flicker, but its action waits for the new answer, which may decide
  another step: Safe Push, Restart, Create Pull Request… and Request Review… disabled with "Waits for Patronus to check
  the approvals again", S says it; a quiet step only opens the pull request and stays.
  [@test] ../../src/lib/branch-next-step.test.ts (`keeps a stale answer while the new one loads, so the row doesn't flicker, and its action waits for the new one`)

### The whole stack

- The whole stack of a root ([Whole Stack](whole-stack.spec.md)) has the same table, its facts those of its stack
  branch `{prefix}/stack/<root>` while the stack forks (`stackFactsOf`, `getStackNextStepNow`, `useStackNextStep`): the
  robot of the whole stack, the robot of a branch of it that lands a part of it (the start waits for it, a dry run
  doesn't), its own pull request (never retargeted: it goes into the target by construction), the readiness of the
  whole stack (`stack:<root>`, below), its stack branch's commits; a branch of it that syncs is its sync. Its texts name
  it "the whole air stack" (`subject`): "Safe Push of the whole air stack waits for Ilya Korennoy". A stack branch the
  sync didn't build disables its Safe Push and its Dry Run with the reason (`getStackBranchBlocker`: "The stack branch
  conflicts at 0caf9066 in config.js: the stack lands in pieces"); row 7 offers "Create Pull Request of the Stack…"
  ("Create Pull Request…" in the slot of the island header and the landing overview), the code owners of the whole
  stack preselected.
  Its Safe Push lands every branch of it ("Safe Push · 7", hover "Lands 7 branches · 14 commits into master in one
  Safe Push: air, bazel, … side-fix and side-test", the main line first, then the side branches). A straight chain has
  no stack branch: its whole stack is the step of its last branch, which waits for the robot of a branch of it as the
  stack branch does (row 2).
  [@test] ../../src/hooks/useStackNextStep.test.ts (`is the Safe Push of the stack branch of a stack that forks, which needs the readiness of the whole stack`, `is the step of the last branch of a straight chain, which has no stack branch: its Safe Push lands the whole stack`, `waits for the robot of a branch of a straight chain, not the failure of the earlier piece; a new parent commit starts anew`, `is disabled with the reason of a stack branch the sync didn't build, for a Dry Run too: the stack lands in pieces`, `waits for the robot of a branch of the stack, which lands a part of it; a Dry Run doesn't`, `offers nothing while the robot of the whole stack runs: it holds every branch of the stack`, `asks for the pull request of the whole stack where the approvals are missing, then waits in it`, `is settled once the pull request of the whole stack merged with the stack branch as it is`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does, the move made from the click`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the stack and whole stack cases answer as the app answers today`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_as_bd_answers_today`)
- The step of the whole stack knows which branches of it merged Safe Pushes landed already (`getWholeStackRest`, the
  same `getLandedBy` as the rows): a piece that landed its main line leaves the rest, which its Safe Push lands. The
  stack branch still carries the landed commits (Patronus finds them in the target), so the start names and counts
  only the rest: "Safe Push" for one branch left, hover and popover "Lands 1 branch · 2 commits into master in one Safe
  Push: side; air, bazel and deploy landed with deploy"; a branch landed with commits after it counts those. Once every
  branch of it landed (pieces, or its own robot), nothing is left: row 1, Safe Push Whole Stack and Dry Run Whole Stack
  disabled with "Landed in master with deploy and side" ("Merged by Patronus" of its own robot).
  [@test] ../../src/hooks/useStackNextStep.test.ts (`lands only the rest after a piece landed its main line, and offers nothing once every branch of it landed`, `offers nothing once its own robot landed every branch of it`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should land only what is left of the whole stack after a piece landed its main line, and offer nothing once every branch of it landed`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the stack and whole stack cases answer as the app answers today`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_as_bd_answers_today`)
- Approvals count only in the pull request whose comment started the robot, so those of the whole stack count for its
  Safe Push only. While its pull request is open, row 7 of a branch of it without a pull request of its own is a quiet
  "In #12" that opens it, the reason on hover ("Approvals in #12, the pull request of the whole air stack, count for
  its Safe Push only"); the Safe Push of the branch itself still needs approvals of its own, as S says. The popover of
  a piece of the stack says it first: "#12, the pull request of the whole air stack, is open: its approvals count only
  for the Safe Push of the whole stack, not for this Safe Push of deploy." (a confirm reason, `wholeStack`).
  [@test] ../../src/lib/branch-next-step.test.ts (`row 7: a branch of a whole stack whose pull request is open points to it, quietly: its approvals count for the Safe Push of the whole stack`)
  [@test] ../../src/lib/safe-push-decision.test.ts (`opens the popover for a key, a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the stack and whole stack cases answer as the app answers today`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_as_bd_answers_today`)

### The surfaces

- The slot (`NextStepSlot`, the step column of the Actions cell, [Branch columns](../../docs/design/ui-theme.md#branch-columns)) renders the step, and hosts the popover of the branch's start
  (`SafePushPopover`, anchored to the slot); "⋯" has "Safe Push" (S) first and in its "Patronus ›" "Safe Push With
  Profile…", "Dry Run" and "Check Approvals Again" (`check-approvals`, always enabled), each disabled with the reason
  of the step, and "Retarget #7 to
  master" for any open pull request into another base; S on the focused row opens the popover, or, disabled, says why in
  an `info` toast (a precondition, `notice`): a short title naming the branch, the reason as the description ("Safe
  Push of auth waits for Ilya Korennoy" / "IJPL Code Platform · requested on #1078", "Safe Push of auth waits for 2
  reviews", "#7 of auth is a draft", "Safe Push of auth can't start" / "Landed in origin/master"), with "Open Pull
  Request" while the step waits in it, the step's own action while it is the user's ("Create Pull Request…", "Create
  Pull Request of the Stack…" for the whole stack, row 7; "Request Review…", row 9; "Retarget #7 to master", row 5),
  and else "Open in Patronus" of the robot that runs on the branch or lands it ("A Safe Push of the last commit is
  running"); a click that waited for an answer of the start (the profiles, a readiness the branch moved past
  meanwhile) says the same notice when the answer is the step's reason; the preview footer, the review window (`getBranchReviewItems`), S in the preview and the
  stack header read the same blocker. Your Turn is not a branch row: its Safe Push of a reviewed pull request has its
  own check ([Review Requests](../repository/review-requests.spec.md)).
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say on S why the whole stack can't start without its pull request, and create it from the notice`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should open Request Review… from the notice of S while nobody of a group is asked`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the retarget of a pull request into the parent's branch as the next step, whose approvals don't count`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should ask for the approvals of the whole stack of a small stack while its root's row shows, and offer its pull request where they are missing`)
- The node of the row, at the left edge of the name cell, renders the step too, as where the branch is on its way
  into master ([Branch Row](branch-row.spec.md#tree), `getLandingNode` over the same facts): ready to land, the user's
  step first, a fix, landing now, a wait, in master. It says what the slot can't: why the slot is empty (row 1: a
  robot runs, the branch is held, landed, blocked) and, in a repository not on Space, which has no slot, the pull
  request it lands through; the header of a stack island has the node of the whole stack.
  [@test] ../../src/lib/landing-node.test.ts (`is ready to land once Patronus answered that a Safe Push can land it, with the branches it lands`, `waits for others: the reviewers, its parent, a robot that lands an ancestor, a base that may be meant`)
- A quiet step is the state as its label (the [UX Writing](../../docs/guidelines/ux-writing.md#actions) exception): a
  ghost `xs` button in `text-muted-foreground` (`destructive` for Changes Requested), the `ClockIcon` before "Awaiting
  …"; the label truncates to the slot; the hover gives the whole text, a line per missing group ("Awaiting Ilya
  Korennoy · IJPL Code Platform", "1 more approval · IJPL Code Platform" for a group that has an approval and needs
  more), then "Open Pull Request #1078" with O, which opens it on the focused row too. The pull request chip keeps its
  own state: the row doesn't say "awaiting" twice.
- The start of a Safe Push renders the step too (`getSafePushDecision`, [Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)):
  it reads the next step with the readiness of the branch as it is (never a stale one: the start waits for the new
  answer, `StartStepOptions`), so a refusal says the reason of the step, and missing approvals refuse it unless the
  chosen profile is known to skip the review (`skipsReview`). Row 7 also offers a [Safe Push without
  review](patronus.spec.md#safe-push-without-review) in the "Create pull request" dialog (`BranchNextStep.withoutReview`),
  never on the Safe Push surfaces, which stay disabled. A push that the rules of the repository make dismiss the
  approvals Patronus counts (`approved`) would start the robot without them: the start is refused with "The push
  dismisses the approvals of #7: push first, then ask for the review again", and the push alone is offered ("Push").
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should say in the popover that Patronus doesn't tell whose approvals count, and refuse the start once they are missing`, `should start with the profile of the last robot when its options said it skips the review`)

### The readiness store

- The session of the repository asks the readiness (`remote_facts::scheduler`, `repository_model::readiness_facts`;
  `facts.readiness` of its view, by subject); `src/stores/review-readiness.ts` only applies its answers, for the row,
  the start, the "Create pull request" dialog (its code owners) and the stack header. The window tells what to ask
  (`src/model/demand.ts`): the subjects a start (a click that waits, its popover) or the dialog needs now
  (`demandReviewReadiness`, `needed`), and of the subjects a surface shows (`showReviewReadiness`, through
  `useBranchNextStep`) those whose next step depends on the readiness (`needsReadiness`: on GitHub with the pull requests
  listed, and past rows 1 to 6; pushed or not, as Patronus judges the local commits, so an unpushed branch's step is
  never an answer left from before; a stacked branch too, for its own pull request), as visible branches and stacks: a
  row on the screen, not one a collapsed stack, side stack or island hides (it is not rendered), and of the root of a
  stack that forks its whole stack too, so a small stack without an island knows the approvals of its whole stack before
  a click; the header of an island (its whole stack, below, or the last branch of a straight chain, also while
  collapsed); the preview; the review window (its chain, or `stack:<root>` of the review of the whole stack).
  [@test] ../../src/stores/review-readiness.test.ts (`tells the session the shown subjects whose step waits for the readiness, the whole stack by its root`)
  [@test] ../../src/model/demand.test.ts (`sends the readiness a surface needs now as needed, and of the shown subjects those whose step waits for it`)
  [@test] ../../src/lib/branch-next-step.test.ts (`asks for the readiness of a branch on GitHub, pushed or not: Patronus judges the local commits`)
  [@test] ../../crates/test-server/src/review_readiness_test.rs (`the_session_asks_the_readiness_a_window_needs_from_the_fake_patronus`)
- The ask of a subject is what its row shows: the tip (the copy the sync built), and the start of the robot: the open
  pull request into the target with its reviewers and author while Patronus names the GitHub repository, else through
  the API (`domain_policy::readiness::start_pull_request`; the page's own `readinessInputOf` follows the same rule).
  The whole stack is one more subject, `stack:<root>` (`stackReadinessKey`, a name no branch has): asked as its stack
  branch (`ReviewReadinessBranch.source`) with the pull request of the whole stack, only while the sync built the stack
  branch (another state has nothing its Safe Push would land).
  [@test] ../../crates/repository-model/src/readiness_facts_test.rs (`the_ask_is_the_tip_a_window_shows_and_the_pull_request_its_robot_starts_from`)
  [@test] ../../crates/domain-policy/src/readiness_test.rs (`a_safe_push_starts_from_the_open_pull_request_into_the_target_while_patronus_names_the_github_repository`)
- What a surface needs and the rechecks of Check Approvals Again go at once, each in a call of its own beside whatever
  is in flight: a click waits for no pre-check but its own. The rest is a queue: the kept answers of all of it first,
  in one read (the object cache keeps them by what Patronus decides on), then the shown subjects before the hidden ones,
  at most 4 pre-checks at a time, in table order, the next as one answers. A hidden subject is asked once it shows; its
  answer stays meanwhile, marked stale when it shows with another input. One exception: a hidden subject whose last
  answer waits for an approval in the pull request of the same tip and start is still asked when a review of it
  changes, so its approval is still news. Another repository is another session, which waits for none of the calls of
  the one before. Patronus not answering for the repository (not logged in, failing), nothing is asked: the subjects a
  window asks for are `unavailable` with why, so a click never waits for an answer that can't come.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`asks_a_demanded_branch_at_once_in_its_own_call_while_the_batch_is_in_flight`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`asks_the_shown_rows_before_the_hidden_awaiting_ones_at_most_four_in_parallel`, `answers_the_cache_before_the_queue`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`a_repository_change_during_a_batch_does_not_wait_for_it`)
  [@test] ../../crates/remote-facts/src/review_readiness_test.rs (`the_kept_answers_come_from_the_cache_without_asking_patronus`)
  [@test] ../../crates/repository-model/src/readiness_facts_test.rs (`new_inputs_make_the_readiness_stale_and_a_hidden_subject_is_asked_only_for_new_reviews`, `a_needed_subject_is_unavailable_while_patronus_does_not_answer_for_the_repository`)
  [@test] ../../crates/domain-policy/src/refresh_test.rs (`a_demanded_readiness_is_asked_at_once_then_the_shown_ones_then_the_hidden_that_wait_for_an_approval`)
- An answer is keyed by its input: the tip, the start and the reviews. A sync onto a new baseline that keeps them asks
  nothing; a new tip or a new review asks once, its readiness stale in the patch that shows it. The answer of the old
  input stays, marked stale, until the new one arrives, so the row doesn't flicker; a start never acts on a stale one,
  nor on one whose input the branch moved past before the session's patch of it (`freshReadiness` compares the key).
  An answer Patronus didn't give is `unavailable`, asked again 5 minutes later on the clock of the session; a read that
  failed after an answer is `unavailable` too, never a stale answer the start would wait on.
  [@test] ../../crates/domain-policy/src/refresh_test.rs (`the_readiness_is_asked_for_new_inputs_a_recheck_and_five_minutes_after_patronus_failed_never_for_the_same_inputs`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`asks_again_five_minutes_after_patronus_did_not_answer`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_readiness_a_window_needs_is_in_its_view_and_stale_in_the_patch_of_a_new_tip`)
  [@test] ../../src/stores/review-readiness.test.ts (`shows the readiness the session answered for the input the branch has, and none while it loads`, `keeps the answer of the old input, stale, while a new review loads: the row doesn't flicker`)
  [@test] ../../src/stores/review-readiness.test.ts (`is not fresh for an input the branch moved past before the session's patch of it`, `is unavailable when the session can't ask Patronus, and when its read failed after an answer`)
  [@test] ../../src/lib/branch-next-step.test.ts (`keeps a stale answer while the new one loads, so the row doesn't flicker, and its action waits for the new one`)
- "Check Approvals Again" refreshes the pull requests from GitHub, then asks the session to ask Patronus again with
  `recheck` (`refresh_facts { readiness, manual, subject }`: new inputs, the answer stale from the press on). An answer
  of an ask that started before it (the queue's) never replaces the recheck's, and its pre-check replaces no answer the
  cache kept since it was asked: the recheck is the last word. While a row waits for an approval (Awaiting), a window
  focus asks GitHub for the pull requests at once, not after the usual 2 minutes: the user comes back from asking for
  the review; and each poll of the review requests asks for them too (also while the header of a whole stack waits).
  The approval that ends the wait is news of the session with "Safe Push" ([Review
  Requests](../repository/review-requests.spec.md#approval-news)); in the pull request of the whole stack, "Ilya
  Korennoy approved the whole air stack" with "Safe Push Whole Stack".
  [@test] ../../src/lib/approval-news.test.ts (`names the whole stack for the approvals of its pull request`)
  [@test] ../../src/stores/review-readiness.test.ts (`asks again with recheck after the pull requests from GitHub, the answer stale meanwhile`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`keeps_the_answer_of_a_recheck_when_the_batch_asked_before_it_answers_later`)
  [@test] ../../crates/repository-model/src/readiness_facts_test.rs (`the_answer_of_an_ask_started_before_a_recheck_never_replaces_the_rechecks`)
  [@test] ../../crates/remote-facts/src/review_readiness_test.rs (`a_background_answer_after_a_recheck_beside_it_keeps_the_answer_of_the_recheck`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should ask Patronus again from Check Approvals Again, after the pull request from GitHub`)

## User Experience

- The slot is as wide as the widest step of the view, at most 10rem, its step a ghost `xs` button with its
  `compactLabel` ("Retarget #7", "Awaiting Review", "Create Pull Request…" of the whole stack), every label of a step
  within the 10rem; a start that runs keeps the width the step had at rest ("Checking…", the spinner and "45%" of its
  push, "Starting…"; "Pushing 45%" and the branches it pushes in its name and hover), so nothing moves under the pointer
  ([UX Writing](../../docs/guidelines/ux-writing.md#actions)).
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`fits every label of a step in its column, so none truncates`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
  [@test] ../../src/lib/branch-next-step.test.ts (`row 5: retargets an open pull request into a virtual branch, of its parent or of one that landed, whose approvals don't count`, `row 10: every missing group has someone asked: the quiet wait names them and opens the pull request`)
- Disabled, a step keeps the focus and the hover
  (`focusableWhenDisabled`), dimmed to `opacity-50`. A quiet step is muted, Changes Requested `text-destructive`.
- Wording follows [UX Writing](../../docs/guidelines/ux-writing.md): Title Case for the steps, sentence case for the
  reasons, people by their full name (the login without one).

## Testing / Local Run

- `pnpm test:unit`: `src/lib/branch-next-step.test.ts`, `src/stores/review-readiness.test.ts`; `cargo nextest run -p
  remote-facts -p repository-model -p domain-policy -E 'test(/readiness/)'` for the queue of the session.
- `pnpm e2e`: `tests/e2e/next-step` (`TestRepositoryBuilder.setReviewAnswers`, the answers of the fake Patronus's
  pre-check `AWAITING_CODE_OWNER`, `APPROVED_BY_CODE_OWNER`, `NOT_EVALUATED`, `NO_REVIEW_NEEDED` of
  `tests/e2e/helpers/patronus-helpers.ts`, judged by the real core); a start from the pull request asserts the
  `/patronus merge` comment the fake GitHub got, a review request the `requested_reviewers` request.

## References

- [Patronus](patronus.spec.md): the robot of a row, the code-owner review, the start of a Safe Push
- [Branch Row](branch-row.spec.md#actions-cell): the Actions cell
- [Pull Requests](pull-requests.spec.md): the reviewers and the author the readiness takes
- [Stack Island](stack-island.spec.md#header): the Safe Push of the whole stack in its header
