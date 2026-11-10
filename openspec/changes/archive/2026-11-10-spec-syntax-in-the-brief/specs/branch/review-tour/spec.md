## MODIFIED Requirements

### Requirement: Brief: A new file is told by its description, a deleted one is deleted

A new spec MUST be told by its `description:` (else the first paragraph of its "Purpose" or "Summary") and the count
of its requirements, read as its format defines it (`spec_syntax`); another new file by its first paragraph of prose below its front matter; a deleted file MUST be
`deleted`.

#### Scenario: A new spec of either format
- **WHEN** the change adds a spec
- **THEN** the entry has its `description:`, else the first paragraph of its "Purpose" or "Summary"
- **AND** the count of its requirements: its requirements of OpenSpec (of a delta, the added and the modified ones),
  else the items of its "Requirements"
  [@test] crates/review-plan/src/steps_test.rs (`a_new_spec_counts_its_requirements_in_either_format`, `the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`)

#### Scenario: Another new file and a deleted file
- **WHEN** the change adds another file of prose, or deletes one
- **THEN** a new file has its first paragraph of prose below its front matter
- **AND** a deleted file is `deleted`

### Requirement: Brief: An entry names its section, its tests and its paragraphs

An entry MUST name the heading of its section of Markdown below the front matter, the paths that the test links its
blocks add name (`provenBy`), each from the root by the rule of its file's format (of an IntelliJ `*.spec.md` from its
directory, of any other file from the root unless it starts `./` or `../`), and its `paragraphs`, each whole, its lines joined without the marker of an item and the
test links.

#### Scenario: A rule with its test links
- **WHEN** a changed item adds test links
- **THEN** the entry names the paths they name (`provenBy`)
- **AND** its `paragraphs` are each whole, its lines joined without the marker of an item and the test links
  [@test] crates/review-plan/src/prose_test.rs (`the_test_lines_of_an_item_are_no_prose`)
  [@test] crates/review-plan/src/steps_test.rs (`the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`)

#### Scenario: A test link of an IntelliJ spec
- **WHEN** a changed rule of an IntelliJ `*.spec.md` adds a test link to `../../src/AgentsPageTest.kt`, its own line
  or a bullet, the path after the marker or in brackets
- **THEN** its `provenBy` names the file from the spec's directory, `plugins/air/src/AgentsPageTest.kt` of
  `plugins/air/spec/actions/agents-page.spec.md`, never the path with its `../` cut
  [@test] crates/review-plan/src/steps_test.rs (`a_test_line_names_its_file_by_the_rule_of_its_format`)

#### Scenario: A scenario of OpenSpec
- **WHEN** the entry is of a scenario of OpenSpec
- **THEN** its heading is the `#### Scenario:`, under its requirement without `Requirement:`
  [@test] crates/review-plan/src/steps_test.rs (`the_brief_places_a_scenario_of_openspec_under_its_requirement_with_its_tests`)
