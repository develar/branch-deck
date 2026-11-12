## MODIFIED Requirements

### Requirement: Moves: Lines deleted in one block and added in another are a move

Rows of lines that one block of a diff deletes and another adds MUST be a move (`moves`), found from the lines
alone: the same lines by their key, the text of a line without the spaces and tabs at its ends; a blank line pairs
with any blank one. A move MUST count only when its exact lines that stand once on each side, among the lines a move
may take, hold 24 characters that are no whitespace, and its ends MUST lie in two blocks. The empty last line of a
text MUST be no line of a move.

#### Scenario: A function at another place
- **WHEN** a function stands below another function now, with the blank line after it
- **THEN** its lines are one move, from its old lines to its new lines, the blank line with it, whatever ends the lines of the texts
- **AND** moved to the end of the file, where the comparison deletes the blank line after it and adds one before it, it is one move that counts that blank line beside its lines
- **AND** the same text, added lines alone and deleted lines alone have no move
  [@test] crates/text-diff/src/moves_test.rs (`lines_deleted_in_one_block_and_added_in_another_are_a_move`)

#### Scenario: The empty last line of a text
- **GIVEN** the empty last line of a text is the line after its last line end, or the one line of an empty text, which the comparison does not count
- **WHEN** a function leaves, with the blank line after it or before it, for the end of a text, and only the new text ends with a line end, so the block at the end of the texts holds its empty last line
- **THEN** the move is the function alone: the deleted blank line is a line to read, with its start, no pair of the empty last line and no blank line that left with the move, as where both texts end with a line end and where neither does
- **AND** the empty last line of the old text is no line that moved with the function before it
- **AND** a blank line that is a line of both texts moves with the function; a text of one line that left and an empty text have no move
  [@test] crates/text-diff/src/moves_test.rs (`the_empty_last_line_of_a_text_is_no_line_of_a_move`)

#### Scenario: Blank lines inside
- **WHEN** the lines that moved hold blank lines, with other whitespace on the two sides
- **THEN** they are lines of the one move, the whitespace of a blank line that changed a range of it where every character counts
- **AND** a blank line indented with the lines of its move is the same line
- **AND** a blank line that pairs with a line of text ends the move, and blank lines alone are no move
  [@test] crates/text-diff/src/moves_test.rs (`the_blank_lines_inside_a_move_are_lines_of_it`)

#### Scenario: A copy
- **WHEN** a function stays where it was and stands a second time, or one of its two copies is deleted
- **THEN** the diff has no move: no line was deleted and added
  [@test] crates/text-diff/src/moves_test.rs (`a_copy_is_no_move`)

#### Scenario: One long line, short lines
- **WHEN** one line of 24 characters that are no whitespace moves, and one of 23
- **THEN** the first is a move and the second none
- **AND** short lines count together, a blank line counts nothing, and the characters of an edited line do not count
  [@test] crates/text-diff/src/moves_test.rs (`one_long_line_is_a_move_and_short_lines_are_none`)

#### Scenario: Lines that cross nothing
- **WHEN** two functions change places and are indented, so one block holds both with no line between them that stays
- **THEN** the block has no move, also beside another block of the file
- **AND** the same lines in two blocks are a move
  [@test] crates/text-diff/src/moves_test.rs (`lines_that_cross_nothing_are_no_move`)

#### Scenario: Two blocks that change places
- **WHEN** two functions change places
- **THEN** the one the comparison deletes and adds is a move, with the blank line between them, the other stays
  [@test] crates/text-diff/src/moves_test.rs (`two_blocks_that_change_places_are_one_move`)

#### Scenario: The whitespace inside a line
- **WHEN** a line moves with other whitespace inside it
- **THEN** it is no exact line: alone no move, among exact lines an edited line whose ranges are its whitespace
  [@test] crates/text-diff/src/moves_test.rs (`the_whitespace_inside_a_line_is_part_of_its_key`)

#### Scenario: Any texts
- **WHEN** any two texts are compared under each policy, and lines move with a few changes
- **THEN** every move has its old lines in one block that is a change and its new lines in another, all of them lines their text counts, each line in one move, exact ends, and lines that are not edited the same on both sides but for the indent of the move
- **AND** each range of an edited line is a text that is another on the other side, of the kind its two sides say, with the same text between the ranges: none is empty on both sides
- **AND** where every character counts, no whitespace at an end of a line changed without a range, but by the indent of the move
  [@test] crates/text-diff/src/moves_test.rs (`the_moves_of_any_texts_are_moves`, `the_moves_of_moved_lines_are_moves`)

#### Scenario: Lines that stand at two places
- **WHEN** deleted lines stand at two places of the new text, or lines of two old places at one
- **THEN** the diff has no move: nothing says which place took them, or which of the two left
- **AND** a place with all the lines takes them where the other holds only lines that stand elsewhere too, and so does the place that holds a line as it was where the other holds it edited
- **AND** of the exact lines of a move only those that stand once on each side count: a short line of its own among lines that stand twice is no move, a long one is
  [@test] crates/text-diff/src/moves_test.rs (`lines_that_stand_at_two_places_are_no_move_and_the_place_with_a_line_of_its_own_takes_them`)

### Requirement: Limits: The moves are bound by work

The search for moves MUST be bound by work, never by time. A side with more than 65,536 lines a move may take has no
moves; a key that more than 16 such new lines have starts no move; the search and the choice each end at 262,144
pairs of lines read, each read counted, with the moves found by then. At most 2,048 pairs of lines of at most 512
UTF-16 code units are compared by word, and the chains grow through 16,384 pairs left unread: past either number a
pair of other keys is apart.

#### Scenario: A key that many lines have
- **WHEN** 10,000 lines of eight keys move
- **THEN** no line starts a move and the search reads no pair of lines
- **AND** with a line of its own among every hundred, the lines are one move, each pair read once
- **AND** of 16 lines of one key the pairs are read, of 17 none; neither is a move, no line of them standing once
  [@test] crates/text-diff/src/moves_test.rs (`a_key_that_many_lines_have_starts_no_chain_and_is_a_line_of_one`)

#### Scenario: The work of a search
- **WHEN** 10,000 lines of five keys with a line of its own among every hundred move, among them fifty lines that each stand sixteen times
- **THEN** the lines are one move, found with fewer than twenty pairs of lines read a line, within the limit, the same when asked again
- **AND** a search that reaches its limit ends with the moves found until then, and so the choice among them
  [@test] crates/text-diff/src/moves_test.rs (`the_work_of_a_search_grows_with_its_lines_and_ends_at_its_limit`)

#### Scenario: The file a search costs most in
- **WHEN** every line of a file stands sixteen times on each side and no two lines follow each other on both, the lines nothing alike or alike but for a number
- **THEN** the search ends at its limit of pairs read (a pair of two lines read for their likeness counts by their length too), the same when asked again, and finds no move: no line of such a file stands once
  [@test] crates/text-diff/src/moves_test.rs (`the_search_of_a_file_whose_every_line_stands_sixteen_times_ends_at_its_work`)

#### Scenario: A chain of many short moves
- **WHEN** a list of YAML of 2,000 items of five lines moves, indented again by two columns a level in place of four; every second of 6,000 lines of such a file moves under another parent; or 2,500 lines of their own move, each before a line with another number and two blank lines
- **THEN** the chain is read once for all its moves, and the work grows with the lines, not with their square: every move is found, each of a line or two (of the list the name of each item, and its image with its port), with at most six pairs of lines read a line
- **AND** read again for each of its moves, where each pair of what is left of a row counts again, the first chain uses up the work of the search, and less than a tenth of the moves is found
- **AND** where the indentation has no meaning the list is one move
- **AND** at the limit of its work the search ends with the moves found until then
  [@test] crates/text-diff/src/moves_test.rs (`a_chain_of_many_short_moves_is_read_once_for_all_of_them`)

#### Scenario: Read once, or again for each move
- **WHEN** a chain of up to 90 pairs (the same line, a similar one, one alike and not similar, a blank one, one at another indent, another line) is read once for all its moves, and again for each of them
- **THEN** the moves are the same, and so are the pairs compared by word and left unread
  [@test] crates/text-diff/src/moves_test.rs (`the_moves_of_a_chain_are_the_same_read_once_or_again_for_each`)

#### Scenario: A sorted list of lines that are alike
- **WHEN** 2,000 lines that are alike but for a number stand in another order, no two in a row on both sides
- **THEN** every line is found as the move of one line it is, the chains having grown through 16,384 pairs they left unread and no pair compared by word, the search within a quarter of its limit
- **AND** without that limit the chains use up the work of the search and lines are not found
  [@test] crates/text-diff/src/moves_test.rs (`a_sorted_list_of_lines_that_are_alike_is_searched_within_the_pairs_left_unread`)

#### Scenario: Over a limit
- **WHEN** a side has more lines a move may take than the limit, more pairs are similar than are compared, or a line is longer than the comparison reads
- **THEN** the side has no moves, the pairs past the number are no edited lines, and the long line ends its move
- **AND** a pair of lines in a row that is no move whatever the pair is, an edited line at the end of its row, is not compared at all
  [@test] crates/text-diff/src/moves_test.rs (`a_side_over_its_lines_and_pairs_over_their_number_are_not_read`)

### Requirement: Subject: A name another file of the subject declares is a rename where it is used

A name that changed at every place of a file that declares none of them MUST be a rename of that file once another
file of the subject declares a name that is not local and renamed it so (`subject.used`). The lines that changed in
such a name alone MUST be runs of the kind `renamed`, labels, beside the runs of the pair; a line that leans on a
name no file declares, or stands in a quiet run of the pair, is in none. The pass MUST add labels alone: the quiet
runs and the starts stay the pair's.

#### Scenario: The calls of a renamed function
- **WHEN** a store renames its function and a report calls it at two places
- **THEN** the report names the rename with its two places, the store as the file that declares it and the line of the declaration, each call is a run of the kind `renamed` that is a label, and the starts of the report are those of its pair
- **AND** the renames of the marks of the report are still those it declares: none
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_another_file_declares_is_a_rename_of_the_file_that_uses_it`)

#### Scenario: A name no file declares
- **WHEN** the report is the one file of its subject, or one of its lines changed in two names of which no file declares the second
- **THEN** the report alone has the marks of its pair, and the line of the two names is in no run and is read, beside the run of the line of the declared name alone
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_no_file_of_the_subject_declares_and_a_line_that_needs_one_have_no_run`)

#### Scenario: A line of such a run is in no move
- **WHEN** two lines of a report changed in the renamed name alone and another file got lines that read as they did
- **THEN** without the file that declares the name the two lines moved to the other file, and with it they are a run of the rename and no move
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_run_the_subject_adds_takes_its_lines_from_the_moves_between_files`)

#### Scenario: Labels alone
- **WHEN** a call is wrapped otherwise at the end of its block, before a line that changed in a name alone, whether the store renamed that name or no file declares it; and in every report made of such statements at random
- **THEN** the wrapped lines are no run, as in the pair: with the name read as one they are a run of formatting, which is quiet and leans on a name it does not hold
- **AND** the quiet runs, the starts, the moves and the renames of the file are those of its pair, every run of the pair lies in a run of its kind, and every run the subject added is of the kind `renamed` and no quiet one
- **AND** a line of the renamed name at the end of its block is a run only when the name after the block is one a file of the subject renamed too
  [@test] crates/file-diffs/src/marks_subject_test.rs (`the_subject_adds_labels_alone_never_a_quiet_run_or_a_stop`)

#### Scenario: A quiet run of the pair
- **WHEN** a file calls a function another file renamed on the line before the two lines of a renamed local, which are a quiet run of its pair
- **THEN** the run the three lines are with the name of the call read as one is none: the two lines stay the quiet run they are, and the line of the call is read, the first of its block
- **AND** the file names the rename of the call with its place
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_run_the_subject_adds_takes_no_line_of_a_quiet_run_of_the_pair`)

### Requirement: Subject: A rename is proven by a complete answer whose every use the subject explains

A declaration MUST be proven renamed (`confirmed`) only when the index answers one declaration of Kotlin or Java,
complete, with no name that means something else, and every use it lists is the declaration, a place of the rename
with its twin listed too, a link of a doc comment, a use without a word or by another word both trees hold alike,
or on a changed line in no run. A line a move took as it is MUST count as neither. The runs whose every place is
proven MUST be named (`subject.confirmed`).

#### Scenario: Every use is a place of the rename
- **WHEN** the index lists the declaration and the two calls of the report as the uses of the function in both trees, complete and without a conflict
- **THEN** the name of the report and the declaration of the store are `confirmed`, the two runs of the report are named as confirmed and are quiet, the report has no start left, and the block of the declaration keeps its start
- **AND** while the index is asked, and without an answer, the same runs are labels and every block is a stop
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_rename_every_use_of_which_the_index_lists_is_confirmed_and_its_declaration_stays_a_stop`)

#### Scenario: A use on a line that is read anyway
- **WHEN** the change also deletes a call of the function in a third file and adds one there, each on lines of its own, and a doc comment outside the subject names the function
- **THEN** the rename is confirmed all the same; the same use on a line the change left alone is a use outside it
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_on_a_line_that_is_read_anyway_and_a_link_of_a_doc_comment_do_not_stand_in_the_way`)

#### Scenario: A use on a line a move took as it is
- **WHEN** a function that calls the renamed function only moved, within its file or to another file, and a new function of the old name answers its call now, as accio answers of such a change: the call listed in the old tree, none in the new one, complete and without a conflict
- **THEN** the rename is `usesOutside`: a view shows the line as the same line at another place, and the old name stands on it
- **AND** the new name on a line that only moved is `otherMeaning`; a place of the rename whose old line a move took as it is, and that stands again at the other end of the move, is no place; a line a move edited in its whitespace alone is the line it was
- **AND** a call renamed on a line of a move is read where it stands now, a changed line, and the rename is confirmed
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_on_a_line_a_move_took_as_it_is_is_not_read_anyway`)

#### Scenario: A use on a line of a pale block
- **WHEN** a third file joins a `return` and the call of the old name on the line after it, read under the policy that counts no whitespace and under the one that counts every character
- **THEN** under the first the block is pale, the lines it was, and the rename is `usesOutside`; under the second the two lines are a change no run takes, and the rename is confirmed
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_on_a_line_of_a_pale_block_is_not_read_anyway`)

#### Scenario: A use without a word, a use by another word
- **WHEN** the index lists a positional argument that sets a renamed property, on a line the change left alone, as it answers of such a change; and a call by the alias of an import in both trees, on a line the change left alone, in a file of the subject one line further down or in a file outside it
- **THEN** the rename is confirmed all the same: neither writes the name
- **AND** a use by another word that only one tree lists, or that the other tree lists on another line or by another word, leaves the rename `incomplete`, never a verdict of what it changed
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_that_writes_no_word_or_the_same_other_word_in_both_trees_does_not_stand_in_the_way`)

#### Scenario: The accio of this machine
- **WHEN** this machine has an accio that answers `renames`, and a repository renames a function with its two calls in another file
- **THEN** the diffs of the commit, with an index of the repository built for the test, confirm the two calls and the declaration
  [@test] crates/file-diffs/src/accio_test.rs (`the_accio_of_this_machine_proves_the_rename_of_a_real_repository`)

## ADDED Requirements

### Requirement: Measurement: A subject is measured as a view reads it

A measurement (`FileDiffs::measure`) MUST read a subject through the stream a view reads and give each file as the
view gets it, with the time to the first content and to the end, the texts of both sides, whether its tokens were
read, which runs are proven whatever their kind, the starts once every proven run is passed, and what of the marks
read again from the texts differs. Diffs made without marks (`FileDiffs::without_marks`) MUST enter no store.

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

### Requirement: Measurement: `brd commits --mark-stats` counts the marks of real commits and prints a sample of each kind

`brd commits --mark-stats` (hidden) MUST measure each commit ahead of the baseline, each commit of `--range
<base>..<head>`, or that range whole (`--whole`), and print, in all and by grammar, the blocks, the stops, the stops
with every proven run passed, the runs of each kind, the moves, the renames by their proof and the times; with
`--mark-sample <N>` N marks of each kind with the lines of both sides, with `--no-marks` the subjects without marks.
It MUST write the object cache only.

#### Scenario: The counts and the sample
- **WHEN** `brd commits --mark-stats --mark-sample 2` runs on the `commit_marks` template
- **THEN** it prints one subject of Kotlin, the quiet kinds and the thresholds of the moves, fifteen blocks of three files and nine stops, the runs of each kind with those that are quiet and proven, the two moves by their size, the renames with what proves them, the move between two files and the times
- **AND** the sample holds two marks of a kind at most, each with its file, its lines of both sides and the lines around them: the wrapped call, a move with its edited line marked, a rename from its first place to its last, the lines that left for another file
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_mark_stats_of_a_commit_count_each_kind_and_print_a_sample_of_their_lines`)

#### Scenario: A range
- **WHEN** it runs with `--range origin/master..HEAD`, with `--whole` too, with an empty range, and with a range that is no two revisions
- **THEN** the commits of the range are the subjects, with the counts of the commits ahead of the baseline; the range whole is one subject named by its two ends; an empty range has none
- **AND** a range of one revision, or of one that is no commit, is refused as invalid arguments
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_subjects_are_the_commits_of_a_range_or_the_range_whole`)

#### Scenario: Without the marks
- **WHEN** it runs with `--no-marks`
- **THEN** every block is a stop, no kind has a run, the sample is empty, and a run with the marks after it counts nine stops again
  [@test] crates/bd-rewrite/src/mark_stats_test.rs (`the_mark_stats_without_the_marks_have_a_stop_at_each_block`)

#### Scenario: The processes
- **WHEN** it runs on three commits ahead of the baseline, on their range, and on the range whole
- **THEN** each commit costs one `diff-tree` and one `cat-file` beside the grouping of `brd commits`; a range costs its two ends and one walk in place of the grouping; the range whole one list and one read
  [@test] crates/bd-cli/src/process_budget_test.rs (`mark_stats_read_the_files_and_the_blobs_of_each_commit_once`)
