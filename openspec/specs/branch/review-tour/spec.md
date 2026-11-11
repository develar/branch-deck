---
name: Review Tour
description: Requirements for the story of a review (experimental), one document that a reviewer who sees a change for the first time reads from its overview into each step's opening above its diffs; the brief (the issue the commits name, the rules of the specs and the prose of the docs the change adds, with the tests their test links name) and the steps, a step per commit whose body tells it, its follow-ups and the commits without a body where their lines landed, of one commit whose message tells its parts by the code they name a step per part, else the concerns, each with the parts of the units that land in master in its order, a declaration it adds first and its uses after it, and the tests of the change that reach its code through its callers; every block of the diff in one stop; the story of each commit once, its code names resolved to the units and the files.
style: plain-1
targets:
  - crates/review-plan/src/steps.rs
  - crates/review-plan/src/message_steps.rs
  - crates/review-plan/src/story.rs
  - crates/review-plan/src/brief.rs
  - crates/review-plan/src/prose.rs
  - crates/review-plan/src/interdiff.rs
  - crates/review-plan/src/lib.rs
  - crates/bd-rewrite/src/review_plan_command.rs
  - src/presenters/review-steps.ts
  - src/presenters/review-story.ts
  - src/components/review/ReviewPrologue.tsx
  - src/components/review/ReviewMap.tsx
  - src/presenters/review-map.ts
  - src/components/review/ContractList.tsx
  - crates/review-plan/src/groups.rs
  - src/components/review/ChapterOpening.tsx
  - src/components/review/ReviewTitleText.tsx
  - src/components/review/GroupOpening.tsx
  - src/components/review/ContractParagraphView.tsx
  - src/components/shared/ReadingColumn.tsx
  - src/components/review/StoryBody.tsx
  - src/presenters/commit-body.ts
  - src/components/review/StepFactsLine.tsx
  - src/components/review/StoryText.tsx
  - src/hooks/useStoryHighlight.ts
  - src/components/commit/CommitReview.tsx
  - src/components/diff/CommitReviewCode.tsx
  - src/components/review/RangeReview.tsx
  - src/components/files/FileTree.tsx
  - src/components/commit/CommitRow.tsx
  - src/components/commit/CommitSubject.tsx
  - src/lib/file-tree.ts
  - src/lib/range-review.ts
  - src/presenters/review-units.ts
  - src/lib/diff/rows.ts
  - src/lib/diff/layout.ts
  - src/components/diff/DiffView.tsx
  - src/components/diff/DiffFindBar.tsx
  - src/lib/diff/find.ts
  - src/lib/diff/find-text.ts
  - src/components/commit/CommitBodyView.tsx
  - src/components/shared/Disclosure.tsx
  - src/components/diff/diff-view.css
  - src/hooks/useSelectReviewFile.ts
  - src/stores/review-seen.ts
  - src/lib/review-position.ts
  - src/components/diff/diff-painter.ts
---

# Review Tour

Status: Experimental
Date: 2026-10-31

## Purpose

A developer who opens a change for the first time asks what changes for the user, why, how, whether it is proven,
what can break and what is noise, in this order. The review window answered the last two first and the first two
never: it opened on a card of a constructor whose uses no change broke, then on line 485 of a file, while the commit
bodies that told the whole story were four truncated subjects in the sidebar. Units and concerns had changed the order
of the file cards and the folders of the tree, not the primitive, which stayed the file.

The story is the review's primitive: a brief, then the steps. The brief says, in the words of the author, what the
change does for its user: the issue its commits name, the rules of the specs, the records of decisions, the guides
and the docs it adds or changes, and the tests their test links name. A step is a part of the change with the
words of its author: a commit whose body tells it, with the rounds that follow it up; each step shows the parts of the
units that land in master, a declaration it adds first and the uses of it after it, and the tests that reach its
code, those the change adds or changes and those it did not touch. Reviews differ: a commit alone whose paragraphs name the code they tell is told by its paragraphs; where the commits tell no story, the concerns of the range are the steps. The vision
and the rounds: [Change Review](../../../../docs/design/change-review.md).

## Goals

- The words of the author before any diff: the bodies of the commits, the rules of the specs, the docs.
- Each change once, as it lands: a step shows the units of the range, never a state a later commit replaced.
- Every block of the diff in one stop, checked: the coverage is a field of the plan.
- No git process of its own: the plan read every commit and its hunks already.

## Non-goals

- A tour a language model writes, or one the author writes with `brd tour`: the next round
  ([Change Review → Next](../../../../docs/design/change-review.md#next)).

## Concepts

**Groups of a Step.** A large step is read as its parts, not as a list of files: a commit of 125 files across four modules (idea-1
IJAI-5083) listed them by name and opened on a generated list.

**Tests That Reach It.** The question a step answers is which tests of the change prove it, not which tests of the repository its code is
reachable from: that is the impact of the change, `brd impact`'s.

**The Story of a Commit.** How to write a body this story reads well: [The Commit Body a Review Reads](../../../../docs/guide/commit-bodies.md).

**The Map.** The shape of the change on one screen before any of its words (`ReviewMap`, `presentMap` of `ReviewPlan.modules`):
the reviewer of a pull request of 141 files read eight screens of prose before the first diff and could not see that
it added a module.

## Requirements

### Requirement: Where a Line Comes From: Each line names the commit that wrote or removed it

The lines of each changed file MUST be carried through the hunks of the commits that change it, oldest first, so that
each line at the head names the commit that wrote it last and each line of the base the commit that removed it,
across a rename too.

#### Scenario: Lines carried through the commits of a range
- **GIVEN** a range whose commits change a file, one after the other, a rename among them
- **WHEN** the plan carries its lines through their hunks, oldest first
- **THEN** each line at the head names the commit that wrote it last
- **AND** each line of the base names the commit that removed it
  [@test] crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)

### Requirement: Where a Line Comes From: A block belongs to the commits of its changed lines

A block of the range MUST belong to the commits of its changed lines; a blank line MUST count for none.

#### Scenario: A block of lines of two commits and a blank line
- **WHEN** a block of the range holds changed lines of two commits and a blank line
- **THEN** it belongs to those two commits
- **AND** the blank line counts for none

### Requirement: Steps of the Commits: A commit with a body of 3 words or more tells a step

A commit whose message has a body of 3 words or more after its subject, its trailers left out, and no `fixup!`,
`squash!` or `amend!` subject, SHALL tell a step.

#### Scenario: A body of a few words tells a step, a fixup none
- **WHEN** a commit has a body of 3 words or more after its subject, its trailers left out
- **THEN** it tells a step
- **AND** a body of only `Co-Authored-By:` tells nothing
- **AND** a commit of a `fixup!`, `squash!` or `amend!` subject tells none
  [@test] crates/review-plan/src/steps_test.rs (`a_body_of_a_few_words_tells_a_step_a_fixup_none`)

### Requirement: Steps of the Commits: The steps of a range are the commits that tell one

When a range of two commits or more has a commit that tells a step, the steps MUST be the commits: each unit of the
range in the step of the commit of most of its changed lines among the commits that tell a step, the later of equal
ones; a unit of a file of no language by its blocks.

#### Scenario: Each unit in the step of the commit of most of its lines
- **GIVEN** a range of two commits or more, some of which tell a step
- **WHEN** the plan cuts its steps
- **THEN** the steps are the commits that tell a step
- **AND** each unit of the range is in the step of the commit of most of its changed lines among them, the later of
  equal ones
  [@test] crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)

#### Scenario: A file of no language
- **WHEN** a file of no language is changed by the commits of two steps
- **THEN** its unit goes to the steps by its blocks
  [@test] crates/review-plan/src/steps_test.rs (`a_file_of_no_language_is_cut_into_the_steps_of_its_blocks`)

### Requirement: Steps of the Commits: A step lists the other steps that changed its lines

A step SHALL list the other steps whose commits changed lines of it (`also`).

#### Scenario: A unit two steps' commits changed
- **WHEN** the commits of another step changed lines of a step
- **THEN** the step lists that other step (`also`)
  [@test] crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)

### Requirement: Steps of the Commits: A round follows up the step whose code it changes

A commit that tells a step MUST follow up the step of an earlier one (`followsUp`) by its lines only when half of its
changed lines of code on the old side or more were written by the commit that starts that step, and it owns no
non-mechanical unit that step did not change.

#### Scenario: A round that rewrites the lines of a step
- **WHEN** half of the changed lines of code on the old side of a commit or more were written by the commit that
  starts an earlier step, and it owns no non-mechanical unit that step did not change
- **THEN** it follows up that step (`followsUp`)
  [@test] crates/review-plan/src/steps_test.rs (`a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)

#### Scenario: A commit that owns a unit the step did not change
- **WHEN** such a commit owns a non-mechanical unit the earlier step did not change, as a declaration of its own it
  removes
- **THEN** it starts its own step, though it rewrites a line of the other
  [@test] crates/review-plan/src/steps_test.rs (`a_commit_that_removes_a_declaration_of_its_own_starts_its_step_though_it_rewrites_a_line_of_another`)

### Requirement: Steps of the Commits: A commit of a mechanical unit or rewritten whole follows up

A commit that tells a step SHALL follow up too when it owns no unit but a mechanical one, joining the step that holds
most of its lines, and when a later commit rewrote all it wrote, joining the step of the commit that rewrote the most
of it. A chain of follow-ups MUST end at a step.

#### Scenario: A commit of a mechanical unit alone
- **WHEN** a commit owns no unit but a mechanical one
- **THEN** it follows up, joining the step that holds most of its lines

#### Scenario: A commit a later commit rewrote whole
- **WHEN** a later commit rewrote all a commit wrote
- **THEN** it follows up, joining the step of the commit that rewrote the most of it
- **AND** a chain of follow-ups ends at a step
  [@test] crates/review-plan/src/steps_test.rs (`a_commit_rewritten_whole_follows_the_step_of_the_commit_that_rewrote_it`)

### Requirement: Steps of the Commits: A commit with a story of its own starts its step

A commit whose body tells a story of its own (a lead and a labelled part, or more than 5 lines as the fold counts
them) MUST start its step however much of an earlier step's code it rewrites: its story, its Look closely included,
MUST NOT be folded under another step.

#### Scenario: A story of its own rewrites the code of another step
- **GIVEN** a commit whose body has a lead and a labelled part, or more than 5 lines as the fold counts them
- **WHEN** it rewrites the code of an earlier step, however much of it
- **THEN** it starts its step, its story, its Look closely included, never folded under another step
- **AND** a verification section such as `Tests:` or `Evidence:` is no labelled part and its lines do not count
- **AND** a Look closely is no labelled part either
  [@test] crates/review-plan/src/steps_test.rs (`a_commit_with_a_story_of_its_own_starts_a_step_though_it_rewrites_the_code_of_another`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should start a step at a commit with a story of its own though it rewrites the code of others, and name it in a step it took the code of, which shows its step`)

### Requirement: Steps of the Commits: A step none of whose code lands names who took it out

A step none of whose code lands MUST name the later commit that took out the most of what it wrote (`takenOutBy`):
"Nothing of it lands: step 5 took it out", "step 5" showing that commit's step, its title in the hover; a subject
inside the sentence does not read.

#### Scenario: A later commit took out what a step wrote
- **WHEN** none of the code of a step lands
- **THEN** it names the later commit that took out the most of what it wrote (`takenOutBy`)
- **AND** its facts read "Nothing of it lands: step 5 took it out", "step 5" showing that commit's step, its title in
  the hover, never its subject inside the sentence
  [@test] crates/review-plan/src/steps_test.rs (`a_commit_with_a_story_of_its_own_starts_a_step_though_it_rewrites_the_code_of_another`)
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should start a step at a commit with a story of its own though it rewrites the code of others, and name it in a step it took the code of, which shows its step`)

### Requirement: Steps of the Commits: A commit without a body is listed where its lines are

A commit without a body SHALL tell no step: it MUST be listed in the step that holds most of its lines (`noStory`),
and the units only such commits wrote MUST be the step `no-story`, last.

#### Scenario: A commit without a body among steps
- **WHEN** a commit of the range has no body
- **THEN** it tells no step and is listed in the step that holds most of its lines (`noStory`)
  [@test] crates/review-plan/src/steps_test.rs (`a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)

#### Scenario: Units only commits without a body wrote
- **WHEN** only commits without a body wrote some units
- **THEN** those units are the step `no-story`, last

### Requirement: Steps of the Commits: The commits of a step come in their order with their titles

The commits of a step MUST be the one that starts it, then those that follow it up, then those of no story, each with
its title; its body is told once, in the story of the commits ([The Story of a Commit](#the-story-of-a-commit)).

#### Scenario: A step of a commit, its rounds and commits of no story
- **WHEN** a step has the commit that starts it, follow-ups and commits of no story
- **THEN** its commits are the one that starts it, then those that follow it up, then those of no story, each with its
  title
- **AND** its body is told once, in the story of the commits
  [@test] crates/review-plan/src/steps_test.rs (`a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)

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

### Requirement: Steps of the Commits: The plan names what is reviewed

The plan MUST name what is reviewed (`ReviewPlan.title`): a branch of the prefix as the title bar does (`layout`),
another branch by its full name, a range by its short ids.

#### Scenario: A virtual branch, another branch, a range
- **WHEN** the plan is of a branch of the prefix
- **THEN** its title names it as the title bar does (`layout`)
- **AND** another branch is named by its full name, a range by its short ids
  [@test] crates/review-plan/src/api_test.rs (`since_the_review_the_api_is_that_of_the_diff_from_the_reviewed_change`)

### Requirement: Steps of the Commits: A mechanical part follows the step that wrote it

A mechanical part MUST follow the step of the commit that wrote it, at its end: the imports a step needs, the
generated files of its sources.

#### Scenario: Imports and generated files
- **WHEN** a commit writes the imports its step needs or the generated files of its sources
- **THEN** they follow the step of that commit, at its end
  [@test] crates/review-plan/src/steps_test.rs (`a_mechanical_change_follows_the_step_that_wrote_it`)

### Requirement: Steps of the Message: The parts of one commit that name its code are its steps

A range of one commit whose body tells two parts or more by the code they name MUST have those parts as its steps, in
the order of the body (`source`: `message`).

#### Scenario: A body of paragraphs that name the code of the commit
- **GIVEN** a range of one commit whose body tells two parts or more by the code they name
- **WHEN** the plan cuts its steps
- **THEN** those parts are its steps, in the order of the body (`source`: `message`)
  [@test] crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

### Requirement: Steps of the Message: A labelled section or a paragraph with its blocks tells a step

The parts that may tell a step SHALL be a labelled section (a label or a heading and its blocks, a verification
aside) and each paragraph, list or table of a run with the blocks written right under it; the lead, a Look closely,
code alone and a verification section MUST be none, but a `Tests:` section tells the step of the tests.

#### Scenario: A labelled section tells a step
- **WHEN** the body of the commit has a label or a heading and its blocks
- **THEN** that section may tell a step, a verification aside
  [@test] crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)

#### Scenario: A paragraph, a list or a table of a run with its blocks
- **WHEN** a paragraph, list or table has blocks written right under it (no blank line between)
- **THEN** it is one part with them: a list under its paragraph is one part
  [@test] crates/review-plan/src/message_steps_test.rs (`a_table_tells_a_step_titled_by_the_first_name_it_resolves`, `a_long_first_sentence_titles_its_step_by_the_first_name_it_resolves_and_a_list_under_a_paragraph_is_one_part`)

#### Scenario: Parts that tell no step
- **WHEN** the body has a lead, a Look closely, code alone or a verification section
- **THEN** none of them tells a step
- **AND** a `Tests:` section tells the step of the tests

### Requirement: Steps of the Message: A part claims the parts of the concerns its code names

A part MUST claim the parts of the concerns (a unit in a file) its code names resolve to, resolved before there are
steps: a declaration its parts, a file its parts in that file; a part two of them name MUST go to the labelled
section before a paragraph, a paragraph before the tests, then to the one that names it most, then to the earlier one.

#### Scenario: A declaration and a file a part names
- **WHEN** a part names a declaration and a file
- **THEN** it claims the parts of the declaration and the parts of the concerns in that file
  [@test] crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

#### Scenario: A concern part two parts name
- **WHEN** two parts name the same part of a concern
- **THEN** it goes to the labelled section before a paragraph, a paragraph before the tests
- **AND** then to the one that names it most, then to the earlier one

### Requirement: Steps of the Message: Fewer than two parts with a claim keep the concerns

Fewer than two parts of the story with a claim MUST keep the concerns as the steps.

#### Scenario: A body that tells no two parts by their code
- **WHEN** the body is one paragraph that names code, two sections that name the same code (the first keeps it), or
  a body of verification alone
- **THEN** fewer than two parts of the story have a claim, and the concerns stay the steps
  [@test] crates/review-plan/src/message_steps_test.rs (`the_concerns_stay_the_steps_of_a_body_that_tells_no_two_parts_by_their_code`)

### Requirement: Steps of the Message: A concern two parts name is cut by its own evidence

A concern one part names MUST go to it whole; a concern two parts name MUST be cut by its own evidence, each of its
parts to the nearest claimed one, of equal distance the part of the better claim, then the earlier one; a part no
evidence reaches MUST go to the part that claims most of the concern.

#### Scenario: A concern of one part's names
- **WHEN** one part names a concern
- **THEN** the concern goes to it whole
  [@test] crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

#### Scenario: A concern two parts name
- **WHEN** two parts name a concern
- **THEN** each of its parts goes to the nearest claimed one by its own evidence: a use the change adds at 1, a use it
  keeps, a rare token or what no evidence joined at 2, a repeated edit at 1
- **AND** of equal distance the part of the better claim, then the earlier one
- **AND** a part no evidence reaches goes to the part that claims most of the concern

### Requirement: Steps of the Message: No evidence is added to the parts

No evidence MUST be added: a concern no part names stays a step of its own after the parts, named by its evidence, a
concern of test code alone joins the step of the tests, and what no evidence joined goes part by part. Every block of
the diff MUST stay in one stop.

#### Scenario: Concerns no part names
- **WHEN** a concern no part names is left
- **THEN** it stays a step of its own after the parts, named by its evidence
- **AND** a concern of test code alone joins the step of the tests
- **AND** what no evidence joined goes part by part: to the part that names it, a test to the tests, the rest stays
  the rest, last
- **AND** every block of the diff stays in one stop
  [@test] crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

### Requirement: Steps of the Message: A step of the message is titled as its author wrote it

A step of the message MUST be titled as its author wrote it (`titleSource`): the label or the heading of its section
(`label`, told once, as its title); else the first sentence of its paragraph when it is 100 characters or fewer, its
backticks aside (`sentence`); else the first code name of the paragraph that resolves, set as code when it is an
identifier (`name`).

#### Scenario: A section of a label
- **WHEN** the part is a section of a label or a heading
- **THEN** that label is its title (`label`), told once, as its title
  [@test] crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

#### Scenario: A paragraph of a short first sentence
- **WHEN** the part is a paragraph whose first sentence is 100 characters or fewer, its backticks aside
- **THEN** that sentence is its title (`sentence`): up to a `.`, `?` or `!` outside backticks before a space or the
  end of the line, not after an abbreviation (`e.g.`), a single letter or a digit
  [@test] crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

#### Scenario: A paragraph of a long first sentence
- **WHEN** the first sentence of the paragraph is longer
- **THEN** the first code name of the paragraph that resolves is its title (`name`), set as code when it is an
  identifier
  [@test] crates/review-plan/src/message_steps_test.rs (`a_long_first_sentence_titles_its_step_by_the_first_name_it_resolves_and_a_list_under_a_paragraph_is_one_part`)

### Requirement: Steps of the Message: A name with spaces is a phrase, not code

A name with spaces MUST be a phrase, not code, whether it names a declaration concern or titles a step of the message.

#### Scenario: A title or a concern of a name with spaces
- **WHEN** a name with spaces names a declaration concern or titles a step of the message
- **THEN** it is set as a phrase, not as code
  [@test] src/presenters/review-steps.test.ts (`sets names with spaces as phrases, not code`)

### Requirement: Steps of the Message: The id of a step of the message is that of its first part

The id of a step of the message MUST be the fingerprint of its first part (its unit and file), so a reword that names
more of its code keeps it.

#### Scenario: A reword that names more of the code
- **WHEN** the paragraph of a step is reworded to name more of its code
- **THEN** the step keeps its id, the fingerprint of its first part
  [@test] crates/review-plan/src/message_steps_test.rs (`a_step_of_the_message_keeps_its_id_when_its_paragraph_names_more`)

### Requirement: Steps of the Message: Each part that tells a step is told in its opening

Of the steps of the message, each part that tells a step MUST be told in its opening, never folded there; a Look
closely in the step of the part before it; the lead, and the parts no step tells, by the rule of the concerns of one
commit ([The Story of a Commit](#the-story-of-a-commit)).

#### Scenario: The parts of a message of steps
- **WHEN** a range of one commit has the steps of its message
- **THEN** each part that tells a step is told in its opening, never folded there
- **AND** a Look closely is told in the step of the part before it
- **AND** the lead and the parts no step tells are told by the rule of the concerns of one commit
  [@test] crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

### Requirement: Steps of the Concerns: A range whose commits tell no story has the concerns as steps

A range of one commit whose body tells no two parts by their code and a range no commit of which tells a step MUST
have the concerns as its steps, each with the commits of its lines, the most lines first (`source`: `concerns` with
its `reason`, `oneCommit` or `noStory`).

#### Scenario: One commit and commits of no story
- **WHEN** a range is of one commit whose body tells no two parts by their code, or no commit of it tells a step
- **THEN** its steps are the concerns, each with the commits of its lines, the most lines first
- **AND** their `source` is `concerns`, its `reason` `oneCommit` or `noStory`
  [@test] crates/review-plan/src/steps_test.rs (`the_concerns_are_the_steps_when_the_commits_tell_no_story`)

### Requirement: Steps of the Concerns: A long branch is told by its commits too

A long branch SHALL be told by its commits too: its concerns were no shorter a list
([Measurements](#measurements)).

#### Scenario: A long branch whose commits tell their steps
- **WHEN** a long branch has commits that tell a step
- **THEN** its steps are the commits, not the concerns, which were no shorter a list

### Requirement: Order of a Step: The stops come in the order of the story, mechanical parts last

The stops of a step MUST come in the order of the story, its mechanical parts last.

#### Scenario: A step of code and mechanical parts
- **WHEN** a step holds code and mechanical parts
- **THEN** its stops come in the order of the story, its mechanical parts last
  [@test] crates/review-plan/src/steps_test.rs (`a_declaration_a_step_adds_comes_first_and_its_uses_right_after_it`)

### Requirement: Order of a Step: A declaration comes before its uses, which follow it

A declaration the step adds or changes the form of MUST come before the stops of the step whose changed lines name it
(in its file or in a file linked to it by that name), and those MUST follow it right after, each with `uses`, unless
more than 6 stops name it.

#### Scenario: A declaration and the stops that name it
- **WHEN** a step adds a declaration or changes its form, and other stops of the step name it in their changed lines,
  in its file or in a file linked to it by that name
- **THEN** the declaration comes first and those stops follow it right after, each with `uses`
  [@test] crates/review-plan/src/steps_test.rs (`a_declaration_a_step_adds_comes_first_and_its_uses_right_after_it`)

#### Scenario: A declaration more than 6 stops name
- **WHEN** more than 6 stops name the declaration
- **THEN** the declaration and those stops keep the order of the story, none of them with `uses`

### Requirement: Groups of a Step: The groups are a reading order inside a step

The groups MUST be a reading order inside a step its author told, never a task of their own (`groups::cut_groups`,
`ReviewStep.groups`, each a run of `start..end` of the stops).

#### Scenario: A large step read as its groups
- **WHEN** a large step is cut into groups
- **THEN** each group is a run of `start..end` of its stops (`ReviewStep.groups`), inside the step
- **AND** no group is a task of its own
  [@test] crates/review-plan/src/groups_test.rs (`a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring`)

### Requirement: Groups of a Step: A large step is cut by module, else by the folders of its module

A step of the commits or of the message MUST be cut by module when its stops of code, tests and resources lie in two
modules or more ([Review Plan → Modules](../review-plan/spec.md#modules)) and it holds more than 12 files, of which a
module the commits add or two modules of 5 files each; by the folders of its one module when it holds more than 25
files there.

#### Scenario: A step of more than 12 files in two modules or more
- **WHEN** the stops of code, tests and resources of a step lie in two modules or more and it holds more than 12
  files, of which a module the commits add or two modules of 5 files each
- **THEN** the step is cut by module
  [@test] crates/review-plan/src/groups_test.rs (`a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring`)

#### Scenario: A step of more than 25 files in one module
- **WHEN** a step holds more than 25 files of its one module
- **THEN** it is cut by the folders of that module: the first two directories below the root without a leading source
  directory, as the module's `directories`
  [@test] crates/review-plan/src/groups_test.rs (`a_large_step_of_one_module_reads_by_its_folders`)

### Requirement: Groups of a Step: A small step and a step of the concerns have no groups

A step that is neither cut by module nor by folders MUST have no groups; a step of the concerns MUST NOT be cut: its
evidence is its group.

#### Scenario: A small step
- **WHEN** a step of the commits or of the message is cut neither by module nor by folders
- **THEN** it has no groups
  [@test] crates/review-plan/src/groups_test.rs (`a_small_step_has_no_groups`)

#### Scenario: A step of the concerns
- **WHEN** a step is of the concerns, however large
- **THEN** it is never cut: its evidence is its group

### Requirement: Groups of a Step: The contract first, the modules by their uses, the mechanical last

The groups MUST come as `contract`, then a group per module or folder, then `mechanical` last; a module whose
declarations another module of the step newly uses MUST come before that one, the order of their first stops between
the others and inside a cycle.

#### Scenario: The groups of a large step in their order
- **WHEN** a large step is cut into groups
- **THEN** the first is `contract`: the stops of the entries of the brief and of the other files of prose of the step
- **AND** a group per module or folder follows, a module whose declarations another module of the step newly uses
  before that one, the order of their first stops between the others and inside a cycle
- **AND** `mechanical` comes last: the build model, the lists of modules, imports alone, whitespace, moved declarations,
  renames, generated files
  [@test] crates/review-plan/src/groups_test.rs (`a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring`, `the_modules_template_reads_as_its_contract_its_modules_and_its_wiring`)

### Requirement: Groups of a Step: A small module joins the group of the module that uses it

A module of fewer than 4 files of the step MUST join the group of the module of the step that newly uses it, which
names it (`also`).

#### Scenario: A module of 3 files another module newly uses
- **WHEN** a module holds fewer than 4 files of the step and another module of the step newly uses it
- **THEN** it joins the group of that module, which names it (`also`)
  [@test] crates/review-plan/src/groups_test.rs (`a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring`)

### Requirement: Groups of a Step: Inside a group the tests come after the code, each stop once

Inside a group the stops MUST keep the order of the step, its tests after its code (`tests`, the first of them).
Every stop MUST be in one group, once.

#### Scenario: Code and tests of a group
- **WHEN** a group holds code and tests
- **THEN** its stops keep the order of the step, its tests after its code, `tests` the first of them
- **AND** a test is a path in a `test`, `tests`, `testSrc` or `testFixtures` directory or with a mark of a test, or a
  unit of test code
- **AND** every stop is in one group, once
  [@test] crates/review-plan/src/groups_test.rs (`a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring`, `the_modules_template_reads_as_its_contract_its_modules_and_its_wiring`)

### Requirement: Groups of a Step: Each group knows the tests that reach its code

Each group MUST know the tests of the change that reach the code of its files and its code none reaches (`tested`, in
the form of the tests of a step, `"schema": 32`), from the same answer of accio as its step; a declaration MUST be of
the group of the first stop of its file outside the mechanical group.

#### Scenario: A test of one group that reaches the code of another
- **WHEN** a test of one group reaches the code of another group
- **THEN** it is that other group's, as a declaration is of the group of the first stop of its file outside the
  mechanical group
- **AND** the code of a small module a group holds is that group's
  [@test] crates/review-plan/src/groups_test.rs (`a_group_knows_the_tests_that_reach_its_code_and_its_code_none_reaches`)

#### Scenario: The contract and the wiring
- **WHEN** the step's tests are answered
- **THEN** the contract and the wiring have none, answered when the step is

### Requirement: Groups of a Step: The group of a new module reads from its entry points

The group of a module the commits add MUST read from its entry points once accio answers them (`entries`): the stops
of their files first, in the order of the entry points, the rest of its code in the order it had, its tests still
last. The files of the step (`paths`) MUST follow its stops.

#### Scenario: A new module whose entry points accio answers
- **WHEN** a group is of a module the commits add and accio answers its entry points (`entries`)
- **THEN** the stops of their files come first, in the order of the entry points
- **AND** the rest of its code keeps the order it had, its tests still last
- **AND** the files of the step (`paths`) follow its stops
  [@test] crates/review-plan/src/groups_test.rs (`a_new_module_names_what_enters_it_and_its_group_starts_with_their_files`)

### Requirement: Tests That Reach It: The code of a step is its changed declarations outside tests

The code of a step SHALL be each declaration of a parsed language whose body or form changed or that it adds, and of
a file it adds each declaration at its top (a new type with its members, as a new type of a changed file is one
unit), in no test file and no test code.

#### Scenario: A new file and a changed one
- **WHEN** a step adds a file and changes declarations of another
- **THEN** its code is each declaration whose body or form changed or that it adds, and each declaration at the top
  of the new file, a new type with its members
  [@test] crates/review-plan/src/steps_test.rs (`a_new_file_is_code_its_tests_are_the_changes_and_their_step_proves_the_other`)

#### Scenario: A test and a fixture of a test
- **WHEN** a step changes a test, a fixture of its class or of its Rust `mod tests`
- **THEN** that is test code, no code of the step: the unit is `test`
  [@test] crates/review-plan/src/steps_test.rs (`a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of`)

### Requirement: Tests That Reach It: A name two declarations of the code share is led by its owner

A name two declarations of the code of a step share MUST be led by its owner (`Owner.run`).

#### Scenario: Two declarations of one name
- **WHEN** two declarations of the code of a step share a name
- **THEN** each is named led by its owner (`Owner.run`)

### Requirement: Tests That Reach It: A test of the change is a test of a unit the range touches

A test of the change SHALL be a test function of a unit the range adds or changes, or of a file it adds (its unit the
file's).

#### Scenario: Tests of a new file and of a changed unit
- **WHEN** the range adds a test file and changes a test function of another
- **THEN** both are tests of the change, a test of the new file of its unit the file's
  [@test] crates/review-plan/src/steps_test.rs (`a_new_file_is_code_its_tests_are_the_changes_and_their_step_proves_the_other`)

### Requirement: Tests That Reach It: The code of the steps is asked of accio in one query

After the plan, with the usages of its API, every declaration of the code of the steps MUST be asked in one
`accio tests --at <path>:<line> --ref <head> --tests <file>… --resolved`, the test files of the tests of the change
given: the given tests that reach it through its callers at any depth, only by calls a type or an import ties.

#### Scenario: The declarations of the steps asked once
- **WHEN** the plan has its steps and the usages of its API
- **THEN** every declaration of the code of the steps is asked in one query, the test files of the tests of the
  change given
- **AND** a fixture of a test is no code a step asks the tests of
  [@test] crates/review-plan/src/steps_test.rs (`the_tests_that_reach_the_code_of_a_step_and_the_code_none_reaches`, `a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of`)

#### Scenario: The cost of the query
- **WHEN** a plan of a branch of a large monorepo asks its tests
- **THEN** the query costs it ~20 s until accio answers it fast ([#191](https://github.com/develar/branch-deck/issues/191))

### Requirement: Tests That Reach It: The tests of the change that reach a step are its proof

The tests of the change that reach the code of a step MUST be its `proof`, by their file, each test once with the
name it reads as, its fewest calls (`depth`), its nearest `way` and the declarations of the step it reaches
(`reaches`); a test of a given file the change did not touch MUST be no fact of the step.

#### Scenario: Tests that reach the code of a step
- **WHEN** tests of the change reach the code of a step
- **THEN** they are its `proof`, by their file, each with the name it reads as, its fewest calls (`depth`) and the
  declarations of the step it reaches (`reaches`)
- **AND** `reached` counts the declarations a test of the change reaches
- **AND** a test of a given file the change did not touch is no fact of the step
  [@test] crates/review-plan/src/steps_test.rs (`the_tests_that_reach_the_code_of_a_step_and_the_code_none_reaches`)

#### Scenario: A test of two ways
- **WHEN** a test reaches the code of a step by two ways
- **THEN** it is listed once, by its nearest `way`: `calls`, `testCode` through the code of tests only, `endToEnd`
  through product code (accio's `class`)
  [@test] crates/review-plan/src/steps_test.rs (`a_test_of_two_ways_is_listed_once_by_its_nearest`)

### Requirement: Tests That Reach It: Code is unreached only when accio walked every way

A declaration no test of the change reaches MUST be `unreached` only when accio walked every way (`complete`); one it
did not MUST be unknown, never "no test". A place accio finds no declaration at (`unavailable`: a line it does not
index) MUST be neither: accio answered nothing of it.

#### Scenario: A declaration no test reaches, every way walked
- **WHEN** no test of the change reaches a declaration and accio walked every way (`complete`)
- **THEN** it is `unreached`
  [@test] crates/review-plan/src/steps_test.rs (`the_tests_that_reach_the_code_of_a_step_and_the_code_none_reaches`)

#### Scenario: A declaration accio did not walk every way of, or found nothing at
- **WHEN** accio did not walk every way of a declaration
- **THEN** it is unknown, never "no test"
- **AND** a place accio finds no declaration at (`unavailable`) is neither unreached nor unknown

### Requirement: Tests That Reach It: A range without tests asks nothing

A range without tests MUST ask nothing: no test of the change reaches its code.

#### Scenario: A range of no test
- **WHEN** a range has no test of the change
- **THEN** the plan asks accio nothing and no test of the change reaches its code
  [@test] crates/review-plan/src/steps_test.rs (`a_range_without_tests_asks_nothing_and_no_test_of_it_reaches_its_code`)

### Requirement: Tests That Reach It: Without accio a step knows no tests and the note says why

`answered` MUST say the steps know their tests; without accio, or without a usable index, a step MUST have none of
them, and `index_note` says why.

#### Scenario: No accio or no usable index
- **WHEN** accio is missing or has no usable index
- **THEN** a step has none of its tests, and `index_note` says why
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Requirement: Tests That Reach It: A step names the steps its tests prove

A step whose units hold tests of the change that reach the code of other steps MUST name those steps (`proves`).

#### Scenario: Tests of one step that reach the code of another
- **WHEN** the units of a step hold tests of the change that reach the code of another step
- **THEN** the step names that step (`proves`)
  [@test] crates/review-plan/src/steps_test.rs (`a_new_file_is_code_its_tests_are_the_changes_and_their_step_proves_the_other`)

### Requirement: Tests That Reach It: A step of a rule of a spec lists the tests its links name

A step that adds or changes a rule of a spec MUST list the tests its test links name (`scenarios`): a scenario that
names no declaration proves it.

#### Scenario: A rule of a spec with its test links
- **WHEN** a step adds or changes a rule of a spec whose test links name tests
- **THEN** the step lists those tests (`scenarios`), a scenario that names no declaration proving it
  [@test] crates/review-plan/src/steps_test.rs (`the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`)

### Requirement: The Story of a Commit: Every commit with a body has its story once

Every commit of the range whose body has a block MUST have its story once (`commits`, oldest first), also when
several steps list it: its id, its subject without the `(prefix)`, its body by the grammar of a body
([Commit Row → Message body](../../commit/commit-row/spec.md)), its code names (`mentions`) and its parts as the
document tells them (`parts`). A commit of a step MUST name no body of its own.

#### Scenario: The story of one commit
- **WHEN** a commit of the range has a body with a block
- **THEN** its story is in `commits` once, oldest first, with its id, its subject without the `(prefix)`, its body by
  the grammar of a body and its code names (`mentions`)
- **AND** its `parts` are each part of the body, its `kind` and `tone`, `folded`, where it is `told` and whether its
  label is the title of its step, `titled`
  [@test] crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)

#### Scenario: A commit several steps list
- **WHEN** several steps list a commit, as the one that starts it and a follow-up or a commit of no story
- **THEN** its story is told once, in `commits`
- **AND** the commit of a step names no body of its own
  [@test] crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`, `a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)

### Requirement: The Story of a Commit: A step of the concerns is titled by the label it tells

The story MUST carry the title of each step of the concerns of one commit that tells a labelled section (`title`: the
label of the first one, a verification and a Look closely aside) with its `titleSource`: `label`; of a step of the
commits `commit`, of a concern no section names `evidence`.

#### Scenario: Steps of the concerns of one commit with labelled sections
- **WHEN** a step of the concerns of one commit tells a labelled section
- **THEN** its `title` is the label of the first one, a verification and a Look closely aside, its `titleSource`
  `label`
- **AND** the `titleSource` of a step of the commits is `commit`, of a concern no section names `evidence`
  [@test] crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)

### Requirement: The Story of a Commit: A code name resolves with no git process

A code name of a block that is no preformatted text MUST resolve to a unit or a changed file of the range, with no git
process and no query of the index.

#### Scenario: The names of a body resolved
- **WHEN** the story reads the code names of a body
- **THEN** each resolves to a unit or a changed file of the range, with no git process and no query of the index
  [@test] crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)

### Requirement: The Story of a Commit: A code name is a span that names code or a token of code

A code name SHALL be a span in backticks that names code and a token outside backticks the grammar of code finds
([Commit Row → Code tokens](../../commit/commit-row/spec.md#code-tokens)), so a name the page sets as code MUST be
the name the story links, a resolved one only adding its dotted underline.

#### Scenario: A span in backticks
- **WHEN** a span in backticks names code: a path, a file name, identifiers joined by `.`, `::` or `#`
- **THEN** it is a code name, a trailing `(…)` or `<…>` and a leading `@` aside
  [@test] crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)

#### Scenario: A token outside backticks
- **WHEN** the grammar of code finds a token outside backticks: `toolWindowId`, `snake_case`, `Owner.member` with a
  capitalised owner, a path, a file name
- **THEN** it is a code name, the one the page sets as code
  [@test] crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)

### Requirement: The Story of a Commit: A plain word and a command are no name

A plain lowercase word MUST be no name, in backticks neither, unless its call (`open()`); a span with a space, its
arguments aside (`open(path, mode)`), MUST be a command, no name.

#### Scenario: A plain word, a call and a command
- **WHEN** a body writes a plain lowercase word, in backticks or not
- **THEN** it is no name, unless its call (`open()`)
- **AND** a span with a space, its arguments aside (`open(path, mode)`), is a command, no name

### Requirement: The Story of a Commit: Each token of a run is a mention, a possessive is prose

A run of found tokens one space apart (`Frame.paint SplashScreen.hide`) that names nothing whole MUST be a mention of
each token that resolves, at its own range; a possessive or a dash after a name (`SplashScreen.hide's`,
`Frame.paint—`) MUST be prose.

#### Scenario: A run of names and a name before a possessive or a dash
- **WHEN** a run of found tokens one space apart names nothing whole
- **THEN** each token of it that resolves is a mention at its own range
- **AND** a possessive or a dash after a name is prose
  [@test] crates/review-plan/src/story_test.rs (`a_name_before_a_possessive_or_a_dash_and_each_name_of_a_run_is_a_mention`)

### Requirement: The Story of a Commit: A path resolves to its changed file, else to nothing

A path MUST resolve to the changed path itself, else the one it ends on at a `/`, else the one changed file of that
name (a `:171` after it aside); two files of that name MUST resolve nothing.

#### Scenario: A path, its end and a file name
- **WHEN** a body names a path
- **THEN** it resolves to the changed path itself, else the one it ends on at a `/`, else the one changed file of that
  name, a `:171` after it aside
- **AND** a name two changed files have resolves nothing
  [@test] crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)

### Requirement: The Story of a Commit: A declaration resolves by the segments of its language

A declaration MUST resolve by its segments: the units of a declaration, and the declarations of a new or a deleted
file by its summary (its unit is the file, the line that of the name, of the old side for a deleted file), whose name
is the last one, with an owner those whose key ends with all of them.

#### Scenario: The separators of each language
- **WHEN** a name has an owner: JVM `Owner.member`, Rust `module::Type::member`, TypeScript `Type#member`
- **THEN** it resolves to the declarations whose key ends with all of its segments, its name the last one
- **AND** a leading crate or package the key does not hold is aside, down to the owner and the name; `crate`, `self`,
  `super`, `Self` aside
  [@test] crates/review-plan/src/story_test.rs (`a_name_resolves_by_the_separators_of_its_language`)

#### Scenario: A declaration of a new or a deleted file
- **WHEN** a name is of a declaration of a new or a deleted file
- **THEN** it resolves by the summary of the file: its unit is the file, the line that of the name, of the old side
  for a deleted file
  [@test] crates/review-plan/src/story_test.rs (`a_name_resolves_by_the_separators_of_its_language`)

### Requirement: The Story of a Commit: One key resolves, of two the one of the commit's own step

One key MUST resolve (overloads are one name); of two keys or more, the one among the stops of the commit's own steps,
of two there the one that lands (not a removal), else nothing.

#### Scenario: A name two declarations share
- **WHEN** a name matches two keys or more
- **THEN** the one among the stops of the commit's own steps resolves, of two there the one that lands (not a removal)
- **AND** else the name resolves to nothing
  [@test] crates/review-plan/src/story_test.rs (`a_name_two_declarations_share_resolves_to_the_one_of_its_step_else_to_nothing`)

### Requirement: The Story of a Commit: A name in backticks resolves to the file named for it

Only a name in backticks MUST resolve to a file by its name: a capitalised name no declaration of the range has to the
file of the units declared inside it, when they are in one (a type whose members changed); a capitalised or compound
one to the one changed file named for it, its case, `-` and `_` aside.

#### Scenario: A type whose members changed
- **WHEN** a capitalised name in backticks is no declaration of the range and the units declared inside it are in one
  file
- **THEN** it resolves to that file

#### Scenario: A file named for the name
- **WHEN** a capitalised or compound name in backticks names one changed file, its case, `-` and `_` aside
- **THEN** it resolves to that file: `Session` of `Session.kt`, `useMeasuredHeights` of `use-measured-heights.ts`
  [@test] crates/review-plan/src/story_test.rs (`a_name_resolves_by_the_separators_of_its_language`)

### Requirement: The Story of a Commit: A bare word resolves to a declaration or a path alone

A bare word MUST resolve to a declaration or a path alone, so a product name in prose (`YouTrack`, `Node.js`) names
no file.

#### Scenario: A product name in prose
- **WHEN** prose names a product outside backticks, as `YouTrack` or `Node.js`
- **THEN** it names no file, resolving only to a declaration or a path
  [@test] crates/review-plan/src/story_test.rs (`a_name_resolves_by_the_separators_of_its_language`)

### Requirement: The Story of a Commit: A mention carries its place, its text and its target

A mention MUST carry its block, the line of its paragraph or the item of its list (0 of a heading or a label), its
UTF-16 range in it, the backticks included, its text, and its `target`. A bare name MUST be kept only when it
resolves; a name in backticks that resolves nothing has `target: null`, so the author sees what did not bind.

#### Scenario: The target of a unit and of a file
- **WHEN** a mention resolves
- **THEN** its `target` is a `unit` with its path, its side (`old` for a removed declaration), the line of its name
  and the step whose stops hold it, a step of the commit first
- **AND** a mention that resolves to a file has a `file` with its step as its target
  [@test] crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)

#### Scenario: A name that resolves nothing
- **WHEN** a name resolves nothing
- **THEN** a bare name is not kept
- **AND** a name in backticks is kept with `target: null`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Requirement: The Story of a Commit: Each part of one commit is told in the step it names most

Of the concerns of one commit, each labelled section (a label or a heading and its blocks up to the next one), else
each block, MUST be told in the step that holds the targets of most of its resolved mentions, the earliest of equal
ones (`told`: `{"kind": "step", "step": 1}`); a run of blocks of no label MUST be cut into a part per place where the
place changes.

#### Scenario: Sections and blocks told in their steps
- **GIVEN** a range of one commit whose steps are its concerns
- **WHEN** a labelled section, else a block, of its body names code of the steps
- **THEN** it is told in the step that holds the targets of most of its resolved mentions, the earliest of equal ones
  (`told`: `{"kind": "step", "step": 1}`)
- **AND** a run of blocks of no label is cut into a part per place where the place changes
  [@test] crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`, `a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)

### Requirement: The Story of a Commit: The lead and a part that names nothing are told in the overview

The lead and a part that names nothing MUST be told in the overview (`overview`), and every part while the steps are
not told.

#### Scenario: The lead, a part of no name, and steps not told
- **WHEN** a part is the lead or names nothing, or the steps are not told
- **THEN** it is told in the overview (`overview`)
  [@test] crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)

### Requirement: The Story of a Commit: A story of several commits keeps the order of its author

A story of several commits MUST keep the order of its author: each part is told with its commit (`commit`), in the
opening of the step it starts or the disclosure of its round. The page shows the parts a place tells and decides none
of it.

#### Scenario: The parts of the bodies of several commits
- **WHEN** the steps are the commits of a range
- **THEN** each part of a body is told with its commit (`commit`), in the opening of the step it starts or the
  disclosure of its round
- **AND** the page shows the parts a place tells and decides none of it

### Requirement: The Story of a Commit: Each stop lists the lines of the told bodies that name it

Each stop MUST list the lines of the told bodies that name it (`marks`: the commit, the block, the line and where the
block is told, `step`, `null` for the overview), once per block.

#### Scenario: A resolved mention marks the stops of its target
- **WHEN** a line of a told body has a resolved mention
- **THEN** it marks the stops of the path of its target in the step of its target (in every step without one)
- **AND** of a declaration only the stops that show its unit, its own or joined, when one does
- **AND** once per block
  [@test] crates/review-plan/src/story_test.rs (`a_line_of_a_told_body_marks_the_stops_it_names_where_it_is_told`, `the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)

### Requirement: The Story of a Commit: Only the commit that starts a step marks its stops

A story of several commits MUST mark only from the commit that starts a step; a follow-up, folded under its step,
marks nothing.

#### Scenario: A follow-up that names code
- **WHEN** a follow-up, folded under its step, names code of the step
- **THEN** it marks nothing; only the commit that starts the step marks
  [@test] crates/review-plan/src/story_test.rs (`a_line_of_a_told_body_marks_the_stops_it_names_where_it_is_told`)

### Requirement: The Story of a Commit: A message changed since the review carries its change

The story of a commit whose message changed since the review (`message_change` of its pair in the interdiff of its
branch, [Review Plan](../review-plan/spec.md#since-your-review); of a commit shown alone, read with its reviewed
commit) MUST carry that change (`change`) and place it. A story whose message did not change has no `change`, its
parts as the grammar folds them.

#### Scenario: Added and removed blocks placed in their parts
- **WHEN** the message of a commit changed since the review
- **THEN** its story carries the `change`, and each part marks the added blocks it holds
- **AND** the removed blocks of a hunk stand in one part (`removals`), that of its first added block, else of the
  block they followed (the first block when they were at the start)
- **AND** they stand before the block `start` when that part holds it (`before`), else at its end
  [@test] crates/review-plan/src/story_test.rs (`a_changed_part_of_a_body_is_marked_and_unfolded_a_verification_too`)

#### Scenario: A message that did not change
- **WHEN** the message did not change since the review
- **THEN** the story has no `change`, its parts as the grammar folds them

### Requirement: The Story of a Commit: A part that shows a change is never folded

A part that shows a change MUST NOT be folded, a verification section neither: what changed is what the reader came
back for.

#### Scenario: A changed part and a verification section
- **WHEN** the story carries a change
- **THEN** a part that shows it is not folded, a verification section neither
  [@test] crates/review-plan/src/story_test.rs (`a_changed_part_of_a_body_is_marked_and_unfolded_a_verification_too`)

### Requirement: The Story of a Commit: A step lists the new API and the new uses between modules

A step MUST list the public declarations it adds (`newApi`, the keys of the `added` entries of `api` whose
declaration or new file its stops show) and the new uses between modules it adds (`moduleEdges`), each in one step,
the first that holds it.

#### Scenario: New API and new links between modules
- **WHEN** the stops of a step show the declaration or new file of an `added` entry of `api`, and new uses between
  modules
- **THEN** the step lists the key in `newApi`
- **AND** the uses in `moduleEdges`: the pair of modules, by their place in `modules`, and its links, by their place
  in `links`; each link in the step whose stop shows its first use, else the first step whose stops show the file that
  uses
- **AND** each in one step, the first that holds it
  [@test] crates/review-plan/src/story_test.rs (`a_step_lists_the_new_api_and_the_new_links_between_modules_it_holds`)

### Requirement: The Story of a Commit: A joined declaration is new API of the step that wrote it

A declaration a stop of another step shows beside its own unit (`StepStop.joined`) MUST be new API of the step that
wrote it, which says where it is shown.

#### Scenario: A declaration a later step adds in the block of another
- **WHEN** a stop of another step shows a declaration beside its own unit (`StepStop.joined`)
- **THEN** it is new API of the step that wrote it, which says where it is shown
  [@test] crates/review-plan/src/steps_test.rs (`a_declaration_a_later_step_adds_in_the_block_of_another_is_the_new_api_of_the_step_that_wrote_it`)

### Requirement: Coverage: Every block of the diff is in one stop

Every block of the diff MUST be in one stop, else in a mechanical unit no stop shows; a file of no blocks counts as
one, a file changed and changed back as none (`coverage`: `blocks`, `inStops`, `mechanical`, `missing`, `twice`).

#### Scenario: The coverage of a plan
- **WHEN** the plan cuts the steps of a range
- **THEN** every block of the diff is in one stop, else in a mechanical unit no stop shows
- **AND** a file of no blocks counts as one, a file changed and changed back as none
- **AND** `coverage` counts them: `blocks`, `inStops`, `mechanical`, `missing`, `twice`
  [@test] crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Requirement: Brief: The issue is the key half the subjects name

`issue` MUST be the issue key the subjects of the commits name: of the keys each subject starts with, the one every
subject that starts with a key names, named by half of them at least; the first such key of the first subject when
several are.

#### Scenario: Subjects that start with keys
- **WHEN** the subjects of the commits start with issue keys (`IJAI-1184 IJAI-5640 Air records…` names both)
- **THEN** `issue` is the key every subject that starts with a key names, named by half of them at least
- **AND** the first such key of the first subject when several are
  [@test] crates/review-plan/src/steps_test.rs (`the_issue_is_one_key_half_the_commits_name_and_none_contradicts`)

### Requirement: Brief: The contract holds the prose the change touches, by its kind

`contract` MUST hold the prose the change adds, removes or changes, by the rank of its kind, then in the order of the
story: a spec, a record of a decision, a guide of agents, a doc; of Markdown, text, reStructuredText and AsciiDoc
only.

#### Scenario: A file of prose by its place
- **WHEN** the change touches a file of prose
- **THEN** a spec is `*.spec.md`, a file below `spec/` or `specs/`, the delta of a change of OpenSpec
- **AND** a record of a decision is below `decisions/`, `adr/` or `adrs/`, the proposal and the design of a change of
  OpenSpec, archived or not
- **AND** a guide of agents is `AGENTS.md`, `CLAUDE.md`, below `.ai/`
- **AND** a doc is below `docs/` or `doc/`, a README
  [@test] crates/review-plan/src/steps_test.rs (`a_file_of_prose_is_a_spec_a_decision_a_guide_or_a_doc_by_its_place`)

#### Scenario: The files of a change of OpenSpec
- **WHEN** the change touches the files of a change of OpenSpec
- **THEN** its tasks and the deltas of an archived one are none, as the diff of the main spec holds their words
  [@test] crates/review-plan/src/steps_test.rs (`a_file_of_openspec_is_a_spec_a_decision_or_none_by_its_place_in_a_change`)

### Requirement: Brief: An entry is a place of a file widened to whole paragraphs

The contract MUST hold an entry per place of a file (`prose`): its changed blocks widened to whole paragraphs on both
sides, never the changed lines of a hard-wrapped one, the blocks whose paragraphs meet one entry (`blocks`).

#### Scenario: A hard-wrapped paragraph changed in one line
- **WHEN** one line of a hard-wrapped paragraph changes
- **THEN** the entry holds the whole paragraph, diffed by word
  [@test] crates/review-plan/src/prose_test.rs (`a_hard_wrapped_paragraph_changed_in_one_line_is_the_whole_paragraph_by_word`)

#### Scenario: Two blocks of one paragraph
- **WHEN** two changed blocks meet one paragraph
- **THEN** they are one entry (`blocks`)
  [@test] crates/review-plan/src/prose_test.rs (`a_rewritten_paragraph_is_removed_and_added_and_two_blocks_of_one_paragraph_are_one`)

### Requirement: Brief: The front matter of a file is no rule

The front matter of a file (from a first line `---` to the next) MUST be no rule, its lines no part of a block, a
block of the front matter alone no entry.

#### Scenario: A change of the front matter
- **WHEN** a change of a file of prose touches its front matter
- **THEN** the front matter is the lines from a first line `---` up to the next, its closing rule
- **AND** its lines are no part of a block, and a block of the front matter alone is no entry
  [@test] crates/review-plan/src/prose_test.rs (`the_front_matter_is_the_lines_up_to_its_closing_rule`)
  [@test] crates/review-plan/src/steps_test.rs (`the_brief_quotes_no_front_matter_no_metadata_and_a_table_of_a_spec_by_its_changed_rows`)

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

### Requirement: Brief: A thematic break and a line of metadata end a paragraph

A thematic break (`---`, `***`, `___`) and a line of metadata (one key, then only references `kind:name` and paths,
comma-separated: `refs: flow:x, spec:y`, `targets: ../../a.kt`) MUST end a paragraph and be no paragraph of their own;
a key alone (`Look closely:`) or followed by words MUST be prose.

#### Scenario: A thematic break
- **WHEN** a paragraph is followed by a thematic break
- **THEN** the break ends it and is no paragraph of its own
  [@test] crates/review-plan/src/prose_test.rs (`a_thematic_break_ends_a_paragraph`)

#### Scenario: A line of metadata and a key of prose
- **WHEN** a line is one key, then only references and paths, comma-separated
- **THEN** it ends a paragraph and is no paragraph of its own
- **AND** a key alone or followed by words is prose
  [@test] crates/review-plan/src/prose_test.rs (`a_line_of_metadata_is_no_prose`)

### Requirement: Brief: A table is its header row and each of its rows

A table (a header row, a delimiter row, then its rows up to a break or a line without a `|`) MUST be its header row and
each of its rows, a paragraph of form `row` each: its text the cells of its header, a line break, then its own cells,
as many as its header has, each ` | ` apart, `header` and `cells` their ranges, the header row itself its cells
alone; a line of pipes without a delimiter row MUST be prose.

#### Scenario: The rows of a table
- **WHEN** a file of prose holds a table, a header row, a delimiter row, then its rows up to a break or a line without
  a `|`
- **THEN** its header row and each of its rows are a paragraph of form `row`: its text the cells of its header, a line
  break, then its own cells, as many as its header has, each ` | ` apart
- **AND** `header` and `cells` are their ranges, the header row itself its cells alone
  [@test] crates/review-plan/src/prose_test.rs (`a_table_with_one_added_row_is_its_header_and_that_row`)

#### Scenario: A line of pipes without a delimiter row
- **WHEN** a line of pipes has no delimiter row under it
- **THEN** it is prose
  [@test] crates/review-plan/src/prose_test.rs (`a_line_of_pipes_without_a_delimiter_row_is_prose`)

### Requirement: Brief: A row is compared and diffed by its own cells

A row MUST be compared and diffed by its own cells, so a table one row is added to is that row under its header, a
renamed column the header row changed.

#### Scenario: A table one row is added to
- **WHEN** a change adds one row to a table
- **THEN** the entry is that row under its header
  [@test] crates/review-plan/src/prose_test.rs (`a_table_with_one_added_row_is_its_header_and_that_row`)

#### Scenario: A changed row and a renamed column
- **WHEN** a change rewrites a row or renames a column
- **THEN** the row is diffed by word in its cells
- **AND** a renamed column is the header row changed
  [@test] crates/review-plan/src/prose_test.rs (`a_changed_row_is_diffed_by_word_in_its_cells`)

### Requirement: Brief: A fenced code block is one paragraph of form code

A fenced code block MUST be one paragraph of form `code`, its contents preserving line breaks and its language when
named; a test-link marker inside it is code, not a test reference. Code paragraphs MUST have no inline code spans or
links, pair only with code paragraphs, and bypass the prose filter; a changed block is diffed by word.

#### Scenario: A fenced block of a file of prose
- **WHEN** a file of prose holds a fenced code block
- **THEN** it is one paragraph of form `code`, its line breaks preserved, its language when named
- **AND** a test-link marker inside it is code, not a test reference
  [@test] crates/review-plan/src/prose_test.rs (`a_fenced_block_is_one_paragraph_with_its_lines_preserved`)

#### Scenario: A changed fenced block in the brief
- **WHEN** a change rewrites a fenced code block
- **THEN** it pairs only with a code paragraph, has no inline code spans or links, bypasses the prose filter and is
  diffed by word, keeping its code form and its lines
  [@test] crates/review-plan/src/steps_test.rs (`a_fenced_code_change_in_the_brief_keeps_its_code_form_and_lines`)

### Requirement: Brief: A long code block first shows its changed lines with context

A code block longer than eight lines, changed on multiple lines, MUST first show the changed lines with one line of
context on either side.

#### Scenario: A long code block changed in several lines
- **WHEN** a code block longer than eight lines changes on multiple lines
- **THEN** it first shows the changed lines with one line of context on either side
  [@test] crates/review-plan/src/prose_test.rs (`a_long_code_block_shows_changed_lines_with_one_line_of_context`)

### Requirement: Brief: A paragraph wrapped anew is no change

A paragraph wrapped anew MUST be no change.

#### Scenario: A paragraph wrapped anew
- **WHEN** a change wraps a paragraph anew, its words the same
- **THEN** it is no change
  [@test] crates/review-plan/src/prose_test.rs (`a_paragraph_wrapped_anew_is_no_change`)

### Requirement: Brief: A removed paragraph and a similar added one are one changed paragraph

A removed paragraph and the first similar added one after the last pair (half of the shorter kept and a third of the
longer, by word) MUST be one `changed` paragraph with the words the change wrote (`inserted`) and removed (`deleted`,
each at the offset of the text it stood before), the rest `added` or `removed`.

#### Scenario: A new paragraph and lines appended to one
- **WHEN** a change adds a paragraph alone, and appends lines to another
- **THEN** the new one is `added` alone, and the other is `changed` by the lines appended to it
  [@test] crates/review-plan/src/prose_test.rs (`a_new_paragraph_is_added_alone_and_lines_appended_to_one_change_it`)

#### Scenario: A rewritten paragraph
- **WHEN** a change rewrites a paragraph and no similar added one pairs with the removed one
- **THEN** it is `removed` and `added`
  [@test] crates/review-plan/src/prose_test.rs (`a_rewritten_paragraph_is_removed_and_added_and_two_blocks_of_one_paragraph_are_one`)

#### Scenario: A paragraph added before a changed one
- **WHEN** a change adds a paragraph before one it changes
- **THEN** the changed one keeps its pair, `changed` with its `inserted` and `deleted` words, and the new one is
  `added`
  [@test] crates/review-plan/src/prose_test.rs (`a_paragraph_added_before_a_changed_one_leaves_it_its_pair`)

### Requirement: Brief: A word pair repeated three times across the brief is a substitution

A delete and insert of words at the same offset, each 2–40 characters and without whitespace, MUST become a
substitution when the pair occurs at least three times across the brief.

#### Scenario: A pair repeated across paragraphs
- **WHEN** a delete and an insert of words at the same offset, each 2–40 characters and without whitespace, occur
  three times or more across the brief
- **THEN** the pair is a substitution
  [@test] crates/review-plan/src/steps_test.rs (`a_substitution_repeated_across_paragraphs_is_named_once_and_marked_in_place`)

#### Scenario: A pair seen twice
- **WHEN** the pair occurs only twice
- **THEN** it is no substitution
  [@test] crates/review-plan/src/steps_test.rs (`a_pair_seen_twice_is_no_substitution`)

### Requirement: Brief: A substitution is named once and marked in place

`substitutions` MUST name each pair once, ordered by places descending, with distinct `paths`. Its index and the range
of its new word MUST be in a paragraph's substitution marks; the pair is removed from `inserted` and `deleted`, and a
paragraph changed only by substitutions is `substituted`.

#### Scenario: The marks of a substitution
- **WHEN** a pair is a substitution
- **THEN** `substitutions` names it once, ordered by places descending, with distinct `paths`
- **AND** each paragraph that holds it marks its index and the range of its new word, the pair removed from
  `inserted` and `deleted`
- **AND** a paragraph changed only by substitutions is `substituted`
  [@test] crates/review-plan/src/steps_test.rs (`a_substitution_repeated_across_paragraphs_is_named_once_and_marked_in_place`)

### Requirement: Brief: A link reads as its text with its target

A link of Markdown MUST read as its text with its target (`links`: a path of the repository resolved against the
file, without its anchor, or a URL as written).

#### Scenario: A link to a file and a link to a page
- **WHEN** a paragraph holds a link of Markdown
- **THEN** it reads as its text, its target in `links`: a path of the repository resolved against the file, without
  its anchor, or a URL as written
  [@test] crates/review-plan/src/prose_test.rs (`a_link_reads_as_its_text_with_its_target`)

### Requirement: Brief: A changed paragraph shows the sentences that hold its changes

Of a changed paragraph of two sentences or more, whatever its length, the entry MUST show the sentences that hold its
changes (`excerpt`) when they leave out a quarter of it at least.

#### Scenario: A paragraph changed in one of its sentences
- **WHEN** a long paragraph of several sentences changes in one of them
- **THEN** its `excerpt` is that sentence, which it shows first
  [@test] crates/review-plan/src/prose_test.rs (`a_paragraph_changed_in_one_of_its_sentences_shows_it_first`)

#### Scenario: A short paragraph of two sentences
- **WHEN** a short paragraph of two sentences changes in one of them, which leaves out a quarter of it at least
- **THEN** its `excerpt` is that sentence, whatever its length
  [@test] crates/review-plan/src/prose_test.rs (`a_short_paragraph_of_two_sentences_changed_in_one_shows_that_sentence`)

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

### Requirement: Brief: Only prose, code blocks and the rows of a rule's tables are quoted

Of every kind of file only prose (three words or more of letters, no key and no value after one), its code blocks and
the rows of the tables of a spec, a decision or a guide MUST be quoted: a changed line of values says nothing, the
heading of its section names the change.

#### Scenario: A changed line of values in a doc
- **WHEN** a change rewrites a line of values (`dark: #88aaff`, `color: red; background: blue`)
- **THEN** the line is not quoted, it says nothing
- **AND** the heading of its section names the change
  [@test] crates/review-plan/src/steps_test.rs (`the_brief_of_a_doc_names_the_section_of_a_changed_value_and_quotes_only_its_prose`)

### Requirement: Brief: A table of a doc, the front matter and metadata are no quote

A table of a doc, its bookkeeping (a map of the specs that own a topic), MUST NOT be quoted, nor the front matter and
the lines of metadata of every file: only their diff card shows them, and no heading names them.

#### Scenario: A table of a doc and a table of a spec
- **WHEN** a change touches a table of a doc, the front matter and lines of metadata
- **THEN** none of them is quoted, only the diff card shows them, and no heading names them
- **AND** a table of a spec is quoted by its changed rows
  [@test] crates/review-plan/src/steps_test.rs (`the_brief_quotes_no_front_matter_no_metadata_and_a_table_of_a_spec_by_its_changed_rows`)

### Requirement: Brief: Each paragraph carries its spans of code

Each quoted paragraph MUST carry its spans of code (`code`, of a row those of each cell) as a body has them.

#### Scenario: Spans of code of a paragraph and of a row
- **WHEN** a quoted paragraph or row holds spans of code
- **THEN** they are its `code`, of a row those of each cell, as a body has them

### Requirement: In `brd`: `brd review-plan` prints the steps and the brief with no git process

`brd review-plan` MUST print `steps` and `brief` (`"schema": 32`), the ids of the commits of the steps and of the
story as short as the others, every mention of a name in backticks with its `target` (`null` where it resolves
nothing), and run no git process for them.

#### Scenario: The plan of a range
- **WHEN** `brd review-plan` prints the plan of a range
- **THEN** it prints `steps` and `brief`, the ids of the commits of the steps and of the story as short as the others
- **AND** every mention of a name in backticks with its `target`, `null` where it resolves nothing
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

#### Scenario: No git process for the steps and the brief
- **WHEN** `brd review-plan` reads the plan of a range
- **THEN** it runs no git process for the steps and the brief
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### Requirement: The Map: The overview shows the shape of the change after its lead

The overview MUST show, after its lead, the shape: "Shape" with the count of the files and lines of the change, then a
row per module with code, tests or resources in the order Rust gives (a module the change adds first). A module of
build files or prose alone MUST have no row.

#### Scenario: The shape of a change of several modules
- **WHEN** the change touches several modules with code, tests or resources
- **THEN** the overview shows, after its lead, "Shape" with the count of the files and lines of the change
- **AND** a row per module in the order Rust gives, a module the change adds first
- **AND** a module of build files or prose alone has no row
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`)

### Requirement: The Map: A row names its module, its state, its share and its counts

A row of the shape MUST show "New module" or "Deleted module" when the change adds or deletes its module, its label in
the code font, a bar of its share of the changed lines against the largest row, its lines and files. The rows MUST
share their columns: the bars of all rows start at one edge, the lines and the files of all rows stand in a column
each, whatever the widths of their counts.

#### Scenario: A new module and a changed one
- **WHEN** the shape lists a module the change adds and one it changes
- **THEN** the new one shows "New module", a deleted one "Deleted module"
- **AND** each shows its label in the code font, a bar of its share of the changed lines against the largest row, its
  lines and files
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`)

#### Scenario: Counts of different widths
- **WHEN** the rows of the shape have counts of different widths
- **THEN** their bars start at one edge, their lines end at one edge and their files start at one edge
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should stand the bars, the lines and the files of the rows of the shape in columns, whatever the widths of their counts`)

### Requirement: The Map: The lines of the shape are what git counts

The lines of the shape, of the change and of each row, MUST be those `git diff --numstat` of the range counts: git's
counts of each file, from the processes the plan runs anyway, with git's default algorithm whatever `diff.algorithm`
says, never the engine's, which aligns some blocks otherwise. A file that several commits change MUST count its pair
as the engine does, since git's count of the range would take a process of its own.

#### Scenario: A file one commit changes that the engine aligns otherwise
- **WHEN** the engine counts the lines of a file one commit changes otherwise than git: a call moved into the test
  above it, +3 −3 by the engine, which matches the call before the closing brace and the blank line, and +1 −1 by git
- **THEN** the shape counts +1 −1
- **AND** the tree and the header of its card count as the engine does, under the whitespace policy
  ([Commit Files](../../commit/commit-files/spec.md#requirement-reading-the-diffs-the-counts-of-a-file-are-the-engines)),
  and the header of a part of a file its own blocks as the engine counts them under `Default`
  [@test] crates/review-plan/src/modules_test.rs (`the_lines_of_a_module_are_what_git_counts_where_the_engine_aligns_otherwise`)
  [@test] crates/trial-check/src/diff_scan_test.rs (`read_commit_diffs_gives_git_counts_beside_the_engines`, `parses_git_counts_after_the_raw_part`)

#### Scenario: The diff config of the user
- **WHEN** the user's config chooses another diff algorithm (`diff.algorithm=histogram`)
- **THEN** git's counts are those of the default algorithm
  [@test] crates/trial-check/src/diff_scan_test.rs (`the_diff_config_of_the_user_changes_neither_the_hunks_nor_the_edges`)

#### Scenario: A file several commits change
- **WHEN** several commits of the range change a file
- **THEN** the shape counts the blocks of its pair as the engine counts them, without the empty line after the final
  line end that a block reaching the end holds, which can differ from git's count of the range
  [@test] crates/review-plan/src/modules_test.rs (`the_lines_of_a_file_several_commits_change_are_the_engines_count_of_its_pair`)

#### Scenario: No process of their own
- **WHEN** the plan reads the changes of its commits, or the diff of a carried scope
- **THEN** git's counts come with them, from the same `log --raw` and the same `diff-tree`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`, `a_review_plan_of_a_range`)

### Requirement: The Map: Under a row, what a new module uses, the folders of a changed one

Under a row, muted, the map MUST show of a new module what it newly uses and the plugins that hold it as content, of a
changed one its folders in words, the most files first, and what it adds beside its code.

#### Scenario: The line under a new module
- **WHEN** the row is of a module the change adds
- **THEN** the line under it names what it newly uses and the plugins that hold it as content ("uses view.gui and shared/code · content of com.intellij.air")
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`)

#### Scenario: The line under a changed module
- **WHEN** the row is of a module the change changes
- **THEN** the line under it names its folders in words, the most files first, from five on the first three and the
  count of the rest ("in ui/embeds, ui/markdown, ui/util and references"), no count of files: the row has it
- **AND** what it adds beside its code ("+ depends on intellij.platform.ide.util.io", "Opens wider: isNightlyBuild")
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`)

### Requirement: The Map: A change of one module shows its folders as the rows

A change of one module MUST show its folders as the rows when it holds more than 12 files there, else no shape.

#### Scenario: A large and a small change of one module
- **WHEN** a change of one module holds more than 12 files there
- **THEN** its folders are the rows of the shape
- **AND** a smaller change has no shape
  [@test] src/presenters/review-map.test.ts (`names the rows past its budget once, and the folders of a large change of one module; a small change has no shape`)

### Requirement: The Map: At most 6 rows, each line truncated at the edge

The shape MUST show at most 6 rows, then "and N more modules" with their labels in its hover; every line MUST truncate
at the edge of the reading column, never wrap.

#### Scenario: More than 6 modules
- **WHEN** the change has more than 6 rows
- **THEN** the shape shows 6, then "and N more modules" with their labels in its hover
- **AND** every line truncates at the edge of the reading column, never wraps
  [@test] src/presenters/review-map.test.ts (`names the rows past its budget once, and the folders of a large change of one module; a small change has no shape`)

### Requirement: The Map: A click on a row shows its first file

A click on a row of the shape MUST show its first file in the story.

#### Scenario: A click on the row of a module
- **WHEN** the user clicks the row of a module
- **THEN** its first file shows in the story
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list the shape of the plan in the rail of the overview, a module showing its first file in the changes`)

#### Scenario: The first file of a row is the one the story shows first
- **WHEN** the shape names the first file of a row
- **THEN** it is the file of the row's first step, or a file of no step after them
  [@test] src/presenters/review-map.test.ts (`shows the first file of a row as the story shows it: of its first step, a file of no step after them`)

### Requirement: The Map: Under a row, what its descriptors register by simple names

Under a row, what its descriptors register, change or remove ([Review Plan → Registrations](../review-plan/spec.md#registrations))
MUST be a line each by simple names, its qualified names or the description of a key and its descriptor in the hover,
a click showing the class it registers (its implementation), else its descriptor.

#### Scenario: The registrations of a module
- **WHEN** the descriptors of a module register, change or remove something
- **THEN** a line each names it by simple names ("+ applicationService TranscriptEmbedPlayers → TranscriptEmbedPlayersImpl", "+ registryKey air.chat.media.inline.playback.enabled = NIGHTLY_ENABLED", "~ … (defaultValue changed)")
- **AND** its qualified names or the description of a key and its descriptor are in the hover
  [@test] src/presenters/review-map.test.ts (`names under the row of a module what its descriptors register by simple names, a click at the class it registers; a content module none, past the budget a count`)

#### Scenario: A click on a registration
- **WHEN** the user clicks the line of a registration
- **THEN** it shows the class it registers (its implementation), else its descriptor
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should name under the row of a module of the map what its descriptor registers, a click showing the class it registers`)

### Requirement: The Map: At most 3 registrations a row and 8 in all

The map MUST show at most 3 registrations a row and 8 in all, then "and N more registrations"; a content module MUST
show none (the line of the new module says where it plugs in); the rail of a pull request has no room for them.

#### Scenario: A content module and registrations past the budget
- **WHEN** a module is a content module
- **THEN** it shows no registration: the line of the new module says where it plugs in
- **AND** past 3 a row or 8 in all the map shows "and N more registrations"
  [@test] src/presenters/review-map.test.ts (`names under the row of a module what its descriptors register by simple names, a click at the class it registers; a content module none, past the budget a count`)

#### Scenario: The rail of a pull request
- **WHEN** the rail of the overview of a pull request lists the rows
- **THEN** it shows no registration, as it has no room for them

### Requirement: The Map: The rail of a pull request lists the same rows

The rail of the overview of a pull request MUST list the same rows
([Review Requests → Review window](../../repository/review-requests/spec.md#review-window)), without their bars, the
badge in its tone on the line under the row.

#### Scenario: The overview of a pull request
- **WHEN** the review of a pull request shows its overview
- **THEN** its rail lists the rows of the shape, without their bars, the badge in its tone on the line under the row
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list the shape of the plan in the rail of the overview, a module showing its first file in the changes`)

### Requirement: The Map: The contract is one line of the new specs and the count of the rest

The map MUST name the contract in one line, "Contract": each new spec by its name and the count of its requirements,
a click its top, then how many specs and docs the change changes ("3 specs and 3 docs changed").

#### Scenario: A new spec and changed specs and docs
- **WHEN** the change adds a spec and changes others and docs
- **THEN** the line "Contract" names the new spec by its name and the count of its requirements ("New spec Transcript Media Playback (14 requirements)", a click its top)
- **AND** then how many specs and docs the change changes ("3 specs and 3 docs changed")
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`, `names the rows past its budget once, and the folders of a large change of one module; a small change has no shape`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

#### Scenario: The name of a spec
- **WHEN** the line names a new spec
- **THEN** it names it by its file
- **AND** a spec of OpenSpec by its capability: `openspec/specs/branch/stack-parents/spec.md` is "Stack Parents", a main spec and the delta of a change alike
  [@test] src/presenters/review-map.test.ts (`names a spec by its file`, `names a spec of OpenSpec by its capability, a main spec and the delta of a change alike`)

### Requirement: The Map: Opens wider names the widened declarations no row names

The map MUST name in the API "Opens wider:" the declarations whose reach alone opened wider that no row of the shape
names, each showing its declaration.

#### Scenario: A declaration whose reach alone opened wider
- **WHEN** the reach alone of a declaration opened wider and no row of the shape names it
- **THEN** the line "Opens wider:" names it, showing its declaration
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)

### Requirement: The Map: The verdict that no use left outside breaks is one muted line

Where Rust's verdict is that no use left outside breaks and nothing can break, the map MUST show that verdict once,
muted, never in the tone of a warning, with the API changes it covers ("No use left outside breaks · 2 API changes"),
a click opening them under the line, each its badge, its name and its place (Possible Breakage lists them no more).

#### Scenario: API changes nothing breaks of
- **WHEN** Rust's verdict is that no use left outside breaks and nothing can break
- **THEN** the map shows "No use left outside breaks · 2 API changes" once, muted, never in the tone of a warning
- **AND** a click opens the changes under the line, each its badge, its name and its place
- **AND** Possible Breakage lists them no more
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)

### Requirement: The Map: One line counts and quotes the texts of the UI the change touches

The map MUST name the texts of the UI the change adds, changes or removes ([UI Texts](../review-plan/spec.md#ui-texts))
in one line between the contract and the API, "UI texts": their counts, the first two quoted, each showing its entry
in its bundle (of a removed one at its old line), then "and N more". The map MUST NOT show the line without a
bundle the change touches.

#### Scenario: Texts of the UI added, changed and removed
- **WHEN** the change adds, changes and removes texts of the UI
- **THEN** the line "UI texts" counts them ("2 new, 1 changed and 1 removed")
- **AND** quotes the first two ("“Play”, “Download…”"; a removed one struck through, of a changed one its new words with its key and its bundle at its line in the hover)
- **AND** each quote shows its entry in its bundle, of a removed one at its old line
  [@test] src/presenters/review-map.test.ts (`counts the texts of the UI a change adds, changes and removes, each its words at its entry, a removed one at its old line; no bundle, no line`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should quote on a line of the map the texts of the UI the change adds, all of them under it on a click, a quote showing its entry in its bundle`)

#### Scenario: No bundle the change touches
- **WHEN** the change touches no bundle
- **THEN** the map shows no line of texts
  [@test] src/presenters/review-map.test.ts (`counts the texts of the UI a change adds, changes and removes, each its words at its entry, a removed one at its old line; no bundle, no line`)

### Requirement: The Map: "and N more" opens every text of the UI under the line

"and N more" MUST open all the texts under the line, a line each: the sign of what the change does to it, its words
(of a changed one the old words struck through, "→", the new), its key muted. What the user reads is a fact of the
change, not a card of a `.properties` diff.

#### Scenario: All the texts of the UI on a click
- **WHEN** the user clicks "and N more" of the line of texts
- **THEN** all of them open under the line, a line each: the sign of what the change does to it ("+", "−", "~"), its words, its key muted
- **AND** of a changed one the old words struck through, "→", the new
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should quote on a line of the map the texts of the UI the change adds, all of them under it on a click, a quote showing its entry in its bundle`)

### Requirement: In the Review Window: The right column is one document that scrolls

With the review plan, the right column MUST be one document that scrolls, one canvas of cards, the bar of the step
above it on that canvas: the overview first (`ReviewPrologue`), a card of prose at the inset of the file cards, the
same card at the same place while the diffs load, then the story of the diffs.

#### Scenario: The review of a plan opens
- **WHEN** the review window opens a range with the review plan
- **THEN** the right column is one document that scrolls, one canvas of cards, the bar of the step above it
- **AND** the overview comes first (`ReviewPrologue`), a card of prose at the inset of the file cards, the same card
  at the same place while the diffs load, then the story of the diffs
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

### Requirement: In the Review Window: The overview opens on its headline

The overview MUST open on its headline: the title YouTrack gives the issue the commits name once it is known, else
the name the plan gives (`ReviewPlan.title`: the branch as the title bar names it, the range of the commits), the
issue key a chip after it, a link with the popover of its issue; none under the title of a pull request, which names
it already.

#### Scenario: The headline of a branch and of a range
- **WHEN** the overview opens
- **THEN** its headline is the title YouTrack gives the issue the commits name once it is known, else the name the plan
  gives (`ReviewPlan.title`)
- **AND** the issue key is a chip after it, a link with the popover of its issue
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

#### Scenario: The overview of a pull request
- **WHEN** the overview is under the title of a pull request
- **THEN** it shows no headline, as the title names it already

### Requirement: In the Review Window: A commit shown alone opens on its whole heading and message

The overview of a commit shown alone MUST show its whole heading and message instead of the headline
(`range-review-commit-header`: its subject, meta line, action and menu, its body whole, of the concerns of its own
plan the parts of it told before the steps).

#### Scenario: The overview of a commit shown alone
- **WHEN** the review shows a commit alone
- **THEN** its overview opens on `range-review-commit-header`: its subject, meta line, action and menu, its body whole
- **AND** of the concerns of its own plan, the parts of the body told before the steps

### Requirement: In the Review Window: The map, Possible Breakage and the Steps follow the headline

After the headline the overview MUST show the map ([The Map](#the-map)), then Possible Breakage, then the Steps.

#### Scenario: The order of the overview
- **WHEN** the overview opens
- **THEN** the map follows the headline, then Possible Breakage, then the Steps
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

### Requirement: In the Review Window: What Changes stands in the opening of the step it changes

What Changes MUST stand in the opening of each step for the entries that step changes (`OpeningView.contract`, under
the label "Contract"), and in the overview only of a review whose steps are not told; its repeated replacements MUST
stand always in the overview, the change's.

#### Scenario: Entries of the brief in the openings of their steps
- **WHEN** the steps are told and a step changes entries of the brief
- **THEN** its opening shows them under the label "Contract" (`OpeningView.contract`)
- **AND** the overview shows What Changes only for a review whose steps are not told, and its repeated replacements
  always
  [@test] src/presenters/review-story.test.ts (`quotes What Changes in the opening of the step whose stops show its entry, each entry in one step`)

### Requirement: In the Review Window: What Changes names each repeated replacement in one row

What Changes MUST show one row per repeated replacement ("Renames `bd` → `brd` · 23 places in 5 files"), the first
three file names buttons and the rest in a tooltip.

#### Scenario: A replacement repeated across files
- **WHEN** the brief has a repeated replacement
- **THEN** What Changes shows one row of it ("Renames `bd` → `brd` · 23 places in 5 files")
- **AND** the first three file names are buttons, the rest in a tooltip
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should name a repeated substitution once in What Changes, its first files buttons that show the file, the others in its tooltip`)

### Requirement: In the Review Window: What Changes shows one header per file

What Changes MUST show one header per file ("Spec rule", "Decision", "Agent guide", "Doc" and its `text-sm` path,
filename in the foreground and directory muted), which opens it.

#### Scenario: The header of a file of prose
- **WHEN** What Changes quotes a file
- **THEN** it shows one header of it, its kind ("Spec rule", "Decision", "Agent guide", "Doc") and its path, filename
  in the foreground and directory muted
- **AND** a click on the header opens the file
  [@test] src/presenters/review-story.test.ts (`words What Changes: a file once, a section once with its tests, its paragraphs whole, a link showing a changed file or opening a page`)

### Requirement: In the Review Window: Each section shows its heading once and its paragraphs whole

Under each header its sections MUST stand behind a muted quote line; each section has its heading once, "Proven by
restart.json" muted after it, and its paragraphs whole, half a line apart.

#### Scenario: Added, removed and changed paragraphs
- **WHEN** a section of What Changes has added, removed and changed paragraphs
- **THEN** one added shows the bar of an addition on the quote line
- **AND** one removed is muted and struck through beside the bar of a removal
- **AND** one changed shows its inserted words tinted and its deletions struck through in place
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell What Changes in whole paragraphs changed by word, a long one by the sentence of its change, and show the file its link names`)
  [@test] src/presenters/review-story.test.ts (`words What Changes: a file once, a section once with its tests, its paragraphs whole, a link showing a changed file or opening a page`)

### Requirement: In the Review Window: A repeated replacement is dotted-underlined in place

A repeated replacement MUST be dotted-underlined with the old word in its tooltip and screen-reader text, and a
paragraph changed only by replacements MUST stay muted in place without a change bar.

#### Scenario: A paragraph of replacements
- **WHEN** a paragraph holds a repeated replacement
- **THEN** it is dotted-underlined, the old word in its tooltip and screen-reader text
- **AND** a paragraph changed only by replacements stays muted in place without a change bar
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the paragraphs of a repeated substitution quiet in the openings of their steps, a change inside code inside its chip and a fenced block as code`)
  [@test] src/presenters/review-story.test.ts (`passes paragraph forms and resolves substitution marks to their original words`)

### Requirement: In the Review Window: An entry is quoted in the first step that shows it

An entry of the brief MUST be quoted in the first step whose stops show its unit and one of its blocks, so a section
two commits change has its part in the opening of each.

#### Scenario: A section two commits change
- **WHEN** two commits of two steps change a section
- **THEN** each step's opening quotes its part of it, in the first step whose stops show its unit and one of its blocks
  [@test] src/presenters/review-story.test.ts (`quotes What Changes in the opening of the step whose stops show its entry, each entry in one step`)

### Requirement: In the Review Window: The contract of an opening folds past 8 blocks

The opening of a step MUST quote its contract whole up to 8 blocks; past them each file folds under the line of its
sections, its header kept, a click opening it, ⌘F finding its words (the opening is one screen, the spec of a large
step is no wall in front of its code).

#### Scenario: A contract of more than 8 blocks
- **WHEN** the contract of a step holds more than 8 blocks
- **THEN** each file folds under the line of its sections ("5 changes in Storage, Limits and Commands"; of a spec the change adds "New spec (9 requirements)", of a file it deletes "Deleted"), its header kept
- **AND** a click opens it, and ⌘F finds its words

### Requirement: In the Review Window: A code span keeps one chip with its diff inside

A code span MUST keep one chip with its inserted and deleted words diffed inside it.

#### Scenario: A change inside a code span
- **WHEN** a change rewrites words inside a code span
- **THEN** the span stays one chip, its inserted and deleted words diffed inside it
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the paragraphs of a repeated substitution quiet in the openings of their steps, a change inside code inside its chip and a fenced block as code`)

### Requirement: In the Review Window: A fenced code block is a pre diffed by words

A fenced code block MUST be a `pre`, preserve line breaks, and be diffed by words without code heuristics; a long
block changed in several lines first shows changed lines and one line of context, with a button "⋯ N more lines" that
opens the whole paragraph and has the tooltip "Show the whole paragraph".

#### Scenario: A fenced block in What Changes
- **WHEN** What Changes quotes a changed fenced code block
- **THEN** it is a `pre`, its line breaks preserved, diffed by words without code heuristics
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the paragraphs of a repeated substitution quiet in the openings of their steps, a change inside code inside its chip and a fenced block as code`)
  [@test] src/presenters/review-story.test.ts (`passes paragraph forms and resolves substitution marks to their original words`)

#### Scenario: A long block changed in several lines
- **WHEN** a long block is changed in several lines
- **THEN** it first shows changed lines and one line of context
- **AND** a button "⋯ N more lines", its tooltip "Show the whole paragraph", opens the whole paragraph

### Requirement: In the Review Window: The rows of one table stand in one compact table

The rows of one table MUST stand in one compact table (`text-sm`) under its header once, muted (`text-xs`), each row
the change adds, removes or changes with the bar of its change at its first cell on the quote line.

#### Scenario: Rows a change adds, removes or changes
- **WHEN** What Changes quotes rows of a table
- **THEN** they stand in one compact table under its header once, muted: the header row with its marks when the change
  touched it, else the cells of the header of its first row
- **AND** each row shows the bar of its change at its first cell on the quote line, a removed one muted and struck
  through
- **AND** the words of its cells are marked as a paragraph's, the words removed between two cells at the end of the
  first
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should quote no front matter and no line of metadata in What Changes, a table of a spec by its added row under its header, and no table of a doc`)
  [@test] src/presenters/review-story.test.ts (`words the rows of a table under their header once, a changed header row heading them, a paragraph between two tables apart`)

### Requirement: In the Review Window: A link shows the file it names, opens a page, else is text

A link MUST show the file it names at its header when the review shows that file, open a page, else stay text.

#### Scenario: A link to a file of the review, to a page, to another file
- **WHEN** a quoted paragraph holds a link
- **THEN** a link to a file the review shows shows it at its header
- **AND** a link to a page opens it, and any other stays text
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell What Changes in whole paragraphs changed by word, a long one by the sentence of its change, and show the file its link names`)
  [@test] src/presenters/review-story.test.ts (`words What Changes: a file once, a section once with its tests, its paragraphs whole, a link showing a changed file or opening a page`)

### Requirement: In the Review Window: The Steps list each step by its number and full title

The Steps MUST list each step by its number and its full title (the title Rust gives the commit that starts it; a step
of the concerns its concern), wrapping, never cut, which shows its opening, then the lead of its body; "Other changes"
remains as a rest item without a number and MUST be excluded from the count beside Steps and the step count in the bar,
the bar and the hover of the map of the files naming it without a number too.

#### Scenario: The steps of the overview
- **WHEN** the overview lists the steps
- **THEN** each shows its number and its full title, wrapping, never cut, then the lead of its body
- **AND** a click on it shows its opening
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should go through the steps from the overview, by J and K and by the title of an opening, the bar and the tree following`, `should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`, `should show the Look closely of a step as a callout, and open a part folded under it from the mark of its section, its block outlined once`)
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

#### Scenario: Other changes
- **WHEN** the plan has a rest of other changes
- **THEN** "Other changes" is listed without a number
- **AND** it is excluded from the count beside Steps and from the step count in the bar
- **AND** the bar and the hover of the map name it without a number: "Other changes", never "Step 2 · Other changes"
  [@test] src/presenters/review-story.test.ts (`does not give Other changes a number in the overview`)
  [@test] src/presenters/review-steps.test.ts (`words Other changes in the hover of the map of the files by its name alone, as it has no number of a step`)

### Requirement: In the Review Window: The facts of a step name the tests that reach its code

The facts of a step MUST stand in one short line: the files of the tests of the change that reach its code ("Tested by
ManagerTest.kt", each showing its first test, its tests in its hover; the opening tells the rest), the code no test of
the change reaches beside a scenario ("No test of the change reaches load"), "Proven by …".

#### Scenario: Tests that reach the code of a step
- **WHEN** tests of the change reach the code of a step
- **THEN** its facts name their files, each showing its first test, its tests in its hover
- **AND** the code no test of the change reaches beside a scenario, and the scenarios that prove it
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

### Requirement: In the Review Window: The facts name the new API, a new module and a widening

The facts MUST name "New API: Cache and Reader" (the new declarations of the modules that were there), "New module
view.gui.media: 13 declarations" (those of a module the change adds, counted, not named: its whole surface is new) and
"Opens wider: mediaDuration" (a declaration whose reach alone opened wider, `widened`, no new API), each name showing
its place.

#### Scenario: New declarations of a module that was there and of a new module
- **WHEN** a step adds declarations to a module that was there and to a module the change adds
- **THEN** the first are named in "New API: …"
- **AND** the others counted, not named, in "New module …: N declarations", their names in the hover of its label,
  which shows the first
  [@test] src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)

#### Scenario: A declaration whose reach alone opened wider
- **WHEN** the reach alone of a declaration of the step opened wider
- **THEN** the facts name it in "Opens wider: …" (`widened`), no new API

### Requirement: In the Review Window: The opening names the new uses between modules

The facts of the opening MUST name the new uses between modules ("New uses between modules: view.gui.media → view.gui
(38 links) and view.gui.media → shared/code"): a pair of modules once, by their labels, its links counted, never the
pairs of their files; in the opening only, the map of the overview names the modules and their uses.

#### Scenario: New uses between modules of a step
- **WHEN** a step adds uses between modules
- **THEN** its opening names each pair of modules once, by their labels, its links counted, each name showing its place
- **AND** the overview names none, its map names the modules and their uses
  [@test] src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)

### Requirement: In the Review Window: The opening says which units another step shows

The units of a step another step's stop shows ("Adds hooks, shown in step 2") MUST be named in the opening only.

#### Scenario: Units shown in another step
- **WHEN** another step's stop shows units of a step
- **THEN** the opening of the step names them ("Adds hooks, shown in step 2"), the overview does not
  [@test] src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)

### Requirement: In the Review Window: A step no test reaches says so in the warning tone

A step accio answered for whose code no test of the change reaches, every way walked, and no scenario proves MUST say
so in the warning tone ("No test of the change reaches paint"); code accio did not walk every way of MUST NOT be named
there.

#### Scenario: A step no test of the change reaches
- **WHEN** no test of the change reaches the code of a step accio answered for, every way walked, and no scenario
  proves it
- **THEN** its facts say "No test of the change reaches paint" in the warning tone
- **AND** code accio did not walk every way of is not named there
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

### Requirement: In the Review Window: A list of names in the facts names up to four

A list of names in the facts MUST name up to four and from five on the first three and the count ("paint, create, run
and 2 more", `joinNames`), the new API and the new links between modules too ("New API: Cache, Reader, Store and 16
more", each name its place).

#### Scenario: A list of five names or more
- **WHEN** a fact of the overview lists five names or more
- **THEN** it names the first three and the count ("paint, create, run and 2 more")
- **AND** so do the new API and the new links between modules ("New API: Cache, Reader, Store and 16 more")

### Requirement: In the Review Window: The opening names up to nine, the rest on a click

The opening MUST name up to nine, of a longer list the first eight and "and 4 more", a button that shows the rest in
place, the focus on the first name it shows (26 test files and 50 names no test reaches are no wall in front of the
code), and so do the facts of a range that lists no step.

#### Scenario: A long fact in the opening of a step
- **WHEN** a fact of an opening lists more than nine names
- **THEN** it names the first eight and "and 4 more"
- **AND** a click on that button shows the rest in place, the focus on the first name it shows
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should name a long fact in the opening of a step by its first eight names, and the rest in place on a click of "and N more"`)

#### Scenario: The facts of a range that lists no step
- **WHEN** a range lists no step
- **THEN** its facts are cut as the opening cuts them

### Requirement: In the Review Window: A name of a fact is set in the code font, led by its type

A name two new declarations share MUST be led by its type ("Event.rightEdge and Update.rightEdge"), as the code of a
step is; every name of a fact MUST be in the code font.

#### Scenario: Two new declarations of one name
- **WHEN** two new declarations of a step share a name
- **THEN** the facts name each led by its type ("Event.rightEdge and Update.rightEdge"), in the code font

### Requirement: In the Review Window: A Look closely of a step is an info callout of its question

Each "Look closely:" section of the body of a step and of the bodies of its rounds MUST show in the overview as an info
callout, its question alone (its first block): the opening tells the rest.

#### Scenario: A Look closely in the body of a step or of a round
- **WHEN** the body of a step or of one of its rounds has a "Look closely:" section
- **THEN** the step in the overview shows it as an info callout, its first block alone
- **AND** the opening tells the rest

### Requirement: In the Review Window: Steps of the concerns and of the message say why

Steps of the concerns and of the message MUST say why, a muted line right under the count of the steps ("One commit:
the steps are its tasks", "One commit: the steps are the parts of its message"); a muted note MUST NOT be pushed to the
far end of the column, only a sum is.

#### Scenario: The steps of one commit
- **WHEN** the steps are the concerns or the parts of the message of one commit
- **THEN** a muted line right under the count of the steps says why ("One commit: the steps are its tasks", "One commit: the steps are the parts of its message")
- **AND** it stands right there, never pushed to the far end of the column
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: A range of one step of no story lists no step

A range whose only step is what no evidence joined, or one concern that covers it, MUST list no step, and its facts
are the facts of the range.

#### Scenario: One concern that covers the range
- **WHEN** the only step of a range is what no evidence joined, or one concern that covers it
- **THEN** the overview lists no step, and its facts are the facts of the range

### Requirement: In the Review Window: Possible Breakage and Since your review close the overview

After the Steps the overview MUST show Possible Breakage (the uses left outside right after the name of an entry) and
Since your review ([Review Plan](../review-plan/spec.md#requirements)); why the uses are unknown is folded under
Possible Breakage when it lists an entry.

#### Scenario: Possible Breakage with an entry
- **WHEN** Possible Breakage lists an entry
- **THEN** the uses left outside stand right after its name, and a click shows its declaration
- **AND** why the uses are unknown is folded under it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)

### Requirement: In the Review Window: Why accio did not answer is one line above the map

Why accio did not answer MUST be one line in the warning tone above the map, Rust's note and what it costs ("accio has
no index of this repository · the uses left outside and the tests of the steps are unknown"): without accio the review
knows no use left outside and no test of a step, so the whole review is less, not one entry of it.

#### Scenario: A review without accio
- **WHEN** accio did not answer the plan
- **THEN** one line in the warning tone above the map says Rust's note and what it costs ("accio has no index of this repository · the uses left outside and the tests of the steps are unknown")

### Requirement: In the Review Window: The document is one reading column

The document MUST be one reading column in the middle of its island (`ReadingColumn`: `max-w-measure`, 65ch of
`text-md`, its lines wrapped to avoid a short last one), every block ending at its edge and none setting a width of
its own; the number of a step hangs in the gutter, so its title and its lead stand at that edge.

#### Scenario: The blocks of the document
- **WHEN** the document shows its overview and openings
- **THEN** every block ends at the edge of the reading column, none setting a width of its own
- **AND** the number of a step hangs in the gutter, its title and its lead at that edge

### Requirement: In the Review Window: The type of the document is one scale

The type of the document MUST be one scale (docs/design/ui-theme.md): its prose `text-md`, its headline (and the
subject of a commit shown alone) `text-lg font-semibold`, the title of an opening `text-base font-semibold`, file
headers and section headings `text-sm font-semibold`, no section heading larger than its file header.

#### Scenario: The sizes of the document
- **WHEN** the document is shown
- **THEN** its prose is `text-md`, its headline `text-lg font-semibold`, the title of an opening `text-base font-semibold`
- **AND** the What Changes and Steps labels both `text-sm font-semibold text-foreground`
- **AND** file headers and section headings `text-sm font-semibold`, the filename foreground, its directory muted
- **AND** an open part `font-semibold`, a step of the contents `font-medium`, its facts `text-xs` muted

### Requirement: In the Review Window: The count line, Dependencies, Repeated Edits and New API are gone

The count line of the old card, its Dependencies, Repeated Edits and New API MUST be gone: the dependency changes and
the same edit are on the rows of the tree and the divider "Same edit in N files", the new API and the links between
modules are facts of their step.

#### Scenario: Where the old card's parts are
- **WHEN** the review is shown with the plan
- **THEN** no card of counts, Dependencies, Repeated Edits or New API is shown
- **AND** the dependency changes and the same edit are on the rows of the tree and the divider "Same edit in N files"
- **AND** the new API and the links between modules are facts of their step

### Requirement: In the Review Window: A group of a large step opens with a divider

A group of a large step ([Groups of a Step](#groups-of-a-step)) MUST open with a divider above the card of its first
section, one muted line: the contract ("Contract · 5 files"), a module by its label, "New module" or "Deleted module"
when the change adds or deletes it, its files and the small modules it holds, a folder by its path.

#### Scenario: The dividers of the groups of a large step
- **WHEN** a large step has groups
- **THEN** the contract opens with "Contract · 5 files"
- **AND** a module by its label, its state, its files and the small modules it holds ("view.gui.media · New module · 59 files · also shared/code"), a folder by its path
- **AND** the tests of a group a divider of their own ("view.gui.media · Tests · 18 files")
- **AND** the mechanical group none, its kinds head their sections as before (`groupHeadings`)
  [@test] src/presenters/review-steps.test.ts (`words the divider of each group and of its tests: the contract, a module by its label and state with the small modules it holds, a folder; the mechanical group none`)

### Requirement: In the Review Window: The opening of a step is a card of prose above its first section

The opening of a step (`ChapterOpening`) MUST be a card of prose a gap above the card of its first section (each
opening of a layer its own card), as tall as its words (the diffs measure it), its words in the reading column of the
overview (`ReadingColumn`) in the middle of the card.

#### Scenario: The opening of a step in the diffs
- **WHEN** the story of the diffs reaches a step
- **THEN** its opening is a card of prose a gap above the card of its first section, as tall as its words
- **AND** its words stand in the reading column of the overview in the middle of the card
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

### Requirement: In the Review Window: An opening shows the number and the full title of its step

An opening MUST show "Step 2" muted for a numbered step, none for "Other changes", then its full title in `text-base`,
wrapping, which a click shows at the top.

#### Scenario: The heading of an opening
- **WHEN** an opening of a numbered step is shown
- **THEN** it shows "Step 2" muted, then its full title in `text-base`, wrapping
- **AND** a click on the title shows it at the top
- **AND** the opening of "Other changes" shows no number
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

### Requirement: In the Review Window: An opening tells the body of the commit that starts its step

An opening MUST tell the body of the commit that starts its step by the grammar of a body ([Commit Row → Message
body](../../commit/commit-row/spec.md)) in `text-md`: its lead a step larger (`text-base`), its paragraphs, lists,
labels as the captions of their blocks, code and measurements in the monospace font, a "Look closely:" section an info
callout, no trailer.

#### Scenario: The body of the commit of a step
- **WHEN** the opening of a step of the commits is shown
- **THEN** it tells the body of the commit that starts it in `text-md`, its lead a step larger (`text-base`)
- **AND** its labels are the captions of their blocks, code and measurements in the monospace font
- **AND** a "Look closely:" section is an info callout, and no trailer is shown
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should show the Look closely of a step as a callout, and open a part folded under it from the mark of its section, its block outlined once`)
  [@test] src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)

### Requirement: In the Review Window: An opening shows what the body changed since the review

An opening MUST show no muted text but what its body removed since the review, struck through where Rust placed it,
beside the blocks it added, marked ([Review Plan](../review-plan/spec.md#in-the-review-window)), and the subject its
commit had below its title.

#### Scenario: A body changed since the review
- **WHEN** the message of the commit of a step changed since the review
- **THEN** its opening shows what the body removed struck through where Rust placed it, beside the blocks it added,
  marked
- **AND** the subject its commit had below its title

### Requirement: In the Review Window: A long body shows its shown parts and one line of the folded

A long body MUST show the parts Rust shows (`parts`), then one line of what is still folded, one chevron for it
("More: The cause, The fix, Tests, Evidence", a part of no label "The rest"), each name its own toggle; an opened part
stands below that line in the order of the body, its label the toggle that folds it again (a chevron before it).

#### Scenario: A part opened from the line of the folded
- **WHEN** the user opens a name of the line of what is still folded
- **THEN** the part stands below that line in the order of the body, its label the toggle that folds it again, a
  chevron before it
- **AND** its name shows once and the text grows downward; the line names only what is still folded
  [@test] src/presenters/review-story.test.ts (`shows the lead of a body and its parts before the fold, the rest and every verification section folded, each named in the outline`)

### Requirement: In the Review Window: A verification section is folded, Look closely and Left out shown

A verification section (Tests, Evidence, Gates, Verified, Results, Proof) MUST be folded always; the author's "Look
closely:" and "Left out:" MUST show always, in their place among the shown parts.

#### Scenario: A body of a verification section and a Look closely
- **WHEN** a body has a verification section, a "Look closely:" and a "Left out:"
- **THEN** the verification section is folded
- **AND** "Look closely:" and "Left out:" show in their place among the shown parts
  [@test] src/presenters/review-story.test.ts (`shows the lead of a body and its parts before the fold, the rest and every verification section folded, each named in the outline`)

### Requirement: In the Review Window: The other commits of a step are rows of one form

After the body the opening MUST list the other commits of the step, each one row of one form at the size of the
text: a muted "Follows up" or "Without a story", its title wrapping at the width of the prose; a commit with a story a
disclosure (`Disclosure`) whose body is set `nested`, never larger than its row.

#### Scenario: Follow-ups and commits without a story
- **WHEN** a step has follow-ups and commits without a story
- **THEN** each is a row of a muted "Follows up" or "Without a story" and its title, wrapping at the width of the prose
- **AND** a commit with a story is a disclosure, one disclosure of the document: the folds of a body, these rows and
  what the overview folds; its body set `nested`
  [@test] src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: The facts of an opening name its tests with their way

The facts of an opening MUST stand muted, each fact a line of its own and every name in the code font showing its
place in the diffs: the files of the tests of the change, each its tests in its hover with their way, its code no test
of the change reaches, each at its declaration, the name selected, nothing of code accio did not walk every way of.

#### Scenario: Tests of the change and code none reaches
- **WHEN** the opening of a step names its facts
- **THEN** the files of the tests of the change show their tests in the hover with their way ("Tested by ManagerTest.kt"; "opens: calls open", "opens: reaches open through the test code", "opens: reaches open end to end")
- **AND** its code no test reaches shows each name at its declaration, selected ("No test of the change reaches show")
- **AND** code accio did not walk every way of is not named
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should go through the steps from the overview, by J and K and by the title of an opening, the bar and the tree following`)
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

### Requirement: In the Review Window: The facts of an opening name what its step proves and adds

The facts of an opening MUST name the steps its tests prove ("Proves step 2", showing that step), "New API: allowed",
"Opens wider: format", of a task what joins its parts, its names code, the warning of a step no test reaches, its
names code too, and the group its section starts.

#### Scenario: The facts of what a step proves and adds
- **WHEN** the tests of a step prove another and it adds API
- **THEN** its opening names "Proves step 2", showing that step, "New API: allowed" and "Opens wider: format", each name
  in the code font showing its place
- **AND** of a task what joins its parts, the warning of a step no test reaches, and the group its section starts
  [@test] src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)

### Requirement: In the Review Window: ⌘F searches the whole review document

⌘F (Ctrl+F off macOS) SHALL search the review document from its overview through the openings and diff rows to the
epilogue. Story text MUST be matched ignoring case, one block at a time, with `.sr-only`, `[aria-hidden=true]` and
`[data-find-skip]` excluded.

#### Scenario: Text of the overview and the openings before the diff rows
- **WHEN** the user searches with ⌘F a word the overview, an opening and diff rows hold
- **THEN** the hits of the overview and the opening come before the matching diff rows, in document order
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should find overview and opening text before the matching diff rows, and open a folded follow-up`)

#### Scenario: Text of the epilogue
- **WHEN** the user searches story text the epilogue holds
- **THEN** the find reaches it and scrolls to the hit
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should search story text in the epilogue and scroll to the hit`)

### Requirement: In the Review Window: A hit in a folded part opens it

Folded story parts and follow-up `Disclosure`s MUST stay hidden in the document and open when the find goes to their
hit; text and diff hits MUST share one document-order count and text hits are highlighted without changing the DOM
React owns.

#### Scenario: A hit in a folded follow-up or a closed body
- **WHEN** the find goes to a hit in a folded part or a closed follow-up
- **THEN** the part opens and the hit stays current
- **AND** text and diff hits share one count, in document order, the text hits highlighted without changing the DOM
  React owns
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should find overview and opening text before the matching diff rows, and open a folded follow-up`, `should open a hit in a closed story body and keep it current`)

### Requirement: In the Review Window: ⌘F works anywhere in the review window

⌘F MUST work anywhere in the review window, including the sidebar; ⌘G and Shift+⌘G step through hits there while the
bar is open.

#### Scenario: Find from the file tree
- **WHEN** the focus is in the file tree and the user presses ⌘F
- **THEN** the find bar opens, and ⌘G and Shift+⌘G step through the hits there while it is open
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should open find from the file tree and step results there with ⌘F and ⌘G @webkit`)

### Requirement: In the Review Window: A task of one commit opens with the parts of its body it tells

A step of the concerns of one commit MUST open with the parts of the body of that commit told in it (`told`), the
label Rust made its title (`titled`) not shown again, and say what joins its parts in the line of its facts ("Joined
by the identifier `accio`"); the lead and the parts told before the steps are in the overview.

#### Scenario: The parts of one commit in its tasks
- **WHEN** a step of the concerns of one commit opens
- **THEN** it shows the parts told in it (`told`), the label of its title (`titled`) not again
- **AND** the line of its facts says what joins its parts ("Joined by the identifier `accio`")
- **AND** the lead and the parts told before the steps are in the overview (in the heading of the commit shown
  alone), so each part of the body is in one place
  [@test] src/presenters/review-story.test.ts (`tells each part of the body of one commit where Rust places it, the label of the title of a step not again, the lead and the rest in the overview`)

### Requirement: In the Review Window: Each part of a story of several commits opens its commit's step

A story of several commits MUST keep the order of its author: each part in the opening of its commit's step.

#### Scenario: The bodies of several commits
- **WHEN** the steps are the commits of a range
- **THEN** each part of a body is in the opening of the step of its commit, in the order of its author

### Requirement: In the Review Window: A step of the message opens with its parts, no commit row

A step of the message MUST open as a step of the concerns does, with the parts told in it, without the row of the
commit (its one commit is the row of no step: the tree lists it in the first step).

#### Scenario: The opening of a step of the message
- **WHEN** a step of the message opens
- **THEN** it shows the parts told in it, without the row of the commit
- **AND** the tree lists its one commit in the first step
  [@test] src/presenters/review-story.test.ts (`tells each part of the message of one commit in the step it is, a title of its first sentence read in its paragraph, a step larger, the commit no step's row`)

### Requirement: In the Review Window: A step titled by its first sentence reads it once

A step titled by the first sentence of its paragraph (`titleSource: "sentence"`) MUST show "Step 2" and that paragraph
as its lead, a step larger, the title only for a screen reader, so the sentence reads once; a step titled by a code
name MUST set it as code.

#### Scenario: A title of the first sentence
- **WHEN** a step of the message is titled by the first sentence of its paragraph
- **THEN** its opening shows "Step 2" and that paragraph as its lead, a step larger
- **AND** the title is only for a screen reader, so the sentence reads once
  [@test] src/presenters/review-story.test.ts (`tells each part of the message of one commit in the step it is, a title of its first sentence read in its paragraph, a step larger, the commit no step's row`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell each step of one commit by the paragraph of its message that names its code, titled by its first sentence, which its opening reads a step larger`)

#### Scenario: A title of a code name
- **WHEN** a step of the message is titled by a code name
- **THEN** its title is set as code

### Requirement: In the Review Window: The overview, the tree and the bar name a step by its title

The overview, the tree and the bar of the step MUST name a step of the message by its title, the tree after its number
("2 · …").

#### Scenario: The names of a step of the message
- **WHEN** a step of the message is listed
- **THEN** the overview, the tree and the bar of the step name it by its title, the tree after its number ("2 · …")
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell each step of one commit by the paragraph of its message that names its code, titled by its first sentence, which its opening reads a step larger`)

### Requirement: In the Review Window: The tree lists the steps, each a group of its files

The tree MUST list the steps, each a group "2 · The open keeps a stored layout" with its files by their path (a step
of the concerns and of the message too, its number in the font of the text before a code name), a file of two steps
with its row in the step of its most lines and a row "also in" each other.

#### Scenario: The steps in the tree
- **WHEN** the review shows the plan
- **THEN** the tree lists each step as a group "2 · The open keeps a stored layout" with its files by their path
- **AND** a file of two steps has its row in the step of its most lines and a row "also in" each other
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)
  [@test] src/presenters/review-steps.test.ts (`names a step by the title Rust gives it, in the case its author wrote it, a step of no story by what it is`)

### Requirement: In the Review Window: A task of one commit is named by the label it tells

A step of the concerns MUST be named by the label of the first section of the body of its commit it tells (`title`,
"The splash"; a verification and a Look closely aside), else by its concern, an identifier set in the code font
wherever the step is named, and keeps its main rows ([Review Concerns](../review-concerns/spec.md#in-the-review-window)).

#### Scenario: A task of a labelled section
- **WHEN** a step of the concerns tells a labelled section of the body of its commit
- **THEN** it is named by the label of the first one it tells ("The splash")
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should title each task of one commit by the label of the section of its body it tells`)

#### Scenario: A task of no label
- **WHEN** a step of the concerns tells no labelled section
- **THEN** it is named by its concern, an identifier set in the code font wherever the step is named: the tree, the
  contents, its opening, the bar of the step, the heading of a card of a story mark
- **AND** it keeps its main rows
  [@test] src/presenters/review-steps.test.ts (`names a step of a concern by the label of the section of the body it tells, else by its concern as code, keeps its main rows, and tells no step of a story Rust tells in none`)

### Requirement: In the Review Window: The one concern that covers the range is no story

The step of the one concern that covers the whole range MUST be no story (`ReviewSteps.told` false, which the plan
decides): no Steps in the overview, which tells the whole body, no opening, no place in the bar of the step, and the
tree lists the files as without the plan.

#### Scenario: One concern of the whole range
- **WHEN** one concern covers the whole range
- **THEN** the overview has no Steps and tells the whole body
- **AND** the step has no opening and no place in the bar of the step, and the tree lists the files as without the
  plan
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell no step of the one concern that covers the range: no Steps, no opening, no place in the bar`)
  [@test] crates/review-plan/src/steps_test.rs (`the_concerns_are_the_steps_when_the_commits_tell_no_story`)
  [@test] src/presenters/review-steps.test.ts (`names a step of a concern by the label of the section of the body it tells, else by its concern as code, keeps its main rows, and tells no step of a story Rust tells in none`)

### Requirement: In the Review Window: The story of the diffs follows the steps

The story of the diffs MUST follow the steps: the first section of each starts with its opening right above its card,
its heading "Step 2 · The open keeps a stored layout" (`review-concern-heading`); a click on it MUST show the step as
the bar of the step does, the opening at the top, the bar and the tree following.

#### Scenario: The opening right above the first card of a step
- **WHEN** the diffs show the first section of a step
- **THEN** its opening stands right above its card, at its measured height, its heading "Step 2 · The open keeps a stored layout"
- **AND** a click on the heading shows the step at the top, the bar and the tree following
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)
  [@test] src/lib/diff/layout.test.ts (`puts the opening of a step at its measured height and the divider of a group between the gap and its card: a scroll to the file shows it, a y in it is the file below`)

### Requirement: In the Review Window: The header of a file names the file alone

The header of a file MUST name the file alone (no step, no group), so the name of every file stands at one x.

#### Scenario: The headers of the files of a step
- **WHEN** the diffs show the cards of a step
- **THEN** each header names its file alone, no step and no group, every name at one x

### Requirement: In the Review Window: The groups of a step are its headings in the tree

A step with groups ([Groups of a Step](#groups-of-a-step)) MUST list them in the tree as the headings of the step, not
as one more level: under its row and its commit rows a row per group at the indent of its files, its icon of what it
holds, its name, its line counts and the files of the chosen owner chip ("3 of 13"), the root of its module in its
hover.

#### Scenario: The rows of the groups
- **WHEN** a step with groups is listed in the tree
- **THEN** a row per group stands at the indent of its files, its name "Contract", the label of its module in the code
  font, its folder, the mechanical group "Wiring" while it holds only the build model and the lists of modules, else
  "Mechanical"
- **AND** its line counts and the files of the chosen owner chip ("3 of 13")
  [@test] src/lib/file-tree.test.ts (`lists a group per section of the step, its files by name with the folder Rust gives them in their module, its tests last, the build files under Wiring`, `words the mechanical group Mechanical when it holds more than build files and lists of modules, and keeps a step without groups a list`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should read a large step by its groups: their files flat by their directory in the module, their tests last, a new module marked, the wiring folded at the end`)

### Requirement: In the Review Window: A new or deleted module is marked on its group

A module the commits add or delete MUST show "New module" (`success`) or "Deleted module" (muted) after its name, the
small modules it holds muted on a second line ("also shared/code") with its counts.

#### Scenario: The group of a new module
- **WHEN** a group is of a module the commits add
- **THEN** its row shows "New module" after its name, the small modules it holds muted on a second line with its
  counts
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should read a large step by its groups: their files flat by their directory in the module, their tests last, a new module marked, the wiring folded at the end`)

### Requirement: In the Review Window: The files of a group are flat, by name and folder

The files of a group MUST be listed flat in the order of its stops, each once, at the first of its stops, each its
name first and then, muted, its `folder` ([Review Plan → Modules](../review-plan/spec.md#modules)); its tests last,
under the muted heading "Tests".

#### Scenario: The files of a group of a module
- **WHEN** the tree lists the files of a group
- **THEN** each shows its name, then muted its directory below what the code and the tests of its module share, the
  resources apart (`embeds`, `markdown`; a test under `testSrc` in the folder of its code, as the map names it),
  nothing when that is its own
- **AND** a file of a small module by the label of that module, a file of the contract and of the mechanical group by
  its whole directory
- **AND** a mechanical file in the mechanical group, as the groups of its kinds ("Build files (4)"), and its tests last
  under "Tests"
  [@test] src/lib/file-tree.test.ts (`lists a group per section of the step, its files by name with the folder Rust gives them in their module, its tests last, the build files under Wiring`)

#### Scenario: A file whose own row is in another step
- **WHEN** a file of a group has its own row in another step
- **THEN** it is a row also in that group, by its directory too
  [@test] src/lib/file-tree.test.ts (`lists a file of a group whose own row is in another step as a row also in that group, by its directory too`)

### Requirement: In the Review Window: The heading of a group sticks and folds as a directory

The heading of a group MUST stick right below the row of its step while its files scroll under it, and fold as a
directory does (a click, its chevron, Space, → and ←; ← from a file goes to it); the mechanical group starts folded,
and in a review of more than 100 files every group but its tests.

#### Scenario: Folding the groups
- **WHEN** the tree shows a step with groups
- **THEN** the mechanical group starts folded, and in a review of more than 100 files every group but its tests
- **AND** a click, its chevron, Space, → and ← fold a group, ← from a file going to it
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should read a large step by its groups: their files flat by their directory in the module, their tests last, a new module marked, the wiring folded at the end`)

### Requirement: In the Review Window: The dependency changes of a module show on its group

The dependency changes of a build file of a module of the step MUST show on the row of its group, where a module the
commits add or delete leaves its own new module to its badge.

#### Scenario: A build file of a module of the step
- **WHEN** a build file of a module of the step changes its dependencies
- **THEN** the changes show on the row of its group
- **AND** of a module the commits add, its own new module is left to its badge
  [@test] src/lib/file-tree.test.ts (`shows the dependency changes of the build file of a module on the row of its group, the new module its badge alone`)

### Requirement: In the Review Window: The rows of the step of the selected file are tinted

The rows of the step of the selected file MUST be tinted, and a line in the primary tone at the chevron of the step
runs from under its row to its last row, under its row when that row sticks, never across its chevron.

#### Scenario: A file of a step selected
- **WHEN** a file of a step is selected
- **THEN** the rows of its step are tinted, a line in the primary tone at the chevron of the step from under its row
  to its last row, under its row when that row sticks, never across its chevron

### Requirement: In the Review Window: A step without groups is listed as before

A step without groups (a small step, a step of the concerns, a review without the plan) MUST be listed as before.

#### Scenario: A small step
- **WHEN** a step has no groups
- **THEN** the tree lists it as before, its files a list
  [@test] src/lib/file-tree.test.ts (`words the mechanical group Mechanical when it holds more than build files and lists of modules, and keeps a step without groups a list`)

### Requirement: In the Review Window: A group of a module or a folder opens above its first card

Each group of a module or of a folder MUST open above its first card, in place of its divider, a card of prose as the
opening of a step, lighter: its name (a module's label in the code font, its root in its hover; a folder), "New
module" (`success`) or "Deleted module", the small modules it holds ("also shared/code"). The contract and the wiring
MUST keep their divider.

#### Scenario: The opening of a group of a module
- **WHEN** the diffs reach the first card of a group of a module
- **THEN** a lighter card of prose opens it, in place of its divider: its label in the code font, its root in its
  hover, "New module" or "Deleted module", the small modules it holds
- **AND** the contract and the wiring keep their divider
  [@test] src/presenters/review-story.test.ts (`opens each group of a module of a large step above its first card: its name, its state, what enters a new module, its tests, its new API by name, its uses between modules by their direction, what its module registers and its texts; the opening of the step keeps none of them`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should open each group of a module above its first card, what it is, its new API, its new uses between modules, what it registers and its texts of the UI, and leave them out of the opening of the step`)

### Requirement: In the Review Window: The opening of a new module's group names what enters it

The opening of the group of a new module MUST first name, a line each in the facts' tone, what enters it, each at its
declaration, its qualified name and the files that use it in the hover; of a complete answer of none "Nothing outside
the module uses it", of a partial one "accio stopped before it asked every declaration", nothing without an answer.

#### Scenario: What enters a new module
- **WHEN** accio answers what enters a module the commits add
- **THEN** the opening of its group first names each ("Entered by TranscriptEmbedPlayersImpl (applicationService of
  TranscriptEmbedPlayers)", "Flags (used in Embed0.kt)"), at its declaration
- **AND** its qualified name and the files that use it are in the hover
  [@test] src/presenters/review-story.test.ts (`opens each group of a module of a large step above its first card: its name, its state, what enters a new module, its tests, its new API by name, its uses between modules by their direction, what its module registers and its texts; the opening of the step keeps none of them`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should name what enters a new module in the opening of its group, read the group from the file of its entry point, and show its declaration on a click`)

#### Scenario: A new module nothing enters, or accio stopped
- **WHEN** accio answers completely that nothing outside the module uses it, or stops before it asked every
  declaration, or gives no answer
- **THEN** the opening says "Nothing outside the module uses it", or "accio stopped before it asked every
  declaration", or nothing

### Requirement: In the Review Window: The opening of a group says what its code does to the rest

The opening of a group MUST say, a line each in the facts' tone, after what enters a new module, what its code does to
the rest: the tests of the
change that reach its code and its code none reaches (`tested`, in the warning tone when none is reached), its new
API by name (of a new module too: the group is that module), what it opens wider, its new uses between modules by
their direction, each name showing its place, every list cut as the opening of a step cuts one.

#### Scenario: The facts of a group
- **WHEN** a group of a module opens
- **THEN** it names its tests and its code none reaches, its new API by name, what it opens wider
- **AND** its new uses between modules by their direction ("Now uses view.gui (32 links) and shared/code", "Now used by view.gui.media (6 links)")
  [@test] src/presenters/review-story.test.ts (`opens each group of a module of a large step above its first card: its name, its state, what enters a new module, its tests, its new API by name, its uses between modules by their direction, what its module registers and its texts; the opening of the step keeps none of them`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should open each group of a module above its first card, what it is, its new API, its new uses between modules, what it registers and its texts of the UI, and leave them out of the opening of the step`)

### Requirement: In the Review Window: The opening of a group names its registrations and texts

The opening of a group MUST name what its module registers in the platform (a line each, as the row of the map, a
click showing the class) and its texts of the UI (as the map's line, in the facts' tone).

#### Scenario: A module that registers a service and adds texts
- **WHEN** the module of a group registers in the platform and adds texts of the UI
- **THEN** its opening names each registration a line, as the row of the map, a click showing the class
- **AND** its texts of the UI as the map's line, in the facts' tone
  [@test] src/presenters/review-story.test.ts (`opens each group of a module of a large step above its first card: its name, its state, what enters a new module, its tests, its new API by name, its uses between modules by their direction, what its module registers and its texts; the opening of the step keeps none of them`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should open each group of a module above its first card, what it is, its new API, its new uses between modules, what it registers and its texts of the UI, and leave them out of the opening of the step`)

### Requirement: In the Review Window: The opening of a step whose groups open keeps only the whole step

The opening of a step whose groups open MUST keep its story, its rounds, its contract and what is of the whole step
(Proven by, Proves, shown in another step, taken out), and say no tests, new API, widening or uses between modules of
its own: its groups say them where their code is.

#### Scenario: A step whose groups open
- **WHEN** a step's groups of modules open
- **THEN** the opening of the step keeps its story, its rounds, its contract, Proven by, Proves, shown in another step
  and taken out
- **AND** it says no tests, new API, widening or uses between modules of its own
  [@test] src/presenters/review-story.test.ts (`opens each group of a module of a large step above its first card: its name, its state, what enters a new module, its tests, its new API by name, its uses between modules by their direction, what its module registers and its texts; the opening of the step keeps none of them`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should open each group of a module above its first card, what it is, its new API, its new uses between modules, what it registers and its texts of the UI, and leave them out of the opening of the step`)

### Requirement: In the Review Window: The window counts the files of a step the reader saw

The window MUST count the files of a step with groups the reader saw, on the row of the step and of each of its groups
from the first one ("3 / 13 seen", muted, after the counts of the step and before those of a group; in the
description of the row too).

#### Scenario: The counts of the files seen
- **WHEN** the reader saw files of a step with groups
- **THEN** the row of the step and of each of its groups show "3 / 13 seen", muted, after the counts of the step and
  before those of a group, and in the description of the row
  [@test] src/lib/file-tree.test.ts (`counts the files of an owner chip and the files seen on the step and on each of its groups`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should count the files of a group the reader saw: a file shown, a card scrolled entirely through the diffs, and say in the bar above the diffs where the reader is`)

### Requirement: In the Review Window: A file is seen when shown on purpose or scrolled through

A file MUST be seen once the window showed it on purpose (its row, J and K at its card, N and P, a place of a mark or
of the document) or its card passed entirely through the viewport of the diffs, its top and its bottom each in view at
some time, its diff read (`DiffLayout.passedCards`: the cards in view at each scroll, no work per frame); a file in
parts once each of its parts was.

#### Scenario: A card scrolled entirely through the diffs
- **WHEN** the top and the bottom of a card were each in view, at once or one scroll after the other
- **THEN** its file is seen, never a card whose diff is not read
- **AND** the card at the top as the diffs scroll is no file shown on purpose
  [@test] src/lib/diff/layout.test.ts (`reports a card once both its top and its bottom were in view, at once or one scroll after the other, and never a card whose diff is not read`)

#### Scenario: A file shown on purpose
- **WHEN** the window shows a file on purpose: its row, J and K at its card, N and P, a place of a mark or of the
  document
- **THEN** the file is seen
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should count the files of a group the reader saw: a file shown, a card scrolled entirely through the diffs, and say in the bar above the diffs where the reader is`)

#### Scenario: A file in parts
- **WHEN** the reader saw the first part of a file the story shows in three, then the two others
- **THEN** the file is seen only after the third part, and a new head counts its parts from zero
  [@test] src/stores/review-seen.test.ts (`sees a file of one card at its card, a file the story shows in parts once each of its parts was seen`)

### Requirement: In the Review Window: A card of a file in parts names its part and its own lines

A card of a file the story shows in two sections or more MUST say which part of the file it is, in the order of the
story ("part 1 of 3"), and the lines its own blocks add and remove as the engine counts them under `Default` (Rust's
count of each block), in place of the counts of the file; a part of deletions alone too ("+0 −4").

#### Scenario: A file in three parts
- **WHEN** the story shows a file in three sections: a new declaration, a removal and an import
- **THEN** their headers say "part 1 of 3 +5 −0", "part 2 of 3 +0 −4" and "part 3 of 3 +1 −0"
- **AND** a file the story shows in one section keeps the counts of the file and names no part
  [@test] src/presenters/review-units.test.ts (`numbers the parts of a file the story shows in several sections, each with the lines of its own blocks, a part of deletions alone too`, `names no part of a file the story shows in one section`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: The lines of a block
- **WHEN** Rust counts the lines of a block that reaches the end of a file and holds the empty line after its final
  line end
- **THEN** the count leaves that line out, so the blocks of the file sum to the engine's count of it
  [@test] crates/review-plan/src/units_test.rs (`each_block_counts_its_lines_as_the_engine_does`)

### Requirement: In the Review Window: The footer names the part of a file at the top

While a card of a file in parts is at the top, the footer MUST name its part after the file ("Step 1 · File 2 / 7 ·
part 2 of 3"), as J and K walk the parts.

#### Scenario: J through the parts of a file
- **WHEN** J walks from the first part of a file to its second and third
- **THEN** the footer says "part 1 of 3", "part 2 of 3" and "part 3 of 3" after the file, nothing at an opening
  between them
  [@test] src/lib/review-position.test.ts (`names the part at the top of a file the story shows in several sections after the file`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

### Requirement: In the Review Window: A fold names the changes another section shows

A fold MUST count only the unchanged lines it hides and name after them each place that shows a block it hides, with
the count of those blocks ("⋯ 9 unchanged lines · 1 change in part 2", "in Other changes" of another concern, a group
by its name alone, "in Imports only", never the count of its heading), a click there showing that section; it MUST
name the lines of a block the whitespace policy ignores ("· 1 line hidden by Whitespace").

#### Scenario: A fold over the change of another part
- **WHEN** a fold of a section hides a block another section shows: an import the group of the imports shows
- **THEN** it counts the unchanged lines alone, 66 where the old side holds 66 and the new one 67
- **AND** it says "1 change in part 2", and a click there shows that section, expanded
  [@test] src/lib/diff/rows.test.ts (`names in a fold of a section the blocks another section shows that it hides, and counts only its unchanged lines`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A fold over the change of a group
- **WHEN** a fold of a section hides a block a group shows: the import of the group headed "Imports only (1)", a line
  of a repeated edit on a file after the first of the group
- **THEN** it names the group by its name alone, "1 change in Imports only", "1 change in Same edit in 2 files"
  [@test] src/presenters/review-units.test.ts (`names a block a group shows by the name of the group on each of its files, never by the count of its heading`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A fold over a line the policy ignores
- **WHEN** a fold hides blocks the whitespace policy ignores: a deleted blank line, a line indented again
- **THEN** it says "24 unchanged lines · 2 lines hidden by Whitespace"
  [@test] src/lib/diff/rows.test.ts (`counts the unchanged lines a fold hides, apart from the lines of the blocks the policy ignores there`)

### Requirement: In the Review Window: The files seen are the state of the window by the head

The files seen MUST be the state of the window (`src/stores/review-seen.ts`), never persisted, by the head reviewed:
another scope of the same head keeps it, a new head counts from zero, as Approve counts the files it opened.

#### Scenario: Another scope and a new head
- **WHEN** the reader switches to another scope of the same head
- **THEN** the files seen are kept
- **AND** a new head counts from zero, and nothing is persisted

### Requirement: In the Review Window: The bar above the diffs says where the reader is in a group

The bar above the diffs MUST say where the reader is: after the step of the section at the top its group and the part
of it seen ("Step 1 / 2 · Play local media… › view.gui.media · 12 / 59 seen"), none for a step read as one list.

#### Scenario: A section of a group at the top
- **WHEN** a section of a group of a step is at the top of the diffs
- **THEN** the bar shows its step, its group and the part of it seen ("Step 1 / 2 · Play local media… › view.gui.media · 12 / 59 seen")
- **AND** a step read as one list shows no group
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should count the files of a group the reader saw: a file shown, a card scrolled entirely through the diffs, and say in the bar above the diffs where the reader is`)

### Requirement: In the Review Window: The story is the one list of the sidebar

The story MUST be the one list of the sidebar: with the plan, a scope of a range of two commits or more ("All
Changes", "Since Your Review" of a branch, of a stack, of a pull request) lists its commits in the tree.

#### Scenario: The plan of a pull request lists its commits in the tree
- **WHEN** the review shows the plan of a range of two commits or more, as of a pull request
- **THEN** its commits are listed in the tree
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`)

### Requirement: In the Review Window: The group row of a step of the commits is its commit

The group row of a step of the commits MUST be the commit that starts it ("2 · The open keeps a stored layout": no row
of its own repeats the title); under it the rows of the commits that follow it up and of those without a story, each
by its title, which gets the width of the row, then its files.

#### Scenario: The rows of a step, its rounds and its commits of no story
- **WHEN** the tree lists a step of the commits
- **THEN** its group row is the commit that starts it, no row of its own repeating the title
- **AND** under it the rows of its follow-ups and of its commits without a story, each by its title, then its files
- **AND** the indent says what each is, the description of the row says it in words ("follows up", "no story"), no
  word on the row
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should make the row of a step its commit and its rounds rows of it, their titles whole at any width, and open the card of the commit by the hover of its mark or subject and by I`)
  [@test] src/lib/file-tree.test.ts (`makes the group of a step the commit that starts it, and lists its rounds and the commits of no story without the issue key, in the case its author wrote it`)

### Requirement: In the Review Window: A directory label is whole or truncated from its start

A file of a group MUST show its directory after its name whole or not at all, never cut to its last letters. A normal
directory label MUST be truncated from the beginning so its deepest path segments stay visible; group titles and step
names are unchanged.

#### Scenario: A long directory of a file
- **WHEN** the directory of a file does not fit its row
- **THEN** a file of a group shows its directory whole or not at all
- **AND** a normal directory label is truncated from the beginning, its deepest path segments visible

### Requirement: In the Review Window: A commit is listed once, of a task the most lines first

A group of a step of the concerns MUST list its commit rows as they are, the most lines first. A commit several
concerns list MUST be listed once, in the one that lists it first, the first of equal ones; a step that lands nothing
keeps its group for its commits.

#### Scenario: A commit of two concerns
- **WHEN** several concerns list one commit
- **THEN** it is listed once, in the one that lists it first, the first of equal ones
- **AND** a group of a step of the concerns lists its commit rows the most lines first
  [@test] src/presenters/review-steps.test.ts (`places each commit of the range in the step it tells or follows up, the one that starts a step of the commits as its step, a commit of the concerns once, the rest in Other commits`)

#### Scenario: A step that lands nothing
- **WHEN** nothing of a step lands
- **THEN** its group stays for its commits
  [@test] src/lib/file-tree.test.ts (`keeps the group of a step that lands nothing for its commits, and lists every commit under Commits without a step`, `keeps the group of a step that lands nothing until narrowed, then lists only steps with shown files`)

### Requirement: In the Review Window: The commits no group lists close the tree

The commits no group lists (of no step, of what no evidence joined) MUST close the tree in "Other commits" ("Commits"
when the tree has no step), by their titles (a commit no step lists by its subject), as the range lists them. The plan
names a commit short, the range in full: they match by prefix.

#### Scenario: Commits of no group
- **WHEN** some commits of the range are of no step or of what no evidence joined
- **THEN** they close the tree in "Other commits", by their titles, as the range lists them
  [@test] src/lib/file-tree.test.ts (`lists the commits of each step above its files, and those of no group (what no evidence joined, no step) last in Other commits, as the range lists them`)
  [@test] src/presenters/review-steps.test.ts (`places each commit of the range in the step it tells or follows up, the one that starts a step of the commits as its step, a commit of the concerns once, the rest in Other commits`)

#### Scenario: A tree of no step
- **WHEN** the tree has no step
- **THEN** every commit is listed under "Commits"
  [@test] src/lib/file-tree.test.ts (`keeps the group of a step that lands nothing for its commits, and lists every commit under Commits without a step`)

### Requirement: In the Review Window: A scope lists only its own commits

A scope MUST list only its own commits: "Since Your Review" of a pull request the commits after the reviewed one, not
those the user saw.

#### Scenario: Since Your Review of a pull request
- **WHEN** the user opens "Since Your Review" of a pull request
- **THEN** the tree lists only the commits after the reviewed one, not those the user saw
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list in the tree of Since Your Review only the commits after the user's review, and mark the reviewed one in All Changes`)
  [@test] src/lib/range-review.test.ts (`lists in the story of Since Your Review only the commits after the review, in All Changes every commit, the reviewed one marked`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)

### Requirement: In the Review Window: A commit row shows what it is since the review and its branch

A commit row, and the row of a step of the commits, MUST show what it is since the review ("New"/"Rewritten"), "Your
review" on the reviewed commit of a pull request (the boundary of the column: the commits after it are those of Since
Your Review), its branch (of the whole stack, and of a part of several branches, which the column heads by branch).

#### Scenario: New and rewritten commits
- **WHEN** commits were added or rewritten since the review
- **THEN** their rows, and the row of their step, show "New" or "Rewritten"
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should mark the step of a commit on top since Mark Reviewed New, and the steps reviewed before nothing`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)

#### Scenario: The reviewed commit of a pull request
- **WHEN** All Changes of a pull request lists the commit the user reviewed
- **THEN** it shows "Your review", the boundary of the column
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list in the tree of Since Your Review only the commits after the user's review, and mark the reviewed one in All Changes`)
  [@test] src/lib/range-review.test.ts (`lists in the story of Since Your Review only the commits after the review, in All Changes every commit, the reviewed one marked`)

#### Scenario: The branch of a commit
- **WHEN** the review is of the whole stack, or of a part of several branches
- **THEN** each commit row names its branch, and the column heads its commits by branch
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)
  [@test] src/lib/range-review.test.ts (`lists all changes, then the commits of the reviewed branch first, each branch headed by its name`)

### Requirement: In the Review Window: A title of the tree wraps whole, its hash in its card

The row of a step and a commit row MUST show their subject whole and their hash in their card (a commit without a body
in the native hover of its title), never a box on the row that takes the width of the title; a title of a step, a
task or a commit wraps as many lines as needed at the width of the sidebar (24rem at first), at words.

#### Scenario: Long titles at the default width
- **WHEN** the tree shows long titles at the default width of the sidebar
- **THEN** each wraps whole, its chevron, icon and title on the first line
- **AND** its owner count, mark, New/Rewritten, branch and line counts follow below
- **AND** the title wraps at words; only a code name breaks within a word
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should make the row of a step its commit and its rounds rows of it, their titles whole at any width, and open the card of the commit by the hover of its mark or subject and by I`, `should show every step and commit title of the tree whole at the default width, its files one line each`)

### Requirement: In the Review Window: A step row sticks, a file row stays one line

The row of a step MUST stick to the top of the tree's scroll area on an opaque background; a file row MUST stay one
line. A
commit-derived step dims its conventional prefix, as the commit row does.

#### Scenario: Scrolling the files of a step
- **WHEN** the files of a step scroll in the tree
- **THEN** its row sticks to the top on an opaque background
- **AND** each file row stays one line
- **AND** a commit-derived step dims its conventional prefix
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show every step and commit title of the tree whole at the default width, its files one line each`)

### Requirement: In the Review Window: A file row says the step of its own row

A row of a file whose own row is in another step MUST say where after its name ("in step 2", a task of no number by
its name, cut at half the row), its hover in words.

#### Scenario: A file of two steps
- **WHEN** a file has its own row in another step
- **THEN** its other row says "in step 2" after its name, a task of no number by its name, cut at half the row
- **AND** its hover says it in words
  [@test] src/lib/file-tree.test.ts (`names the step of the own row of a file shown in another step after its name`)

### Requirement: In the Review Window: A row of a commit with a body has the mark of its card

The row of a step of the commits and a commit row, of a commit with a body, MUST show the mark of its card, which its hover and `I` on the focused row open
as in the main window ([Commit Row](../../commit/commit-row/spec.md#commit-card)), Escape giving the focus back to the
row; a commit row MUST be no file (no position, no count, no owner mark of its own: it is muted with its step).

#### Scenario: The card of a commit from its row
- **WHEN** the user hovers the mark of a commit row with a body, or presses `I` on the focused row
- **THEN** the card of the commit opens as in the main window, Escape giving the focus back to the row
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should make the row of a step its commit and its rounds rows of it, their titles whole at any width, and open the card of the commit by the hover of its mark or subject and by I`)

#### Scenario: A commit row is no file
- **WHEN** the tree shows a commit row
- **THEN** it has no position, no count and no owner mark of its own: it is muted with its step

### Requirement: In the Review Window: The column lists only the scopes while the tree lists commits

While the tree lists the commits of the plan, the column above it MUST list only the scopes; without the plan, for a range of one commit and for a commit
shown alone the column lists every commit as before.

#### Scenario: The column with and without the commits in the tree
- **WHEN** the tree lists the commits of the plan
- **THEN** the column above it lists only the scopes
- **AND** without the plan, for a range of one commit and for a commit shown alone, it lists every commit as before

### Requirement: In the Review Window: Under a search or an owner chip the tree is what it found

While a search or owner chip narrows the files, the tree MUST be what it found: only a group with a file it kept lists
its commits, and no "Other commits".

#### Scenario: A search that keeps some files
- **WHEN** a search narrows the files
- **THEN** only the groups of the steps with a file it kept list their commits, and the tree has no "Other commits"
  [@test] src/lib/file-tree.test.ts (`lists under a search only the commits of the steps whose files it found, no Other commits, and says when it lists none`)

### Requirement: In the Review Window: While the tree lists no commit the column lists them all

While the tree lists no commit (the filter kept none of their groups or no file, the range has no changed file, the
plan is still read) the column MUST list every commit again, so a commit can still be chosen.

#### Scenario: A search that keeps no row of a commit
- **WHEN** the search keeps none of the rows of the commits
- **THEN** the column lists every commit again, so a commit can still be chosen
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should give the commits back to the column while the search keeps none of their rows, and copy a commit row by the copy key and its menu`)
  [@test] src/lib/file-tree.test.ts (`lists under a search only the commits of the steps whose files it found, no Other commits, and says when it lists none`)

### Requirement: In the Review Window: A click on a commit row shows the opening of its step

A click on a commit row or on the row of a step of the commits (and the focus, as ↑/↓ move it) MUST select it and show
the opening of its step at the top of the diffs, as the title of the step in the overview; a click on the row of a
folded step opens it too.

#### Scenario: A click on the row of a step
- **WHEN** the user clicks a commit row or the row of a step of the commits, or moves the focus to it with ↑/↓
- **THEN** it is selected and the opening of its step shows at the top of the diffs
- **AND** a click on the row of a folded step opens it too
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the step on a click, fold it by its chevron, Space, Left and Right, copy the hash of its commit, and show the commit alone on Enter, Show Commit Alone and Option+Down`)

### Requirement: In the Review Window: The row of a step of the commits folds by its chevron and keys

The row of a step of the commits MUST fold by its chevron, Space, → and ←, never by a click on the row; a step of its
commit alone that lands nothing in the scope holds no row and folds nothing (no chevron, no line counts); the group of
a step of the concerns, a directory, Mechanical and Other changes fold by a click as before.

#### Scenario: Folding the row of a step
- **WHEN** the user presses Space, → or ← on the row of a step of the commits, or clicks its chevron
- **THEN** it folds or opens, and a click on the row does not fold it
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the step on a click, fold it by its chevron, Space, Left and Right, copy the hash of its commit, and show the commit alone on Enter, Show Commit Alone and Option+Down`)

#### Scenario: A step of its commit alone that lands nothing
- **WHEN** a step of its commit alone lands nothing in the scope
- **THEN** it holds no row and folds nothing: no chevron, no line counts

### Requirement: In the Review Window: Enter, Show Commit Alone and ⌥↑/⌥↓ show the commit alone

Enter on a commit row or on the row of a step of the commits, its menu item Show Commit Alone and ⌥↑/⌥↓ MUST show the
commit alone (its heading, its own plan), and "All Changes" of the column goes back. The heading of a commit of the
whole stack shown alone keeps "Review <branch>".

#### Scenario: The commit of a row shown alone
- **WHEN** the user presses Enter on a commit row, chooses Show Commit Alone or presses ⌥↓
- **THEN** the review shows the commit alone, its heading and its own plan
- **AND** "All Changes" of the column goes back
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the step on a click, fold it by its chevron, Space, Left and Right, copy the hash of its commit, and show the commit alone on Enter, Show Commit Alone and Option+Down`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show the plan of a commit shown alone from its row in the tree: its overview and the story of its diffs @webkit`)

#### Scenario: A commit of the whole stack shown alone
- **WHEN** a commit of the whole stack is shown alone
- **THEN** its heading keeps "Review <branch>"
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)

### Requirement: In the Review Window: The menu of a commit row and ⌘C

The right-click menu of a commit row MUST be Show Commit Alone, then the menu of the commit of the column; ⌘C MUST copy
its hash.

#### Scenario: The menu and the copy key of a commit row
- **WHEN** the user right-clicks a commit row or presses ⌘C on it
- **THEN** its menu is Show Commit Alone, then the menu of the commit of the column
- **AND** ⌘C copies its hash
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the step on a click, fold it by its chevron, Space, Left and Right, copy the hash of its commit, and show the commit alone on Enter, Show Commit Alone and Option+Down`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should give the commits back to the column while the search keeps none of their rows, and copy a commit row by the copy key and its menu`)

### Requirement: In the Review Window: A declaration without a block of its own joins a stop

A declaration without a block of its own (of one side, its lines in a block another unit holds) MUST be no stop of its
own: Rust joins it to the stop that shows the block of the line of its name (`StepStop.joined`, a stop of its own step
first), whose section names it after the heading of that block ("allowed — new · hooks — removed").

#### Scenario: A declaration whose lines another unit's block holds
- **WHEN** a declaration has no block of its own, its lines in a block another unit holds
- **THEN** it joins the stop that shows the block of the line of its name, a stop of its own step first
- **AND** that section names it after the heading of the block ("allowed — new · hooks — removed")
  [@test] crates/review-plan/src/steps_test.rs (`a_declaration_without_a_block_joins_the_stop_of_the_block_of_its_line_each_of_its_own_step`, `a_removed_declaration_whose_block_a_later_step_shows_is_a_part_of_that_stop_and_of_no_stop_of_its_own`)
  [@test] src/presenters/review-steps.test.ts (`joins a declaration without a block of its own to the heading of the block Rust names, marked with its step where it is another's`)

### Requirement: In the Review Window: A joined unit is marked with its step where it is another's

A joined unit MUST be marked with its step where it is another step's ("hooks — removed in step 1", "passes — new in
step 5": " · " joins the units; a step of the concerns by its number too, what no evidence joined by its name); in a
story whose steps are not told (one commit or one pull request of one task) as well, unmarked there.

#### Scenario: A unit of another step
- **WHEN** a section shows a joined unit of another step
- **THEN** it marks it with that step ("hooks — removed in step 1", "passes — new in step 5")
  [@test] src/presenters/review-steps.test.ts (`joins a declaration without a block of its own to the heading of the block Rust names, marked with its step where it is another's`)

#### Scenario: A unit of another step of the concerns
- **WHEN** the joined unit is of another told step of a concern
- **THEN** it is marked by the number of that step too, what no evidence joined by its name
  [@test] src/presenters/review-steps.test.ts (`marks a joined unit of another told step of a concern by the number of that step`)

#### Scenario: A story whose steps are not told
- **WHEN** the steps of the story are not told, one commit or one pull request of one task
- **THEN** the joined unit is named after the heading of its block as well, unmarked
  [@test] src/presenters/review-steps.test.ts (`words a joined unit after the heading of its block and keeps the marks of the stop in a story whose steps are not told`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell the whole body of a range of one commit of one task in its heading, whose mark opens its card, name a removed declaration after the heading of its block, and give a commit without a body no mark and no tab stop`)

### Requirement: In the Review Window: A step whose unit another step shows says so in its facts

A step whose unit another step's section shows MUST say so in the line of its facts ("Removes hooks, shown in step 2",
`ReviewStep.shownElsewhere`), each name in the code font showing its place, "step 2" a link to that step, and its tree
MUST list the file of that section as a row "also in" it, its own row in that step.

#### Scenario: A removed declaration shown in another step
- **WHEN** a step removes a declaration another step's section shows
- **THEN** its facts say "Removes hooks, shown in step 2", the name in the code font showing its place, before the
  commits
- **AND** "step 2" is a link to that step
- **AND** its tree lists the file of that section as a row "also in" it, its own row in that step
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)
  [@test] crates/review-plan/src/steps_test.rs (`a_removed_declaration_whose_block_a_later_step_shows_is_a_part_of_that_stop_and_of_no_stop_of_its_own`)

#### Scenario: Tests among the units shown elsewhere
- **WHEN** tests are among those units
- **THEN** they are unnamed and counted after the names ("Adds window and 2 tests, shown in step 1", "Removes a test, shown in step 1")
- **AND** a Kotlin test is named by a sentence

### Requirement: In the Review Window: A declaration of two stops joins once, a group's block none

A declaration of two stops (a part of two concerns) MUST join once, the stop of the step whose stop holds its line
first; a block only a group shows (a mechanical change, a repeated edit, deleted files) MUST join none.

#### Scenario: A declaration of two concerns and a block of a group
- **WHEN** a declaration without a block is a part of two concerns
- **THEN** it joins once, the stop of the step whose stop holds its line first
- **AND** a block only a group shows joins none
  [@test] crates/review-plan/src/steps_test.rs (`a_declaration_without_a_block_of_two_concerns_joins_once_and_never_a_group`, `a_mechanical_change_follows_the_step_that_wrote_it`)

### Requirement: In the Review Window: A step left with no stop keeps its opening above the next

A step left with no stop (all it changes shown so, or nothing of it lands) MUST keep its opening and show no card: its
opening stands right above the opening of the next step that has a card, in the one layer of openings of that
section, J going from it to that opening and K back.

#### Scenario: A step whose code a later step's card shows
- **WHEN** a later step's card shows all the code of a step
- **THEN** its opening stands right above the opening of that step, with no card, its unit named in that card
- **AND** J goes from it to that opening and K back
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open a step whose code another step's card shows above the opening of that step, with no card, its unit named in that card, J and K through both openings`)
  [@test] crates/review-plan/src/steps_test.rs (`a_removed_declaration_whose_block_a_later_step_shows_is_a_part_of_that_stop_and_of_no_stop_of_its_own`)

### Requirement: In the Review Window: A last step with no stop opens in the epilogue

A last step left with no stop MUST open after the last card, in the epilogue below it, J going to it from the last
card ("Next: Step 2 · …" at its end) and K back. Its row of the tree, its step of the overview and a link "shown in
step N" show it; the bar, the tree and a step of the overview follow the opening at the top.

#### Scenario: A last step whose code an earlier step's card shows
- **WHEN** an earlier step's card shows all the code of the last step
- **THEN** its opening stands below the last card, in the epilogue, a gap above it
- **AND** its row, the overview, the end of that card ("Next: Step 2 · …") and J show it, K going back to the card
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open a last step whose code an earlier step's card shows below the last card, from its row, the overview, the end of that card and J, K going back to the card`)
  [@test] crates/review-plan/src/steps_test.rs (`a_removed_declaration_whose_block_an_earlier_step_shows_leaves_the_last_step_no_stop`)
  [@test] src/lib/diff/layout.test.ts (`puts the epilogue after the last card, a gap above it, as an opening of the last file, and keeps a top in it from its top`)

### Requirement: In the Review Window: A declaration whose line no stop shows stays a stop

A declaration whose line no stop shows (or only a group) MUST stay a stop, its heading joined to the one before it; a
section of no change of its own MUST show only the rows of the blocks other sections show. The coverage of the steps
stays exact.

#### Scenario: A section of no change of its own
- **WHEN** a section holds no change of its own
- **THEN** it shows only the rows of the blocks other sections show
- **AND** the coverage of the steps stays exact
  [@test] src/lib/diff/rows.test.ts (`makes a section of no change of its own the rows of the blocks other sections show, alone`)

### Requirement: In the Review Window: The bar of the step at the top is one line

Above the diffs the bar of the step of the section at the top (`review-step-bar`) MUST be one line, "Step 3 / 4 · The
view paints with the first frame", the whole title in its hover and no text of the body, "Overview · 4 steps" while
the overview is at the top. It MUST follow the diffs as they scroll and as J, K, a row of the tree and a step of the
overview move them, and a click shows the opening of the step (of the first one from the overview).

#### Scenario: The bar as the diffs scroll
- **WHEN** the diffs scroll, or J, K, a row of the tree or a step of the overview move them
- **THEN** the bar follows, naming the step of the section at the top, "Overview · 4 steps" while the overview is at the top
- **AND** a click on the bar shows the opening of the step, of the first one from the overview
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should go through the steps from the overview, by J and K and by the title of an opening, the bar and the tree following`)

#### Scenario: The bar at the end of a document
- **WHEN** J goes through the sections of a document shorter than the view below its first part, to the last
- **THEN** each comes to the top, the diffs scrolling past their end, and the bar, the footer and the tree name it
- **AND** a wheel to the end shows the last card at the top, the space past the end empty below it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should scroll past the end of a short document, so J brings each section to the top, the bar, the footer and the tree naming it, and a wheel to the end shows the space below the last card`)

### Requirement: In the Review Window: The bar names what is no step alone

What the story tells as no step MUST be named alone in the bar above the diffs, with no place among the steps: "Other
changes" at its opening and its cards, a group the story shows after its steps by its name without the count of its
heading ("Imports only"), the footer naming no step there; a use left outside is of the step of the section before
it. A click shows the opening of Other changes, the first card of such a group.

#### Scenario: The bar at what is no step
- **WHEN** Other changes, or a group after the steps, is at the top of the diffs of a review of one step
- **THEN** the bar names it alone, "Other changes", "Imports only", never "Step 2 / 1", and the footer names no step
- **AND** a click on the bar at Other changes shows its opening
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should name in the bar above the diffs a step by its place, Other changes and the group of the imports after the story by their name alone, a click showing the opening of Other changes`)
  [@test] src/presenters/review-units.test.ts (`names in the bar above the diffs a step by its place among the told steps and its title, Other changes and a group after the story by their name alone`, `names in the bar a use left outside by the step of the section before it, a group after the story from any of its files, a file of no unit by Other changes, and has no bar in a story that tells no step`)

### Requirement: In the Review Window: J and K walk the document

J and K MUST walk the document: from the overview J shows the opening of the first step, from an opening its card,
from a card the next card or the opening of the next step; K the same way back, from the opening of the first step to
the overview. The first J of a review that opens on its overview, nothing scrolled nor chosen yet, MUST show the
opening of the first step too.

#### Scenario: J and K through the openings and the cards
- **WHEN** the user presses J from the overview and on
- **THEN** J shows the opening of the first step, then its card, then the next card or the opening of the next step
- **AND** K goes the same way back, from the opening of the first step to the overview
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should go through the steps from the overview, by J and K and by the title of an opening, the bar and the tree following`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

#### Scenario: The first J of a review just opened
- **WHEN** a review opens on its overview and the user presses J before anything scrolled the diffs or chose a section
- **THEN** J shows the opening of the first step, its row selected
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show the opening of the first step on the first J of a review just opened on its overview`)

### Requirement: In the Review Window: A stop at a card shows the heading of its group whole

A stop at a card (J and K at a card, a row of the tree) MUST show what stands above it in its section whole or not at
all: the heading of a group ("Imports only (1)", "view.gui · Tests · 1 file") at the top, whole, the card right below
it; an opening, a stop of its own, wholly above the view, never a cut edge of either under the bar of the step.

#### Scenario: J to the first card of a group and to a card below an opening
- **WHEN** J goes from the opening of a step to its card, from the opening of Other changes to its card, and to the
  first card of the group of the imports
- **THEN** each opening goes wholly above the view
- **AND** the heading "Imports only (1)" stands whole at the top, the card right below it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should stop at the first card of a group with the heading of the group whole above it, and at a card below an opening with the opening wholly above the view`)
  [@test] src/lib/diff/layout.test.ts (`stops at a card with the heading of its group at the top with it, whole, and at a card below an opening with the opening above the view`)

### Requirement: In the Review Window: The tree selects what is at the top

While an opening is at the top the tree MUST select the row of its step, while a card is its file's; the overview
keeps the selection. A row of a file of the tree MUST show its card, the opening of its step above the view.

#### Scenario: The selection follows the document
- **WHEN** an opening is at the top
- **THEN** the tree selects the row of its step, and while a card is at the top the row of its file
- **AND** the overview keeps the selection
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should go through the steps from the overview, by J and K and by the title of an opening, the bar and the tree following`)

#### Scenario: A row of a file
- **WHEN** the user chooses a row of a file in the tree
- **THEN** its card shows, the opening of its step above the view

### Requirement: In the Review Window: An owner chip narrows the steps J and K walk

An owner chip MUST filter the tree and diffs to its files; J and K MUST step only through cards and openings of steps
with shown files.

#### Scenario: The story of an owner chip
- **WHEN** the user chooses an owner chip
- **THEN** the tree and the diffs show only its files and the steps that hold them
- **AND** J and K step only through cards and openings of steps with shown files
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show only the steps and files of an owner chip, and navigate within its story`)

### Requirement: Anchors: A resolved code name of a body is a link wherever its line shows

A code name of a body that resolves (`Mention.target`) MUST be a link wherever the document shows its line (an
opening, the overview, the contents; a label, a heading and the title of a Look closely too): in the code font, a chip
when the author wrote it in backticks, its dotted underline the one mark of a link. A name that resolves nothing MUST
stay code; a plain word MUST NOT be a link.

#### Scenario: Names of a body in an opening and in the overview
- **WHEN** the document shows a line of a body with a name that resolves
- **THEN** the name is a link in the code font, a chip when the author wrote it in backticks, its dotted underline the
  one mark of a link
- **AND** a name that resolves nothing stays code, and a plain word is no link
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

### Requirement: Anchors: A link of a name says what its card says

The accessible name of a link of a name MUST be the name it shows, its description what its card says in one line
("New · Manager.kt:6", "src/a/Frame.kt · in step 3"), its hover a card of the same look as the card of a story mark
(`MentionCard`); inside the card of a story mark the name is only a link, no second card.

#### Scenario: The card of a name
- **WHEN** the user hovers a link of a name
- **THEN** its card shows the status of a declaration as a badge ("New", "Removed", "Body changed", "Form changed", "New file"), its file and line
- **AND** "in step 5" only when the step that shows it is not the one whose opening tells the line, never the name again
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

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

### Requirement: Anchors: The header of a section a body names carries a mark of the story

Back: the header of a section a told body names MUST carry a small mark of the story at its end, its height kept: the
section of the path of the target in the step of the target, the one that shows its unit (by its id) when one does.

#### Scenario: A section a told body names
- **WHEN** a told body names the code of a section
- **THEN** its header carries a small mark of the story at its end, its height kept
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

### Requirement: Anchors: The card of a story mark quotes the lines that name the section

The hover of a mark MUST open a card of the story (`bg-popover`, 28rem wide at most, 8px from the edges of the window):
where it is told once as its heading, then each line that names the section, quoted, its code names set and linked as
the story sets them; the card opens above the header, never over the header or the lines of its section.

#### Scenario: The hover of a mark
- **WHEN** the user hovers a story mark
- **THEN** its card names where it is told once as its heading ("Step 2 · The open keeps a stored layout", "Overview"), then each line that names the section, quoted
- **AND** it opens above the header, under the whole header where no room is above it, never over the header or the
  lines of its section
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)

#### Scenario: The focus of a mark from the keyboard
- **WHEN** the mark gets the focus from the keyboard
- **THEN** the card opens too, read and not entered: Escape closes it, Enter shows the story, whose names are links
  there

### Requirement: Anchors: A click on a mark shows the line where it is told, tinted once

A click on a mark MUST show the first of its lines where it is told, the opening of its step at the top (the overview
for a part told before the steps), its part opened when a fold held it, the block tinted for a moment (the warning
colour fading, never a ring like the focus; a label or a heading too). The tint MUST play once.

#### Scenario: A click on a mark
- **WHEN** the user clicks a story mark
- **THEN** the first line that names the section shows where it is told, the opening of its step at the top
- **AND** a fold that held it opens, and its block is tinted for a moment
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`, `should show the Look closely of a step as a callout, and open a part folded under it from the mark of its section, its block outlined once`)

#### Scenario: A story that shows later
- **WHEN** the story shows later: the commit shown alone, the whole story again, the files a search hid
- **THEN** it outlines nothing and opens no fold

### Requirement: Anchors: A body is told where the document shows it, as Rust marks the stops

A body MUST be told where the document shows it, as Rust marks the stops (`marks`): the commit that starts a step in
its opening, the concerns of one commit where `told` says; a follow-up, folded under its step, marks nothing, and a
copy of a lead in the contents is no place a mark shows; a section shows the marks of the stops it holds.

#### Scenario: The marks of a story of commits
- **WHEN** the steps are the commits of a range
- **THEN** the marks show the body of the commit that starts a step in its opening, a follow-up none
- **AND** a section shows the marks of the stops it holds
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)

### Requirement: Anchors: A commit shown alone is told by its own plan

A commit shown alone MUST be told by its own plan, so its marks show its own story.

#### Scenario: A commit shown alone under Since Your Review
- **WHEN** the user shows a commit alone under Since Your Review
- **THEN** its own plan tells it: its story, its steps and the marks back to it
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell a commit shown alone under Since Your Review by its own plan: its story, its steps and the marks back to it`)

## User Experience

```json
"steps": {
  "source": {"kind": "commits"},
  "steps": [
    {"id": "8fd70458",
     "commits": [{"id": "8fd70458", "title": "the project open shows the Project view only when …",
                  "role": "starts"}],
     "concern": null,
     "stops": [
       {"unit": "declaration:…/ToolWindowManagerImpl.kt:…isShownOnStartupAllowed:id:String", "path": "…/ToolWindowManagerImpl.kt",
        "blocks": null, "uses": null, "also": [], "concern": null},
       {"unit": "declaration:…/IdeProjectFrameAllocator.kt:…openProjectViewIfNeeded:", "path": "…/IdeProjectFrameAllocator.kt",
        "blocks": null, "uses": "isShownOnStartupAllowed", "also": [2], "concern": null}],
     "tests": {"answered": true,
               "proof": [{"path": "…/DeferredToolWindowRegistrationTest.kt",
                          "tests": [{"unit": "…", "path": "…/DeferredToolWindowRegistrationTest.kt", "line": 171,
                                     "name": "the project open may show only the tool windows that the stored layout shows",
                                     "depth": 1, "way": "calls", "reaches": ["isShownOnStartupAllowed"]}]}],
               "reached": 1, "unreached": [], "scenarios": []},
     "proves": [],
     "newApi": [], "moduleEdges": [], "title": "the project open shows the Project view only when …", "takenOutBy": null}],
  "coverage": {"blocks": 76, "inStops": 76, "mechanical": 0, "missing": 0, "twice": 0},
  "commits": [
    {"id": "8fd70458", "title": "the project open shows the Project view only when …",
     "body": {"blocks": [{"type": "paragraph", "lines": ["Three startup steps showed the Project view at each open: …"]},
                         {"type": "label", "text": "Tests", "kind": "verification", "runIn": false},
                         {"type": "list", "ordered": false, "start": 1, "items": ["DeferredToolWindowRegistrationTest …"]}],
              "trailers": ["Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"], "lead": true,
              "parts": [{"start": 0, "end": 1, "kind": "lead", "tone": "plain", "folded": false},
                        {"start": 1, "end": 3, "kind": "section", "tone": "verification", "folded": true}]},
     "mentions": [{"block": 0, "item": 0, "start": 41, "end": 66, "text": "isShownOnStartupAllowed",
                   "target": {"kind": "unit", "unit": "declaration:…/ToolWindowManagerImpl.kt:…isShownOnStartupAllowed:id:String",
                              "path": "…/ToolWindowManagerImpl.kt", "side": "new", "line": 171, "step": 0}},
                  {"block": 0, "item": 0, "start": 80, "end": 94, "text": "LayoutPolicy", "target": null}],
     "parts": [{"start": 0, "end": 1, "kind": "lead", "tone": "plain", "folded": false, "told": {"kind": "commit"}, "titled": false},
               {"start": 1, "end": 3, "kind": "section", "tone": "verification", "folded": true, "told": {"kind": "commit"},
                "titled": false}]}]},
"brief": {
  "issue": "IJAI-4673",
  "contract": [{"kind": "spec", "unit": "other:plugins/air/spec/frame/agent-sessions-window.spec.md", "path": "…",
                "block": 0, "heading": "Startup and persistence", "newSpec": null, "deleted": false,
                "added": ["The window keeps its stored tool window layout across a restart. …"], "removed": [],
                "provenBy": ["tests/integration/flow-profiles/resources/flow-profiles/agent-sessions-window-restart.json"]}]}
```

## Open Questions / Risks

- A follow-up by the lines it rewrites joins a task of its own to the step that created its file: of 8 such joins of
  a branch of 66 commits, 2 were tasks of their own. Their bodies stay in the step.
- "No test of the change reaches" is as good as the call graph of accio: a call through reflection, a dependency
  injection or a lambda passed far away is no edge, and a call only the name ties is not walked. The answer of the
  given tests costs a plan of 52 places ~20 s and ~100 s of CPU on the IntelliJ monorepo (2026-11-06), against 2.6 s
  for the depth-3 walk it replaced; the review waits for JetBrains/accio#376
  ([#191](https://github.com/develar/branch-deck/issues/191)).

## Measurements

`brd review-plan` (schema 10, a debug build) on 2026-10-31, every range read without the object cache:

| Ranges | Steps | Source | Follow-ups | Coverage |
|---|---|---|---|---|
| idea-1, 14 virtual branches of `develar` | 62 | by commits on 6 (5 of 2–5 commits, `air-local-launch` of 66 in 30 steps), by concerns on 8 of one commit | 36, all of `air-local-launch`: of 8 judged, 6 a round of the step, 2 a task of their own | every block in one stop |
| this repository, 24 runs of 2–8 commits of one scope | 54 | by commits on all | 4: 3 a round of the step, 1 plausible | every block in one stop |

- The steps of the commits are the steps their authors made: on `startup-tool-window-place` the refactor, the rule
  and its three uses, the early paint and the restart story, each with its body; the concerns of that range had put
  the code of three commits into one and cut the story into three.
- A cap of 12 steps that made the concerns the steps of a long branch was measured on `air-local-launch`: its 42
  concerns and a rest of 201 stops were no shorter a list than its 30 steps of commits, and lost their bodies. There
  is no cap.
- The steps of the message (`brd review-plan` schema 15, a debug build, 2026-11-02): of 31 commits of `develar` in
  idea-1 with a body of 40 words or more and 3 changed files or more, and IJAI-5582 (`terminal-split-url`), 10 are
  told by their parts, their 68 concerns now 48 steps; of the last 15 commits of this repository, 6, 45 concerns now 36
  steps. Every block stays in one stop. On IJAI-5582 the 12 concerns, the core under the token `TerminalOffset` and 7
  of them tests and their helpers, became the marker, the way of the markers, the joiner with its Look closely, the
  batches of the frontend and the tests, each titled by its paragraph, then the DTO field no paragraph names and the
  rest. A body that lists its changes as one list stays on the concerns: the list is one part.
- The tests that reach the code (`brd review-plan` schema 13, 2026-11-01, the 5 virtual branches of `develar` in
  idea-1 then, 18 steps, 11 of them with code): a test reaches code of 10 of the 11, and in 8 of those a test the
  change did not touch does; no step is marked unproven. The text match of the changed lines of the range's tests it
  replaced had marked 20 of 116 steps of 14 branches, most of them reached through an entry. 9 steps list code no test
  reaches, among it constants and properties, whose readers accio does not walk yet.
