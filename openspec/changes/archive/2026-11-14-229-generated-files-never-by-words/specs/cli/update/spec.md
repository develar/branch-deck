## ADDED Requirements

### Requirement: Rounds: A generated file is never resolved by words

A file that `generated-files` calls generated (by its name, its attributes `linguist-generated` or `-diff`, or the
header a tool wrote into a side) MUST NOT be resolved by words: its conflict opens a round, whose file names why
(`generated_by`) and whose `hint` says to take a side, then run the generator and commit its output as a `fixup!`.

#### Scenario: A generated file whose conflict its words resolve
- **WHEN** an update conflicts in a generated file whose words would resolve the conflict
- **THEN** a round opens, and its file has `generated_by` and a `hint` with the takes and the `fixup!` of the commit
- **AND** `--status` names `generated_by` too
  [@test] crates/bd-update/src/resolve_test.rs (`a_generated_file_whose_conflict_its_words_resolve_opens_a_round_and_the_finish_names_it_to_regenerate`)

#### Scenario: A file a person writes with the same conflict
- **WHEN** the same conflict is in a file a person writes
- **THEN** the words resolve it, and the update applies without a session
  [@test] crates/bd-update/src/resolve_test.rs (`a_generated_file_whose_conflict_its_words_resolve_opens_a_round_and_the_finish_names_it_to_regenerate`)

### Requirement: Update: `verify.regenerate` names the generated files both sides changed

An update that applies (at once or at the finish of a session) MUST list in `verify.regenerate` each generated file that
the local commits and the baseline both changed since their merge base, each to another blob, and that the new HEAD
keeps, with `generated_by`: also without a conflict, since a generated file can be stale without one.

#### Scenario: A round of a generated file taken from `onto`
- **WHEN** a session finishes after a round of a generated file took `onto`
- **THEN** `verify.regenerate` names the file with `generated_by`
  [@test] crates/bd-update/src/resolve_test.rs (`a_generated_file_whose_conflict_its_words_resolve_opens_a_round_and_the_finish_names_it_to_regenerate`)

#### Scenario: A generated file that git merges
- **WHEN** an update applies without a conflict, and both sides changed a generated file
- **THEN** `verify.regenerate` names it
  [@test] crates/bd-update/src/resolve_test.rs (`an_update_without_a_conflict_names_the_generated_files_both_sides_changed`)

#### Scenario: A generated file of one side
- **WHEN** only one side changed a generated file, both sides made the same change, or the new HEAD deleted it
- **THEN** `verify.regenerate` does not name it
  [@test] crates/git-ops/src/generated_test.rs (`the_generated_files_both_sides_changed_are_the_ones_to_regenerate`)
  [@test] crates/bd-update/src/resolve_test.rs (`an_update_without_a_conflict_names_the_generated_files_both_sides_changed`)

## MODIFIED Requirements

### Requirement: Update: `verify.paths` names the files to build and test

At exit 0 after a resolution, `verify.paths` MUST name the files to build and test: the files of the rounds that kept
their commit with `how` edited, commit, deleted or words, and the files of the commits an update resolved by words;
the first 100 (101 rather than one left out). A file of `onto` is the baseline's and is not in it; without such files
and without `verify.regenerate`, `verify` MUST be absent.

#### Scenario: An edited file
- **WHEN** a session finishes with a round resolved by an edit
- **THEN** `verify.paths` names its file
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

#### Scenario: An update resolved by words
- **WHEN** an update whose conflict resolves by words applies
- **THEN** `verify.paths` names the files of the commit the words resolved
  [@test] crates/bd-update/src/resolve_test.rs (`an_update_whose_conflict_resolves_by_words_applies_and_names_the_commit`)

#### Scenario: A drop leaves nothing to verify
- **WHEN** the only round drops its commit
- **THEN** `verify` is absent
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_with_every_line_of_the_commit_drops_without_a_question`)
