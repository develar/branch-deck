## MODIFIED Requirements

### Requirement: The sidebar and the diffs: A change, a hit and a declaration land in their card

A change N and P show, a hit of the find and a declaration shown MUST land in their card, the opening of its step above
the view, so their place on screen never depends on the height of the opening; below the heading of a group, the
heading at the top whole, the card right below it.

#### Scenario: N and P, a hit of the find, a declaration
- **WHEN** N or P shows a change, the find goes to a hit or a declaration is shown
- **THEN** it lands in its card, the opening of its step above the view
- **AND** in the first card of a group the heading of the group stands whole at the top above it
  [@test] src/lib/diff/layout.test.ts (`shows a change, a hit of the find and a declaration in a card below the opening of its step: the top of the card is their least top`)
  [@test] src/lib/diff/layout.test.ts (`stops at a card with the heading of its group at the top with it, whole, and at a card below an opening with the opening above the view`)
