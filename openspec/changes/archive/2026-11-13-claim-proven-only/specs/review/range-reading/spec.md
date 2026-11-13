## MODIFIED Requirements

### Requirement: Reading: A file no proof can be made of is named with why

The reading MUST name each file no proof can be made of, with why: binary, too large, a submodule, not in the clone, a
file of code or resources with a change to read and no grammar, a rename that is not local the index did not prove,
or one that waits for it (the plan is then not final). The changes to read of the proven files MUST be summed apart by
role (`toReadProven`): a change of a file not proven is counted and proves nothing.

#### Scenario: What cannot be proven
- **WHEN** a range changes a binary file and a text without a grammar
- **THEN** both are named, as binary and as having no grammar, and the change of the text is not among the proven changes of its role
  [@test] crates/review-plan/src/reading_test.rs (`a_reading_sums_what_the_badge_of_each_file_counts_by_its_role_and_names_what_cannot_be_proven`)

#### Scenario: A private member renamed where no index answers
- **WHEN** a range renames a private member of a reformatted Kotlin file, and no index answers for it
- **THEN** the file is named as not proven by its renames, its changes to read are counted, and none of them is a proven change of its role
  [@test] crates/review-plan/src/reading_test.rs (`the_changes_of_a_file_no_proof_is_made_of_are_summed_apart`)

### Requirement: Author's word: The diff proves the word wrong in one case

The word MUST be flagged only when the reading proves it wrong: refactor while changes of code are left to read, tests
while changes outside the tests are, docs while changes outside the prose are, each in a file the reading proves. A
change in a file no proof can be made of (a rename that waits for the index or that it did not prove, no grammar, a
binary file) proves nothing: the summary names the file as not proven. Never a fix or a feature, which nothing in a
diff proves.

#### Scenario: The word against the reading
- **WHEN** a refactoring leaves code to read, or none; tests change code; docs change a test; a rename waits; a private member is renamed where no index answers; code is left in a proven file beside one not proven; tests change a script of no grammar
- **THEN** the first, the tests, the docs and the proven file are flagged with what is left outside their role in the files the reading proves; the others are not
  [@test] crates/domain-policy/src/author_claim_test.rs (`the_word_is_wrong_only_when_the_diff_leaves_changes_to_read_outside_its_role`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`the_view_of_a_range_flags_the_word_of_its_author_only_where_the_diff_proves_it_wrong`)

### Requirement: `brd`: brd review-plan prints the reading of the pair

`brd review-plan` MUST print the `reading` of its pair, the same `RangeReading` the window reads, with `"schema": 36`.

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** the output of `brd review-plan` with its `reading` matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
