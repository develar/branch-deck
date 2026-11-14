## ADDED Requirements

### Requirement: Units: A declaration renamed where the index has not proven it is one unit

A declaration renamed by a rename its file declares, not proven by the index, MUST be one unit with the declaration of
its old name, named by the new one, with `renamed_from` (`old`, `wait`), when both are of one kind and key but the last
name, the old key gone, and the same code but for renamed names and whitespace. Its members pair under the new name.
Any other pair MUST stay as without a rename.

#### Scenario: A private function renamed
- **GIVEN** a private function `sumOf` renamed to `totalOf` at its declaration and its three calls, and no index
- **WHEN** the plan is read
- **THEN** `totalOf` is one unit `behaviour` with `renamed_from` `sumOf`, `notProven`, and no unit is `sumOf`
- **AND** the plan sent while the index is asked says `checking`, the same units
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_body_changed_in_a_rename_no_index_proved_is_told_by_the_rename`, `a_rename_the_index_is_asked_of_is_checking_until_it_answers`)

#### Scenario: A body that changed with the name
- **WHEN** the function renamed rounds otherwise too, or a class renamed has a member that changed
- **THEN** the new one is `added` and the old one `removed`
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_declaration_renamed_with_more_than_its_name_stays_removed_and_added`, `a_class_renamed_is_one_unit_with_its_members`)

#### Scenario: A class renamed
- **WHEN** a class is renamed with no other change, and another file uses it
- **THEN** the class is one unit with `renamed_from`, no member is a unit, and the use is told by its rename
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_class_renamed_is_one_unit_with_its_members`)

#### Scenario: A rename the index proved
- **WHEN** the accio of this machine proves the rename
- **THEN** `totalOf` is `added` and `sumOf` `removed`, as without the pairing, and the story is told again where the plan sent before paired them
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_rename_the_index_proved_leaves_no_word_on_the_body`)

## MODIFIED Requirements

### Requirement: Units: A declaration of one side alone is added or removed

A declaration of one side alone whose lines a block changes MUST be `added` or `removed`, also when another unit holds
that block: then it has no block. A declaration renamed where the index has not proven it is no declaration of one side
alone ([A declaration renamed](#requirement-units-a-declaration-renamed-where-the-index-has-not-proven-it-is-one-unit)).

#### Scenario: A new and a removed declaration
- **WHEN** a block changes the lines of a declaration of the new side alone, and of one of the old side alone
- **THEN** the first is `added` and the second `removed`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: Another unit holds the block
- **WHEN** another unit holds the block that changes the lines of a declaration of one side alone
- **THEN** the declaration is still `added` or `removed`, and it has no block

### Requirement: In the Review Window: A body changed in a rename the index has not proven is worded as the rename

The heading of a unit with `renamed` MUST word the renames and their wait in place of "body changed": "balance — only
renamed sumOf → totalOf, not proven", "… checking" while the index is asked, the names as a sentence lists them; the
badge of the card of its name MUST say "Renamed, not proven" or "Renamed, checking". The heading of a unit with
`renamed_from` MUST say "totalOf — renamed from sumOf, not proven" ("… checking"), after what else of its form changed
("parameters changed, renamed from sumOf, not proven"), and its badge the same as of `renamed`. A unit `behaviour`
without either keeps "body changed".

#### Scenario: The heading of a rename no index proved
- **GIVEN** a review with its plan of a commit that renames a private `sumOf` to `totalOf`, and no index
- **WHEN** the review shows the diffs
- **THEN** the heading of a call that changed in the name alone says "balance — only renamed sumOf → totalOf, not proven"
- **AND** the heading of a call whose line also changed in another way says "half — body changed"
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] src/presenters/review-units.test.ts (`words a body that changed in renamed names alone as the renames, not proven or checking, as Rust decided`)

#### Scenario: The heading of a declaration renamed
- **GIVEN** the same review
- **WHEN** it shows the declaration of `totalOf`
- **THEN** its heading says "totalOf — renamed from sumOf, not proven", and no heading names `sumOf`
- **AND** "… checking" while the index is asked, and "totalOf — parameters changed, renamed from sumOf, not proven" where its parameters changed too
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] src/presenters/review-units.test.ts (`words a declaration renamed where the index has not proven it as renamed from its old name, after what else of its form changed`)

#### Scenario: A pull request that renames a private helper
- **GIVEN** the review of a pull request whose one commit renames the private `sumOf` of a ledger to `totalOf`, and no index
- **WHEN** its changes show
- **THEN** the header says "4 changes to read in 1 file · 1 file not proven", and the headings of its three calls "balance — only renamed sumOf → totalOf, not proven", "balanceOn — …" and "averageAmount — …"
- **AND** the heading of the helper says "totalOf — renamed from sumOf, not proven"; no heading says "body changed", "new" or "removed"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say of the calls and the declaration of a helper renamed where no index proves it that they are the rename, not proven, as the header says of its file`)

#### Scenario: The card of the name
- **WHEN** a mention of the story names such a unit
- **THEN** its card says "Renamed, not proven", "Renamed, checking" while the index is asked, the same of a declaration renamed, and "Body changed" of a unit without either
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 38`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices`, its `fingerprint` and, of a body changed in a rename the index has not proven, `renamed`,
of a declaration renamed so, `renamed_from`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 38`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
  `line`, its `slices` and its `fingerprint`
- **AND** `kind` is `shape` with `changes`, `behaviour`, `added`, `removed`, `newFile`, `deletedFile`, `deletedFiles`
  with `directory`, `mechanical` with `mechanical`, `repeatedEdit` with `group`, or `other` with `reason`
- **AND** each of its `slices` has `path`, `old_path`, `whole`, the `blocks` by `index` with their `old` and `new` lines
  from 1, both included, `null` for a side with none, and a `fingerprint`
- **AND** a `behaviour` unit whose body changed in renamed names alone that the index has not proven has `renamed`, its
  `names` (`old`, `new`) and its `wait` (`checking`, `notProven`); no other unit has it
- **AND** a declaration renamed where the index has not proven it is one unit with `renamed_from`, its `old` name and its
  `wait`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_renamed_of_a_body_changed_in_a_rename_alone_and_of_the_declaration`)
