---
name: Review Units
description: Requirements for the units of a review (experimental), the changes a reviewer reads once instead of the files; the pair of the review, its blocks cut by the declaration that holds them into a change of a form, of a body, a new or removed declaration, a new file, a group of mechanical changes or of a repeated edit over its files, and a change of no declaration; every block in one unit, the order of the story, and a fingerprint of each change that a rebase or an amend keeps.
style: plain-1
targets:
  - crates/review-plan/src/units.rs
  - crates/review-plan/src/pair.rs
  - crates/review-plan/src/whitespace.rs
  - crates/trial-check/src/declaration_checks.rs
  - crates/file-diffs/src/service.rs
  - crates/bd-rewrite/src/review_plan_command.rs
  - src/presenters/review-units.ts
  - src/lib/diff/rows.ts
  - src/components/diff/DiffView.tsx
  - src/components/commit/CommitReview.tsx
  - src/hooks/useReviewPlan.ts
  - src/pages/ReviewPage.tsx
  - src/components/review/PullRequestReview.tsx
---

# Review Units

Status: Experimental
Date: 2026-10-30

## Purpose

A review plan knows what each hunk of a branch does: which declaration it changes, whether the form of that
declaration changed or its body alone, which hunks only move imports or whitespace, which edit repeats over many
files. A review that lists files throws this away: a file with one real change and one change of imports is read as a
whole, a change of a body of a public function is no line of the plan, and an edit that 30 files repeat is read 30
times.

A unit is one change a reviewer reads once: a change of the form of a declaration, a change of its body, a new or a
removed declaration, a new file, all the changes of one mechanical kind over every file that has them, one repeated
edit over its files, and a change that no declaration holds. The plan cuts the diff the review shows into units
(`ReviewPlan::units`), every block in one, and puts them in the order of the story: a declaration before its uses, a
test right after its subject, the mechanical changes last. Each unit has a fingerprint of its change that stays the
same when a rebase or an amend keeps the change: what a later review knows it has read, and what a note to the agent
is anchored to. The vision of the review, its progress and the rounds of polish:
[Change Review](../../../../docs/design/change-review.md).

## Goals

- Cut every block of the reviewed diff into exactly one unit, and put every changed file in one unit at least: the
  units hide nothing and show nothing twice.
- Tell a change of a form from a change of a body, for every declaration, public or not.
- Read one repeated edit and one kind of mechanical change once, over all of its files.
- Name a change by its content, not by its place: the same change on another base or after an amend has the same
  fingerprint, and the whitespace policy of a window changes neither the identity nor the fingerprint.
- Cost no git process of its own: the units read the blocks, the summaries and the hunk text the plan reads already.

## Non-goals

- A move of a block between files as one unit: the units see a removal and an addition.
- The usages of a change of a body: accio answers the callers of the API alone, until a query of many names costs one
  process (JetBrains/accio#74).
- A change of a form that the summary does not see (`Form`): a Kotlin return type that is inferred changes with the
  body, and the unit is `behaviour`.

## Requirements

### Requirement: Pair: The units cut the pair the review shows

The units MUST cut the pair the review shows: per file the reviewed commits change, the blocks of its blob before the
first commit that changes it against its blob after the last one, under `Default`, as the diffs of the range show them.

#### Scenario: A file two commits change
- **GIVEN** a file that two of the reviewed commits change
- **WHEN** the units are cut
- **THEN** the file is cut by the blocks of its blob before the first commit that changes it against its blob after
  the last one, under `Default`, as the diffs of the range show them
  [@test] crates/review-plan/src/units_test.rs (`the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice`)

### Requirement: Pair: The blocks come from the commit's hunks or one batch of blobs

A file one commit changes MUST take the hunks of that commit; a file several commits change MUST take its first blob
against its last, with one `cat-file --batch` for the blobs of all such files that the memory of the diffs and the
`file_diff` store lack.

#### Scenario: A file one commit changes
- **GIVEN** a file that one of the reviewed commits changes
- **WHEN** the units are cut
- **THEN** the file takes the hunks of that commit

#### Scenario: Files several commits change
- **GIVEN** files that several of the reviewed commits change, some of whose blobs the memory of the diffs and the
  `file_diff` store lack
- **WHEN** the units are cut
- **THEN** each such file takes its first blob against its last
- **AND** the blobs they lack are read by one `cat-file --batch` for all such files

### Requirement: Pair: A file the same before and after the commits has no unit

A file whose blob and path are the same before and after the commits is no file of the pair: it MUST have no unit.

#### Scenario: A file changed and changed back
- **GIVEN** a file whose blob and path are the same before and after the commits
- **WHEN** the units are cut
- **THEN** it is no file of the pair and has no unit

### Requirement: Units: Every block is in exactly one unit

Every block of every file of the pair MUST be in exactly one unit, and every file of the pair in one at least.

#### Scenario: The changes of a file and the files of the pair
- **GIVEN** a file with a change of a form, of a body, a new and a removed declaration and a new import, a file of no
  language and a new file
- **WHEN** the units are cut
- **THEN** every block of every file of the pair is in exactly one unit
- **AND** every file of the pair is in one unit at least
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

### Requirement: Units: A block goes to the first unit it fits

A block MUST go to the repeated edit of its file's group when the block is that edit; else to the imports when its
changed lines are each of an import statement or blank, one at least; else to the whitespace when it changes
whitespace alone, in a file of a grammar its lines equal by tokens; else to the declaration that holds most of its
changed lines; else to the change outside any declaration, one unit per run of such blocks.

#### Scenario: A block that is the repeated edit
- **WHEN** a block is the repeated edit of its file's group
- **THEN** it goes to the repeated edit of that group
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`)

#### Scenario: A block of imports alone
- **WHEN** a block is no repeated edit and its changed lines are each of an import statement or blank, one at least
  (`file_diffs::service::imports_only`, the test the diffs pale a block by)
- **THEN** it goes to the imports
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A block of whitespace alone
- **WHEN** a block is neither of those and changes whitespace alone ([Review Plan](../review-plan/spec.md#requirement-mechanical-changes-whitespace-is-told-by-the-tokens-of-a-grammar)):
  its lines are equal by tokens, as those of a call wrapped inside its brackets
- **THEN** it goes to the whitespace
- **AND** a block that changes the spaces inside a string does not
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`, `a_block_of_whitespace_is_told_by_its_tokens`)

#### Scenario: A block inside declarations
- **WHEN** a block is none of those and declarations hold its changed lines
- **THEN** it goes to the declaration that holds most of its changed lines, the innermost one around each line
- **AND** the lines are read on the new side, on the old side for a block that only removes
  [@test] crates/review-plan/src/units_test.rs (`a_block_belongs_to_the_declaration_of_most_of_its_lines`)

#### Scenario: Blocks outside any declaration
- **WHEN** blocks are none of those and no declaration holds their changed lines
- **THEN** they go to the change outside any declaration, one unit per run of such blocks

### Requirement: Units: The declarations of a file are its spanned declarations

The declarations of a file MUST be those of its summary that are no `Binding`, none the parser may have misread
(`in_error`) and none inside a function or a value (a local): `spanned_declarations`. Their lines and those of the
import statements, which a summary counts as git does, MUST be read as lines of the diffs, which the blocks name: a
lone `\r` ends a line there too.

#### Scenario: A binding, a misread declaration and a local
- **GIVEN** a summary with a `Binding`, a declaration the parser may have misread (`in_error`) and a declaration inside
  a function or a value (a local)
- **WHEN** the declarations of the file are read (`spanned_declarations`)
- **THEN** none of the three is a declaration of the file

#### Scenario: A block below a lone carriage return
- **GIVEN** a file with a lone `\r` above its declarations
- **WHEN** a block changes the code of a declaration below it, and another wraps a call of a declaration
- **THEN** the first goes to its declaration, a change of its behaviour, and the second to the whitespace
  [@test] crates/review-plan/src/units_test.rs (`a_block_below_a_lone_carriage_return_goes_to_the_declaration_of_its_lines`)

### Requirement: Units: A file without a full summary is one unit other

A side whose summary is missing, degraded or truncated MUST make the file one unit `other` (`unparsed`).

#### Scenario: A summary missing, degraded or truncated
- **WHEN** the summary of a side of a file is missing, degraded or truncated
- **THEN** the file is one unit `other` (`unparsed`)

### Requirement: Units: Declarations are paired by key, form, then order

A declaration MUST be paired across the two sides by its key and, among overloads of one key, by an equal form, then
in order.

#### Scenario: Two overloads of one key
- **GIVEN** two overloads of one name, of which only one has a changed body
- **WHEN** the declarations are paired
- **THEN** each overload is paired with the one of an equal form
- **AND** only the one whose body changed is a unit, named by its signature
  [@test] crates/review-plan/src/units_test.rs (`an_overload_is_a_unit_of_its_own`)

### Requirement: Units: A pair is shape, behaviour or whitespace

A pair whose form differs MUST be `shape`, with the parts that changed; one whose form is the same and whose lines
differ by more than whitespace MUST be `behaviour`; one whose lines are equal by tokens too is whitespace, and its
blocks MUST go to the unit of whitespace. The lines of a pair MUST be those of the declaration and of every block of
its unit, from the first to the last on each side: a block may reach past the declaration that holds most of it.

#### Scenario: A change of a form and a change of a body
- **WHEN** the form of a paired declaration differs, and of another only its body changed
- **THEN** the first is `shape`, with the parts that changed
- **AND** the second is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A pair whose lines are equal by tokens
- **WHEN** the form of a paired declaration is the same and its lines are equal by tokens
- **THEN** it is whitespace, and its blocks go to the unit of whitespace

#### Scenario: A space inside a string of a body
- **WHEN** the body of a paired declaration differs only by the spaces inside a string
- **THEN** it is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`a_block_of_whitespace_is_told_by_its_tokens`)

#### Scenario: A block that reaches past its declaration
- **WHEN** a block joins the two lines of a declaration and changes the comment after it, and the lines of the declaration alone are equal by tokens
- **THEN** the declaration is `behaviour`: the lines of its block are not
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_is_whitespace_only_with_every_line_of_its_blocks`)

### Requirement: Units: A declaration of one side alone is added or removed

A declaration of one side alone whose lines a block changes MUST be `added` or `removed`, also when another unit holds
that block: then it has no block.

#### Scenario: A new and a removed declaration
- **WHEN** a block changes the lines of a declaration of the new side alone, and of one of the old side alone
- **THEN** the first is `added` and the second `removed`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: Another unit holds the block
- **WHEN** another unit holds the block that changes the lines of a declaration of one side alone
- **THEN** the declaration is still `added` or `removed`, and it has no block

### Requirement: Units: A declaration inside a new or removed one is part of its unit

A declaration of one side alone inside another of that side alone (a member of a new type, a method of a removed
class) MUST be part of the unit of the outermost one, and so MUST its blocks.

#### Scenario: A member of a new type
- **WHEN** the commits add a type with a member
- **THEN** the member and its blocks are part of the unit of the type, not a unit of its own
  [@test] crates/review-plan/src/units_test.rs (`a_member_of_a_new_type_is_part_of_the_type`)

### Requirement: Units: A mechanical file is one whole slice of the unit of its kind

A file whose change is mechanical (`files`: a build file, imports only, whitespace only, a pure rename, a generated
file) MUST be one slice of the unit of its kind, the file whole; a generated file the commits add or delete too.

#### Scenario: A mechanical file
- **WHEN** the change of a file is mechanical (`files`)
- **THEN** the file is one slice of the unit of its kind, the file whole
  [@test] crates/review-plan/src/units_test.rs (`the_generated_files_are_one_unit_after_the_other_mechanical_ones`)

#### Scenario: A generated file the commits add or delete
- **WHEN** the commits add or delete a generated file
- **THEN** it is one slice of the unit of the generated files, the file whole

### Requirement: Units: A new file and a deleted file are units of their own

A file the commits add MUST be one unit `newFile`, a file they delete one unit `deletedFile` in its place in the story.

#### Scenario: A new file
- **WHEN** the commits add a file
- **THEN** it is one unit `newFile`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A deleted file
- **WHEN** the commits delete a file
- **THEN** it is one unit `deletedFile` in its place in the story
  [@test] crates/review-plan/src/units_test.rs (`a_deleted_file_is_a_unit_and_the_deleted_files_of_a_directory_are_one`)
  [@test] src/presenters/review-units.test.ts (`shows a deleted file whole in its place, and the files deleted from one directory as a group, each collapsed`)

### Requirement: Units: The files deleted from one directory are one unit

3 or more files deleted from one directory MUST be one unit `deletedFiles` with the directory, a section per file,
each collapsed ("Deleted"), under "Deleted from old/a/ (3)".

#### Scenario: Three files deleted from one directory
- **WHEN** the commits delete 3 or more files from one directory, `old/a/`
- **THEN** they are one unit `deletedFiles` with the directory
- **AND** it shows a section per file, each collapsed ("Deleted"), under "Deleted from old/a/ (3)"
  [@test] crates/review-plan/src/units_test.rs (`a_deleted_file_is_a_unit_and_the_deleted_files_of_a_directory_are_one`)
  [@test] src/presenters/review-units.test.ts (`shows a deleted file whole in its place, and the files deleted from one directory as a group, each collapsed`)

### Requirement: Units: A file the review cannot cut is one unit other with the reason

A binary file, a file over 8 MiB, a change of its type, a change of the line ends alone where the indentation has a
meaning, and a file of no language the review parses MUST each be one unit `other` with the reason.

#### Scenario: A file of no language
- **WHEN** the commits change a file of no language the review parses
- **THEN** it is one unit `other` with the reason
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A binary file, a large file, a change of type or of the line ends
- **WHEN** the file is binary, over 8 MiB, changes its type, or changes only its line ends where the indentation has
  a meaning
- **THEN** it is one unit `other` with the reason

### Requirement: Units: A mechanical kind and a repeated edit are one unit each

One kind of mechanical change MUST be one unit over every file that has it, and one repeated edit one unit over its
files.

#### Scenario: A repeated edit and the whitespace over their files
- **GIVEN** the same edit of three files, and a file that only reindents one line
- **WHEN** the units are cut
- **THEN** the repeated edit is one unit over its three files
- **AND** the whitespace is one unit over the files that have it
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`)

#### Scenario: The generated files
- **WHEN** the commits change several generated files
- **THEN** they are one unit over all of them
  [@test] crates/review-plan/src/units_test.rs (`the_generated_files_are_one_unit_after_the_other_mechanical_ones`)

### Requirement: Order: The units follow the order of the files of the plan

The units MUST come in the order of the files of the plan (`order`: a declaration before its uses, a test right after
its subject), each unit at the first block it holds, a unit without a block right after the unit of the block that
changed its lines, the units without a block of one block in the order of their lines; a repeated edit at the place
of its first file.

#### Scenario: A declaration before its use
- **GIVEN** a file that declares a function and a file that uses it, both changed
- **WHEN** the units are put in order
- **THEN** the unit of the declaration comes before the unit of its use, each at the first block it holds
  [@test] crates/review-plan/src/units_test.rs (`the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice`)

#### Scenario: A unit without a block
- **WHEN** a unit has no block
- **THEN** it comes right after the unit of the block that changed its lines

#### Scenario: The units without a block of one block
- **WHEN** one block changes the lines of several declarations that hold none of it
- **THEN** they follow the unit of the block in the order the source declares them, not by their names, so the
  heading of the block names them so
  [@test] crates/review-plan/src/units_test.rs (`the_declarations_of_one_block_follow_in_the_order_of_their_lines`)

#### Scenario: A repeated edit
- **WHEN** a repeated edit spans several files
- **THEN** it comes at the place of its first file

### Requirement: Order: The mechanical units come last, by kind

The mechanical units MUST come last, by kind: build files, imports, whitespace, renames, the generated files last of
all (`mechanical:generated`), which a reviewer skips whole.

#### Scenario: The generated files after the other mechanical units
- **WHEN** the commits change generated files beside other files
- **THEN** the mechanical units come last, in the order build files, imports, whitespace, renames
- **AND** the generated files are last of all (`mechanical:generated`), which a reviewer skips whole
  [@test] crates/review-plan/src/units_test.rs (`the_generated_files_are_one_unit_after_the_other_mechanical_ones`)

### Requirement: API and Tests: A unit is api by the API of the plan

A unit of a declaration MUST be `api` exactly when it has its entry in the API of the plan, or would have it by a
change of its form, by the surface of its declaration on each side
([Review Plan](../review-plan/spec.md#api-surface)).

#### Scenario: An addition
- **WHEN** a unit is an addition
- **THEN** it is `api` when it is API after the commits
- **AND** an addition of unstable API is not
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)

#### Scenario: A removal, a change of a form or of a body
- **WHEN** a unit is a removal, a change of its form or of its body
- **THEN** it is `api` when it is API or unstable API on a side
- **AND** a private declaration is not
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)
  [@test] crates/review-plan/src/api_test.rs (`a_change_of_form_to_or_from_api_is_listed`)

#### Scenario: A promotion and the declarations that are no API
- **WHEN** a unit is a promotion, or of a test, a fixture of its class or module, an entry point or an override
- **THEN** a promotion is `api`
- **AND** a test, a fixture of its class or module, an entry point and an override are not
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)

### Requirement: API and Tests: A unit of no declaration is neither api nor test

A unit of no declaration MUST be neither `api` nor `test`.

#### Scenario: A unit of no declaration
- **WHEN** a unit is of no declaration
- **THEN** it is neither `api` nor `test`

### Requirement: API and Tests: A unit of test code is test

A unit of a declaration MUST be `test` when it is test code (a test, a type that declares one, a member of such a type
or module), after the commits, before them for a removal.

#### Scenario: A test and its fixture
- **WHEN** the commits change a test and a fixture of its class
- **THEN** both units are `test`
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)

#### Scenario: A removed test
- **WHEN** the commits remove a test
- **THEN** its unit is `test` by what it is before the commits

### Requirement: Identity: The id names a unit within a plan

`id` MUST name a unit within a plan: of a declaration its file, key and signature.

#### Scenario: The id of a declaration
- **WHEN** a unit is of a declaration
- **THEN** its `id` is its file, key and signature
- **AND** the same change of that declaration on another base has the same `id`
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_names_the_change_wherever_it_sits`)

### Requirement: Identity: The fingerprint names the change across plans

`fingerprint` MUST name the change across plans: of a declaration a digest of its key, its signature and the content
of its tokens on both sides ([Equal by Tokens](../../commit/equal-by-tokens/spec.md)); of the blocks of a slice
their file and the content of their tokens; of a file taken whole its path and blobs; of a group the set of the
fingerprints of its slices. A side without tokens MUST be read by its text without whitespace.

#### Scenario: The same change on another base, and another change
- **GIVEN** a change of the body of a declaration
- **WHEN** the same change sits on another base, below a line added before the declaration
- **THEN** its unit has the same `fingerprint`
- **AND** a different change of that body has another one
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_names_the_change_wherever_it_sits`)

#### Scenario: A slice, a file taken whole and a group
- **WHEN** a unit is of the blocks of a slice, of a file taken whole, or a group
- **THEN** the fingerprint of the blocks of a slice is of their file and the content of their tokens
- **AND** that of a file taken whole of its path and blobs
- **AND** that of a group of the set of the fingerprints of its slices

#### Scenario: The same change with other whitespace
- **WHEN** the same change of a body is written with other whitespace: wrapped, indented again
- **THEN** its unit has the same `fingerprint`
- **AND** so has the same change of a function that is the last of its file: the content is the texts of the tokens and the tree between them
- **AND** a change that differs by the spaces inside a string, or by a line break that ends a statement, has another one
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_is_the_same_after_a_change_of_whitespace_alone`)

### Requirement: Identity: A digest is the first 16 bytes of a SHA-256

A digest MUST be the first 16 bytes of a SHA-256, so that it is the same on every machine and in every version.

#### Scenario: The digest of a fingerprint
- **WHEN** a fingerprint is computed
- **THEN** its digest is the first 16 bytes of a SHA-256, the same on every machine and in every version

### Requirement: Identity: The whitespace policy changes neither the id nor the fingerprint

The kind of a unit SHALL be allowed to change with the whitespace policy of a window; its `id` and its `fingerprint`
MUST NOT change with it.

#### Scenario: Another whitespace policy of a window
- **WHEN** a window reads another whitespace policy
- **THEN** the kind of a unit may change
- **AND** its `id` and its `fingerprint` stay the same

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 32`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices` and its `fingerprint`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 32`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
  `line`, its `slices` and its `fingerprint`
- **AND** `kind` is `shape` with `changes`, `behaviour`, `added`, `removed`, `newFile`, `deletedFile`, `deletedFiles`
  with `directory`, `mechanical` with `mechanical`, `repeatedEdit` with `group`, or `other` with `reason`
- **AND** each of its `slices` has `path`, `old_path`, `whole`, the `blocks` by `index` with their `old` and `new` lines
  from 1, both included, `null` for a side with none, and a `fingerprint`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Requirement: In the Review Window: The diffs are the story of the units

With the plan, the diffs of All Changes and of Since Your Review MUST be the story of the units, in their order
(`presentNarrative`), by the steps of its story, each step's parts together
([Review Tour](../review-tour/spec.md#in-the-review-window), a step of the concerns as
[Review Concerns](../review-concerns/spec.md#in-the-review-window) lays it out).

#### Scenario: The diffs of a review with the plan
- **GIVEN** a review with the plan
- **WHEN** All Changes or Since Your Review shows its diffs
- **THEN** they are the story of the units, in their order (`presentNarrative`), by the steps of its story, each
  step's parts together
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

### Requirement: In the Review Window: A run of units of one file is one section

In the story, a run of units of one file MUST be one section, each change under its heading (none over a block of a
file of no language the review parses, whose header names it), a unit with no block of its own beside the heading
before it; a group MUST be a section per file of it.

#### Scenario: A run of changes of one file
- **WHEN** several units of one file follow each other in the story
- **THEN** they are one section, each change under its heading ("open — parameters changed", "close — body changed",
  "fresh — new", "Changes outside a declaration")
- **AND** a unit with no block of its own stands beside the heading before it
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`, `words a change by its kind`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

#### Scenario: A block of a file of no language
- **WHEN** a block is of a file of no language the review parses
- **THEN** no heading stands over it, and the header of the file names it

#### Scenario: A group
- **WHEN** a group of the story spans several files
- **THEN** it is a section per file of it
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`)

### Requirement: In the Review Window: A heading sets the name in the code font

A heading MUST set the name of its declaration in the code font and what changed in the font of the text, muted; a
heading of no declaration MUST be in the text colour.

#### Scenario: The heading of a declaration and of no declaration
- **WHEN** a section shows the heading of a change
- **THEN** the name of its declaration is in the code font and what changed in the font of the text, muted
- **AND** a heading of no declaration is in the text colour

### Requirement: In the Review Window: A mechanical group starts collapsed

A mechanical group MUST start collapsed on every file, its header saying the kind (of a generated file why it is
generated, [Review Plan](../review-plan/spec.md#in-the-review-window)).

#### Scenario: The group of the imports
- **WHEN** the story shows the group of the imports
- **THEN** it starts collapsed on every file, its header saying the kind
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

#### Scenario: The group of the generated files
- **WHEN** the story shows the group of the generated files
- **THEN** each file starts collapsed, its header saying why it is generated
  [@test] src/presenters/review-units.test.ts (`fold each with why it is generated: its name, its attributes, the line its tool wrote`)

### Requirement: In the Review Window: A repeated edit shows once on its first file

A repeated edit MUST show on its first file under "Same edit in N files", the other files collapsed, "Same edit as
Alpha.kt".

#### Scenario: The same edit in four files
- **GIVEN** the same edit in Alpha, Beta, Gamma and Delta
- **WHEN** the story shows the repeated edit
- **THEN** it shows on Alpha under "Same edit in N files"
- **AND** the other files are collapsed, "Same edit as Alpha.kt"
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`)

### Requirement: In the Review Window: A block another section shows is one row

A block that another section shows MUST be one row in the section of its file, "Shown in Imports only": a group by its
name, on each of its files, never the count of its heading; no change for N and P.

#### Scenario: A block of the imports in its file's section
- **WHEN** the section of a file holds a block that the group of the imports shows
- **THEN** the block is one row there, "Shown in Imports only", without the count of its heading "Imports only (1)"
- **AND** N and P pass it, no change of that section
  [@test] src/lib/diff/rows.test.ts (`makes the rows of a section of a review: a heading before each change it marks, one row for a block another section shows`)
  [@test] src/presenters/review-units.test.ts (`names a block a group shows by the name of the group on each of its files, never by the count of its heading`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

### Requirement: In the Review Window: A click on the row opens the other section

A click on the row of a block another section shows, or on its place in a fold, MUST open that section, expanded, as
the section at the top: the tree and the footer follow it.

#### Scenario: The row of the imports and the row of a repeated edit
- **WHEN** the user clicks the row of a block another section shows
- **THEN** that section opens, expanded
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`, `should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

#### Scenario: The place of a block in a fold
- **WHEN** the user clicks the place of a block in a fold ("1 change in step 1")
- **THEN** that section opens, expanded, and the footer names it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

### Requirement: In the Review Window: A heading and a row of words are one line high

A heading and a row of words MUST have the height of a line.

#### Scenario: The height of a heading and of a row
- **WHEN** a section shows a heading or a row of words
- **THEN** it has the height of a line

### Requirement: In the Review Window: A file of the tree opens its first open section

A file of the tree MUST open its first open section (of its concern), of a file in parts its first open part, a
place of the card the section that shows its line.

#### Scenario: A file of the tree whose change is in two sections
- **GIVEN** a file whose own change is in one section and whose repeated edit is in the group of the edit
- **WHEN** the user clicks its row in the tree
- **THEN** its first open section (of its concern) opens
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

#### Scenario: A file in parts
- **WHEN** the user clicks the row of a file the story shows in three parts
- **THEN** its first part opens, "part 1 of 3" in the footer
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A place of the card
- **WHEN** the user opens a place of the card
- **THEN** the section that shows its line opens

### Requirement: In the Review Window: J and K step through the sections

J and K MUST step through the sections, the tree following the section at the top.

#### Scenario: K and J
- **WHEN** the user presses K, then J, in the diffs
- **THEN** each key steps to the section before or after
- **AND** the tree selects the file of the section at the top
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

### Requirement: In the Review Window: The uses left outside follow the change

After the section of a change of a form or a removal, each of its first 5 uses left outside (`outside_sites` of its API
entry, by its key and file) MUST be a section of its own, "Left outside": the file at the head of the review around the
line of the use (`get_outside_place`, the lines of context of a change before and after it).

#### Scenario: A changed API with uses left outside
- **GIVEN** a change of the form of `helper` that leaves a use in `src/Base.kt` as it was
- **WHEN** the story shows the section of the change
- **THEN** each of its first 5 uses left outside is a section of its own after it, "Left outside"
- **AND** each shows the file at the head of the review around the line of the use, under the heading "helper — left
  outside: src/Base.kt:1"
  [@test] src/presenters/review-units.test.ts (`follow the section of the change: each its file around the use, at most five`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show after a changed API the uses it left outside, each its file at the head around the use, none that the head lacks`)

### Requirement: In the Review Window: A use left outside is no change and no row of the tree

A section of a use left outside MUST be no change for N and P and no row of the tree, which keeps the file of the
review selected.

#### Scenario: The tree keeps the file of the review
- **WHEN** the story shows a section of a use left outside
- **THEN** N and P pass it
- **AND** the tree has no row of it and keeps the file of the review selected
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show after a changed API the uses it left outside, each its file at the head around the use, none that the head lacks`)

### Requirement: In the Review Window: A use in a file the head lacks shows nothing

A use left outside in a file the head lacks MUST show nothing.

#### Scenario: Uses in files the head lacks
- **WHEN** the index names uses in files the head lacks
- **THEN** they show nothing
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show after a changed API the uses it left outside, each its file at the head around the use, none that the head lacks`)

### Requirement: In the Review Window: A commit shown alone has its own plan

The story MUST show in every review window with the plan: a commit of the branch review shown alone (Enter or Show
Commit Alone on its row in the tree, ⌥↑/⌥↓) MUST have its own plan (`ReviewSubject::Commit`, the commit against its
first parent) for its card and its diffs, beside the plan of the part, which stays the card under Since Your Review.

#### Scenario: A commit shown alone from its row in the tree
- **WHEN** the user shows a commit of the branch review alone (Enter or Show Commit Alone on its row in the tree,
  ⌥↑/⌥↓)
- **THEN** its card and its diffs are of its own plan, the commit against its first parent (`ReviewSubject::Commit`)
- **AND** the plan of the part stays beside it, the card under Since Your Review
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show the plan of a commit shown alone from its row in the tree: its overview and the story of its diffs @webkit`)
  [@test] crates/review-plan/src/units_test.rs (`a_commit_is_read_against_its_first_parent`)

### Requirement: In the Review Window: A commit window and a pull request read their plan

The review window of a commit and the review of a pull request MUST read the plan of what they show, and show none for
a repository that turns it off (`disabled`).

#### Scenario: The review window of a commit
- **WHEN** the review window of a commit opens
- **THEN** its overview and its diffs are of the plan of that commit
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show the plan of a commit in the review window of a commit: its overview and the story of its diffs`)

#### Scenario: The review of a pull request
- **WHEN** the review of a pull request opens in a repository that turns the plan on
- **THEN** it shows the plan of the pull request above its changes
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`)

#### Scenario: A repository that turns the plan off
- **WHEN** the repository turns the plan off (`disabled`)
- **THEN** the review window of a commit and the review of a pull request show no plan

### Requirement: In the Review Window: A file the units do not cut shows whole

A file whose blocks are not those of the units (the window reads another whitespace policy than `Default`) and a file
no unit names MUST show whole, in the place of its first unit, else after the story.

#### Scenario: Another whitespace policy
- **WHEN** the window reads another whitespace policy than `Default`, and the blocks of a file are not those of its
  units
- **THEN** the file shows whole, in the place of its first unit
  [@test] src/presenters/review-units.test.ts (`shows a file whole where its blocks are not those of the units, and a file no unit names after the story`)

#### Scenario: A file no unit names
- **WHEN** no unit names a file of the review
- **THEN** it shows whole after the story
  [@test] src/presenters/review-units.test.ts (`shows a file whole where its blocks are not those of the units, and a file no unit names after the story`)

## User Experience

`brd review-plan` prints, after `repeated_edits`:

```json
"units": [
  {"id": "declaration:src/a/Api.kt:a.open:path:String,mode:Int", "kind": "shape", "changes": ["parameters"],
   "name": "open", "key": "a.open", "signature": "path:String,mode:Int", "api": true, "test": false, "line": 3,
   "slices": [{"path": "src/a/Api.kt", "old_path": null, "whole": false,
     "blocks": [{"index": 0, "old": [3, 3], "new": [3, 3]}], "fingerprint": "5f0c…"}], "fingerprint": "5f0c…"},
  {"id": "new:src/a/Session.kt", "kind": "newFile", "name": "Session.kt", "api": false, "test": false,
   "slices": [{"path":
   "src/a/Session.kt", "old_path": null, "whole": true, "blocks": [], "fingerprint": "a1d9…"}], "fingerprint": "a1d9…"},
  {"id": "mechanical:importsOnly", "kind": "mechanical", "mechanical": "importsOnly", "api": false, "test": false,
   "slices": […]}
]
```

The section of a run: `diff-unit-heading` before a change (the name in the font of the code, bold, the words in Inter,
muted, a top border), and a block of
another section one `diff-unit-elsewhere` row in the tint of a fold. The first file of a group has the group on the
divider above its card, in `text-xs text-muted-foreground` (`review-section-heading`), never in its header: "Same edit
in 4 files", "Imports only (2)", "Left outside".

## Open Questions / Risks

- A block that changes two declarations goes to the one of most of its lines; the other has its own unit only when
  it is new or removed.
- The pair of a file several commits change is diffed under `Default` here; a window under another whitespace policy
  shows other blocks, which the window must map to the units by their lines.
- A container whose name changed gives its members new keys: they are removed and added.
- A Kotlin return type that is inferred is no part of the form: a change of it is `behaviour`.

## Measurements

`brd review-plan` (release build) on `~/projects/idea-1`, branch `air-local-launch`, 2026-10-30, the summaries in the
object cache (a next round of a branch: its blobs parsed before), accio answering every query:

| Range | Files | Units | `behaviour` | Other kinds | API entries | Time |
|---|---|---|---|---|---|---|
| `HEAD~1..HEAD` | 31 | 43 | 17 (39%) | 8 `newFile`, 8 `added`, 8 `other`, 2 `mechanical` | 33 | 0.19 s |
| `HEAD~3..HEAD` | 38 | 57 | 24 (42%) | 9 `newFile`, 10 `added`, 2 `removed`, 1 `shape`, 9 `other`, 2 `mechanical` | 36 | 0.22 s |
| `HEAD~5..HEAD` | 40 | 67 | 29 (43%) | 9 `newFile`, 16 `added`, 1 `removed`, 1 `shape`, 9 `other`, 2 `mechanical` | 36 | 0.11 s |

About 40% of the units are a change of a body alone, which the plan before the units did not show at all.
