## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Modules: Every changed file has its module

`modules` MUST list the module of every changed file once: the root accio answers for its path at the head reviewed
(the nearest directory above it whose build files declare a module: JPS, Bazel, Cargo, npm, Go, Python, Maven, Gradle,
MSBuild), or the deeper root it answers at the base, which the head declares no module at (a module the commits
delete), else the root of the repository (`""`, named and labelled `(root)`). `files[].module` names it by its place.

#### Scenario: The module of a changed file
- **WHEN** a file changes below a directory whose build files declare a module at the head reviewed
- **THEN** `modules` lists that module once, and `files[].module` names it by its place
- **AND** a file under no such directory is of the root of the repository (`""`, named and labelled `(root)`)
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)

#### Scenario: A module the commits delete
- **WHEN** the commits delete a build file, which the head lacks, and files below it
- **THEN** those files stay in that module
  [@test] crates/review-plan/src/modules_test.rs (`the_files_of_a_module_the_commits_delete_stay_in_it`)

### Requirement: Modules: A module is added, deleted or changed

A module MUST be `deleted` when its root is of the base alone, `added` when the head declares a module there and no
file of it has that root at the base, else `changed`. Its `name` is the one its build files give it (its `*.iml` by the
file's stem, its crate as its `Cargo.toml` writes it, its package, its Maven, Gradle, MSBuild, Go or Python project),
else of the module a dependency change gives a build file at its root, else of its directory.

#### Scenario: A deleted module
- **WHEN** the commits delete the build file at the root of a module and the head has no module there
- **THEN** the module is `deleted`
  [@test] crates/review-plan/src/modules_test.rs (`the_files_of_a_module_the_commits_delete_stay_in_it`)

#### Scenario: An added or a changed module
- **WHEN** the head declares a module the base does not, or the commits change files of a module both declare, its build file renamed too
- **THEN** the module is `added`, else `changed`
- **AND** its `name` is the one its build files give it, of the module a dependency change gives a build file at its root, else of its directory
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`, `a_module_is_added_or_deleted_by_the_roots_of_the_two_sides`)

### Requirement: Links: The build files are those of the head reviewed

The build files MUST be those of the head reviewed, not of the checkout: a pull request is not checked out, and a
module it adds is in no other tree. accio reads them at that revision (`accio modules --ref <head>`).

#### Scenario: A head that is not checked out
- **WHEN** the head reviewed is not checked out and adds a module
- **THEN** the modules of its files are read from that revision
- **AND** the links cross the module it adds
  [@test] crates/review-plan/src/links_test.rs (`the_links_of_a_head_that_is_not_checked_out_cross_the_module_it_adds`)

### Requirement: Links: A link between modules is cross_module

A link MUST be `cross_module` when the modules of its two files differ (the roots of [Modules](#requirement-modules-every-changed-file-has-its-module)),
and `new_between_modules` when it is also `added`: a new dependency of a module, which the review lists.

#### Scenario: A link between two modules
- **WHEN** the modules of the two files of a link differ
- **THEN** the link is `cross_module`
- **AND** `new_between_modules` when it is also `added`: a new dependency of a module, which the review lists
  [@test] crates/review-plan/src/links_test.rs (`a_link_between_two_modules_crosses_them`)

### Requirement: In the App: A fact of the plan shows when its query answered

`get_review_plan` MUST send the plan on its channel as it stands, each with what it still waits for (`waits`): the
plan of git alone before the index answered (`index`), when there is a query to ask; the plan with the usages, the
registrations and the entry points once they answered, while the tests of the steps are still asked (`tests`); then
the plan whole, which the window shows in turn. The plan of git MUST NOT wait for the modules, of the index too: it
comes without them, then with them.

#### Scenario: A plan with a query to ask
- **WHEN** `get_review_plan` reads a plan with a query of the index to ask
- **THEN** it first sends the plan of git alone (`waits` is `index`): no usages of its API, its `verdict` `pending`, no file counting `breaks`, no registration, no entry point of a new module, no test of a step
- **AND** then the plan whole (`waits` null), and the window replaces what it shows with each
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
  [@test] crates/review-plan/src/registrations_test.rs (`the_plan_sent_while_the_index_is_asked_has_no_registration`)

#### Scenario: The plan of git does not wait for the modules
- **GIVEN** a plan whose head adds a module, its modules still asked of the index
- **WHEN** git has read the rest of the plan
- **THEN** it sends the plan of git alone (`waits` is `index`) with no module and no file of one
- **AND** then, once the modules answered, the plan with them (`waits` is `index`) while the index is asked what enters the new module, then the plan whole
  [@test] crates/review-plan/src/stages_test.rs (`the_plan_of_git_is_sent_before_the_modules_answered`)

#### Scenario: The tests of the steps are still asked
- **GIVEN** a plan whose steps have code and whose range has tests: the tests of its steps are a query of the index
- **WHEN** the usages, the registrations and the entry points answered while that query still runs
- **THEN** it sends the plan with them (`waits` is `tests`), each step whose code is asked `awaited` and none `answered`
- **AND** then the plan whole, with the tests that reach the code of its steps, no step `awaited`
  [@test] crates/review-plan/src/stages_test.rs (`the_plan_shows_the_usages_while_the_tests_of_its_steps_are_asked`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the uses left outside while the index is still asked the tests of the steps, each step saying its tests are being found, then the tests that reach it`)

#### Scenario: The tests of the steps are no query of the index
- **WHEN** the range has no test, so the index is asked nothing of the tests of its steps
- **THEN** no plan waits for the tests alone and no step is `awaited`: the plan of git alone, without its modules and then with them, then the plan whole
  [@test] crates/review-plan/src/stages_test.rs (`a_range_without_tests_has_no_stage_of_its_tests`)

#### Scenario: The words while the usages are pending
- **WHEN** the window shows a plan that waits for `index`
- **THEN** it words an API without usages "Finding the uses…"
  [@test] src/presenters/review-plan.test.ts (`says the uses are being found while Rust asks the index`)

#### Scenario: The words once only the tests are asked
- **WHEN** the window shows a plan that waits for `tests`
- **THEN** an API without usages is "The uses are unknown", never "Finding the uses…": its usages answered
  [@test] src/presenters/review-story.test.ts (`says what a plan that waits for the index still finds: the uses only while nothing answered, the tests of each step Rust asks them of`)

#### Scenario: The stages in the window
- **WHEN** the events of a read come: the plan that waits for `index`, the one that waits for `tests`, the plan whole
- **THEN** the window shows each as it comes, and the read ends with the plan whole
  [@test] src/lib/review-plan-answer.test.ts (`shows each stage of a read as it comes: the plan that waits for the index, the one that waits for its tests, the plan whole`)

## REMOVED Requirements

### Requirement: Modules: The modules read the trees in the batch of the summaries

**Reason**: the modules are accio's (`accio modules`), no tree of git: a plan of build files alone reads no tree.
**Migration**: [Modules: The modules are accio's, one process a side](#requirement-modules-the-modules-are-accios-one-process-a-side); the budget of a plan of build files alone is [Dependency Changes: The texts of the build files come with the hunks](#requirement-dependency-changes-the-texts-of-the-build-files-come-with-the-hunks).
