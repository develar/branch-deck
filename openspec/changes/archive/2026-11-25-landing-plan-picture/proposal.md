# Proposal

## Why

The landing plan is a fact of the session (units, edges, waves, Land Together with its price), but the app showed it
only one edge at a time, on a row or in the popover of a start. A user who cuts work into pieces needs to see the way
into master at once: what lands now, what is next, what waits for what, and where two pieces may land in one robot at
what price. The popover offered Land Together without any price at all.

## What Changes

- Landing has two sections, Plan (the first) and Statistics; the statistics are read only while they show.
- The Plan draws the waves as columns, each unit a card (node, robot with its time, size, checks), each edge a curve with
  its evidence on hover, each offer of Land Together a dashed frame around the pair with its price and the actions of
  its edge; the parked branches apart; a stale plan says so.
- "Show Plan" beside "Show Statistics" in the landing overview; the Status cell "Lands after X" opens the Plan with its
  edge highlighted; a click on a card shows the row of its unit.
- The Safe Push popover says the price of Land Together under its buttons.
- A Land Together of a stack that forks leaves the Safe Push of the whole stack as the next step, held while its stack
  branch is not built (already so: an e2e test pins it).

Capabilities: `branch/landing-plan`, `repository/main-views`, `repository/landing-overview`.
