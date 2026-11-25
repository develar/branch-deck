# Proposal

## Why

The row of the root of a stack that forks hosts its whole stack (the change before), and the landing overview and the
Plan show the unit of that stack. The whole stack had no landing needs of its own: while a branch of it needed a branch
outside it, the row and the overview said "Safe Push · 3" with a node ready to land, and only the gate of the start
refused it. A ready node and a slot the start then refuses is the defect the landing plan exists to remove.

## What Changes

- The whole stack takes the needs of its branches outside it, the needs its unit in the plan has
  (`landing_edges::whole_stack_refusals`; its edges `landing_edges`, lifted to its root): a need of a branch of the stack
  is met by the whole stack and dropped. Its step takes rows 0, 1′ and "Lands after X" as a branch's does, so the row of
  its root, the overview, the Plan and `brd status` say one thing: "Needs stills-codec" (Land Together, Land X First,
  Restart X), "Lands after stills-codec", and "Safe Push · N" ready only without an open need.
- The decision of its start reads the needs the view found fresh: its popover says why before the start (Start Anyway);
  while the needs of new tips are computed the gate of the start checks it, as before. `brd safe-push --whole-stack`
  gives its step the needs of its gate.

Capabilities: `branch/next-step`, `branch/whole-stack`.
