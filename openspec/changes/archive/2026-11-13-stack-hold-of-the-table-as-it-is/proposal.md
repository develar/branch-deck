# Proposal

## Why

A review of `push-reads-the-branch-as-it-is` found the stack branch still held as a view of another table decided it:
only "every branch landed" was read against the table as it is (`derive::stack_hold_now`); a parked member, a stack
branch the sync did not build and its own robot came as the view said them. A member parked since the view let Push
with Children and the push of the whole stack send the stack branch with the parked commits; a robot of the whole
stack that ended since the view refused the push ("A Safe Push of the whole air stack is running"). None of the four
readers had a test that failed on the old read. The start read the holds of the view by a filter of its own
(`LandingFacts` over a decided view: no check of the robot), the push a refused start offers read the push blocker the
view derived of its own table (`landing.push`), and `specta(skip)` hid the fields of the view the page must not read
from its types only: serde still sent them in the view, its patches and the mirror of the IDE.

## What Changes

- `derive::stack_hold_now` decides the hold of a stack branch of the table and the robots as they are
  (`stack_branch_hold` over the parked member of the table, what landed each member whole now, the robot of the whole
  stack as it is now, the stack branch of the table); every reader calls it (`Holds::stack_hold`): the push view, Push
  with Children, the push of the stack branch alone, the push of the pull request of the whole stack, the step of the
  whole stack, the start. The step of the whole stack names its own robot while it holds the stack branch
  (`NextStepView.hold`), which the page reads instead of the view.
- One rule of what landed a branch whole now (`derive::landed_whole_now`: its robot merged, every commit of the branch
  as it is landed); the start reads the holds of a decided view by `hold_now`, as every reader does.
- The push an action needs is refused of the table as it is: `domain_policy::push::action_push` names what of its chain
  stops it (`stop`), `derive::action_push_of` words it, the view sends it (`BranchPush.actionBlocker`), which the push a
  refused start offers reads. `BranchLanding.push` and `domain_policy::holds::push_blocker` go.
- What only Rust reads of the view is `#[serde(skip)]` (`landedBy`, `landedByRobot`, the hold of a branch and of a stack
  branch, `StackLanding.rest`): off the wire, the patches and the mirror; the frame keeps it beside the wire part
  (`warm_start::KeptPatronus`), `FRAME_SCHEMA` 15.
- `lint:arch` (`view-landing-read`) refuses a read of `landing.hold` and `landing.push` too; `isHoldingRobot` and
  `getBranchLanding` go.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/patronus`: the holds of a view are read of the branches and the robots as they are by every reader, the
  stack branch's too; the push an action needs is refused of the table as it is.
- `branch/parked-branches`: the stack branch of a stack that holds a parked branch pushes nothing, also while the view
  is of the table before the park.
- `branch/next-step`: what holds the stack branch of a whole stack of a view of another table is read of the table and
  the robots as they are; nothing the page must not read goes on the wire.

## Impact

- `crates/domain-policy/src/{push,holds}.rs`, `crates/domain-types/src/{repository_view,session}.rs`,
  `crates/repository-model/src/{derive,facts,warm_start}.rs`, `crates/operations/src/{push,pull_request}.rs`;
  `src/lib/bindings.ts`.
- `src/stores/{patronus,branch-sync}.ts`, `src/lib/{push-run,patronus}.ts`, `scripts/lint-arch.mjs`;
  `docs/design/{landing,git-operations}.md`.
