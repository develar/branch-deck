## ADDED Requirements

### Requirement: Check: A generated file never resolves by words

With the resolve by words on, a conflict in a file that `generated-files` calls generated (its name, its attributes
`linguist-generated` or `-diff`, or the header a tool wrote into a side) MUST stay a conflict: the check stops at its
commit. The copies of a sync still resolve it by words, on purpose for now (a conflict there recurs on every sync of
two branches that change one generated file): the open trade-off of
[develar/branch-deck#232](https://github.com/develar/branch-deck/issues/232).

#### Scenario: A conflict of a generated file that its words resolve
- **WHEN** the replay of the check meets a conflict in a generated file whose words resolve it
- **THEN** the replay stops at that commit, the first conflict of the update
  [@test] crates/git-ops/src/word_replay_test.rs (`a_conflict_in_a_generated_file_stops_the_replay_though_its_words_resolve`)

#### Scenario: By its header, its attributes, its name
- **WHEN** the update's resolve by words meets a generated file of each kind
- **THEN** each stays a conflict, and the resolve of a sync merges the same conflict by words
  [@test] crates/git-ops/src/word_resolve_test.rs (`a_generated_file_stays_a_conflict_of_an_update_and_a_written_one_resolves`)
