---
name: Range Reading
description: Requirements for what a range leaves to read, proven from the full marks of its diffs and summed by the role of each file, with the files no proof can be made of and the proofs that passed the rest; its verdict, the author's word of a label or a conventional type checked against it, and where the review window of a pull request and of a stack shows them.
style: plain-1
targets:
  - crates/domain-types/src/range_reading.rs
  - crates/review-plan/src/reading.rs
  - crates/file-diffs/src/range_files.rs
  - crates/domain-policy/src/range_reading.rs
  - crates/domain-policy/src/author_claim.rs
  - crates/app-api/src/reads/range_readings.rs
  - crates/app-api/src/reads/review_prefetch.rs
  - crates/object-cache/src/range_reading.rs
  - src/hooks/useRangeReadings.ts
  - src/presenters/range-reading.ts
  - src/components/review/ReviewHeader.tsx
  - src/components/review/PullRequestReview.tsx
---

# Range Reading

Status: Experimental
Date: 2026-11-13

## Purpose

A reviewer of many pull requests reads first what needs reading. What a pull request is cannot be told from its title
or its labels, which are the author's word; what it leaves to read can be proven from its diff. The marks of the diffs
pass formatting, renamed names, imports and moves file by file; a range sums those counts by the role of each file and
says why the rest needs no reading, so a refactoring says "Nothing to read" and a label its diff contradicts is
flagged, without a guess, a setting or an LLM.

## Goals

- One count: the sum of the badges of the files of the window, by role, from the marks of `file-diffs`.
- A verdict that is a fact (nothing to read, tests only, docs only, what is left), never a kind of change.
- The author's word shown as the author's, checked in the one case a diff proves.
- A stack read in one read, its layers with nothing to read folded and passed.

## Non-goals

- Guessing a feature or a fix; an LLM; a setting or a toggle.
- Writing anything to GitHub (a label, a comment).
- Asking the index in the background: the read ahead of the waiting requests of Your Turn leaves the renames across
  files to the review window of a request (Open Questions).

## Requirements

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

### Requirement: Reading: The proofs that passed a change are read from the marks

The reading MUST list each kind of proof that passed a change, once, in a fixed order, from the marks themselves:
formatting, comments and renamed names by the kind of a quiet run or one the index confirmed, imports by a block of the
imports alone with a start, moves by the moved lines of a file, moved files with no changed line, line endings alone.

#### Scenario: A range of formatting alone
- **WHEN** a range adds the spaces around an operator
- **THEN** its proofs are formatting alone
  [@test] crates/review-plan/src/reading_test.rs (`a_reading_sums_what_the_badge_of_each_file_counts_by_its_role_and_names_what_cannot_be_proven`)

### Requirement: Reading: The plan reads its pair with one listing more

The plan MUST read what its pair leaves to read from the diffs the window shows of it (`base..head`, a commit, the
scope of Since Your Review) before its texts, so the blobs of those diffs serve them: one `diff-tree` of the range
more, a range being never in the object cache, and no `cat-file` more; it asks the index of its renames after its own
queries.

#### Scenario: The processes of a plan with its reading
- **WHEN** `brd review-plan` plans a range, files of no language, build files, texts of the UI, or since the review
- **THEN** each runs one `diff-tree` more than before its reading, and as many `cat-file` or one fewer
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`, `a_review_plan_of_modified_files`, `a_review_plan_of_build_files`, `a_review_plan_of_ui_texts`, `a_review_plan_since_the_review`, `a_review_plan_since_the_review_on_a_moved_baseline`)

### Requirement: Reading: The layers of a stack are read in one process a phase

What the layers of a stack leave to read MUST be one read of them all, never a plan per layer: one `diff-tree --stdin`
of their file lists, one `cat-file --batch` of the blobs no cache has, then the index asked of the renames layer by
layer, the bottom first; a layer is told as its diffs say it, and once more with the answer of the index.

#### Scenario: Three layers
- **WHEN** the three layers of a stack are read: formatting, a body, a test
- **THEN** they leave nothing, one change of code and one of tests to read, and the read runs one `diff-tree` and one `cat-file`
  [@test] crates/review-plan/src/reading_test.rs (`the_layers_of_a_stack_are_read_in_one_process_a_phase`)
  [@test] crates/git-ops/src/commit_files_test.rs (`the_files_of_several_ranges_come_from_one_diff_tree`)

### Requirement: Reading: The read ahead asks no index

What the read ahead of the waiting requests reads MUST ask no index (a query of a tree far from the index costs tens of
seconds, JetBrains/accio#777): while the machine has an index, a rename across files waits for it, "· checking renames",
until a review window of the request asks the index itself; without one it is not proven, as the window says it.

#### Scenario: A rename across files
- **WHEN** the read ahead reads a range that renames a function and its use in another file
- **THEN** the rename waits for the index on a machine with one, and is not proven on one without; nothing is asked
  [@test] crates/review-plan/src/reading_test.rs (`the_read_ahead_leaves_a_rename_across_files_to_the_index_and_asks_none`)

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

### Requirement: Verdict: A narrow list and the sum of a stack

A narrow list (the layers of a stack in the column of its window) MUST say a verdict compactly, "Nothing to read", "4
to read", "Tests only · 1 to read", the whole verdict in its hover. The sum of a stack MUST count each thing of its own
unit: "1 of 7 with nothing to read · 14 changes to read", "Nothing to read" when no layer has anything.

#### Scenario: The compact words and the sum
- **WHEN** the verdicts of a list and the sums of stacks are worded
- **THEN** they read as above, "2 being read" while layers are read
  [@test] src/presenters/range-reading.test.ts (`says a verdict compactly for a narrow list`, `sums the layers of a stack, each count of its own unit`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`a_stack_counts_its_layers_and_opens_on_the_first_one_to_read_once_those_below_are_known`)

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
while changes outside the tests are, docs while changes outside the prose are, each in a file the reading proves. A
change in a file no proof can be made of (a rename that waits for the index or that it did not prove, no grammar, a
binary file) proves nothing: the summary names the file as not proven. Never a fix or a feature, which nothing in a
diff proves.

#### Scenario: The word against the reading
- **WHEN** a refactoring leaves code to read, or none; tests change code; docs change a test; a rename waits; a private member is renamed where no index answers; code is left in a proven file beside one not proven; tests change a script of no grammar
- **THEN** the first, the tests, the docs and the proven file are flagged with what is left outside their role in the files the reading proves; the others are not
  [@test] crates/domain-policy/src/author_claim_test.rs (`the_word_is_wrong_only_when_the_diff_leaves_changes_to_read_outside_its_role`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`the_view_of_a_range_flags_the_word_of_its_author_only_where_the_diff_proves_it_wrong`)

### Requirement: Window: The header says what the pull request leaves to read

The header of the review window of a pull request, and of a layer of a stack, MUST say what it leaves to read once its
fetch answered and the repository reads review plans, read by `get_range_readings`; its labels MUST stand as chips,
and the author's word the diff proves wrong is said once, as in a row of Your Turn: the label's own chip in the
warning tone, or a compact chip of the title after them, its hover why.

#### Scenario: Formatting alone
- **WHEN** a pull request changes the spaces between tokens alone, labelled refactoring
- **THEN** its header says "Nothing to read · formatting", the label shows, and no chip flags it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say a pull request of formatting alone leaves nothing to read, its label of a refactoring then holding`)

#### Scenario: A refactoring that changes a body
- **WHEN** a pull request labelled refactoring changes the body of a function
- **THEN** its header says "1 change to read in 1 file", the label chip "refactoring" in the warning tone with its hover, no other chip, and the header of its changes keeps the reading
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should flag a pull request labelled a refactoring whose diff changes the body of a function`)
  [@test] src/presenters/range-reading.test.ts (`words the author's word the diff proves wrong as the author's`)

### Requirement: `brd`: brd review-plan prints the reading of the pair

`brd review-plan` MUST print the `reading` of its pair, the same `RangeReading` the window reads, with `"schema": 36`.

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** the output of `brd review-plan` with its `reading` matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

### Requirement: Memory: A final reading is kept on disk

A final reading that is what it is whatever the index (no rename waits for one, none left unproven for want of one)
MUST be kept in memory and in the store of the repository (`object_cache::range_reading`) by the version of the
readings, its whitespace policy and its two commits, so the next start reads no diff of it; any other reading MUST
stay out of the store.

#### Scenario: The next start
- **WHEN** a final reading, one that waits for the index and one no index proved are kept, and the app starts again
- **THEN** the first is read from the store and kept in memory again; the others are not
  [@test] crates/app-api/src/reads/range_readings_test.rs (`a_final_reading_is_kept_on_disk_and_shows_in_the_next_process_at_once`)
  [@test] crates/object-cache/src/cache_test.rs (`range_readings_are_read_by_their_version_and_policy`)

## User Experience

- The header of a pull request, under its chips (and alone under the title while its changes show): `text-xs
  text-muted-foreground`, one line, "Nothing to read · formatting, renamed names", "Tests only · 3 changes to read in
  2 files", "12 changes to read in 5 files · 2 files not proven", "· checking renames" while the index is asked; its
  hover names the files not proven with why.
- The labels of the pull request as `outline` chips, muted, after the other chips; a label the diff proves wrong first
  of them as a `warning` chip, a word of the title as a `warning` chip after them, "refactor: in the title", each with
  the hover "The author calls it refactoring, but 3 changes of code in 2 files are left to read: no formatting,
  rename, import or move proves them".
- Review Stack: the overview of All Changes says "10 of 12 with nothing to read · 7 changes to read" under the meta of
  the stack; each layer its reading after its commits, a layer with nothing to read muted with an `outline` `xs`
  Approve after it. The column has each layer's compact reading after its title ("Nothing to read", `success`; "4 to
  read", `outline`).

## Testing / Local Run

- `cargo nextest run -p review-plan -E 'test(/reading_test/)'`, `-p domain-policy -E 'test(/range_reading|author_claim/)'`,
  `-p git-ops -E 'test(/several_ranges/)'`.
- `pnpm test:unit src/presenters/range-reading.test.ts`.
- `pnpm e2e tests/e2e/review-requests/review-requests.spec.ts -g "What a range leaves to read"`, on the
  `stacked_reading` template (four branches over `auth.js`, two of them formatting alone), the review plan turned on
  in the repository (`turnOnReviewPlan`).

## Measurements

- 2026-11-13, the IntelliJ monorepo (idea-1), 20 waiting pull requests: one batched `layer_readings` without the index
  took about 1 s wall, 0.5 s CPU and 2 git processes warm, 2 s wall and 5 s CPU cold; one call per pull request cost 4 to
  5 times more, 20 to 41 processes. With the index, `accio renames` of a tree far from the index took 20 to 28 s and about
  140 s CPU a call, and accio's cache keeps only the last pair of trees, so alternating heads were cold each time
  (JetBrains/accio#777): the read ahead asks no index.

## Open Questions / Risks

- The reading is under the default whitespace policy, as the plan reads its pair: a window on another policy may show
  badges whose sum is not the reading.
- A file whose parse failed is counted as its marks say (its runs unproven, its changes to read) but not named "not
  proven": the marks do not say that a parse failed.
- The window, its plan and the read of the layers may each ask the index of the same renames while none has answered
  yet; the memory of the answers serves every later one. The plan asks last to make that rare.
- The window opens a stack on its first layer to read even when that is the bottom; whether a stack with nothing
  folded should open on All Changes is to be seen with real chains.
- The read ahead of the waiting requests asks no index (JetBrains/accio#777, fixed in accio, not worked around here): a
  rename across files says "checking renames" in its row until a review window of the request asks the index, and the
  next listing takes that window's reading from memory. Once a query of a tree far from the index is cheap, the
  background read asks the index too, below the app.
- A reading kept on disk names a file not in the clone (a partial clone) as not proven, though a later fetch may bring
  it; the version of the readings or the store being cleared reads it again.

## References

- [Quiet Changes](../../commit/quiet-changes/spec.md): `DiffReading`, the count of a file.
- [Review Requests](../../repository/review-requests/spec.md#requirement-review-window-review-stack-reviews-a-stack-in-one-window): Review Stack.
- [Review Plan](../../branch/review-plan/spec.md): `ReviewPlan.reading`, `brd review-plan`.
- [Change Review](../../../../docs/design/change-review.md), decision 28.
