## ADDED Requirements

### Requirement: Moves: Lines deleted in one block and added in another are a move

Rows of lines that one block of a diff deletes and another block adds MUST be a move (`moves`), found from the lines
of the two texts alone: the same lines by their key, the text of a line without the spaces and tabs at its ends, the
whitespace inside it kept; a blank line pairs with any blank line. A move MUST count only when its exact lines hold
24 characters that are no whitespace, and its two ends MUST lie in two blocks. The empty last line of a text MUST be
no line of a move.

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

### Requirement: Moves: A move says the columns its lines were indented by

`indent` of a move MUST be the columns of its shift, negative for lost ones: the same whitespace added to the indent
of each of its lines, or taken from it, at one end of the indent. An indent is a text, not a number of columns.
Where the indentation of the file has a meaning and the policy counts the whitespace at the ends of a line, a line
whose indent did not move by the shift MUST be an edited line of the quarter of its move; elsewhere `indent` is the
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
- **AND** a line at another indent than the others is an edited line whose first range is its indent, before the ranges of its text; more than a quarter of such lines, or one at an end, are no lines of the move
- **AND** a line that stayed at its indent while the others shifted, at the first column or at an indent of its own, is an edited line without a range, never one with a range that is empty or the same text on both sides; two such lines of six end the move
- **AND** a line of a recipe whose tab became a space is an edited line whose range is its indent; when every line of the recipe did, only its target moved
- **AND** two lines that gained two columns, one of spaces and one of tabs, did not move alike: the second is an edited line
- **AND** under a policy that leaves the whitespace at the ends of a line out, no indent is a change of a move
  [@test] crates/text-diff/src/moves_test.rs (`where_the_indentation_has_a_meaning_a_line_at_another_indent_is_edited`)

### Requirement: Moves: A move holds at most a quarter of edited lines between exact ones

A move MUST hold edited lines only between exact ones, at most a quarter of its lines that are not blank: a pair of
lines that is similar by word, the rule of a row of the Inline view. `edited` MUST name the edited lines of a move by
their place in it; `oldRanges` and `newRanges` MUST give what changed in each as the ranges of a block, each a text
that is another on the other side. A move none of whose lines has a range has empty lists.

#### Scenario: An edited line
- **WHEN** a function moves with an argument added to one of its five lines, a word changed, or a word removed
- **THEN** it is one move, and the line has its ranges, typed as those of a block, in the offsets of its own line where the lines were indented too
- **AND** two edited lines of five, an edited line at an end, and a line that is another line are no lines of the move: the exact lines beside them are asked alone
  [@test] crates/text-diff/src/moves_test.rs (`a_move_holds_at_most_a_quarter_of_edited_lines_between_exact_ones`)

### Requirement: Moves: The longest move is taken first, and a line is in one move

A line of a side MUST be in one move at most. Of the moves the lines allow, the longest MUST be taken first, then the
one with fewer edited lines, then the first by its new and its old lines; what is left of a move that shares lines
with a taken one is asked again. The moves MUST be a function of the two texts, the blocks, the runs, the whitespace
policy of the diff and whether the indentation of the file has a meaning, in the order of their new lines.

#### Scenario: Two places
- **WHEN** deleted lines stand at two places of the new text, or lines of two old places at one
- **THEN** the first place takes them, and the other lines are added or deleted lines
- **AND** a place with all the lines is taken before one with a part of them, and one without an edited line before one with it
  [@test] crates/text-diff/src/moves_test.rs (`of_two_places_the_longest_move_is_taken_then_the_first`)

#### Scenario: What is left of a longer move
- **WHEN** three functions leave one block, the first two for one place and the last two for another
- **THEN** the first place takes its two functions, and the other place the one that is left
  [@test] crates/text-diff/src/moves_test.rs (`a_line_is_in_one_move_and_the_rest_of_a_longer_one_is_asked_again`)

### Requirement: Moves: A move counts the blank lines that moved with it

Blank lines beside the old lines of a move, in their block, and as many beside its new lines at its other end, in
their block, MUST be counted by the move (`blanksBefore`, `blanksAfter`) and be no entry of `moves`: the comparison
deletes a function with the blank line before it and adds it with the one after it. A blank line that none of the
other block pairs with, or one of another text where the policy counts its whitespace, MUST stay a deleted or an
added line.

#### Scenario: The blank line on the other side
- **WHEN** a function is deleted with the blank line before it and added with the blank line after it, or the other way round
- **THEN** the move of the function counts the blank line, before its old lines and after its new ones (`blanksBefore`) or the other way round (`blanksAfter`), and the block the lines left has no start
- **AND** on the same side of both it is a line of the move
- **AND** of two deleted blank lines and one added, one moved and the other is a deleted line with a start; a line between the blank line and the move keeps it where it is
- **AND** a blank line of spaces deleted where an empty one is added is a deleted line with a start where every character counts, and moved with the function under a policy that leaves its whitespace out
  [@test] crates/text-diff/src/moves_test.rs (`the_blank_lines_that_moved_with_a_move_are_counted_beside_it`)

### Requirement: Moves: A move is never quiet and needs no grammar

A move MUST never be quiet: its lines are in no run and in no block that is pale or cut off, so the block its new
lines stand in always has a start. Every pair of texts MUST have its moves, with its content, whatever its grammar;
whether the indentation of a path has a meaning is part of what the diff is kept by.

#### Scenario: The lines of a run
- **WHEN** a run holds lines of a block, quiet or not
- **THEN** a move takes only the lines beside it
- **AND** the lines of a block that is pale under its policy, or that the policy cut off, are in no move
  [@test] crates/text-diff/src/moves_test.rs (`the_lines_of_a_run_and_of_a_block_that_is_no_change_are_in_no_move`)

#### Scenario: With the content, whatever the grammar
- **WHEN** the diffs of a commit stream, a rule moved in a Python file, a text file and a Starlark file of the same two blobs
- **THEN** the content of each file has its move and its starts, from the comparison, from memory, from the store of another app and for a scan
- **AND** the line of the rule that stayed at its indent while the others shifted is an edited line in the Python file and the Starlark file alone
  [@test] crates/file-diffs/src/service_test.rs (`the_moves_of_a_text_come_with_its_content_and_are_those_of_its_indentation`)

#### Scenario: Files edited at random
- **WHEN** files of every grammar are edited at random and some of their lines stand at another place
- **THEN** no line of a move is in a run or in another move, and the block of its new lines has a start
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`)

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
- **AND** 16 lines of one key are a move, 17 are none
  [@test] crates/text-diff/src/moves_test.rs (`a_key_that_many_lines_have_starts_no_chain_and_is_a_line_of_one`)

#### Scenario: The work of a search
- **WHEN** 10,000 lines of five keys move, among them fifty lines that each stand sixteen times
- **THEN** the lines are one move, found with fewer than twenty pairs of lines read a line, within the limit, the same when asked again
- **AND** a search that reaches its limit ends with the moves found until then, and so the choice among them
  [@test] crates/text-diff/src/moves_test.rs (`the_work_of_a_search_grows_with_its_lines_and_ends_at_its_limit`)

#### Scenario: The file a search costs most in
- **WHEN** every line of a file stands sixteen times on each side and no two lines follow each other on both, the lines nothing alike or alike but for a number
- **THEN** the search ends at its limit of pairs read (a pair of two lines read for their likeness counts by their length too), with moves that are moves, the same when asked again
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

## MODIFIED Requirements

### Requirement: Starts: The reading of a block starts at its first line outside its quiet runs

`starts` MUST name the line the reading of each block starts at, as `[block, side, line]`, in the order of the
blocks: the quiet runs cut a block into parts, and the reading starts at the first part with a line to read, at its
first new line (side 1), at its first old line that no move took (side 0) when it has no new one. A block the policy
cut off and a block without a line to read MUST have no entry. `starts` MUST be the one source of where a reader
stops.

#### Scenario: A diff without runs
- **WHEN** two texts are compared and nothing is marked
- **THEN** every block that counts has its start: its first new line, the first old line of deleted lines, a pale block too
- **AND** a block `IgnoreBlankLines` cut off has none
  [@test] crates/text-diff/src/model_test.rs (`a_diff_without_runs_starts_the_reading_at_each_block_that_counts`, `a_diff_starts_the_reading_at_each_block_that_counts`)

#### Scenario: Quiet runs in a block
- **WHEN** a block has quiet runs
- **THEN** its reading starts after a run at its start, at an old line that stands before the run of all its new lines, and nowhere when the runs leave no line
- **AND** a run that is a label moves no start
  [@test] crates/text-diff/src/model_test.rs (`the_reading_of_a_block_starts_after_its_quiet_runs`)

#### Scenario: The lines a move took
- **WHEN** a move took old lines of a block
- **THEN** the reading of the block starts at its first old line that no move took, at its new lines when it has some, and nowhere when the moves, or the moves and the quiet runs, leave no line
- **AND** the blank lines that left with a move are nothing to read either
- **AND** the block the lines stand in now starts at its first new line
  [@test] crates/text-diff/src/model_test.rs (`the_old_lines_of_a_move_are_nothing_to_read`)

#### Scenario: Any blocks and runs
- **WHEN** any blocks have any runs in the order of both sides, and moves took any of their old lines
- **THEN** a block has an entry exactly when it is not cut off and a line of it is to read, a new line in no quiet run or an old line in no quiet run that no move took, and the entry names the first such line of the first part that has one
  [@test] crates/text-diff/src/model_test.rs (`starts_agree_with_the_runs`)

### Requirement: Runs: A block is cut at the line boundaries both sides share

A block of several changes MUST be cut where both sides have a line boundary, crossed by no token, with the same
tokens of the code before it. Between the cuts, the longest rows of lines whose tokens are the same are runs of
formatting, the blank lines inside them with them; lines between such rows that differ in their comments alone are a
run of comments. A run MUST have lines on both sides and lie inside one block.

#### Scenario: Lines wrapped into a block, a change beside lines indented again
- **WHEN** lines are wrapped into a new `if` and indented, or indented lines stand beside a line that really changed
- **THEN** the indented lines are a quiet run, and the reading of the block starts at the `if`, at the changed line, at a comment added between them, or at the old line removed between them
- **AND** a comment worded again among them is a run of comments of its own
- **AND** a comment that moved to a line of its own is in the run of formatting, and comments worded again around an added comment are one run of comments
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_is_cut_into_runs_at_the_line_boundaries_both_sides_share`)

#### Scenario: No boundary both sides share
- **WHEN** a wrapped call and a changed call stand on one line of a side
- **THEN** the block has no run
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_is_cut_into_runs_at_the_line_boundaries_both_sides_share`)

#### Scenario: The template of the E2E tests
- **WHEN** the commit of the `commit_marks` template is read
- **THEN** each of its cases is a block of its own: the wrapped call a quiet run, the comment a label, the renamed names, the real change and the lines that left for another file blocks with a start
- **AND** `accounts` is a move that counts the blank line that moved with it, and `describe` a move indented by two columns with one edited line: the blocks they stand in have a start, the blocks they left have none
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_template_has_each_case_in_a_block_of_its_own`)

### Requirement: Limits: The time of a comparison changes no mark

The marks of a diff MUST be a function of its blocks, its two texts, its whitespace policy and how its paths are read
(their grammar, the meaning of their indentation), the same on every machine and under every load. A diff the time cut short
(`lineLevelOnly` within the size of the inner ranges) MUST have the marks of its blocks: the runs read the blocks and
the tokens, the moves the blocks and the lines, never the inner ranges.

#### Scenario: A diff the time cut short
- **WHEN** the inner ranges of a diff run out of their time, or are not asked for
- **THEN** the diff is by lines alone and has the runs, the moves and the starts of the diff in time, from the comparison and from memory
- **AND** a diff whose lines ran out of time too is one block of both texts, with the marks of that block
  [@test] crates/file-diffs/src/marks_test.rs (`a_diff_the_time_cut_short_has_the_marks_of_its_blocks`)
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_a_diff_out_of_time_are_those_of_the_diff_in_time`, `the_moves_of_a_text_come_with_its_content_and_are_those_of_its_indentation`)
