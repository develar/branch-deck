## ADDED Requirements

### Requirement: Modules: A root of generated files alone is no module

A root whose changed files are all generated (`mechanical: "generated"`) MUST be no module of the review, and its files
MUST have no module (`files[].module` `null`), their `folder` still below their root. A root with a file changed by
hand MUST stay a module, its generated files with it.

#### Scenario: A generator's run over thousands of modules
- **WHEN** the commits change the generated build file of modules whose other files they leave as they were, and the
  code and the build file of another module
- **THEN** `modules` lists that other module alone, its generated build file in it
- **AND** each other generated build file has no module
  [@test] crates/review-plan/src/modules_test.rs (`a_root_of_generated_files_alone_is_no_module`)

### Requirement: Dependency Changes: A generated manifest has none

A build file whose change is generated MUST have no entry in `dependencies`: its entries are the output of its
generator, as its files are; one written by hand MUST keep its own.

#### Scenario: A generated build file and one written by hand
- **WHEN** the commits add the same dependency to a generated build file and to one written by hand
- **THEN** `dependencies` lists the entry of the one written by hand alone
  [@test] crates/review-plan/src/modules_test.rs (`a_generated_manifest_has_no_dependency_change`)

### Requirement: Files: The plan of an agent folds the generated files

`brd review-plan` MUST keep the first 20 generated files of `files`, of `order`, of the slices of the unit `generated`
and of the stops and the paths of each step, and print how many each left out (`files_omitted`, `order_omitted`,
`slices_omitted`, `stopsOmitted`, `pathsOmitted`), as `brd` cuts a list. Every other entry MUST stay whole.

#### Scenario: A generator's run of 25 files
- **WHEN** `brd review-plan` runs on a range whose commits write 25 generated files and change a file by hand
- **THEN** `files`, `order`, the slices of the unit `generated` and the stops and paths of its step each keep 20 of
  them and count the 5 they left out
- **AND** the file changed by hand is in each, and a step of no generated file counts nothing
- **AND** the `start`, `tests` and `end` of each group of a step count the stops it prints
- **AND** a list one longer than 20 is printed whole, and the review window reads the whole plan
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`the_generated_files_of_a_generators_run_fold_past_the_first_20`)

## MODIFIED Requirements

### Requirement: Modules: Every changed file has its module

`modules` MUST list the module of every changed file once: the root accio answers for its path at the head reviewed
(the nearest directory above it whose build files declare a module: JPS, Bazel, Cargo, npm, Go, Python, Maven, Gradle,
MSBuild), or the deeper root it answers at the base, which the head declares no module at (a module the commits
delete), else the root of the repository (`""`, named and labelled `(root)`). `files[].module` names it by its place;
a root whose changed files are all generated is none ([A root of generated files alone is no
module](#requirement-modules-a-root-of-generated-files-alone-is-no-module)).

#### Scenario: The module of a changed file
- **WHEN** a file changes below a directory whose build files declare a module at the head reviewed
- **THEN** `modules` lists that module once, and `files[].module` names it by its place
- **AND** a file under no such directory is of the root of the repository (`""`, named and labelled `(root)`)
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)

#### Scenario: A module the commits delete
- **WHEN** the commits delete a build file, which the head lacks, and files below it
- **THEN** those files stay in that module
  [@test] crates/review-plan/src/modules_test.rs (`the_files_of_a_module_the_commits_delete_stay_in_it`)

### Requirement: Since Your Review: brd review-plan prints the interdiff and its schema version

`brd review-plan` MUST print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`, `commits`
with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, `message_change`, and `scope` with `base`,
`head` and `carried`), with `"schema": 41`, and its output MUST be in `brd schema`; the other outputs of `brd` keep
`"schema": 2`.

#### Scenario: The plan since the review
- **WHEN** `brd review-plan` runs on a marked branch
- **THEN** it prints the branch, `base`, `head` and the `interdiff` with `"schema": 41`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists the output of `brd review-plan`, and the output matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

#### Scenario: The versions of the schema
- **WHEN** a reader of an older plan reads the versions
- **THEN** version 3 has `files` in place of `risk` and `usages` in place of `fan_in`, version 4 the `units` ([Review Units](../review-units/spec.md)), version 5 the `generated` files with why, the lock files among them, version 6 a deletion as real work (no `mechanical: "deleted"`; the units `deletedFile` and `deletedFiles`), version 7 the `concerns` ([Review Concerns](../review-concerns/spec.md)), version 8 their `elsewhere`, version 9 the name `file`, version 10 the `steps` and the `brief` ([Review Tour](../review-tour/spec.md)), version 11 the `api` and `test` of a unit ([Review Units](../review-units/spec.md)) in place of its `public`, version 12 the `still_bound` and `by_name` usages and `unbroken` in place of `updated`, version 13 the `reached` and `unreached` tests of a step ([Review Tour](../review-tour/spec.md)) in place of `named` and `unnamed`, version 14 the story of each commit once (`steps.commits`, [Review Tour](../review-tour/spec.md#the-story-of-a-commit)) in place of the `body` of a commit of a step, and the `newApi` and `moduleLinks` of a step, version 15 the `parts` of a story where they are `told` in place of its `toldIn`, the `joined` units and the `marks` of a stop, the `shownElsewhere` units of a step, the state `reworded`, the `message_change` of a commit, the `change` of a story and the `removals` of its parts, version 16 the `paragraphs` of an entry of the brief, whole and changed by word, with their `links` and `excerpt`, in place of its `added` and `removed`, and its `blocks` in place of its `block` ([Review Tour](../review-tour/spec.md#brief)); version 19 added repeated replacements to the brief, version 20 a widening as an `added` entry with `widened`, the reach in the forms and the `verdict`, version 21 the form `row` of a paragraph of the brief, the `excerpt` of every changed paragraph of several sentences and no paragraph of front matter or metadata ([Review Tour](../review-tour/spec.md#brief)), version 22 the `table` block of a body ([Commit Row](../../commit/commit-row/spec.md#message-body)) and its `summary`, version 23 the `modules` of the changed files ([Modules](#modules)), the `module` of a file and the `moduleEdges` of a step in place of its `moduleLinks`, version 24 the `groups` of a large step ([Review Tour](../review-tour/spec.md#groups-of-a-step)) in place of the `concern` of a stop, version 25 the `registrations` ([Registrations](#registrations)), version 26 the `texts` of the UI ([UI Texts](#ui-texts)), version 27 the `folder` of each file ([Modules](#modules)), version 28 the `tested` of each group of a large step ([Groups of a Step](../review-tour/spec.md#groups-of-a-step)), version 29 the `entries` of a module the change adds ([Modules](#modules)), version 30 the `title` of a step, of a commit of a step and of a story as `{prefix, text}`, version 31 the forms `before` and `after` of the `api` with each parameter type as the source writes it and a Java parameter as Java writes it, version 32 the `whitespace` of a file of a grammar by its tokens ([Mechanical Changes](#requirement-mechanical-changes-whitespace-is-told-by-the-tokens-of-a-grammar)) and the fingerprints of the units by them ([Review Units](../review-units/spec.md#identity)), version 33 the mechanical kind `moved` of a unit, the declarations that stand at another place of their file with the same code, folded where their place has no meaning ([Review Units](../review-units/spec.md#requirement-units-a-pair-is-shape-behaviour-moved-or-whitespace)), version 34 the `change` of a concern, what it does to the declaration of its name ([Review Concerns](../review-concerns/spec.md)), version 35 the `reading` of the pair, what it leaves to read by the role of each file, the files no proof can be made of and the proofs that passed a change ([Range Reading](../../review/range-reading/spec.md)), version 36 the `toReadProven` and `filesToReadProven` of each role of the `reading`, what is left to read in the files it proves, version 37 the `renamed` of a unit `behaviour` whose body changed in renamed names alone that the index has not proven ([Review Units](../review-units/spec.md#requirement-units-a-body-changed-in-a-rename-the-index-has-not-proven-says-so)), version 38 the `renamed_from` of a declaration renamed where the index has not proven it, one unit in place of a removed and an added one ([Review Units](../review-units/spec.md#requirement-units-a-declaration-renamed-where-the-index-has-not-proven-it-is-one-unit)), version 39 the `moved_from` of an `added` declaration and the `moved_to` of a `removed` one whose code the commits moved whole between two files, still two units ([Review Units](../review-units/spec.md#requirement-units-a-declaration-moved-whole-to-another-file-says-where)), version 40 the `unit` of each of those places, the unit that shows it, and the `moved_out` of a slice that holds nothing but declarations moved whole to other files, read at their new places ([Review Units](../review-units/spec.md#requirement-units-the-place-a-move-left-is-nothing-to-read-where-it-holds-the-move-alone)), version 41 the generated files folded past the first 20 of each list, each with the count it left out ([Files](#requirement-files-the-plan-of-an-agent-folds-the-generated-files))
