## MODIFIED Requirements

### Requirement: Steps of the Commits: Rust titles each commit without the issue key of the brief

Rust MUST title every commit it names (`StepCommit.title`, `StoryCommit.title`) and a step of the commits by the
commit that starts it (`ReviewStep.title`): its subject without the `(prefix)` and the issue key the brief names, in
the case its author wrote it, never capitalised, its conventional prefix apart (`ReviewTitle.prefix`), so the tree,
the contents, the opening and the bar write it alike: the prefix dimmed in the code font, no link inside the button.

#### Scenario: A subject of a prefix and the issue key of the brief
- **WHEN** a subject has a `(prefix)` and the issue key the brief names, with a `:`, `,` or dash after it
- **THEN** its title is the subject without them, in the case its author wrote it, never capitalised
  [@test] crates/review-plan/src/steps_test.rs (`a_commit_is_titled_by_its_subject_without_the_issue_key_of_the_brief_in_the_case_of_its_author`)

#### Scenario: A subject that starts with several keys
- **WHEN** the subject starts with several keys, the key of the brief among them
- **THEN** that key goes, the others kept: `IJAI-1184 IJAI-5640 Air records…` is `IJAI-1184 Air records…`
  [@test] crates/review-plan/src/steps_test.rs (`a_commit_is_titled_by_its_subject_without_the_issue_key_of_the_brief_in_the_case_of_its_author`)

#### Scenario: A subject with a conventional prefix
- **WHEN** what is left of a subject starts with a conventional prefix (`cleanup platform:`, `fix(vm)!:`) and text
  after it
- **THEN** Rust gives the prefix with its colon apart from the text; a prefix alone or a capitalised word before a
  colon is no prefix
- **AND** the tree, the contents, the opening and the bar of the step set the prefix dimmed in the code font and the
  text in the font of the text, with no link
  [@test] crates/review-plan/src/steps_test.rs (`a_commit_is_titled_by_its_subject_without_the_issue_key_of_the_brief_in_the_case_of_its_author`)
  [@test] src/presenters/review-steps.test.ts (`keeps the prefix Rust split off the title of a commit apart, the label of the step the title as one line`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should set the prefix of a commit title apart, dimmed in the code font, in the tree, the contents, the opening and the bar alike`)

#### Scenario: The title of a commit in `brd review-plan`
- **WHEN** `brd review-plan` prints the story of a commit
- **THEN** its title is the one Rust gives it (`StoryCommit.title`), its `prefix` and its `text`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Requirement: Anchors: A click on a name shows its place as an entry of the overview does

A click on a link of a name MUST show it as an entry of the overview does, its file selected, a declaration at the
line of its name in the section of its unit (`ReviewPlace.unit`), never in another section of its file that shows
that line, the name Rust resolved (`MentionTarget.name`) selected, a file at its header.

#### Scenario: A click on a name of a declaration and of a file
- **WHEN** the user clicks the link of a declaration
- **THEN** its file is selected, the diffs at the line of its name, the name Rust resolved selected
- **AND** the link of a file shows it at its header
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)

#### Scenario: A name of a declaration whose file is cut into two sections
- **WHEN** the review cuts the file of a declaration into two sections and the user clicks the link of its name
- **THEN** the name is selected in the section that shows the change of its unit, not in the other that shows its line
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show a named declaration in the section of its unit when its file is cut into two`)
