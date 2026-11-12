# Proposal

## Why

The plan of a review asks the index of the repository (accio) four queries at once: the usages of its removed and
changed API, the registrations of its plugin descriptors, what enters each new module, and the tests of the change
that reach the code of its steps. The first three answer in seconds. The tests are cold after each new tree of a
branch and then take a minute or more on a loaded machine. The plan waited for all four together, so the uses left
outside and the verdict of the breakage, which a reviewer reads first, waited for the tests. A window that asked the
plan the read ahead was reading (below the app, at nice 19) got no plan at all until that reading ended.

The cost of the tests query is accio's ([#191](https://github.com/develar/branch-deck/issues/191)): no cap, no
deadline and no cache of its answers here. What is ours is that one slow answer holds the fast ones.

## What Changes

- `review_plan_of` sends the plan in stages, each with what it still waits for (`PlanWait`): the plan of git alone
  before the index is asked (`index`), the plan with the usages, the registrations and the entry points while the
  tests of the steps are still asked (`tests`), then the plan whole. The tests query runs beside the others for the
  whole reading and is applied last. A reading whose tests are no query of the index has no such stage.
- `ReviewPlanEvent::Plan` carries `waits` in place of `pending`; a step whose code the index is asked of is marked
  (`StepTests::awaited`) in the plans sent meanwhile, never in the plan a reading answers, so `brd review-plan`
  prints what it printed.
- The leader of a reading publishes each stage beside its flight; a window that joins the reading gets the latest
  one at once and each later one as it comes, then the plan. Only the plan the reading answers is kept in the memory.
- A window that joins a reading led below the app (the read ahead) makes it yield: its queries are killed, it ends
  without a plan, and the window reads the plan itself at the priority of the app.
- The page says what is awaited: "Finding the uses left outside…" while nothing of the index answered, "Finding the
  tests of the change…" once only the tests are asked, and "Finding its tests…" in each step whose code is asked.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: the plan in stages on its channel, a joined reading shows its stages, the read ahead yields to
  a window, what the memory keeps, the words of the overview.
- `branch/review-tour`: the tests of the steps are asked beside the other queries and hold none of them; a step says
  its tests are being found.

## Impact

- `crates/review-plan/src/lib.rs` (`review_plan_of`, `PlanWait`, `ReviewPlanEvent`), `crates/review-plan/src/steps.rs`
  (`TestsAsk`, `ReviewSteps::apply_tests`, `StepTests::awaited`).
- `crates/app-api/src/reads/review_plan.rs` (`Stage`, `Follower`), `crates/app-api/src/reads/review_prefetch.rs`.
- `crates/test-server/src/state.rs` (`FixtureIndexes`, the held answer `stepTests`).
- `src/lib/review-plan-answer.ts`, `src/hooks/useReviewPlan.ts`, `src/presenters/review-story.ts`,
  `src/components/review/ReviewPrologue.tsx`, `src/components/review/StepFactsLine.tsx`, `src/lib/bindings.ts`.
- `docs/design/change-review.md`, `docs/design/git-operations.md`, `docs/design/e2e-testing.md`.
