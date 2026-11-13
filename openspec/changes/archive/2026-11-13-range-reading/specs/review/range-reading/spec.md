## ADDED Requirements

### Requirement: Reading: A reading sums the badges of the files by their role

The reading of a range MUST sum, for every changed file, its blocks of changed lines and those with a start past the
blocks of the imports alone (`DiffReading::stops(true)`, the badge of the file in a review with its plan), by the role
of the file by its path (code, tests, prose, build, resources), from the full marks of the diffs under the default
whitespace policy: never a second model of the diff.

#### Scenario: A reading sums the badges of the files
- **WHEN** a range reformats a Kotlin file, and another changes a body, a test, a guide, a text without a grammar and a binary file
- **THEN** the first leaves nothing to read of its one change of code, the proof formatting
- **AND** the second leaves one change of each role to read, the binary file and the text without a grammar not proven, and its sum is the sum of the badges of the window
  [@test] crates/review-plan/src/reading_test.rs (`a_reading_sums_what_the_badge_of_each_file_counts_by_its_role_and_names_what_cannot_be_proven`)

### Requirement: Reading: A file no proof can be made of is named with why

The reading MUST name each file no proof can be made of, with why: binary, too large, a submodule, not in the clone, a
file of code or resources with a change to read and no grammar, a rename that is not local which the index did not
prove; a rename that waits for the index MUST be named as waiting, and the plan sent before the index answered is
then not final.

#### Scenario: What cannot be proven
- **WHEN** a range changes a binary file and a text without a grammar
- **THEN** both are named, as binary and as having no grammar
  [@test] crates/review-plan/src/reading_test.rs (`a_reading_sums_what_the_badge_of_each_file_counts_by_its_role_and_names_what_cannot_be_proven`)

### Requirement: Reading: The proofs that passed a change are read from the marks

The reading MUST list each kind of proof that passed a change, once, in a fixed order, from the marks themselves:
formatting, comments and renamed names by the kind of a quiet run or one the index confirmed, imports by a block of the
imports alone with a start, moves by the moved lines of a file, moved files with no changed line, line endings alone.

#### Scenario: A range of formatting alone
- **WHEN** a range adds the spaces around an operator
- **THEN** its proofs are formatting alone
  [@test] crates/review-plan/src/reading_test.rs (`a_reading_sums_what_the_badge_of_each_file_counts_by_its_role_and_names_what_cannot_be_proven`)

### Requirement: Reading: The layers of a stack are read in one process a phase

What the layers of a stack leave to read MUST be one read of them all, never a plan per layer: one `diff-tree --stdin`
of their file lists, one `cat-file --batch` of the blobs no cache has, then the index asked of the renames layer by
layer, the bottom first; a layer is told as its diffs say it, and once more with the answer of the index.

#### Scenario: Three layers
- **WHEN** the three layers of a stack are read: formatting, a body, a test
- **THEN** they leave nothing, one change of code and one of tests to read, and the read runs one `diff-tree` and one `cat-file`
  [@test] crates/review-plan/src/reading_test.rs (`the_layers_of_a_stack_are_read_in_one_process_a_phase`)
  [@test] crates/git-ops/src/commit_files_test.rs (`the_files_of_several_ranges_come_from_one_diff_tree`)

### Requirement: Verdict: Nothing to read only when nothing is left and every file is proven

The verdict of a reading (`domain_policy::range_reading::reading_summary`) MUST be nothing to read, with its proofs,
only when no change is left to read and no file cannot be proven, a rename waiting for the index among them; else tests
only or docs only when every file left, to read or not proven, is of tests or of prose; else what is left.

#### Scenario: The verdicts
- **WHEN** every change is passed; a binary file changed; only tests are left; only prose; code and tests; a file of code not proven beside tests; a rename waits for the index
- **THEN** the verdicts are nothing to read with the proofs; what is left with one file not proven; tests only; docs only; what is left; what is left; what is left, waiting
  [@test] crates/domain-policy/src/range_reading_test.rs (`a_reading_is_nothing_to_read_only_when_no_change_is_left_and_every_file_is_proven`)

### Requirement: Verdict: The page words the verdict

The page MUST word the verdict Rust decided and decide nothing of it: "Nothing to read · formatting, renamed names",
"Tests only · 3 changes to read in 2 files", "Docs only · …", "12 changes to read in 5 files", then "· 2 files not
proven" and "· checking renames" where they hold; the hover names the files not proven with why.

#### Scenario: The words
- **WHEN** the verdicts are worded
- **THEN** they read as above, and the hover "Not proven: icon.png (binary), build.sh (no grammar)"
  [@test] src/presenters/range-reading.test.ts (`says the verdict of a reading on one line, with what cannot be proven and what waits for the index`, `names the files that cannot be proven with why, for a hover`)

### Requirement: Author's word: A label or a conventional type of a closed list

The author's word (`domain_policy::author_claim::claim_of`) MUST be read only from a closed list: a label refactor,
refactoring, cleanup, test, tests, docs or documentation, case-insensitive, else the conventional type `refactor`,
`test`, `tests` or `docs` the title starts with (`type(scope)!: `); any other type, a word before a colon and the words
after the type ("; unchanged") say nothing.

#### Scenario: The words of the author
- **WHEN** a request has such a label, a label and a type, such a type, a fix, a feature, `proxy:`, "; unchanged" or nothing
- **THEN** the label is the word, then the type; the others say nothing
  [@test] crates/domain-policy/src/author_claim_test.rs (`the_word_of_the_author_is_a_label_or_a_conventional_type_of_a_closed_list`)

### Requirement: Author's word: The diff proves the word wrong in one case

The word MUST be flagged only when the reading proves it wrong: refactor while changes of code are left to read, tests
while changes outside the tests are, docs while changes outside the prose are; never while a rename waits for the
index, and never a fix or a feature, which nothing in a diff proves.

#### Scenario: The word against the reading
- **WHEN** a refactoring leaves code to read, or none; tests change code; docs change a test; a rename waits
- **THEN** the first, the tests and the docs are flagged with what is left outside their role; the others are not
  [@test] crates/domain-policy/src/author_claim_test.rs (`the_word_is_wrong_only_when_the_diff_leaves_changes_to_read_outside_its_role`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`the_view_of_a_range_flags_the_word_of_its_author_only_where_the_diff_proves_it_wrong`)

### Requirement: Window: The header says what the pull request leaves to read

The header of the review window of a pull request, and of a layer of a stack, MUST say what it leaves to read once its
fetch answered and the repository reads review plans, read by `get_range_readings`; its labels MUST stand as chips, and
a word the diff proves wrong as a warning chip after them, "Labelled refactoring · 1 change to read", its hover why.

#### Scenario: Formatting alone
- **WHEN** a pull request changes the spaces between tokens alone, labelled refactoring
- **THEN** its header says "Nothing to read · formatting", the label shows, and no chip flags it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say a pull request of formatting alone leaves nothing to read, its label of a refactoring then holding`)

#### Scenario: A refactoring that changes a body
- **WHEN** a pull request labelled refactoring changes the body of a function
- **THEN** its header says "1 change to read in 1 file", the chip "Labelled refactoring · 1 change to read" with its hover, and the header of its changes keeps the reading
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should flag a pull request labelled a refactoring whose diff changes the body of a function`)
  [@test] src/presenters/range-reading.test.ts (`words the author's word the diff proves wrong as the author's`)

### Requirement: `brd`: brd review-plan prints the reading of the pair

`brd review-plan` MUST print the `reading` of its pair, the same `RangeReading` the window reads, with `"schema": 35`.

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** the output of `brd review-plan` with its `reading` matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
