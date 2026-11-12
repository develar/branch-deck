## ADDED Requirements

### Requirement: Order, Identity, Name: A concern says what it does to its name

A concern MUST say what it does to its name (`change`): what the change does to the declaration of that name among
its units (of a token, the one so named), `adds`, `removes`, `form` with the parts of its form that changed or `body`;
`replaces` of a replacement; of its one file or its directory `adds` or `removes` where every part adds or deletes a
file whole; else `mixed`, nothing a title may say.

#### Scenario: A token that names a declaration the concern adds
- **WHEN** a token joined a new function and the config that lists it by its name
- **THEN** the concern `adds` it
- **AND** a token the changed lines of new docs only name is `mixed`: a word or a name they newly use is no addition
  [@test] crates/review-plan/src/concerns_test.rs (`a_concern_adds_its_token_only_when_it_adds_the_declaration_of_that_name`)

#### Scenario: A token no declaration is named by
- **WHEN** a token joined the configs that drop one server
- **THEN** the concern is `mixed`
  [@test] crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)

#### Scenario: A replacement
- **WHEN** the concern is named by a replacement
- **THEN** it `replaces`
  [@test] crates/review-plan/src/concerns_test.rs (`a_generated_file_follows_the_concern_of_the_source_its_header_names`)

#### Scenario: A declaration whose parameters changed
- **WHEN** the concern is named by a declaration whose parameters changed
- **THEN** it changes the `form` of it, its `changes` the parameters
  [@test] crates/review-plan/src/concerns_test.rs (`a_changed_declaration_and_the_changes_of_its_uses_are_one_concern`)

#### Scenario: The files deleted from a directory
- **WHEN** the concern is the files the commits delete from one directory, named by it
- **THEN** it `removes` them
  [@test] crates/review-plan/src/concerns_test.rs (`a_concern_of_the_files_deleted_from_a_directory_removes_them`)

#### Scenario: A file of several changes
- **WHEN** the concern is named by its one file, whose parts neither all add nor all delete it
- **THEN** it is `mixed`
  [@test] crates/review-plan/src/concerns_test.rs (`a_source_and_its_generated_copies_are_named_by_the_file`)

## MODIFIED Requirements

### Requirement: Order, Identity, Name: A concern a token joined is named by its token

The name MUST be the evidence of the concern: of a concern a token joined, the rare token with the most of its nodes in
it (more than half), a whole identifier before a part of one (`zebrafish-mode`, not its `zebrafish`), a simple word
before a compound one, the one its changed lines name first.

#### Scenario: Configs joined by an identifier
- **WHEN** a token joined the configs that drop one server
- **THEN** the concern is named by that token
  [@test] crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)

#### Scenario: A whole identifier before a part of one
- **WHEN** the changed lines of a concern name a whole identifier and a part of it (`zebrafish-mode`, `zebrafish`)
- **THEN** the concern is named by the whole identifier
  [@test] crates/review-plan/src/concerns_test.rs (`a_name_of_a_library_joins_nothing_a_word_of_a_string_does`)

#### Scenario: A token though the uses of a declaration joined more
- **WHEN** the uses of a changed declaration joined a concern twice and a rare token of a doc joined it once
- **THEN** the concern is named by the token: named by the declaration, the concerns of the lab had their names less
  often in the subjects of their commits ("Measurements")
  [@test] crates/review-plan/src/concerns_test.rs (`a_concern_a_token_joined_is_named_by_it_though_the_uses_of_a_declaration_joined_it_more_often`)

### Requirement: In `brd`: `brd review-plan` prints the concerns

`brd review-plan` MUST print `concerns` (`"schema": 34`): per concern its `id`, `name`, its `change` (since 34), its
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
  `file` with `path`, `directory` with `directory`, or `rest`

#### Scenario: What a concern does to its name
- **WHEN** `brd review-plan` prints the `change` of a concern
- **THEN** it is `kind` `adds`, `removes`, `replaces`, `form` with `changes`, `body` or `mixed`

### Requirement: In the Review Window: The tree lists the concerns at its top

With a concern beside the rest, the tree of a review with its plan MUST list the concerns at its top, in their order:
a row per concern, titled by what it does to the name of its evidence (`change`), else by that name alone, with its
files and lines, why its parts are together in its hover. The step of the concern MUST be titled so wherever it is
named: the tree, the contents, its opening, the bar of the step, the divider of its section.

#### Scenario: The rows of the concerns
- **GIVEN** a plan with concerns beside the rest
- **WHEN** the tree of the review shows
- **THEN** a row per concern stands at its top, in their order, titled by its evidence, with its files and lines
- **AND** its hover says why its parts are together
  [@test] src/lib/file-tree.test.ts (`lists the concerns, each with its files by their path, a file of another concern also, its generated files folded; then Other changes and Mechanical`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

#### Scenario: The name and the hover of a concern
- **WHEN** a concern is named by a token, a replacement, a declaration, a file or a directory
- **THEN** its row names it by that evidence ("proxy", "grep → accio", the name of a declaration, the directory or
  "Root")
- **AND** its hover says why its parts are together ("Joined by the identifiers ij-mcp-proxy, ij-proxy")
  [@test] src/presenters/review-concerns.test.ts (`names a concern by its evidence, and says why its parts are together`)

#### Scenario: What a concern does, in its title
- **WHEN** a concern adds or removes the declaration of its name, changes its form or its body
- **THEN** its title says it before the name, a sentence set as text: "Add probe", "Remove legacy", "Change the
  parameters of open" ("visibility", "constructors" alone, else "signature"), "Change open"
- **AND** a concern that does nothing a title may say, a replacement, the root and Other changes keep the name alone
  ("ijproxy", "grep → accio", "guide.md", "Root"), an identifier set as code
  [@test] src/presenters/review-concerns.test.ts (`titles a concern by what it does to its name, a sentence, and a concern of several changes by its name alone, code`)
  [@test] src/presenters/review-steps.test.ts (`names a step of a concern by the label of the section of the body it tells, else by its concern as code, keeps its main rows, and tells no step of a story Rust tells in none`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should name in the bar above the diffs a step by its place, Other changes and the group of the imports after the story by their name alone, a click showing the opening of Other changes`)
