# Proposal

## Why

A review of `merged-robot-lands-what-it-took` found:
- Its rule right in every state, but the preview contradicted it. The spec wants a robot of an earlier state kept in
  the preview and the overview "with the notice of what changed", while the page gave every merged robot the commit
  state "current", so air-ide-ahp showed "Merged" with no dot and no notice beside a row that offered a Safe Push.
  The dot requirement ("A merged robot MUST get no dot") contradicted it, and the page could not decide it: with no
  remote status loaded, its commit state is "current" anyway.
- Its change of the slot where no start is offered had no test and no line in the spec: a failed or canceled robot of
  an earlier state now gives way there too.
- A merged robot whose commit Patronus didn't name still landed the branch whole.
- The scenario of commits a merged robot never took named the wrong step for Patronus pending and a failed sync.
- The whole stack had no test of its own.
- An older gap: while the Patronus view of a new table is derived, the stale view says a branch with a commit since
  landed whole (`landed_by.after` empty), and the row showed it landed until the new view came.

## What Changes

- `NextStep.earlier_robot`: the id of the robot of the subject while it is of an earlier state of it, the verdict of
  the slot (`is_earlier`). The chip words it (`RobotFlags.earlier`, `getPatronusChipState`), with no rule of its own:
  the dot, and for a merged robot the notice that the merge didn't land the newest commits. The preview, the
  overview, the node's hover and the title bar read it of the step of their subject.
- A merged robot without a commit is of an earlier state (`is_outdated`).
- The answers of a view are read against the commits the branch has now: landed whole only while every commit of the
  branch is one it landed (`LandedByView::is_whole_of`, `left_of`; `LandedFacts.branches`; `BranchTable::landed_whole`),
  in what landed a subject, the reason of a start, its own robot, the holds of what landed, Push with Children, Push Up
  to Here and the rest of a whole stack.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/next-step`: a merged robot lands only the commits it took, also without a named commit, of a whole stack,
  and by a view of an older table no commit added since; the steps while Patronus is pending and after a failed sync.
- `branch/patronus`: a robot of an earlier state gives way also where nothing starts, and keeps its dot and notice in
  the preview as Rust decides it; no dot only for a merged robot of the branch as it is.

## Impact

- `crates/domain-policy/src/next_step.rs`, `crates/domain-policy/src/landed.rs`, `crates/domain-types`
  (`NextStep`, `LandedByView`, fixtures), `crates/repository-model/src/{derive,action_view,branch_table}.rs`, the
  `LandedFacts` of `bd-remote` and `operations`; `src/lib/bindings.ts`.
- The page: `src/lib/patronus.ts`, `src/hooks/useRowRobot.ts`, `src/lib/robot-overview.ts`, `src/hooks/useRobotOverview.ts`,
  `PatronusChip`, `LandingNodeHover`, `ReviewRobotStatus`.
- A case of the scenario table (`whole-stack-merged-robot-of-an-earlier-stack-branch`, the robot `olderStack` of its
  vocabulary), tests of the rule, the Status cell, the landed state, the derivation, the chip and an E2E of the preview.
