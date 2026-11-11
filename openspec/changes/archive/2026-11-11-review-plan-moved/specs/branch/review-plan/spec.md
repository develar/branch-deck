## MODIFIED Requirements

### Requirement: Since Your Review: brd review-plan prints the interdiff and its schema version

`brd review-plan` MUST print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`, `commits`
with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, `message_change`, and `scope` with `base`,
`head` and `carried`), with `"schema": 33`, and its output MUST be in `brd schema`; the other outputs of `brd` keep
`"schema": 2`.

#### Scenario: The plan since the review
- **WHEN** `brd review-plan` runs on a marked branch
- **THEN** it prints the branch, `base`, `head` and the `interdiff` with `"schema": 33`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists the output of `brd review-plan`, and the output matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

#### Scenario: The versions of the schema
- **WHEN** a reader of an older plan reads the versions
- **THEN** version 3 has `files` in place of `risk` and `usages` in place of `fan_in`, version 4 the `units` ([Review Units](../review-units/spec.md)), version 5 the `generated` files with why, the lock files among them, version 6 a deletion as real work (no `mechanical: "deleted"`; the units `deletedFile` and `deletedFiles`), version 7 the `concerns` ([Review Concerns](../review-concerns/spec.md)), version 8 their `elsewhere`, version 9 the name `file`, version 10 the `steps` and the `brief` ([Review Tour](../review-tour/spec.md)), version 11 the `api` and `test` of a unit ([Review Units](../review-units/spec.md)) in place of its `public`, version 12 the `still_bound` and `by_name` usages and `unbroken` in place of `updated`, version 13 the `reached` and `unreached` tests of a step ([Review Tour](../review-tour/spec.md)) in place of `named` and `unnamed`, version 14 the story of each commit once (`steps.commits`, [Review Tour](../review-tour/spec.md#the-story-of-a-commit)) in place of the `body` of a commit of a step, and the `newApi` and `moduleLinks` of a step, version 15 the `parts` of a story where they are `told` in place of its `toldIn`, the `joined` units and the `marks` of a stop, the `shownElsewhere` units of a step, the state `reworded`, the `message_change` of a commit, the `change` of a story and the `removals` of its parts, version 16 the `paragraphs` of an entry of the brief, whole and changed by word, with their `links` and `excerpt`, in place of its `added` and `removed`, and its `blocks` in place of its `block` ([Review Tour](../review-tour/spec.md#brief)); version 19 added repeated replacements to the brief, version 20 a widening as an `added` entry with `widened`, the reach in the forms and the `verdict`, version 21 the form `row` of a paragraph of the brief, the `excerpt` of every changed paragraph of several sentences and no paragraph of front matter or metadata ([Review Tour](../review-tour/spec.md#brief)), version 22 the `table` block of a body ([Commit Row](../../commit/commit-row/spec.md#message-body)) and its `summary`, version 23 the `modules` of the changed files ([Modules](#modules)), the `module` of a file and the `moduleEdges` of a step in place of its `moduleLinks`, version 24 the `groups` of a large step ([Review Tour](../review-tour/spec.md#groups-of-a-step)) in place of the `concern` of a stop, version 25 the `registrations` ([Registrations](#registrations)), version 26 the `texts` of the UI ([UI Texts](#ui-texts)), version 27 the `folder` of each file ([Modules](#modules)), version 28 the `tested` of each group of a large step ([Groups of a Step](../review-tour/spec.md#groups-of-a-step)), version 29 the `entries` of a module the change adds ([Modules](#modules)), version 30 the `title` of a step, of a commit of a step and of a story as `{prefix, text}`, version 31 the forms `before` and `after` of the `api` with each parameter type as the source writes it and a Java parameter as Java writes it, version 32 the `whitespace` of a file of a grammar by its tokens ([Mechanical Changes](#requirement-mechanical-changes-whitespace-is-told-by-the-tokens-of-a-grammar)) and the fingerprints of the units by them ([Review Units](../review-units/spec.md#identity)), version 33 the mechanical kind `moved` of a unit, the declarations that stand at another place of their file with the same code, folded where their place has no meaning ([Review Units](../review-units/spec.md#requirement-units-a-pair-is-shape-behaviour-moved-or-whitespace))

### Requirement: Modules: A module the commits add knows what enters it

A module the commits add MUST know what enters it (`entries`, `"schema": 33`): its declarations something outside it
uses, a class a plugin descriptor registers first (`registered`: the extension point and what it is registered for,
the interface of a service), then those the code of other modules uses (`usedBy`, each file at its first use, a
descriptor's own file aside); a use by the name alone or from test code MUST enter nothing.

#### Scenario: A new module used from outside
- **GIVEN** the commits add a module whose declarations a plugin descriptor registers and the code of other modules uses
- **WHEN** the plan is read
- **THEN** its `entries` list first the classes the descriptor registers (`registered`), then the declarations other modules use (`usedBy`, each file at its first use)
  [@test] crates/review-plan/src/groups_test.rs (`a_new_module_names_what_enters_it_and_its_group_starts_with_their_files`)

#### Scenario: A use by the name alone or from test code
- **WHEN** a declaration of the new module is used only by its name, or only from test code
- **THEN** it enters nothing

### Requirement: In the Review Window: A mechanical change starts collapsed and says why

The diffs MUST be the story of the units ([Review Units](../review-units/spec.md#in-the-review-window)): a mechanical
change starts collapsed, its header saying why ("Build file", "Imports only", "Whitespace only", "Moved only",
"Renamed only"; a generated file what its name makes it, "Lock file", "Minified", "Source map", "Generated (.gitattributes)", or the line
its tool wrote, cut at 100 characters). The chevron of the header opens it.

#### Scenario: A generated file
- **WHEN** the diffs show a generated file
- **THEN** it starts collapsed, its header saying why: what its name makes it ("Lock file", "Minified", "Source map"), "Generated (.gitattributes)", or the line its tool wrote, "Generated by tools/render.mjs; edit src/guide.md", cut at 100 characters
- **AND** a rendering stands at the end of the concern of its source
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)
  [@test] src/presenters/review-units.test.ts (`fold each with why it is generated: its name, its attributes, the line its tool wrote`)
  [@test] src/presenters/review-plan.test.ts (`says why a generated file is folded: its name, its attributes, the line its tool wrote, a long one cut`)

#### Scenario: Another mechanical change
- **WHEN** the diffs show a build file, a change of the imports or of whitespace only, declarations that moved, or a rename
- **THEN** it starts collapsed, its header saying "Build file", "Imports only", "Whitespace only", "Moved only" or "Renamed only"
- **AND** the declarations that moved are one group over their files, headed "Moved" with the count of its files
- **AND** the chevron of the header opens it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] src/presenters/review-plan.test.ts (`names the Mechanical group and its groups by kind`)
  [@test] src/presenters/review-units.test.ts (`folds the declarations that moved as one group over their files, headed Moved with their count, each file saying Moved only`)
