## ADDED Requirements

### Requirement: Review window: A stack opens on its first layer to read and folds the others

Review Stack MUST read what every layer leaves to read in one read, open on the first layer with something to read once
the layers below it are read, unless the user chose an item first, and let ↑/↓ pass a layer with nothing to read. The
overview of All Changes MUST sum the layers and fold each layer with nothing to read, muted, with its proofs and its
Approve on its head.

#### Scenario: A stack of four, two with nothing to read
- **WHEN** the window reviews a stack whose bottom and third layer change formatting alone
- **THEN** it opens on the second layer, ↓ shows the top, ↑ the second again and no further
- **AND** All Changes says "2 with nothing to read · 2 to read · 2 changes", the two folded with "Nothing to read · formatting" and Approve, which approves that layer on its head
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open a stack on its first layer to read, fold the layers with nothing to read and pass them by ↑/↓`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`a_stack_counts_its_layers_and_opens_on_the_first_one_to_read_once_those_below_are_known`)
  [@test] src/presenters/range-reading.test.ts (`sums the layers of a stack`)
