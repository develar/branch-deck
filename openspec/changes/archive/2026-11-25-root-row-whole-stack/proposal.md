# Proposal

## Why

A stack that forks lands as its whole stack, one robot of `{prefix}/stack/<root>`, and the landing overview and the
Plan say so of its unit ("Safe Push · 4"). Without an island the whole stack had no place on the rows: the slot of the
root's row showed the root's own "Safe Push", which lands the root alone, so the row and the overview disagreed and the
obvious button landed the wrong thing; the whole stack started only from "Whole Stack ›" of the root's menu.

## What Changes

- Rust decides that the row of the root of a stack that forks, which lands as its whole stack
  (`landing_plan::lands_as_whole_stack`, the rule of the unit of the plan), hosts that whole stack
  (`NextStepView::hosts_whole_stack`): its node is the node of the whole stack unless something of the root itself
  decides it (`landing_node::root_own_node`), its Status cell the state the whole stack waits in
  (`row_status::hosting_row_status`); `brd status` says it of the branch (`next_step.hosts_whole_stack`).
- The slot of that row shows the step of the whole stack ("Safe Push · 4", disabled by `StackNotBuilt` as before), S
  and the preview start it, and its "⋯" leads with "Safe Push Whole Stack" (S), the root's own start after it as "Safe
  Push Root Only" ("Dry Run Root Only" in "Patronus ›"). In an island the header shows that step: the root's row leaves
  its slot to the header, as it leaves Push with Children. A straight chain keeps its rows: its last branch's Safe Push
  is its whole stack. A stack with a parked branch lands in pieces: its root's row stays its own.
- The toast of a merge names a whole stack that waited as "Safe Push Whole Stack".

Capabilities: `branch/whole-stack`, `branch/next-step`, `branch/branch-row`, `cli/agent-cli`.
