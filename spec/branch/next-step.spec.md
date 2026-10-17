---
name: Next Step
description: Requirements for the one model of what to do next with a branch (getBranchNextStep), the first matching row of a table of its states (a sync, a failure, a block, a hold, landed, a robot that runs or merged, a robot to restart, a stacked branch, a pull request into another base or a draft, the approvals that count missing, waited for or approved, as Patronus judges them), rendered by the Safe Push slot of the row, the "⋯" menu, S, the branch preview, the review window, the stack header and the Safe Push form; the readiness store that asks Patronus for it in the background, and Check Approvals Again.
style: plain-1
targets:
  - ../../src/lib/branch-next-step.ts
  - ../../src/hooks/useBranchNextStep.ts
  - ../../src/stores/review-readiness.ts
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/hooks/usePatronusReviewRequirement.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/branch/BranchPreviewActions.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/components/branch/InlinePatronusStart.tsx
  - ../../src/components/branch/PatronusReviewBlock.tsx
  - ../../src/components/shared/InlineForm.tsx
  - ../../src/stores/pull-requests.ts
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
"Safe Push…" is disabled with the same reason, no action is offered whose failure is known.

## Goals

- One answer to "what now" for a branch, the same in the row, the menu, the keys, the preview, the review window and
  the stack header.
- Never offer an action that Patronus would fail; say why instead, and hand over the action that helps.
- A wait reads as a wait: the state is the label, and it opens the pull request it waits in.

## Non-goals

- Judging approvals in the app: Patronus is the only judge; the app renders its readiness.
- The notification when an approval arrives ([Review Requests](../repository/review-requests.spec.md)).

## Requirements

### The table

- The next step is the first row that matches. "Safe Push… elsewhere" is the reason every other surface disables
  "Safe Push…" with (`safePushBlocker`, `getSafePushBlocker`); a Dry Run is never held back by approvals, a draft, a
  pull request into another base or a robot that lands an ancestor (`dryRunBlocker`).

  | # | State | Slot | Safe Push… elsewhere |
  |---|---|---|---|
  | 1 | Syncing, failed to sync, no commits, blocked, held by a Safe Push, landed, a robot runs, merged (by the robot, or the pull request of these commits) | nothing; the Status cell says it | disabled: "Wait for the sync to finish", "Landed in origin/master", "Merged by Patronus", "Pull request #7 is merged", … (a running robot of an older commit: enabled, the start replaces it) |
  | 2 | A robot failed or was canceled, not for approvals | "Restart Safe Push" (Safe Push… where Patronus can't restart it) | enabled |
  | 3 | A robot failed for approvals, started from the open pull request into the target, readiness `approved` | "Restart Safe Push" | enabled |
  | 4 | A stacked branch (its start goes through the root) | "Safe Push ‹root›…", the form of the root | as the root |
  | 5 | An open pull request into another base | "Retarget #7 to master" | disabled: "Approvals in #7 don't count: its base is release-252" |
  | 6 | An open draft | quiet "Draft", opens it | disabled: "#7 is a draft: Patronus merges it once it is ready for review" |
  | 7 | `missing`, no open pull request (none, or a closed one) | "Create Pull Request…" (the code owners preselected) | disabled: "Code owners must approve in a pull request: IJPL Code Platform" |
  | 8 | `missing`, a member of a missing group requested changes | quiet "Changes Requested" in `destructive`, opens the pull request | disabled: "Ilya Korennoy requested changes on #1078" |
  | 9 | `missing`, a missing group has nobody asked | "Request Review…" | disabled: "Waits for the approval of Air Platform: nobody of it is requested on #1078" |
  | 10 | `missing`, someone of every missing group is asked | quiet "Awaiting Ilya Korennoy" ("Awaiting 2 Reviews" for several people), opens the pull request | disabled: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078" |
  | 11 | `approved`, `notRequired`, `unknown`, `unavailable`, not asked yet | "Safe Push…" | enabled |

  [@test] ../../src/lib/branch-next-step.test.ts (`row 1: offers nothing while the Status cell says why, and disables Safe Push and Dry Run with the reason`, `row 1: a merged pull request of other commits (new work under the same name) blocks nothing`, `row 2: restarts a failed or canceled robot, or offers Safe Push… where Patronus can't restart it`, `row 3: restarts a robot that failed for approvals only from the pull request they now count in`, `row 4: a stacked branch lands root first, Safe Push… as the root`, `row 5: retargets an open pull request into another base, whose approvals don't count`, `row 6: a draft is a quiet step that opens it; Patronus merges it once it is ready, a dry run runs`, `row 7: missing approvals without an open pull request: Create Pull Request…, the code owners named`, `row 8: changes requested by a member of a missing group: a destructive quiet step that opens the pull request`, `row 9: a missing group with nobody asked: Request Review…, disabled until the commits are pushed`, `row 10: every missing group has someone asked: the quiet wait names them and opens the pull request`, `row 11: Safe Push… for every other readiness, and before the first answer`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`, `should offer Safe Push… once the code owner approved, and name whose approval counts`, `should keep Safe Push… while Patronus doesn't tell whose approvals count, and say so`, `should offer Create Pull Request… with the code owner when approvals count only in one`, `should retarget a pull request into another base, whose approvals don't count`, `should offer nothing after a merge, and say so in the menu`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`, `should offer no Safe Push of a branch whose pull request merged`)
- Rows 8 to 10 count only the groups still short of approvals: changes requested by a member of an approved group
  don't block, as Patronus doesn't block on them. With work the remote lacks the push slot shows the push and the step
  still shows the wait; Request Review… is disabled then: "Push first, so the reviewers see the commits". A missing
  group nobody can approve (`noEligibleReviewer`) disables Request Review…: "Nobody of IJPL Code Platform can approve
  these changes".
  [@test] ../../src/lib/branch-next-step.test.ts (`row 8: changes requested in an approved group don't block, as Patronus doesn't`, `rows 8 to 10: the wait shows with work to push; the push slot pushes it`)
- Row 3: a robot that failed for approvals keeps the pull request step of before ("Open Pull Request", or "Create Pull
  Request…" without one) while the readiness is not known (not asked yet, `unknown`, `unavailable`): a restart may fail
  again. Approved, but started without the pull request: "Safe Push…", a new start from it counts the approvals. The
  restart of such a robot in "⋯" is disabled with the reason of rows 7 to 10 while the approvals are missing.
  [@test] ../../src/lib/branch-next-step.test.ts (`row 3: keeps the pull request step while nobody can tell whether the approvals count`)
- Row 4: the root lands first, its Safe Push gates the stack: the slot opens the Safe Push form of the root (the row is
  revealed first, also in a collapsed stack), disabled with the root's reason. "Safe Push…" of the stacked branch (its
  menu, S, the preview) opens its own form, which leads root first, and is disabled as the root's is. The header of a
  stack island keeps the Safe Push of the last branch of its main line ([Stack Island](stack-island.spec.md#header)),
  disabled as the root's is.
- An inline action of the row disables the action of the slot ("Wait for the action on the branch to finish"), never a
  quiet step: it only opens the pull request.
  [@test] ../../src/lib/branch-next-step.test.ts (`disables the action of the slot while an inline action of the row runs, never the quiet steps`, `disables the Safe Push of a branch a robot of its ancestor lands, its slot says why`)
- The pull requests are a fact of their own: until GitHub lists them (not configured, signed out, SSO, not loaded),
  the readiness counts as `unavailable` and rows 5 to 10 don't apply: the table never offers a Create Pull Request… for
  a branch that may have one. A repository only on Space (no GitHub) never asks; its Safe Push stays possible, its
  merge requests are deprecated and the robot decides. A repository not on Space has no slot.
  [@test] ../../src/lib/branch-next-step.test.ts (`treats the readiness as unavailable without the pull requests: never a duplicate Create Pull Request…`, `has no slot in a repository that is not on Space, and keeps the reasons of the branch`)

### The surfaces

- The slot (`NextStepSlot`, `w-40` in every row) renders the step; "⋯" has "Safe Push…" (S), "Dry Run…" and "Check
  Approvals Again" (`check-approvals`, always enabled), each disabled with the reason of the step; S on the focused row
  opens the form, or, disabled, says the same reason in an `info` toast (a precondition), with "Open Pull Request"
  while the step waits in it; the preview footer, the review window (`getBranchReviewItems`), S in the preview and the
  stack header read the same blocker. Your Turn is not a branch row: its Safe Push of a reviewed pull request has its
  own check ([Review Requests](../repository/review-requests.spec.md)).
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)
- A quiet step is the state as its label (the [UX Writing](../../docs/guidelines/ux-writing.md#actions) exception): a
  ghost `xs` button in `text-muted-foreground` (`destructive` for Changes Requested), the `ClockIcon` before "Awaiting
  …"; the label truncates to the slot; the hover gives the whole text, a line per missing group ("Awaiting Ilya
  Korennoy · IJPL Code Platform", "1 more approval · IJPL Code Platform" for a group that has an approval and needs
  more), then "Open Pull Request #1078" with O, which opens it on the focused row too. The pull request chip keeps its
  own state: the row doesn't say "awaiting" twice.
- The Safe Push form waits for the readiness of the branch as it is (never a stale one) and, with approvals missing on
  GitHub, disables Start with the reason on its hover: "Patronus checks the approvals when the robot starts and fails
  without them" (`InlineForm.submitDisabledReason`). Its review block is driven by the readiness
  ([Patronus](patronus.spec.md#code-owner-review)).
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should say in the form whose approvals don't count, and disable Start while Patronus would fail it`)

### The readiness store

- `src/stores/review-readiness.ts` is the one reader of the readiness, for the row, the form, the "Create pull
  request" dialog (its code owners) and the stack header. It asks `get_branch_review_readiness` for the branches whose
  next step depends on it (`needsReadiness`: pushed, on GitHub with the pull requests listed, and past rows 1 to 6),
  in table order (there is no cheaper signal of the visible rows: every row is rendered), the branches a form or the
  dialog shows first, at most 8 per call, one call in flight. The start is the one of the robot: the open pull request
  into the target with its reviewers and author, else through the API (`readinessInputOf`).
  [@test] ../../src/stores/review-readiness.test.ts (`asks for the wanted branches in one batch, with the start the robot would take`, `keeps one batch in flight, of at most READINESS_BATCH_SIZE branches`, `asks only the branches whose next step depends on the readiness`)
- An answer is keyed by its input: the tip, the start and the reviews. A sync onto a new baseline that keeps them asks
  nothing (the backend keeps the answers on disk by what Patronus decides on); a new tip or a new review asks once. The
  answer of the old input stays, marked stale, until the new one arrives, so the row doesn't flicker; the form never
  acts on a stale one. A failed answer is `unavailable`, asked again after 5 minutes on the next change.
  [@test] ../../src/stores/review-readiness.test.ts (`asks nothing again while the input stays: another sync, the same reviews`, `keeps the answer of the old input, stale, while a new review loads: the row doesn't flicker`)
  [@test] ../../src/lib/branch-next-step.test.ts (`keeps a stale answer while the new one loads, so the row doesn't flicker`)
- "Check Approvals Again" (and Recheck in the form) refreshes the pull requests from GitHub, then asks Patronus again
  with `recheck`, the answer stale meanwhile. While a row waits for an approval (Awaiting), a window focus asks GitHub
  for the pull requests at once, not after the usual 2 minutes: the user comes back from asking for the review.
  [@test] ../../src/stores/review-readiness.test.ts (`asks again with recheck after the pull requests from GitHub, the answer stale meanwhile`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should ask Patronus again from Check Approvals Again, after the pull request from GitHub`)

## User Experience

- The slot is `w-40`, its step a ghost `xs` button; disabled, it keeps the focus and the hover
  (`focusableWhenDisabled`), dimmed to `opacity-50`. A quiet step is muted, Changes Requested `text-destructive`.
- Wording follows [UX Writing](../../docs/guidelines/ux-writing.md): Title Case for the steps, sentence case for the
  reasons, people by their full name (the login without one).

## Testing / Local Run

- `pnpm test:unit`: `src/lib/branch-next-step.test.ts`, `src/stores/review-readiness.test.ts`.
- `pnpm e2e`: `tests/e2e/next-step` (`TestRepositoryBuilder.setReviewAnswers`, the fixtures `AWAITING_CODE_OWNER`,
  `APPROVED_BY_CODE_OWNER`, `NOT_EVALUATED`, `NO_REVIEW_NEEDED` of `tests/e2e/helpers/patronus-helpers.ts`).

## References

- [Patronus](patronus.spec.md): the robot of a row, the code-owner review, the Safe Push form
- [Branch Row](branch-row.spec.md#actions-cell): the Actions cell
- [Pull Requests](pull-requests.spec.md): the reviewers and the author the readiness takes
- [Stack Island](stack-island.spec.md#header): the Safe Push of the main line
