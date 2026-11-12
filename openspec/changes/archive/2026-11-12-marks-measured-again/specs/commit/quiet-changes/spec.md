## RENAMED Requirements

- FROM: `### Requirement: Moves: A move holds at most a quarter of edited lines between exact ones`
- TO: `### Requirement: Moves: A move holds at most a third of edited lines between exact ones`

## MODIFIED Requirements

### Requirement: Moves: A move holds at most a third of edited lines between exact ones

A move MUST hold edited lines only between exact ones, at most a third of its lines that are not blank: a pair of
lines that is similar by word, the rule of a row of the Inline view. `edited` MUST name the edited lines of a move by
their place in it; `oldRanges` and `newRanges` MUST give what changed in each as the ranges of a block, each a text
that is another on the other side. A move none of whose lines has a range has empty lists.

#### Scenario: An edited line
- **WHEN** a function moves with an argument added to one of its five lines, a word changed, or a word removed
- **THEN** it is one move, and the line has its ranges, typed as those of a block, in the offsets of its own line where the lines were indented too
- **AND** two edited lines of five, an edited line at an end, and a line that is another line are no lines of the move: the exact lines beside them are asked alone
  [@test] crates/text-diff/src/moves_test.rs (`a_move_holds_at_most_a_third_of_edited_lines_between_exact_ones`)

### Requirement: Moves: A move says the columns its lines were indented by

`indent` of a move MUST be the columns of its shift, negative for lost ones: the same whitespace added to the indent
of each of its lines, or taken from it, at one end of the indent. An indent is a text, not a number of columns.
Where the indentation of the file has a meaning and the policy counts the whitespace at the ends of a line, a line
whose indent did not move by the shift MUST be an edited line of the third of its move; elsewhere `indent` is the
shift most of its exact lines agree on.

#### Scenario: Lines indented into a block
- **WHEN** a function moves into a block and is indented by two spaces, by a tab, or out of it
- **THEN** the move says `2`, `1`, or the negative number
- **AND** of lines indented unevenly it says the columns of most of them, and a line that stayed at its indent is the same line
- **AND** tabs for the spaces of every line are no shift: the move says `0` and each indent is a range
- **AND** whitespace added before the text of each line, after its tab, is a shift as whitespace added before the tab is
  [@test] crates/text-diff/src/moves_test.rs (`a_move_says_the_columns_its_lines_were_indented_by`)

#### Scenario: A file whose indentation has a meaning
- **WHEN** lines move in such a file, all at the same indent or all shifted alike
- **THEN** the move is the same as in any file, with its shift
- **AND** a line at another indent than the others is an edited line whose first range is its indent, before the ranges of its text; a third of the lines of a move may be such lines; more of them, or one at an end, are no lines of the move
- **AND** a line that stayed at its indent while the others shifted, at the first column or at an indent of its own, is an edited line without a range, never one with a range that is empty or the same text on both sides; three such lines of six end the move
- **AND** a line of a recipe whose tab became a space is an edited line whose range is its indent; when every line of the recipe did, only its target moved
- **AND** two lines that gained two columns, one of spaces and one of tabs, did not move alike: the second is an edited line
- **AND** under a policy that leaves the whitespace at the ends of a line out, no indent is a change of a move
  [@test] crates/text-diff/src/moves_test.rs (`where_the_indentation_has_a_meaning_a_line_at_another_indent_is_edited`)

### Requirement: Moves: The whitespace at the ends of a line that changed otherwise than its move is a range

Under the policy that counts every character, a line of a move whose indent is another text and did not move by the
shift, or whose whitespace after its text is another, MUST be an edited line of the move with that whitespace as a
range, a blank line too: a view that shows the old lines of a move once hides no change. Where the indentation has
no meaning it MUST count in no third and end no move. Under a policy that leaves that whitespace out it MUST be no
change of a move.

#### Scenario: Under each policy
- **WHEN** a function moves and two of its five lines are indented by two more columns than the others, a line of a raw string by eight more, a line lost or got its indent, got tabs for its spaces, or the whitespace after its text was added, lost or changed
- **THEN** under the policy that counts every character it is one move, each such line an edited line whose range is its indent or the whitespace that ends it, beside the ranges of its text
- **AND** two such lines of five, and such a line at an end of the move, are lines of the move
- **AND** under `Trim`, `Ignore` and `IgnoreBlankLines` the move has no such range, where the indentation has a meaning too
  [@test] crates/text-diff/src/moves_test.rs (`the_whitespace_at_the_ends_of_a_line_that_changed_otherwise_than_its_move_is_a_range`)

### Requirement: Measurement: A subject is measured as a view reads it

A measurement (`FileDiffs::measure`) MUST read a subject through the stream a view reads and give each file as the
view gets it, with the times, both texts, whether its tokens were read, the runs proven whatever their kind, the
starts once they are passed, and what of the marks read again differs. With an index it MUST ask of the renamed
names, and a run it confirmed is proven and no difference; with other thresholds of a move, the moves under them.
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
- **THEN** without an index it is refused as invalid arguments; with one the two names are `confirmed`, the two calls are quiet and proven, two stops fewer are left, the marks read again are those of the view, and the time of the answer is printed
- **AND** without the flag no index is asked, whatever index there is
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_index_is_asked_of_the_renamed_names_and_refused_where_the_machine_has_none`)

#### Scenario: Other thresholds of a move
- **WHEN** it runs with `--min-move-chars 16` and with `--max-edited 1/2` on a commit that moves a line of 17 characters and a function with two of its five lines edited
- **THEN** it counts and samples what each search adds beside the moves of the views: the short line as a move added, the function as a move that takes the place of the one line of the views; the moves of the views are counted as before
- **AND** a share that is none (`1`, `3/2`, `1/0`) is refused as invalid arguments
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`other_thresholds_of_a_move_count_and_print_the_moves_they_add`)
  [@test] crates/text-diff/src/moves_test.rs (`a_measurement_reads_the_moves_under_other_thresholds`)

#### Scenario: Without the marks
- **WHEN** it runs with `--no-marks`
- **THEN** every block is a stop, no kind has a run, the sample is empty, and a run with the marks after it counts nine stops again
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_mark_stats_without_the_marks_have_a_stop_at_each_block`)

#### Scenario: The processes
- **WHEN** it runs on three commits ahead of the baseline, on their range, and on the range whole
- **THEN** each commit costs one `diff-tree` and one `cat-file` beside the grouping of `brd commits`; a range costs its two ends and one walk in place of the grouping; the range whole one list and one read
  [@test] crates/bd-cli/src/process_budget_test.rs (`mark_stats_read_the_files_and_the_blobs_of_each_commit_once`)
