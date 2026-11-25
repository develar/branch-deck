## MODIFIED Requirements

### Requirement: Fan-in: The plan says once whether a use left outside breaks

The plan MUST say once whether a use left outside breaks (`verdict`), over its removed and changed entries: `breaks` when
one has a use left outside that can break, whatever the others; else `unknown` when the `usages` of one are null or not
`unbroken` (unknown, cut, tied by the name alone); else `noUseBreaks`; `pending` in the plan the window gets while the
index is asked. `brd review-plan` MUST name the entries that decide it (`verdict_by`, by their place in `api`).

#### Scenario: The verdict of a plan
- **WHEN** the plan has removed and changed entries
- **THEN** its `verdict` is `breaks` when one has a use left outside that can break, else `unknown` when the usages of one are unknown, cut or tied by the name alone, else `noUseBreaks`
- **AND** `verdict_by` names the entries with a use that can break, else those whose usages are unknown, cut or tied
  by the name alone, none for `noUseBreaks`
  [@test] crates/review-plan/src/api_test.rs (`the_verdict_says_whether_a_use_left_outside_breaks`, `the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

#### Scenario: An API of additions and widenings alone
- **WHEN** the API of the plan holds only additions and widenings
- **THEN** the `verdict` is `noUseBreaks`
  [@test] crates/review-plan/src/api_test.rs (`a_widening_alone_is_an_addition_that_breaks_nothing`)

#### Scenario: While the index is asked
- **WHEN** the window gets the plan before the index answered
- **THEN** its `verdict` is `pending`

### Requirement: Since Your Review: brd review-plan prints the interdiff and its schema version

`brd review-plan` MUST print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`, `commits`
with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, `message_change`, and `scope` with `base`,
`head` and `carried`), with `"schema": 42`, and its output MUST be in `brd schema`; the other outputs of `brd` keep
`"schema": 2`.

#### Scenario: The plan since the review
- **WHEN** `brd review-plan` runs on a marked branch
- **THEN** it prints the branch, `base`, `head` and the `interdiff` with `"schema": 42`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists the output of `brd review-plan`, and the output matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

#### Scenario: The versions of the schema
- **WHEN** a reader of an older plan reads the versions
- **THEN** version 3 has `files` in place of `risk` and `usages` in place of `fan_in`, version 4 the `units` ([Review Units](../review-units/spec.md)), version 5 the `generated` files with why, the lock files among them, version 6 a deletion as real work (no `mechanical: "deleted"`; the units `deletedFile` and `deletedFiles`), version 7 the `concerns` ([Review Concerns](../review-concerns/spec.md)), version 8 their `elsewhere`, version 9 the name `file`, version 10 the `steps` and the `brief` ([Review Tour](../review-tour/spec.md)), version 11 the `api` and `test` of a unit ([Review Units](../review-units/spec.md)) in place of its `public`, version 12 the `still_bound` and `by_name` usages and `unbroken` in place of `updated`, version 13 the `reached` and `unreached` tests of a step ([Review Tour](../review-tour/spec.md)) in place of `named` and `unnamed`, version 14 the story of each commit once (`steps.commits`, [Review Tour](../review-tour/spec.md#the-story-of-a-commit)) in place of the `body` of a commit of a step, and the `newApi` and `moduleLinks` of a step, version 15 the `parts` of a story where they are `told` in place of its `toldIn`, the `joined` units and the `marks` of a stop, the `shownElsewhere` units of a step, the state `reworded`, the `message_change` of a commit, the `change` of a story and the `removals` of its parts, version 16 the `paragraphs` of an entry of the brief, whole and changed by word, with their `links` and `excerpt`, in place of its `added` and `removed`, and its `blocks` in place of its `block` ([Review Tour](../review-tour/spec.md#brief)); version 19 added repeated replacements to the brief, version 20 a widening as an `added` entry with `widened`, the reach in the forms and the `verdict`, version 21 the form `row` of a paragraph of the brief, the `excerpt` of every changed paragraph of several sentences and no paragraph of front matter or metadata ([Review Tour](../review-tour/spec.md#brief)), version 22 the `table` block of a body ([Commit Row](../../commit/commit-row/spec.md#message-body)) and its `summary`, version 23 the `modules` of the changed files ([Modules](#modules)), the `module` of a file and the `moduleEdges` of a step in place of its `moduleLinks`, version 24 the `groups` of a large step ([Review Tour](../review-tour/spec.md#groups-of-a-step)) in place of the `concern` of a stop, version 25 the `registrations` ([Registrations](#registrations)), version 26 the `texts` of the UI ([UI Texts](#ui-texts)), version 27 the `folder` of each file ([Modules](#modules)), version 28 the `tested` of each group of a large step ([Groups of a Step](../review-tour/spec.md#groups-of-a-step)), version 29 the `entries` of a module the change adds ([Modules](#modules)), version 30 the `title` of a step, of a commit of a step and of a story as `{prefix, text}`, version 31 the forms `before` and `after` of the `api` with each parameter type as the source writes it and a Java parameter as Java writes it, version 32 the `whitespace` of a file of a grammar by its tokens ([Mechanical Changes](#requirement-mechanical-changes-whitespace-is-told-by-the-tokens-of-a-grammar)) and the fingerprints of the units by them ([Review Units](../review-units/spec.md#identity)), version 33 the mechanical kind `moved` of a unit, the declarations that stand at another place of their file with the same code, folded where their place has no meaning ([Review Units](../review-units/spec.md#requirement-units-a-pair-is-shape-behaviour-moved-or-whitespace)), version 34 the `change` of a concern, what it does to the declaration of its name ([Review Concerns](../review-concerns/spec.md)), version 35 the `reading` of the pair, what it leaves to read by the role of each file, the files no proof can be made of and the proofs that passed a change ([Range Reading](../../review/range-reading/spec.md)), version 36 the `toReadProven` and `filesToReadProven` of each role of the `reading`, what is left to read in the files it proves, version 37 the `renamed` of a unit `behaviour` whose body changed in renamed names alone that the index has not proven ([Review Units](../review-units/spec.md#requirement-units-a-body-changed-in-a-rename-the-index-has-not-proven-says-so)), version 38 the `renamed_from` of a declaration renamed where the index has not proven it, one unit in place of a removed and an added one ([Review Units](../review-units/spec.md#requirement-units-a-declaration-renamed-where-the-index-has-not-proven-it-is-one-unit)), version 39 the `moved_from` of an `added` declaration and the `moved_to` of a `removed` one whose code the commits moved whole between two files, still two units ([Review Units](../review-units/spec.md#requirement-units-a-declaration-moved-whole-to-another-file-says-where)), version 40 the `unit` of each of those places, the unit that shows it, and the `moved_out` of a slice that holds nothing but declarations moved whole to other files, read at their new places ([Review Units](../review-units/spec.md#requirement-units-the-place-a-move-left-is-nothing-to-read-where-it-holds-the-move-alone)), version 41 the generated files folded past the first 20 of each list, each with the count it left out ([Files](#requirement-files-the-plan-of-an-agent-folds-the-generated-files)), version 42 the `verdict_by` of the plan, the entries of `api` that decide its `verdict`
