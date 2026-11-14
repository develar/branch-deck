## MODIFIED Requirements

### Requirement: Measurement: A subject is measured as a view reads it

A measurement (`FileDiffs::measure`) MUST read a subject through the stream of a view and give each file as the
view gets it, with the times of the read and of each stage of the index, both texts, whether its tokens were read,
the runs proven whatever their kind, the starts with them passed, and what of the marks read again differs. With an
index it MUST ask the renamed names: a run it confirmed is proven. Under other thresholds of a move, their moves.
Diffs without marks MUST enter no store.

#### Scenario: The template of the marks
- **WHEN** the commit of the `commit_marks` template is measured
- **THEN** its files are those a view reads, the first content came before the end, and `Ledger.kt` has its two texts, its tokens read, the wrapped call and the three lines of the renamed local proven and the comment not, and five starts with the proven runs passed
- **AND** the marks read again are the marks of the view in every file, and the calls of the name another file declares are proven by no index
  [@test] crates/file-diffs/src/marks_measure_test.rs (`a_measured_subject_is_what_a_view_reads_with_its_texts_its_proofs_and_its_times`)

#### Scenario: Without the marks
- **WHEN** the same commit is measured without its marks, with a store
- **THEN** every file has no run, no move, no rename and nothing of its subject, a start at each block, and no entry in the store
- **AND** a view that reads the commit next gets its marks
  [@test] crates/file-diffs/src/marks_measure_test.rs (`a_subject_read_without_marks_has_a_stop_at_each_block_and_no_store_keeps_it`)

#### Scenario: A run the index confirmed
- **WHEN** a commit renames a function its file declares and calls at two places, and is measured without an index and with one that lists the declaration and both calls in both trees
- **THEN** without the index the two calls are labels, each a stop, and no run is proven; with it they are quiet and proven, the declaration alone is read, and the marks read again from the texts are those of the view
  [@test] crates/file-diffs/src/marks_measure_test.rs (`a_run_the_index_confirmed_is_proven_and_the_marks_read_again_are_those_of_the_view`)

#### Scenario: The time of each stage of the index
- **WHEN** a commit takes a function whole to another file under another owner, with a file whose call writes the new owner in front of it, and is measured with an index that answers the move and the uses of the function, and without an index
- **THEN** with the index the renames, the moves and the uses that follow the move each have a time, the moves and the uses are asked once each, and the call is confirmed; without it no stage has a time
- **AND** a subject no file of which may follow a move has the times of the renames and the moves and none of the uses
- **AND** a commit that only adds lines, measured with the index, asks no moves and has no time of them
  [@test] crates/file-diffs/src/marks_measure_test.rs (`each_stage_of_the_answer_of_the_index_has_its_time`, `a_run_the_index_confirmed_is_proven_and_the_marks_read_again_are_those_of_the_view`)

### Requirement: Measurement: `brd commits --mark-stats` counts the marks of real commits and prints a sample of each kind

`brd commits --mark-stats` MUST measure each commit ahead of the baseline, each commit of `--range`, or that range
whole (`--whole`), and print, in all and by grammar, what the marks find and cost; `--mark-sample` adds a sample of
each kind with its lines, `--no-marks` reads without marks, `--index` asks the index, `--min-move-chars` and
`--max-edited` count what other thresholds add. `--index` without accio, a commit id and a filter MUST be refused.
It MUST write the object cache only.

#### Scenario: The counts and the sample
- **WHEN** `brd commits --mark-stats --mark-sample 2` runs on the `commit_marks` template
- **THEN** it prints one subject of Kotlin, the quiet kinds and the thresholds of the moves, fifteen blocks of three files and nine stops, the runs of each kind with those that are quiet and proven, the two moves by their size, the renames with what proves them, the move between two files and the times
- **AND** the sample holds two marks of a kind at most, each with its file, its lines of both sides and the lines around them: the wrapped call, a move with its edited line marked, a rename from its first place to its last, the lines that left for another file
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_mark_stats_of_a_commit_count_each_kind_and_print_a_sample_of_their_lines`)

#### Scenario: A range
- **WHEN** it runs with `--range origin/master..HEAD`, with `--whole` too, with an empty range, and with a range that is no two revisions
- **THEN** the commits of the range are the subjects, with the counts of the commits ahead of the baseline; the range whole is one subject named by its two ends; an empty range has none
- **AND** a range of one revision, or of one that is no commit, is refused as invalid arguments; so are a commit id, `--branch`, `--unassigned` and `--include-generated` beside it
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_subjects_are_the_commits_of_a_range_or_the_range_whole`)

#### Scenario: The index
- **WHEN** it runs with `--index` on the `commit_marks` template, with no index, and with one that lists the declaration of the renamed function and its two calls
- **THEN** without an index it is refused as invalid arguments; with one the two names are `confirmed`, the two calls are quiet and proven, two stops fewer are left, the marks read again are those of the view, and the times of the renames and of the moves are printed (`index_ms`, `moves_ms`) of the one commit (`count`), and none of the uses that follow a move (`moved_uses_ms`), which no file of it may follow
- **AND** without the flag no index is asked, whatever index there is
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_index_is_asked_of_the_renamed_names_and_refused_where_the_machine_has_none`)

#### Scenario: Other thresholds of a move
- **WHEN** it runs with `--min-move-chars 16` and with `--max-edited 1/2` on a commit that moves a line of 17 characters and a function with two of its five lines edited
- **THEN** it counts and samples what each search adds beside the moves of the views: the short line as a move added, the function as a move that takes the place of the one line of the views; the moves of the views are counted as before
- **AND** a share that is none (`1`, `3/2`, `1/0`) is refused as invalid arguments
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`other_thresholds_of_a_move_count_and_print_the_moves_they_add`)
  [@test] psi-syntax:crates/line-diff/src/moves_test.rs (`a_measurement_reads_the_moves_under_other_thresholds`)

#### Scenario: Without the marks
- **WHEN** it runs with `--no-marks`
- **THEN** every block is a stop, no kind has a run, the sample is empty, and a run with the marks after it counts nine stops again
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_mark_stats_without_the_marks_have_a_stop_at_each_block`)

#### Scenario: The processes
- **WHEN** it runs on three commits ahead of the baseline, on their range, and on the range whole
- **THEN** each commit costs one `diff-tree` and one `cat-file` beside the grouping of `brd commits`; a range costs its two ends and one walk in place of the grouping; the range whole one list and one read
  [@test] crates/bd-cli/src/process_budget_test.rs (`mark_stats_read_the_files_and_the_blobs_of_each_commit_once`)
