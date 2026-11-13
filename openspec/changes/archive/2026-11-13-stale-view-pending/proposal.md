# Proposal

## Why

A review of `merged-robot-earlier-state` found the guard against a view of another table half done:
- The whole stack read what is left of it, and whether its own robot landed it, from the stale view (`StackLanding.rest`
  of the view), so a stack whose member got a commit showed "Merged" with nothing to land while its member offered a
  Safe Push; the filter in `stack_landing` was dead, as only the derivation of the view itself calls it.
- The reverse flicker: a reword, squash or move of an older commit (and an update) gives every commit above it new
  ids. While the view of the new table is derived, a branch landed by content then lost its landing and its hold, and
  its slot offered a Safe Push for a moment.
- `is_whole_of` asked `after` to be empty while `left_of` didn't: of a view naming commits after what landed that the
  branch no longer has, the step contradicted itself.
- A merged robot whose commit Patronus didn't name had the dot and no notice.
- Several readers of the guard had no test that failed without them, and the closes of a merge read a stale view too.

## What Changes

- A view knows a branch while each commit it has now is one the view says landed or names after it
  (`LandedByView::knows`); of one it doesn't, the view is of another table: Patronus is pending for the branch and for a
  whole stack of it (row 0, no start, not landed, no robot of an earlier state), and every reader of what the view says
  landed it reads nothing (`LandedFacts`, `BranchTable::known_landed`, `view_knows`, `left_after`): the step, the start,
  the holds, Push with Children, Push Up to Here, the closes of a merge.
- What is left of a whole stack, and whether its own robot landed it, is read of the table as it is (`stack_rest`, the
  one function of the view's derivation and of the steps).
- `is_whole_of` is `left_of` empty.
- The notice of a merge of no named commit: "Patronus named no commit of this merge, so it may not have landed the
  newest commits."
- The chip of the row, the node's hover and the title bar read no earlier state: they show a running robot or one of
  the subject as it is, whose dot the remote status measures as Rust does.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/next-step`: a view of another table leaves its branch and whole stack pending; commits named after what
  landed that the branch no longer has leave it landed whole.
- `branch/patronus`: the notice of a merge of no named commit.

## Impact

- `crates/domain-types/src/repository_view.rs`, `crates/domain-policy/src/{landed,next_step,stack}.rs`,
  `crates/repository-model/src/{branch_table,derive,action_view,facts}.rs`, the registry of the scenario table.
- `src/lib/patronus.ts`, `src/lib/robot-overview.ts`, `src/hooks/useRowRobot.ts`, `LandingNodeHover`,
  `ReviewRobotStatus`; `docs/design/landing.md`.
