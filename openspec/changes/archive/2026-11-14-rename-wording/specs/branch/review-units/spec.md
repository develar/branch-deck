## ADDED Requirements

### Requirement: Units: A body changed in a rename the index has not proven says so

A unit `behaviour` whose every changed line is a run of the kind `Renamed` of a name that is not local, which the index
of the repository has not proven, MUST say those renames and what they wait on (`renamed`: its `names`, each `old` and
`new`, and `wait`): `checking` while the index is asked, `notProven` where none answered or it did not list every use.
It MUST be read from the marks the reading of the plan proves, the same that say the file is not proven in the header
([Range Reading](../../review/range-reading/spec.md)), and read again with the answer of the index. A rename the index
confirmed, a run that is quiet, a local name, a rename the index says is more than a rename and a body with any other
change MUST leave the unit `behaviour` without `renamed`.

#### Scenario: The calls of a renamed private function without an index
- **GIVEN** a private function `sumOf` renamed to `totalOf` at its declaration and its three calls, and no index
- **WHEN** the plan is read
- **THEN** the units of the three calls are `behaviour` with `renamed` `sumOf` → `totalOf`, `notProven`
- **AND** a call whose function also changed in another line is `behaviour` without `renamed`
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_body_changed_in_a_rename_no_index_proved_is_told_by_the_rename`)

#### Scenario: While the index is asked
- **GIVEN** the same rename and an index that does not prove it
- **WHEN** the plan is sent before the index answered, and then whole
- **THEN** each plan sent before says `checking`, and the plan whole `notProven`
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_rename_the_index_is_asked_of_is_checking_until_it_answers`)

#### Scenario: A rename the index proved
- **WHEN** the accio of this machine proves the rename
- **THEN** no unit has `renamed`, and the reading proves the file
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_rename_the_index_proved_leaves_no_word_on_the_body`)

### Requirement: In the Review Window: A body changed in a rename the index has not proven is worded as the rename

The heading of a unit with `renamed` MUST word the renames and their wait in place of "body changed": "balance — only
renamed sumOf → totalOf, not proven", "… checking" while the index is asked, the names as a sentence lists them; the
badge of the card of its name MUST say "Renamed, not proven" or "Renamed, checking". A unit `behaviour` without it
keeps "body changed".

#### Scenario: The heading of a rename no index proved
- **GIVEN** a review with its plan of a commit that renames a private `sumOf` to `totalOf`, and no index
- **WHEN** the review shows the diffs
- **THEN** the heading of a call that changed in the name alone says "balance — only renamed sumOf → totalOf, not proven"
- **AND** the heading of a call whose line also changed in another way says "half — body changed"
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] src/presenters/review-units.test.ts (`words a body that changed in renamed names alone as the renames, not proven or checking, as Rust decided`)

#### Scenario: A pull request that renames a private helper
- **GIVEN** the review of a pull request whose one commit renames the private `sumOf` of a ledger to `totalOf`, and no index
- **WHEN** its changes show
- **THEN** the header says "4 changes to read in 1 file · 1 file not proven", and the headings of its three calls "balance — only renamed sumOf → totalOf, not proven", "balanceOn — …" and "averageAmount — …"
- **AND** no heading says "body changed"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say of a call that changed in a rename no index proved that it is the rename, not proven, as the header says of its file`)

#### Scenario: The card of the name
- **WHEN** a mention of the story names such a unit
- **THEN** its card says "Renamed, not proven", "Renamed, checking" while the index is asked, and "Body changed" of a unit without `renamed`
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

## MODIFIED Requirements

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 37`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices`, its `fingerprint` and, of a body changed in a rename the index has not proven, `renamed`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 37`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
  `line`, its `slices` and its `fingerprint`
- **AND** `kind` is `shape` with `changes`, `behaviour`, `added`, `removed`, `newFile`, `deletedFile`, `deletedFiles`
  with `directory`, `mechanical` with `mechanical`, `repeatedEdit` with `group`, or `other` with `reason`
- **AND** each of its `slices` has `path`, `old_path`, `whole`, the `blocks` by `index` with their `old` and `new` lines
  from 1, both included, `null` for a side with none, and a `fingerprint`
- **AND** a `behaviour` unit whose body changed in renamed names alone that the index has not proven has `renamed`, its
  `names` (`old`, `new`) and its `wait` (`checking`, `notProven`); no other unit has it
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
