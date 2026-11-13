# Proposal

## Why

A review of `page-reads-the-table-as-it-is` found the class still open in the push: after a commit of login that the
view of the robots never saw, the step said "Refresh Patronus", the menu offered the push, and a click refused it
"Landed in master with oauth: nothing of it is left to push". The pushes read the hold of the view as it decided it
(`PushFacts::state`, `alone_push_of`, `Holds::reason`; of a whole stack `HeadedStack.hold`, `stack_branch_along`, the
push of the stack branch and the push of its pull request), and so did the page (`factsOf` of its store). The guard of
the page against reading the view was a lint of literal reads only, the store kept getters no page code called, the
toast of a merge read `landedByRobot` of the view, and docs repeated a heading.

## What Changes

- One rule of the holds of a view (`derive::hold_now`): its hold while its robot holds pushes; a hold of what landed
  while every commit of the branch as it is now landed. Every push reads it (`Holds::hold`); of a whole stack,
  `derive::stack_hold_now` holds the stack branch as landed only while every branch of it is what landed.
- The step view of a branch carries what holds its push now (`NextStepView.hold`); the page reads it and no hold of
  the view. The types the page gets have no `landedBy`, `landedByRobot`, `StackLanding.rest` or `BranchLanding.hold`
  (`specta(skip)`): a read of them is a type error; `lint:arch` stays the second guard, wider.
- The news of a merge names what it landed of the branches the view knows (`NewsEvent::RobotMerged::landed`), which
  its toast names.
- The dead getters of the store go (`getLandedBy`, `getStackLanding`, `useStackLanding`, `useBranchLanding`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/next-step`: a view of another table holds no push of a branch it doesn't know, and of a whole stack; a
  branch built on one it doesn't know is pending unless the view knows it landed whole.

## Impact

- `crates/repository-model/src/{derive,facts}.rs`, `crates/operations/src/{push,pull_request}.rs`,
  `crates/domain-types/src/{next_step,repository_view,session}.rs`; `src/lib/bindings.ts`.
- `src/stores/patronus.ts`, `src/lib/patronus-failing.ts`, `src/model/patch.ts`, `src/platform/air/view-mirror.ts`,
  `scripts/lint-arch.mjs`; `docs/design/landing.md`.
