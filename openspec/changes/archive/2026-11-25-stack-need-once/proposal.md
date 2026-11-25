# Proposal

## Why

The row of the root of a stack that forks shows what its whole stack needs ("Needs stills-codec", the edge of its unit
in the landing plan). Every branch built on it said the same need in its own slot, so one need of one unit read as
three yellow buttons, the complaint the plan exists to answer. And the state of a waiting row ("Lands after
stills-codec") truncated to "Lands after stills-co…" in the Status column while the step's slot beside it was empty.

## What Changes

- A branch of a stack whose root's row hosts its whole stack, every need of whose start is an edge of the unit, says it
  as a state (`StepKind::WithStack`: "Needs stills-codec with lane-run", quiet, a press shows the row of the root):
  the need shows once, on the root, which fixes it. A need the unit lacks stays the branch's own row 1′; a need that
  is landing stays "Lands after X".
- The state a step waits in reads whole up to 16rem in the `state` track (`data-step-wait`): it stands for the step,
  whose slot is empty.

Capabilities: `branch/next-step`, `branch/branch-row`.
