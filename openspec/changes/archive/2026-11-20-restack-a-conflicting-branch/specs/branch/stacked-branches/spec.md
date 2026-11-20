## ADDED Requirements

### Requirement: UI: A branch whose sync conflicts restacks on what it needs

A branch whose commits conflict on the baseline because they need a branch it is not built on MUST be restacked on it
the way any branch is: Stack On…, the drop on the "Stack on ‹name›" chip, Stack Together and Unstack run on it, the
sync after the restack builds it on its new parent, and Undo brings the conflict back.

#### Scenario: Stack On… of a conflicting branch
- **GIVEN** `ui` conflicts on master because it changes a line `api` changed
- **WHEN** the user stacks `ui` on `api` with Stack On… or by a drop on the "Stack on api" chip
- **THEN** `ui` is stacked on `api` and applies cleanly
- **AND** Undo brings the conflict back
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a branch whose sync conflicts on the branch it needs from the menu and by a drop on its Stack on chip, the conflict gone`)
