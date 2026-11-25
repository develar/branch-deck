## MODIFIED Requirements

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 43`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices`, its `fingerprint` and, of a body changed in a rename the index has not proven, `renamed`,
of a declaration renamed so, `renamed_from`, of a declaration moved whole to another file, `moved_from` or `moved_to`;
of a slice that holds nothing but declarations moved whole to other files, `moved_out`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 43`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
  `line`, its `slices` and its `fingerprint`
- **AND** `kind` is `shape` with `changes`, `behaviour`, `added`, `removed`, `newFile`, `deletedFile`, `deletedFiles`
  with `directory`, `mechanical` with `mechanical`, `repeatedEdit` with `group`, or `other` with `reason`
- **AND** each of its `slices` has `path`, `old_path`, `whole`, the `blocks` by `index` with their `old` and `new` lines
  from 1, both included, `null` for a side with none, and a `fingerprint`
- **AND** a `behaviour` unit whose body changed in renamed names alone that the index has not proven has `renamed`, its
  `names` (`old`, `new`) and its `wait` (`checking`, `notProven`); no other unit has it
- **AND** a declaration renamed where the index has not proven it is one unit with `renamed_from`, its `old` name and its
  `wait`
- **AND** an `added` declaration moved whole from another file has `moved_from`, a `removed` one moved whole to another
  file `moved_to`, each the `path` of the other place, the `line` of its name there, from 1, and the `unit` that shows
  it; no other unit has them
- **AND** a slice that holds nothing but declarations moved whole to other files has `moved_out: true`; no other slice
  has it
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_renamed_of_a_body_changed_in_a_rename_alone_and_of_the_declaration`, `prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)
