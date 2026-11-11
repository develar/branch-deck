## ADDED Requirements

### Requirement: Renames: The runs of a renamed name are labels until their kind is switched on

The kind `renamed` MUST NOT be quiet before a measurement of real ranges has read a sample of its proven runs and
found no change of behaviour among them: until `is_quiet` switches it on, every run of it MUST be a label and every
block of it a stop. The pair MUST name the proven runs that are labels so (`PairMarks::proven`), and the marks of
a view MUST be the marks as they are proven (`PairMarks::as_proven`) but for the quiet of those runs and the starts
that follow.

#### Scenario: A renamed local as a view gets it
- **WHEN** a local renamed at its five places is read as a view reads it, in each grammar
- **THEN** the diff has the rename and its runs, none of them quiet and each named as proven, and every block its start at its first line
- **AND** as proven the same runs are quiet, and no block has a start
  [@test] crates/file-diffs/src/marks_renames_test.rs (`the_runs_of_a_renamed_local_are_labels_until_their_kind_is_switched_on`)

#### Scenario: Every diff of the tests
- **WHEN** the files of the corpus are edited at random, or have a name renamed, and each diff is read as proven and as a view reads it
- **THEN** the marks of the view are the proven ones with the runs of a kind that is not switched on as labels, and the starts of those runs
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`)

## MODIFIED Requirements

### Requirement: Limits: The marks are bound by work, never by time

The limits of the marks MUST be of work, never of time. A block whose sides are the same code needs no comparison.
Else the tokens of its code are compared only within `MAX_BLOCK_TOKENS` a side, `MAX_TOKEN_EDITS` tokens that differ
and the work left of its file (`MAX_FILE_TOKEN_WORK`); a block over a limit MUST have no run cut out of it. The
blocks of a rename are compared again within what the first reading left of that work.

#### Scenario: A block over a limit
- **WHEN** the sides of a block differ in more tokens than the limit, a side has more tokens than the limit, or the work of the file is spent
- **THEN** no run is cut out of the block, and its reading starts at its first line
- **AND** the same block within the limits has its run, and lines of the same code are a run without a comparison
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_over_a_limit_of_work_has_no_run_cut_out_of_it`)

#### Scenario: The blocks of a rename
- **WHEN** a block holds a renamed name, a wrapped call and a line with another change, and is read again with the names read as one
- **THEN** with the work of the file it has its run of the rename beside the run of the wrapped call; with one step less than the comparison costs it keeps the run of the wrapped call the first reading found, and no run of the rename
- **AND** a block that is the same code once the names are read as one has its runs without a comparison, with no work left too
- **AND** a block the first reading cut nothing out of has the same runs: the wrapped call is a run of formatting there too
  [@test] crates/file-diffs/src/marks_renames_test.rs (`the_blocks_of_a_rename_are_cut_again_within_the_work_the_file_has_left`)

#### Scenario: The comparison gives up by its edits
- **WHEN** two sequences differ in more elements than the edits allowed
- **THEN** the comparison answers "too big" after work bound by the edits, and within them the changes of the unbound comparison
  [@test] crates/text-diff/src/lcs/diff_test.rs (`the_changes_within_a_number_of_edits_are_the_changes_or_too_big`)
