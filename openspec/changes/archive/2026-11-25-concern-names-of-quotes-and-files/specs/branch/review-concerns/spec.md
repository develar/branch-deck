## MODIFIED Requirements

### Requirement: Edges: A sentence a config or a literal quotes joins by its compound names

Of a sentence a config, a literal or a code span of prose quotes (a string or a span with a space: a heading an
OpenSpec delta quotes, the title of a test a link of a spec names) only its compound names MUST count, with `_`, `-`,
a digit or a capital past the first letter (`sessions-window-show`, `toolWindowId`).

#### Scenario: Sentences of configs, text files and string literals
- **WHEN** configs, text files and string literals add sentences that share words, and two configs a sentence with the
  same compound name
- **THEN** the shared words join nothing
- **AND** the compound name joins its two configs
  [@test] crates/review-plan/src/concerns_test.rs (`a_sentence_a_config_quotes_joins_by_its_compound_names_alone`)

#### Scenario: A sentence a code span of prose quotes
- **WHEN** two Markdown files quote in a code span sentences that share words (`### Requirement: A kestrel plugin: A
  section loads its layout`, `a kestrel plugin states the layout of its section`)
- **THEN** the shared words join nothing
  [@test] crates/review-plan/src/concerns_test.rs (`a_sentence_a_config_quotes_joins_by_its_compound_names_alone`)

### Requirement: Order, Identity, Name: Without a token, a declaration, a file or a directory names it

A concern no token names MUST be named by its first changed declaration; else by its one file (`file`: a source with
the generated copies that follow it); else by the one name of its files in several directories (`files`:
`BUILD.bazel` of 35 modules a repeated edit joined); else by the directory its files share.

#### Scenario: A changed declaration
- **WHEN** no token names the concern and it changes a declaration
- **THEN** the concern is named by its first changed declaration
  [@test] crates/review-plan/src/concerns_test.rs (`a_changed_declaration_and_the_changes_of_its_uses_are_one_concern`)

#### Scenario: A source and its generated copies
- **WHEN** the concern is one source with the generated copies that follow it, and no other evidence
- **THEN** the concern is named by its one file, `file`
  [@test] crates/review-plan/src/concerns_test.rs (`a_source_and_its_generated_copies_are_named_by_the_file`)

#### Scenario: Files of one name in several directories
- **WHEN** no token, declaration or one file names the concern, and its files have one name in several directories
- **THEN** it is named by that name, `files`, never by the root they share
  [@test] crates/review-plan/src/concerns_test.rs (`files_of_one_name_in_several_directories_are_named_by_it`)
  [@test] src/presenters/review-concerns.test.ts (`names a concern by its evidence, and says why its parts are together`)

#### Scenario: Files of one directory
- **WHEN** no token, declaration, one file or one name of its files names the concern
- **THEN** it is named by the directory its files share

### Requirement: In `brd`: `brd review-plan` prints the concerns

`brd review-plan` MUST print `concerns` (`"schema": 43`): per concern its `id`, `name`, its `change` (since 34), its
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
