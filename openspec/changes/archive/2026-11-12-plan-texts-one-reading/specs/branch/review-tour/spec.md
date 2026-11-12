## MODIFIED Requirements

### Requirement: Brief: A removed paragraph and a similar added one are one changed paragraph

A removed paragraph and the first similar added one after the last pair (half of the shorter kept and a third of the
longer, by word) MUST be one `changed` paragraph with the words the change wrote (`inserted`) and removed (`deleted`,
each at the offset of the text it stood before), the rest `added` or `removed`. The word diff of a pair MUST be bounded
by its work (the changes the comparison may count), never by time, so that the brief of a range is the same under any
load of the machine.

#### Scenario: A new paragraph and lines appended to one
- **WHEN** a change adds a paragraph alone, and appends lines to another
- **THEN** the new one is `added` alone, and the other is `changed` by the lines appended to it
  [@test] crates/review-plan/src/prose_test.rs (`a_new_paragraph_is_added_alone_and_lines_appended_to_one_change_it`)

#### Scenario: A rewritten paragraph
- **WHEN** a change rewrites a paragraph and no similar added one pairs with the removed one
- **THEN** it is `removed` and `added`
  [@test] crates/review-plan/src/prose_test.rs (`a_rewritten_paragraph_is_removed_and_added_and_two_blocks_of_one_paragraph_are_one`)

#### Scenario: A paragraph added before a changed one
- **WHEN** a change adds a paragraph before one it changes
- **THEN** the changed one keeps its pair, `changed` with its `inserted` and `deleted` words, and the new one is
  `added`
  [@test] crates/review-plan/src/prose_test.rs (`a_paragraph_added_before_a_changed_one_leaves_it_its_pair`)
