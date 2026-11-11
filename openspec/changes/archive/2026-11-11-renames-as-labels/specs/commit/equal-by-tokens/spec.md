## ADDED Requirements

### Requirement: Equality: Two names read as one compare as the same where the tree is

A reader that takes two spellings for one name MUST be able to give the tokens of its places one text on both sides,
each place found by its line and its column of the view. The texts then MUST compare as any tokens do: the same
where the tree around the names is the same, and no other token changes.

#### Scenario: A renamed local
- **WHEN** a local is renamed at its three places, one after a character of two UTF-16 units, and the tokens of the places get one text on both sides
- **THEN** the two texts are the same by tokens, which they are not without it
- **AND** no token starts inside a name, at a blank or past the text, and a place past the tokens is left out
- **AND** the name as an argument of a call, where it was an operand, is another token under the one text
  [@test] crates/code-symbols/src/tokens_test.rs (`two_names_read_as_one_compare_as_the_same_where_the_tree_is`)
