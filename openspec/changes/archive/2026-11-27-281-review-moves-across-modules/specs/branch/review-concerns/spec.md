## MODIFIED Requirements

### Requirement: In `brd`: `brd review-plan` prints the concerns

`brd review-plan` MUST print `concerns` (`"schema": 44`): per concern its `id`, `name`, its `change` (since 34), its
`parts` (`unit`, `path`, `blocks`), its `paths`, its `elsewhere` (since 8), its `evidence` (`kind`: `uses`, `group`,
`token`, `rest`, its `detail` and `joins`, the most joins first) and its `fingerprint`.

#### Scenario: The concerns of a range
- **WHEN** `brd review-plan` prints a range
- **THEN** it prints `concerns`, each with its `id`, `name`, `change`, `parts` (`unit`, `path`, `blocks`), `paths`,
  `elsewhere`, `evidence` (`kind`, `detail`, `joins`, the most joins first) and `fingerprint`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

#### Scenario: The sources of a name
- **WHEN** `brd review-plan` prints the `name` of a concern
- **THEN** it is `source` `token` with `token`, `replacement` with `removed` and `added`, `declaration` with `name`,
  `file` with `path`, `files` with `name` (since 43), `directory` with `directory`, or `rest`

#### Scenario: What a concern does to its name
- **WHEN** `brd review-plan` prints the `change` of a concern
- **THEN** it is `kind` `adds`, `removes`, `replaces`, `form` with `changes`, `body` or `mixed`
