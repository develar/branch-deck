## ADDED Requirements

### Requirement: In the Review Window: A stop at a card shows the heading of its group whole

A stop at a card (J and K at a card, a row of the tree) MUST show what stands above it in its section whole or not at
all: the heading of a group ("Imports only (1)", "view.gui · Tests · 1 file") at the top, whole, the card right below
it; an opening, a stop of its own, wholly above the view, never a cut edge of either under the bar of the step.

#### Scenario: J to the first card of a group and to a card below an opening
- **WHEN** J goes from the opening of a step to its card, from the opening of Other changes to its card, and to the
  first card of the group of the imports
- **THEN** each opening goes wholly above the view
- **AND** the heading "Imports only (1)" stands whole at the top, the card right below it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should stop at the first card of a group with the heading of the group whole above it, and at a card below an opening with the opening wholly above the view`)
  [@test] src/lib/diff/layout.test.ts (`stops at a card with the heading of its group at the top with it, whole, and at a card below an opening with the opening above the view`)
