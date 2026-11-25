# Proposal

## Why

The edges of stage 1 say each need once and a wait as a wait. The plan they make is still the page's guess: the unit of
the Landing overview is computed in TS, nothing says what lands now, next and after what, and the only way to land two
pieces in one robot is a restack whose price nobody names. The plan becomes a fact of the session that the app and
`brd plan` read, with an honest offer to land a piece with the piece it needs, and the actions of every edge come from
one builder in Rust.

## What Changes

- `domain_policy::landing_plan`: the units (a lone branch, a stack by declared parent without its parked branches; the
  node of its whole stack), the edges of every row lifted to the root of its unit, the waves in the topological order of
  the edges (Landing now, Next, After …), the parked branches apart; stale while the needs are computed.
- Land Together, offered only for a need of the build that nothing lands (`EdgeState::Open`) between units that neither
  land nor failed: the restack of the root onto the branch it needs (`RewriteIntent::Restack` with the refusal it fixes,
  one undo), "Land Together · Safe Push Whole Stack" when the stack they make forks; its price as risks (robots, checks
  of Patronus, a failure holds both, where the approvals count, the code owners the wider diff asks, off Patronus the
  stack of GitHub they become).
- `domain_policy::actions::edge_actions`: one builder of the actions of an edge for the slot, "⋯" and the popover: Land
  Together, Land X First, Restart X (of the robot of the branch it needs), Group … into …, the Safe Push of a cycle.
- The view carries the units keyed (`RepositoryView::landing_units`) and the rest of the plan
  (`RepositoryView::landing_plan`); the Landing overview reads the units of Rust, no rule left in TS.
- The Safe Push popover: a short detail (commits by short hash, the use by file name and line, their whole words on
  hover, no sentence that repeats a button) and the actions of the edge as buttons, Group and Start among them.

Capabilities: `branch/landing-plan`, `repository/landing-overview`, `branch/stack-parents`, `branch/next-step`,
`branch/patronus`.
