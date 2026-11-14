## MODIFIED Requirements

### Requirement: Modules: The modules are accio's, one process a side

The roots of the modules MUST come from accio, `accio modules --no-build --ref <revision> --paths-from -`, one
process at the head reviewed and one at the base (of Since Your Review the start of its scope, the reviewed tip where
its carry conflicts), at once with the reads of git, of an accio whose `accio versions` gives `answers.modules` 1.
Without both answers every file MUST be of the root of the repository, which is no reason of the `index_note`.

#### Scenario: The modules of a side
- **WHEN** a plan reads the modules of its files
- **THEN** one `accio modules --no-build --ref <revision> --paths-from -` reads them at each side, the paths on its standard input
- **AND** an accio without the answer or of another revision of it is not asked, and one without an index answers none
  [@test] crates/file-diffs/src/accio_test.rs (`the_modules_of_a_side_are_read_in_one_process_of_an_accio_that_answers_them`)

#### Scenario: The modules the accio of this machine answers
- **WHEN** the accio of this machine has an index of the repository
- **THEN** the modules of the plan are the roots, names and states its answers give, a Python project among them
  [@test] crates/review-plan/src/modules_test.rs (`the_modules_of_a_review_are_those_accio_answers`)

#### Scenario: The test data of a module
- **GIVEN** a module whose `*.iml` holds its directory as content, a `go.mod` of a test case below its `testData`
- **WHEN** a commit changes its code and that test case and adds a `pyproject.toml` to another case, and the accio of this machine answers the modules
- **THEN** every file is of that module, `changed`: a build file of its test data makes no module
  [@test] crates/review-plan/src/modules_test.rs (`a_build_file_in_the_test_data_of_a_module_makes_no_module`)

#### Scenario: Without both answers
- **WHEN** the index answers the modules of neither side, or of one alone, the other answering none or failing
- **THEN** every file is of the root of the repository (`""`, named and labelled `(root)`), `changed`
  [@test] crates/review-plan/src/modules_test.rs (`a_module_is_added_or_deleted_by_the_roots_of_the_two_sides`)
  [@test] crates/review-plan/src/modules_test.rs (`the_modules_of_one_side_alone_put_every_file_at_the_root`)

#### Scenario: Since Your Review whose carry conflicts
- **GIVEN** a branch marked reviewed with a module, rewritten onto a baseline that changed a line the review changed, and a commit on top
- **WHEN** its plan since the review reads the rewritten and the new commits, its carry conflicting
- **THEN** their modules are compared with the reviewed tip: the module the reviewer read is `changed`, not `added`
  [@test] crates/review-plan/src/modules_test.rs (`since_the_review_whose_carry_conflicts_compares_the_modules_with_the_reviewed_tip`)
