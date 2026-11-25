# Proposal

## Why

Agents cut the work into the pieces the app lands, and they need the same plan the app shows: what lands now, next and
after what, and where two pieces may land in one robot at what price, with the command that does it.

## What Changes

- `brd plan`, read only: the units, the edges with their kind, state, evidence and actions (each with its command), the
  waves, the parked branches and the proposals of Land Together with their price and `command`, the plan of
  `repository_model::derive::landing_plan_of` over what `brd status` reads; `brd schema plan`.
- The agent help and the skill name it; a proposal's command waits for the user's approval.

Capabilities: `branch/landing-plan`, `cli/agent-cli`.
