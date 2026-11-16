## MODIFIED Requirements

### Requirement: Keyboard: The keys of a row act only where the row has the focus

The keys of a row MUST act while the focus is on the branch row (one of its buttons too, a disabled one included,
which takes only Enter and Space and presses nothing) or on the table, never in an input, a menu, a popover, an inline
form or the commit list of an expanded row; and only without a modifier, but ⌥← and ⌥→, which collapse and expand the
stack of the row ([Stacked Branches](../stacked-branches/spec.md#ui)).

#### Scenario: A key in an input or a menu
- **WHEN** the focus is in an input, a menu, a popover, an inline form or the commit list of an expanded row, or the key has a modifier
- **THEN** the key of the row does nothing

#### Scenario: ⌥← and ⌥→
- **WHEN** the user presses ⌥← or ⌥→ on a row
- **THEN** the stack of the row collapses or expands

#### Scenario: A key on a disabled button of the row
- **GIVEN** the focus on a disabled button of a row: the Safe Push of its slot that `brd` runs, or its push that waits for the lock
- **WHEN** the user presses ↑, ↓, S or P
- **THEN** the focus moves to the previous or the next branch row, and S or P says why it can't start, as from the row
- **AND** Enter and Space press nothing
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should take the keys of the row from its disabled Safe Push: the arrows move to the rows beside, S says why it can't start, Enter presses nothing`)
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should take the keys of the row from a push waiting for the lock: the arrows move to the rows beside, P says whom it waits for, Enter pushes nothing`)
