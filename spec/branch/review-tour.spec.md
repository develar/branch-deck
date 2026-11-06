---
name: Review Tour
description: Requirements for the story of a review (experimental), one document that a reviewer who sees a change for the first time reads from its overview into each step's opening above its diffs; the brief (the issue the commits name, the rules of the specs and the prose of the docs the change adds, with the tests their test links name) and the steps, a step per commit whose body tells it, its follow-ups and the commits without a body where their lines landed, of one commit whose message tells its parts by the code they name a step per part, else the concerns, each with the parts of the units that land in master in its order, a declaration it adds first and its uses after it, and the tests that reach its code through its callers, the change's and the others; every block of the diff in one stop; the story of each commit once, its code names resolved to the units and the files.
style: plain-1
targets:
  - ../../crates/review-plan/src/steps.rs
  - ../../crates/review-plan/src/message_steps.rs
  - ../../crates/review-plan/src/story.rs
  - ../../crates/review-plan/src/brief.rs
  - ../../crates/review-plan/src/prose.rs
  - ../../crates/review-plan/src/interdiff.rs
  - ../../crates/review-plan/src/lib.rs
  - ../../crates/bd-rewrite/src/review_plan_command.rs
  - ../../src/presenters/review-steps.ts
  - ../../src/presenters/review-story.ts
  - ../../src/components/review/ReviewPrologue.tsx
  - ../../src/components/review/ChapterOpening.tsx
  - ../../src/components/review/ContractParagraphView.tsx
  - ../../src/components/shared/ReadingColumn.tsx
  - ../../src/components/review/StoryBody.tsx
  - ../../src/presenters/commit-body.ts
  - ../../src/components/review/StepFactsLine.tsx
  - ../../src/components/review/StoryText.tsx
  - ../../src/hooks/useStoryHighlight.ts
  - ../../src/components/commit/CommitReview.tsx
  - ../../src/components/diff/CommitReviewCode.tsx
  - ../../src/components/review/RangeReview.tsx
  - ../../src/components/files/FileTree.tsx
  - ../../src/lib/file-tree.ts
  - ../../src/lib/range-review.ts
  - ../../src/presenters/review-units.ts
  - ../../src/lib/diff/rows.ts
  - ../../src/lib/diff/layout.ts
  - ../../src/components/diff/DiffView.tsx
  - ../../src/components/diff/DiffFindBar.tsx
  - ../../src/lib/diff/find.ts
  - ../../src/lib/diff/find-text.ts
  - ../../src/components/commit/CommitBodyView.tsx
  - ../../src/components/shared/Disclosure.tsx
  - ../../src/components/diff/diff-view.css
  - ../../src/hooks/useSelectReviewFile.ts
---

# Review Tour

Status: Experimental
Date: 2026-10-31

## Summary

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
and the rounds: [Change Review](../../docs/design/change-review.md).

## Goals

- The words of the author before any diff: the bodies of the commits, the rules of the specs, the docs.
- Each change once, as it lands: a step shows the units of the range, never a state a later commit replaced.
- Every block of the diff in one stop, checked: the coverage is a field of the plan.
- No git process of its own: the plan read every commit and its hunks already.

## Non-goals

- A tour a language model writes, or one the author writes with `brd tour`: the next round
  ([Change Review → Next](../../docs/design/change-review.md#next)).

## Requirements

### Where a Line Comes From
- The lines of each changed file must be carried through the hunks of the commits that change it, oldest first, so
  that each line at the head names the commit that wrote it last and each line of the base the commit that removed it,
  across a rename too. A block of the range belongs to the commits of its changed lines; a blank line counts for none.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)

### Steps of the Commits
- A commit whose message has a body of 3 words or more after its subject, its trailers left out (a body of only
  `Co-Authored-By:` tells nothing), and no `fixup!`, `squash!` or `amend!` subject, tells a step. When a range of two commits or more has one, the steps must be the commits: each unit of the
  range in the step of the commit of most of its changed lines among the commits that tell a step, the later of equal
  ones; a unit of a file of no language by its blocks. The step lists the other steps whose commits changed lines of
  it (`also`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`, `a_file_of_no_language_is_cut_into_the_steps_of_its_blocks`, `a_body_of_a_few_words_tells_a_step_a_fixup_none`)
- A commit that tells a step must follow up the step of an earlier one (`followsUp`) when half of its changed lines of
  code on the old side or more were written by the commit that starts that step, when it owns no unit but a
  mechanical one (it joins the step that holds most of its lines), and when a later commit rewrote all it wrote (it
  joins the step of the commit that rewrote the most of it). A chain of follow-ups ends at a step. A commit whose body
  tells a story of its own (a lead and a labelled part, or more than 5 lines as the fold counts them; a verification
  section such as `Tests:` or `Evidence:` is no labelled part and its lines do not count, nor is a Look closely a
  labelled part) must start its step however much of an earlier step's code it rewrites: its story, its Look closely included, is never folded
  under another step. A step none of whose code lands must name the later commit that took out the most of what it
  wrote (`takenOutBy`): "Nothing of it lands: step 5 took it out", "step 5" showing that commit's step, its title in the
  hover; a subject inside the sentence does not read.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`, `a_commit_rewritten_whole_follows_the_step_of_the_commit_that_rewrote_it`, `a_commit_with_a_story_of_its_own_starts_a_step_though_it_rewrites_the_code_of_another`)
  [@test] ../../src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should start a step at a commit with a story of its own though it rewrites the code of others, and name it in a step it took the code of, which shows its step`)
- A commit without a body tells no step: it is listed in the step that holds most of its lines (`noStory`), and the
  units only such commits wrote are the step `no-story`, last.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)
- The commits of a step must be the one that starts it, then those that follow it up, then those of no story, each
  with its title; its body is told once, in the story of the commits ([The Story of a Commit](#the-story-of-a-commit)).
  Rust titles every commit it names (`StepCommit.title`, `StoryCommit.title`) and a step of the commits by the commit
  that starts it (`ReviewStep.title`): its subject without the `(prefix)` and the issue key the brief names, with a
  `:`, `,` or dash after it, also from among the keys the subject starts with, the others kept (`IJAI-1184 IJAI-5640
  Air records…` is `IJAI-1184 Air records…`), in the case its author wrote it, never capitalised, so the tree, the contents, the
  opening, the bar and the card of one commit write it alike. The plan names what is reviewed (`ReviewPlan.title`): a
  branch of the prefix as the title bar does (`layout`), another branch by its full name, a range by its short ids.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_commit_is_titled_by_its_subject_without_the_issue_key_of_the_brief_in_the_case_of_its_author`)
  [@test] ../../crates/review-plan/src/api_test.rs (`since_the_review_the_api_is_that_of_the_diff_from_the_reviewed_change`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
- A mechanical part must follow the step of the commit that wrote it, at its end: the imports a step needs, the
  generated files of its sources.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_mechanical_change_follows_the_step_that_wrote_it`)

### Steps of the Message
- A range of one commit whose body tells two parts or more by the code they name must have those parts as its steps,
  in the order of the body (`source`: `message`). The parts that may tell a step are a labelled section (a label or a
  heading and its blocks, a verification aside) and each paragraph or list of a run with the blocks written right under
  it (no blank line between: a list under its paragraph is one part); the lead, a Look closely, code alone and a
  verification section are none, but a `Tests:` section tells the step of the tests. A part claims the parts of the
  concerns (a unit in a file) its code names resolve to, resolved before there are steps: a declaration its parts, a
  file its parts in that file; a part two of them name goes to the labelled section before a paragraph, a paragraph
  before the tests, then to the one that names it most, then to the earlier one. Fewer than two parts of the story with
  a claim (a body of one paragraph that names code, two sections that name the same code: the first keeps it, a body of
  verification alone) keep the concerns as the steps.
  [@test] ../../crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`, `the_concerns_stay_the_steps_of_a_body_that_tells_no_two_parts_by_their_code`)
  [@test] ../../crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)
- A concern one part names must go to it whole; a concern two parts name must be cut by its own evidence, each of its
  parts to the nearest claimed one (a use the change adds at 1, a use it keeps, a rare token or what no evidence
  joined at 2, a repeated edit at 1), of equal distance the part of the better claim, then the earlier one; a part no
  evidence reaches goes to the part that claims most of the concern. No evidence is added: a concern no part names
  stays a step of its own after the parts, named by its evidence, a concern of test code alone joins the step of the
  tests, and what no evidence joined goes part by part (to the part that names it, a test to the tests, the rest stays
  the rest, last). Every block of the diff stays in one stop.
  [@test] ../../crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)
- A step of the message must be titled as its author wrote it (`titleSource`): the label or the heading of its
  section (`label`, told once, as its title); else the first sentence of its paragraph, up to a `.`, `?` or `!` outside
  backticks before a space or the end of the line, not after an abbreviation (`e.g.`), a single letter or a digit,
  when it is 100 characters or fewer, its backticks aside (`sentence`); else the first code name of the paragraph
  that resolves, set as code (`name`). Its id must be the fingerprint of its first part (its unit and file), so a
  reword that names more of its code keeps it.
  [@test] ../../crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`, `a_long_first_sentence_titles_its_step_by_the_first_name_it_resolves_and_a_list_under_a_paragraph_is_one_part`, `a_step_of_the_message_keeps_its_id_when_its_paragraph_names_more`)
- Of the steps of the message, each part that tells a step must be told in its opening, never folded there; a Look
  closely in the step of the part before it; the lead, and the parts no step tells, by the rule of the concerns of one
  commit ([The Story of a Commit](#the-story-of-a-commit)).
  [@test] ../../crates/review-plan/src/message_steps_test.rs (`the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order`)

### Steps of the Concerns
- A range of one commit whose body tells no two parts by their code and a range no commit of which tells a step must
  have the concerns as its steps, each with the commits of its lines, the most lines first (`source`: `concerns` with
  its `reason`, `oneCommit` or `noStory`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_concerns_are_the_steps_when_the_commits_tell_no_story`)
- A long branch is told by its commits too: its concerns were no shorter a list ([Measurements](#measurements)).

### Order of a Step
- The stops of a step must come in the order of the story, its mechanical parts last; a declaration the step adds or
  changes the form of comes before the stops of the step whose changed lines name it (in its file or in a file linked
  to it by that name), and those follow it right after, each with `uses`, unless more than 6 stops name it.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_declaration_a_step_adds_comes_first_and_its_uses_right_after_it`)
- A step that holds parts of two concerns or more that a use or a group joined must mark each stop with its concern.

### Tests That Reach It
The question a step answers is which tests of the change prove it, not which tests of the repository its code is
reachable from: that is the impact of the change, `brd impact`'s.
- The code of a step is each declaration of a parsed language whose body or form changed or that it adds, and of a
  file it adds each declaration at its top (a new type with its members, as a new type of a changed file is one unit),
  in no test file and no test code (a test, a fixture of its class or of its Rust `mod tests`: the unit is `test`). A
  name two such declarations share is led by its owner (`Owner.run`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of`, `a_new_file_is_code_its_tests_are_the_changes_and_their_step_proves_the_other`)
- A test of the change is a test function of a unit the range adds or changes, or of a file it adds (its unit the
  file's).
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_new_file_is_code_its_tests_are_the_changes_and_their_step_proves_the_other`)
- After the plan, with the usages of its API, every declaration of the code of the steps must be asked in one
  `accio tests --at <path>:<line> --ref <head> --depth 3 --resolved` (`TEST_DEPTH`, `within`): the tests that reach it
  through its callers, only by calls a type or an import ties. The tests of the change that reach the code of a step
  are its `proof`, by their file, each test once with the name it reads as, its fewest calls (`depth`) and the
  declarations of the step it reaches (`reaches`); the tests the change did not touch only when they call the code
  (`callers`, depth 1), one that reaches it through other callers being no fact of the step; `reached` counts the
  declarations a test reaches. A declaration no test reaches is `unreached` only when accio walked every caller; one
  accio stopped short of at the depth with callers left is a count (`unfollowed`), unknown, never "no test". A place
  accio finds no declaration at (`unavailable`: a line it does not index) is neither: accio answered nothing of it.
  `answered` says accio answered; without accio, or without a usable index, a step has none of them, and `index_note`
  says why. The given tests of the change asked at any depth, by the class of the path, wait for accio
  ([#179](https://github.com/develar/branch-deck/issues/179)).
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_tests_that_reach_the_code_of_a_step_and_the_code_none_reaches`, `a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of`)
- A step whose units hold tests of the change that reach the code of other steps must name those steps (`proves`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_new_file_is_code_its_tests_are_the_changes_and_their_step_proves_the_other`)
- A step that adds or changes a rule of a spec must list the tests its test links name (`scenarios`): a scenario
  that names no declaration proves it.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`)

### The Story of a Commit
How to write a body this story reads well: [The Commit Body a Review Reads](../../docs/guide/commit-bodies.md).
- Every commit of the range whose body has a block must have its story once (`commits`, oldest first), also when
  several steps list it: its id, its subject without the `(prefix)`, its body by the grammar of a body
  ([Commit Row → Message body](../commit/commit-row.spec.md)), its code names (`mentions`), its parts as the
  document tells them (`parts`: each part of the body, its `kind` and `tone`, `folded`, where it is `told` and
  whether its label is the title of its step, `titled`), and the title of each step of the concerns of one commit that
  tells a labelled section (`title`: the label of the first one, a verification and a Look closely aside; `titleSource`
  `label`, of a step of the commits `commit`, of a concern no section names `evidence`). A commit of a step names no
  body of its own.
  [@test] ../../crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`, `a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)
- A code name of a block that is no preformatted text must resolve to a unit or a changed file of the range, with no
  git process and no query of the index: a span in backticks that names code (a path, a file name, identifiers joined
  by `.`, `::` or `#`; a trailing `(…)` or `<…>` and a leading `@` aside), and a token outside backticks the grammar of
  code finds ([Commit Row → Code tokens](../commit/commit-row.spec.md#code-tokens): `toolWindowId`, `snake_case`,
  `Owner.member` with a capitalised owner, a path, a file name), so a name the page sets as code is the name the story
  links, a resolved one only adding its dotted underline. A plain lowercase
  word is no name, in backticks neither, unless its call (`open()`); a span with a space, its arguments aside
  (`open(path, mode)`), is a command, no name. A run of found tokens one space apart (`Frame.paint SplashScreen.hide`)
  that names nothing whole is a mention of each token that resolves, at its own range; a possessive or a dash after a
  name (`SplashScreen.hide's`, `Frame.paint—`) is prose.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`, `a_name_before_a_possessive_or_a_dash_and_each_name_of_a_run_is_a_mention`)
- A path must resolve to the changed path itself, else the one it ends on at a `/`, else the one changed file of that
  name (a `:171` after it aside); two files of that name resolve nothing. A declaration must resolve by its segments:
  the units of a declaration, and the declarations of a new or a deleted file by its summary (its unit is the file,
  the line that of the name, of the old side for a deleted file), whose name is the last one, with an owner those whose key ends with all of them (a leading crate or
  package the key does not hold aside, down to the owner and the name; `crate`, `self`, `super`, `Self` aside): JVM
  `Owner.member`, Rust `module::Type::member`, TypeScript `Type#member`. One key resolves (overloads are one name); of
  two keys or more, the one among the stops of the commit's own steps, of two there the one that lands (not a
  removal), else nothing. Only a name in backticks: a capitalised name no declaration of the range has resolves to the
  file of the units declared inside it, when they are in one (a type whose members changed); a capitalised or compound
  one to the one changed file named for it, its case, `-` and `_` aside (`Session` of `Session.kt`,
  `useMeasuredHeights` of `use-measured-heights.ts`). A bare word resolves to a declaration or a path alone, so a
  product name in prose (`YouTrack`, `Node.js`) names no file.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_name_two_declarations_share_resolves_to_the_one_of_its_step_else_to_nothing`, `a_name_resolves_by_the_separators_of_its_language`)
- A mention must carry its block, the line of its paragraph or the item of its list (0 of a heading or a label), its
  UTF-16 range in it, the backticks included, its text, and its `target`: a `unit` with its path, its side (`old`
  for a removed declaration), the line of its name and the step whose stops hold it, a step of the commit first; a
  `file` with its step. A bare name is kept only when it resolves; a name in backticks that resolves nothing has
  `target: null`, so the author sees what did not bind.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
- Of the concerns of one commit, each labelled section (a label or a heading and its blocks up to the next one), else
  each block, must be told in the step that holds the targets of most of its resolved mentions, the earliest of equal
  ones (`told`: `{"kind": "step", "step": 1}`); a run of blocks of no label is cut into a part per place where the
  place changes; the lead and a part that names nothing are told in the overview (`overview`), and every part while
  the steps are not told. A story of several commits keeps the order of its author: each part is told with its
  commit (`commit`), in the opening of the step it starts or the disclosure of its round. The page shows the parts a
  place tells and decides none of it.
  [@test] ../../crates/review-plan/src/story_test.rs (`the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`, `a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)
- Each stop must list the lines of the told bodies that name it (`marks`: the commit, the block, the line and where
  the block is told, `step`, `null` for the overview): a resolved mention marks the stops of the path of its target in
  the step of its target (in every step without one), of a declaration only the stops that show its unit, its own or
  joined, when one does; once per block. A story of several commits marks only from the commit that starts a step;
  a follow-up, folded under its step, marks nothing.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_line_of_a_told_body_marks_the_stops_it_names_where_it_is_told`, `the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue`)
- The story of a commit whose message changed since the review (`message_change` of its pair in the interdiff of
  its branch, [Review Plan](review-plan.spec.md#since-your-review); of a commit shown alone, read with its reviewed
  commit) must carry that change (`change`) and place it: each part marks the added blocks it holds, and the removed
  blocks of a hunk stand in one part (`removals`), that of its first added block, else of the block they followed (the
  first block when they were at the start), before the block `start` when that part holds it (`before`), else at its
  end. A part that shows a change must not be folded, a verification section neither: what changed is what the reader
  came back for. A story whose message did not change has no `change`, its parts as the grammar folds them.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_changed_part_of_a_body_is_marked_and_unfolded_a_verification_too`)
- A step must list the public declarations it adds (`newApi`, the keys of the `added` entries of `api` whose
  declaration or new file its stops show) and the links it adds between two modules (`moduleLinks`, by their place in
  `links`, in the step whose stop shows the first use of the link, else the first step whose stops show the file that
  uses), each in one step, the first that holds it. A declaration a stop of another step shows beside its own unit
  (`StepStop.joined`) is new API of the step that wrote it, which says where it is shown.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_step_lists_the_new_api_and_the_new_links_between_modules_it_holds`)
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_declaration_a_later_step_adds_in_the_block_of_another_is_the_new_api_of_the_step_that_wrote_it`)

### Coverage
- Every block of the diff must be in one stop, else in a mechanical unit no stop shows; a file of no blocks counts as
  one, a file changed and changed back as none (`coverage`: `blocks`, `inStops`, `mechanical`, `missing`, `twice`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Brief
- `issue` must be the issue key the subjects of the commits name: of the keys each subject starts with (`IJAI-1184
  IJAI-5640 Air records…` names both), the one every subject that starts with a key names, named by half of them at
  least; the first such key of the first subject when several are.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_issue_is_one_key_half_the_commits_name_and_none_contradicts`)
- `contract` must hold the prose the change adds, removes or changes, by the rank of its kind, then in the order of
  the story: a spec (`*.spec.md`, a file below `spec/` or `specs/`), a record of a decision (below `decisions/`, `adr/`
  or `adrs/`), a guide of agents (`AGENTS.md`, `CLAUDE.md`, below `.ai/`), a doc (below `docs/` or `doc/`, a README);
  of Markdown, text, reStructuredText and AsciiDoc only. An entry per place of a file (`prose`): its changed blocks
  widened to whole paragraphs on both sides, never the changed lines of a hard-wrapped one, the blocks whose
  paragraphs meet one entry (`blocks`); the heading of its section of Markdown, the paths that the test links its
  blocks add name (`provenBy`), and its `paragraphs`, each whole, its lines joined without the marker of an item and
  the test links. A paragraph wrapped anew is no change; a removed paragraph and the first similar added one after the
  last pair (half of the shorter kept and a third of the longer, by word) are one `changed` paragraph with the words
  the change wrote (`inserted`) and removed (`deleted`, each at the offset of the text it stood before), the rest
  `added` or `removed`. A link of Markdown reads as its text with its target (`links`: a path of the repository
  resolved against the file, without its anchor, or a URL as written); of a paragraph longer than 280 UTF-16 code
  units changed in a few sentences, the sentences that hold its changes (`excerpt`) when they leave out a quarter of it
  at least. A new spec its `description:` and the count of the items of its "Requirements"; a deleted file `deleted`.
  Of a doc only prose (three words or more of letters, no key and no value after one) is quoted: a changed line of
  values (`dark: #88aaff`, `color: red; background: blue`) says nothing, the heading of its section names the change;
  the spans of code of each paragraph (`code`) as a body has them.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`, `a_file_of_prose_is_a_spec_a_decision_a_guide_or_a_doc_by_its_place`, `the_brief_of_a_doc_names_the_section_of_a_changed_value_and_quotes_only_its_prose`)
  [@test] ../../crates/review-plan/src/prose_test.rs (`a_hard_wrapped_paragraph_changed_in_one_line_is_the_whole_paragraph_by_word`, `a_paragraph_wrapped_anew_is_no_change`, `a_new_paragraph_is_added_alone_and_lines_appended_to_one_change_it`, `a_rewritten_paragraph_is_removed_and_added_and_two_blocks_of_one_paragraph_are_one`, `a_paragraph_added_before_a_changed_one_leaves_it_its_pair`, `the_test_lines_of_an_item_name_its_tests`, `a_link_reads_as_its_text_with_its_target`, `a_long_paragraph_changed_in_one_sentence_shows_it_first`)

### In `brd`
- `brd review-plan` must print `steps` and `brief` (`"schema": 18`), the ids of the commits of the steps and of the
  story as short as the others, every mention of a name in backticks with its `target` (`null` where it resolves
  nothing), and run no git process for them.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### In the Review Window
- With the review plan, the right column must be one document that scrolls, one canvas of cards, the bar of the step
  above it on that canvas: the overview first (`ReviewPrologue`), a card of prose at the inset of the file cards, the
  same card at the same place while the diffs load, then the story of the diffs. The overview must open on its headline: the title YouTrack gives the issue the commits
  name once it is known, else the name the plan gives (`ReviewPlan.title`: the branch as the title bar names it, the
  range of the commits), the issue key a chip after it, a link with the popover of its issue; none under the title of
  a pull request, which names it already; a commit shown alone its
  whole heading and message instead (`range-review-commit-header`: its subject, meta line, action and menu, its body
  whole, of the concerns of its own plan the parts of it told before the steps). Then What Changes, one header per
  file ("Spec rule", "Decision", "Agent guide", "Doc" and its path, which open the file), under it each section the
  change touches: its heading once, "Proven by restart.json" muted after it, its paragraphs whole, half a line apart:
  one the change adds with the bar of an addition in the gutter, one it removes muted and struck through beside the
  bar of a removal, one it changes with what it wrote tinted and what it removed struck through where it stood; of a
  long one the sentences of its change between ellipses, either of which opens it whole; a link its text, which shows
  the file it names at its header when the review shows that file, opens a page, else stays text; then the Steps, each its number and its full
  title (the title Rust gives the commit that starts it; a step of the concerns
  its concern), wrapping, never cut, which shows its opening; the lead of its body; its facts in one short line: the
  files of the tests of the change that reach its code ("Tested by ManagerTest.kt", each showing its first test, its
  tests in its hover; the opening tells the rest), the code no test reaches beside a scenario ("No test reaches load"),
  "Proven by …", "New API: Cache and Reader", "New link between modules: Reader.kt → Cache.kt", each name showing its
  place; a step accio answered for whose code no test reaches, every caller walked, and no scenario proves says so in
  the warning tone ("No test reaches paint"); code accio stopped short of is never named there; a list of names in the facts
  names up to four and from five on the first three and the count ("paint, create, run and 2 more", `joinNames`), the
  new API and the new links between modules too ("New API: Cache, Reader, Store and 16 more", each name its place),
  which the opening names whole; a name two new declarations share led by its type ("Event.rightEdge and
  Update.rightEdge"), as the code of a step is; every name of a fact in the code font; and each
  "Look closely:" section of
  its body and of the bodies of its rounds as an info callout, its question alone (its first block): the opening
  tells the rest. Steps of the concerns and of the message say why, a muted line right under the count of the steps
  ("One commit: the steps are its tasks", "One commit: the steps are the parts of its message"); a muted note is
  never pushed to the far end of the column, only a sum is; a range whose
  only step is what no evidence joined, or one concern that covers it, lists no step, and its facts are the facts of the range. Then Possible
  Breakage (the uses left outside right after the name of an entry) and Since your review ([Review
  Plan](review-plan.spec.md#requirements)); why the uses are unknown is folded under Possible Breakage when it lists an
  entry, else one muted line ("Uses unknown: no index of this repository"). The document is one reading column in the
  middle of its island (`ReadingColumn`: `max-w-measure`, 65ch of `text-md`, its lines wrapped to avoid a short last
  one), every block ending at its edge and none setting a width of its own; the number of a step hangs in the gutter,
  so its title and its lead stand at that edge. The type of the document is one scale: its prose `text-md`, its
  headline (and the subject of a commit shown alone) `text-lg font-semibold`, the title of an opening `text-base
  font-semibold`, a heading of What Changes and an open part `font-semibold`, a step of the contents `font-medium`, its
  facts and the header of a file `text-xs` muted (docs/design/ui-theme.md). The count line of the old card,
  its Dependencies, Repeated Edits and New API are gone: the dependency changes and the same edit are on the rows of
  the tree and the divider "Same edit in N files", the new API and the links between modules are facts of their step.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`, `should tell What Changes in whole paragraphs changed by word, a long one by the sentence of its change, and show the file its link names`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../src/presenters/review-story.test.ts (`words What Changes: a file once, a section once with its tests, its paragraphs whole, a link showing a changed file or opening a page`, `words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`, `opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)
- The opening of a step (`ChapterOpening`) must be a card of prose a gap above the card of its first section (each
  opening of a layer its own card), as tall as its words (the diffs measure it), its words in the reading column of
  the overview (`ReadingColumn`) in the middle of the card: "Step 2" muted, then its full title in `text-base`, wrapping, which a click shows
  at the top; the body of the commit that starts it by the grammar of a body ([Commit Row → Message
  body](../commit/commit-row.spec.md)) in `text-md`: its lead a step larger (`text-base`), its paragraphs, lists, labels as the captions of their
  blocks, code and measurements in the monospace font, a "Look closely:" section an info callout, no trailer and no
  muted text but what its body removed since the review, struck through where Rust placed it, beside the blocks it
  added, marked ([Review Plan](review-plan.spec.md#in-the-review-window)), and the subject its commit had below its
  title. A long body shows the
  parts Rust shows (`parts`), then one line of what is still folded, one chevron
  for it ("More: The cause, The fix, Tests, Evidence", a part of no label "The rest"), each name its own toggle; an
  opened part stands below that line in the order of the body, its label the toggle that folds it again (a chevron
  before it), so its name shows once and the text grows downward; the line names only what is still folded. A
  verification section (Tests, Evidence, Gates, Verified, Results, Proof) is folded always; the author's "Look
  closely:" and "Left out:" show always, in their place among the shown parts. Then the other commits of the step, each one row
  of one form at the size of the text: a muted "Follows up" or "Without a story", its title wrapping at the width of
  the prose; a commit with a story a disclosure (`Disclosure`, one disclosure of the document: the folds of a body,
  these rows and what the overview folds) whose body is set `nested`, never larger than its row; then the facts of the
  step, muted, each fact a line of its own and every name, each in the code font showing its place in the diffs: the
  files of the tests of the change, each its tests in its hover ("Tested by ManagerTest.kt"), how many tests the
  change did not touch call its code, their names in its hover ("1 existing test calls it"), its code no test reaches,
  each at its declaration, the name selected ("No test reaches show"), of a step no test reaches whose walk accio cut
  "No test found within 3 calls", the steps its tests prove ("Proves step 2", showing that step), "New API: allowed";
  of a task what joins its parts, its names code), the warning of a step no test reaches, its names code too, and the
  group its section starts.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)
  [@test] ../../src/presenters/review-story.test.ts (`shows the lead of a body and its parts before the fold, the rest and every verification section folded, each named in the outline`, `words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`, `opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)
- ⌘F (Ctrl+F off macOS) searches the review document from its overview through the openings and diff rows to the
  epilogue. Story text is matched ignoring case, one block at a time, with `.sr-only`, `[aria-hidden=true]` and
  `[data-find-skip]` excluded. Folded story parts and follow-up `Disclosure`s stay hidden in the document and open when
  the find goes to their hit; text and diff hits share one document-order count and text hits are highlighted without
  changing the DOM React owns. ⌘F works anywhere in the review window, including the sidebar; ⌘G and Shift+⌘G step
  through hits there while the bar is open.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should find overview and opening text before the matching diff rows, and open a folded follow-up`, `should search story text in the epilogue and scroll to the hit`, `should open a hit in a closed story body and keep it current`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should open find from the file tree and step results there with ⌘F and ⌘G @webkit`)
- A step of the concerns of one commit must open with the parts of the body of that commit told in it (`told`), the
  label Rust made its title (`titled`) not shown again, and say what joins its parts in the line of its facts
  ("Joined by the identifier `accio`"); the lead and the parts told before the steps are in
  the overview (in the heading of the commit shown alone), so each part of the body is in one place. A story of
  several commits keeps the order of its author: each part in the opening of its commit's step.
  [@test] ../../src/presenters/review-story.test.ts (`tells each part of the body of one commit where Rust places it, the label of the title of a step not again, the lead and the rest in the overview`)
- A step of the message must open as a step of the concerns does, with the parts told in it, without the row of the
  commit (its one commit is the row of no step: the tree lists it in the first step); a step titled by the first
  sentence of its paragraph (`titleSource: "sentence"`) shows "Step 2" and that paragraph as its lead, a step larger,
  the title only for a screen reader, so the sentence reads once; a step titled by a code name sets it as code. The
  overview, the tree and the bar of the step name the step by its title, the tree after its number ("2 · …").
  [@test] ../../src/presenters/review-story.test.ts (`tells each part of the message of one commit in the step it is, a title of its first sentence read in its paragraph, a step larger, the commit no step's row`)
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should tell each step of one commit by the paragraph of its message that names its code, titled by its first sentence, which its opening reads a step larger`)
- The tree must list the steps, each a group "2 · The open keeps a stored layout" with its files by their path (a
  step of the concerns and of the message too, its number in the font of the text before a code name), a file
  of two steps with its row in the step of its most lines and a row "also in" each other; a step of the concerns is
  named by the label of the first section of the body of its commit it tells (`title`, "The splash"; a verification
  and a Look closely aside), else by its concern, an identifier set in the code font wherever the step is named (the
  tree, the contents, its opening, the bar of the step, the heading of a card of a story mark), and keeps its main rows
  ([Review Concerns](review-concerns.spec.md#in-the-review-window)). The step of the one concern that covers the whole range is no story
  (`ReviewSteps.told` false, which the plan decides): no Steps in the overview, which tells the whole body, no
  opening, no place in the bar of the step, and the tree lists the files as without the plan. The
  story of the diffs must follow the steps: the first section of each starts with its opening right above its card,
  its heading "Step 2 · The open keeps a stored layout" (`review-concern-heading`); a click on it must show the step as
  the bar of the step does, the opening at the top, the bar and the tree following. The header of a file names the
  file alone (no step, no group), so the name of every file stands at one x.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)
  [@test] ../../src/presenters/review-steps.test.ts (`names a step by the title Rust gives it, in the case its author wrote it, a step of no story by what it is`, `names a step of a concern by the label of the section of the body it tells, else by its concern as code, keeps its main rows, and tells no step of a story Rust tells in none`)
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should title each task of one commit by the label of the section of its body it tells`, `should tell no step of the one concern that covers the range: no Steps, no opening, no place in the bar`)
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_concerns_are_the_steps_when_the_commits_tell_no_story`)
  [@test] ../../src/lib/diff/layout.test.ts (`puts the opening of a step at its measured height and the divider of a group between the gap and its card: a scroll to the file shows it, a y in it is the file below`)
- The story must be the one list of the sidebar: with the plan, a scope of a range of two commits or more ("All
  Changes", "Since Your Review" of a branch, of a stack, of a pull request) lists its commits in the tree. The group
  row of a step of the commits is the commit that starts it ("2 · The open keeps a stored layout": no row of its own
  repeats the title); under it the rows of the commits that follow it up and of those without a story, each by its
  title, which gets the width of the row: the indent says what it is, the
  description of the row says it in words ("follows up", "no story"), no word on the row; then its files, a file
  of a group its directory after its name whole or not at all, never cut to its last letters. A group of a step of the
  concerns lists its commit rows as they are, the most lines first. A commit several concerns list is listed once, in
  the one that lists it first, the first of equal ones; a step that lands nothing keeps its group for its commits; the
  commits no group lists (of no step, of what no evidence joined) close the tree in "Other commits" ("Commits" when the
  tree has no step), by their titles (a commit no step lists by its subject), as the range lists them. The plan names a commit short, the range in full:
  they match by prefix. A scope lists only its own commits: "Since Your Review" of a pull request the commits after
  the reviewed one, not those the user saw. A commit row, and the row of a step of the commits, shows what it is
  since the review ("New"/"Rewritten"), "Your review" on the reviewed commit of a pull request (the boundary of the
  column: the commits after it are those of Since Your Review), its branch (of the whole stack, and of a part of
  several branches, which the column heads by branch); the row of a step and a commit row their subject whole and
  their hash in their card (a commit without a body in the native hover of its title), never a box on the row that
  takes the width of the title; a title of a step,
  of a task or of a commit wraps to a second line (its chevron, icon, mark and line counts on its first line), cut
  only past two at the width of the sidebar (24rem at first); a row of a file whose own row is in another step says
  where after its name ("in step 2", a task of no number by its name, cut at half the row), its hover in words; either, of a
  commit with a body, the mark of its card, which its hover and `I` on the focused row open as in the main window
  ([Commit Row](../commit/commit-row.spec.md#commit-card)), Escape giving the focus back to the row; a commit row is
  no file (no position, no count, no owner mark of its own: it is muted with its step). The column above the tree
  then lists only the scopes; without the plan, for a range of one commit and for a commit shown alone the column
  lists every commit as before.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should make the row of a step its commit, its rounds rows of it, show the step on a click, fold it by its chevron, Space, Left and Right, show the commit alone on Enter, Show Commit Alone and Option+Down, and mark a step new since the review New`)
  [@test] ../../src/lib/file-tree.test.ts (`names the step of the own row of a file shown in another step after its name`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`, `should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`, `should list in the tree of Since Your Review only the commits after the user's review, and mark the reviewed one in All Changes`)
  [@test] ../../src/presenters/review-steps.test.ts (`places each commit of the range in the step it tells or follows up, the one that starts a step of the commits as its step, a commit of the concerns once, the rest in Other commits`)
  [@test] ../../src/lib/file-tree.test.ts (`lists the commits of each step above its files, and those of no group (what no evidence joined, no step) last in Other commits, as the range lists them`, `keeps the group of a step that lands nothing for its commits, and lists every commit under Commits without a step`, `makes the group of a step the commit that starts it, and lists its rounds and the commits of no story without the issue key, in the case its author wrote it`, `keeps the group of a step that lands nothing as its commit, lists a step under a search only with a file it found, and its commit in Commits without a step`)
  [@test] ../../src/lib/range-review.test.ts (`lists in the story of Since Your Review only the commits after the review, in All Changes every commit, the reviewed one marked`, `lists all changes, then the commits of the reviewed branch first, each branch headed by its name`)
- While the search of the files has text, the tree is what it found: only a group with a file it kept lists its
  commits, and no "Other commits". While the tree lists no commit (the search kept none of their groups or no file,
  the range has no changed file, the plan is still read) the column must list every commit again, so a commit can
  still be chosen.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should give the commits back to the column while the search keeps none of their rows, and copy a commit row by the copy key and its menu`)
  [@test] ../../src/lib/file-tree.test.ts (`lists under a search only the commits of the steps whose files it found, no Other commits, and says when it lists none`)
- A click on a commit row or on the row of a step of the commits (and the focus, as ↑/↓ move it) must select it and
  show the opening of its step at the top of the diffs, as the title of the step in the overview; a click on the row
  of a folded step opens it too. The row of a step of the commits folds by its chevron, Space, → and ←, never by a
  click on the row; a step of its commit alone that lands nothing in the scope holds no row and folds nothing (no
  chevron, no line counts); the group of a step of the concerns, a directory, Mechanical and Other changes fold by a click as
  before. Enter on a commit row or on the row of a step of the commits, its menu item Show Commit Alone and ⌥↑/⌥↓
  must show the commit alone (its heading, its own plan), and "All Changes" of the column goes back. Its right-click
  menu is Show Commit Alone, then the menu of the commit of the column; ⌘C copies its hash. The heading of a commit of
  the whole stack shown alone keeps "Review <branch>".
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should make the row of a step its commit, its rounds rows of it, show the step on a click, fold it by its chevron, Space, Left and Right, show the commit alone on Enter, Show Commit Alone and Option+Down, and mark a step new since the review New`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show the plan of a commit shown alone from its row in the tree: its overview and the story of its diffs @webkit`, `should give the commits back to the column while the search keeps none of their rows, and copy a commit row by the copy key and its menu`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)
- A declaration without a block of its own (of one side, its lines in a block another unit holds) must be no stop of
  its own: Rust joins it to the stop that shows the block of the line of its name (`StepStop.joined`, a stop of its own
  step first), whose section names it after the heading of that block ("allowed — new · hooks — removed"), marked with
  its step where it is another step's ("hooks — removed in step 1", "passes — new in step 5": " · " joins the units;
  a step of the concerns by its number too, what no evidence joined by its name); in a story whose steps are not told
  (one commit or one pull request of one task) as well, unmarked there. A step whose unit another step's section shows
  says so in the line of its facts ("Removes hooks, shown in step 2", `ReviewStep.shownElsewhere`), "step 2" a link to that
  step, and its tree lists the file of that section as a row "also in" it, its own row in that step. A declaration of
  two stops (a part of two concerns) joins once, the stop of the step whose stop holds its line first; a block only a
  group shows (a mechanical change, a repeated edit, deleted files) joins none. A step left with no stop (all it
  changes shown so, or nothing of it lands) keeps its opening and shows no card: its opening stands right above the
  opening of the next step that has a card, in the one layer of openings of that section, J going from it to that
  opening and K back; after the last card, in the epilogue below it, J going to it from the last card ("Next: Step 2 ·
  …" at its end) and K back. Its row of the tree, its step of the overview and a link "shown in step N" show it; the
  bar, the tree and a step of the overview follow the opening at the top. A declaration whose line no stop shows (or
  only a group) stays a stop, its heading joined to the one before it; a section of no change of its own shows only the
  rows of the blocks other sections show. The coverage of the steps stays exact.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_removed_declaration_whose_block_a_later_step_shows_is_a_part_of_that_stop_and_of_no_stop_of_its_own`, `a_removed_declaration_whose_block_an_earlier_step_shows_leaves_the_last_step_no_stop`, `a_declaration_without_a_block_joins_the_stop_of_the_block_of_its_line_each_of_its_own_step`, `a_declaration_without_a_block_of_two_concerns_joins_once_and_never_a_group`, `a_mechanical_change_follows_the_step_that_wrote_it`)
  [@test] ../../src/presenters/review-steps.test.ts (`joins a declaration without a block of its own to the heading of the block Rust names, marked with its step where it is another's`, `words a joined unit after the heading of its block and keeps the marks of the stop in a story whose steps are not told`, `marks a joined unit of another told step of a concern by the number of that step`)
  [@test] ../../src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open a step whose code another step's card shows above the opening of that step, with no card, its unit named in that card, J and K through both openings`, `should tell the whole body of a range of one commit of one task in its heading, whose mark opens its card, name a removed declaration after the heading of its block, and give a commit without a body no mark and no tab stop`, `should open a last step whose code an earlier step's card shows below the last card, from its row, the overview, the end of that card and J, K going back to the card`)
  [@test] ../../src/lib/diff/layout.test.ts (`puts the epilogue after the last card, a gap above it, as an opening of the last file, and keeps a top in it from its top`)
  [@test] ../../src/lib/diff/rows.test.ts (`makes a section of no change of its own the rows of the blocks other sections show, alone`)
- Above the diffs the bar of the step of the section at the top (`review-step-bar`) must be one line, "Step 3 / 4 · The
  view paints with the first frame", the whole title in its hover and no text of the body, "Overview · 4 steps" while
  the overview is at the top; it follows the diffs as they scroll and as J, K, a row of the tree and a step of the
  overview move them, and a click shows the opening of the step (of the first one from the overview).
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)
- J and K must walk the document: from the overview J shows the opening of the first step, from an opening its card,
  from a card the next card or the opening of the next step; K the same way back, from the opening of the first step
  to the overview. While an opening is at the top the tree selects the row of its step, while a card is its file's;
  the overview keeps the selection. Under an owner chip J and K step through the cards of its files only. A row of a
  file of the tree shows its card, the opening of its step above the view.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should mark the steps and the files of an owner chip in the whole story, and move N and J through its sections only`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Anchors
- A code name of a body that resolves (`Mention.target`) must be a link wherever the document shows its line (an
  opening, the overview, the contents; a label, a heading and the title of a Look closely too): in the code font, a chip when the author wrote it in backticks, its dotted
  underline the one mark of a link; its accessible name the name it shows, its description what its card says in one
  line ("New · Manager.kt:6", "src/a/Frame.kt · in step 3"), its hover a card of the same look as the card of a story mark (`MentionCard`): the status of a
  declaration as a badge ("New", "Removed", "Body changed", "Form changed", "New file"), its file and line, and "in
  step 5" only when the step that shows it is not the one whose opening tells the line, never the name again; inside
  the card of a story mark the name is only a link, no second card. A click shows it as an entry of the overview does, its file selected, a declaration at the line of its name, the name
  Rust resolved (`MentionTarget.name`) selected, a file at its header. A name that resolves nothing stays code; a plain word is never a link.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)
  [@test] ../../src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)
- Back: the header of a section a told body names must carry a small mark of the story at its end, its height kept:
  the section of the path of the target in the step of the target, the one that shows its unit (by its id) when one
  does. Its hover opens a card of the story (`bg-popover`, 28rem wide at most, 8px from the edges of the window):
  where it is told once as its heading ("Step 2 · The open keeps a stored layout", "Overview"), then each line that
  names the section, quoted, its code names set and linked as the story sets them; the card opens above the header
  (under the whole header where no room is above it), never over the header or the lines of its section, and on the
  focus of the mark from the keyboard too, read and not entered (Escape closes it; Enter shows the story, whose names
  are links there); a click shows the first of them where it is told, the opening of its step at the top (the
  overview for a part told before the steps), its part opened when a fold held it, the block tinted for a moment (the
  warning colour fading, never a ring like the focus; a label or a heading too). The tint plays once: a story that shows later (the commit shown alone, the whole
  story again, the files a search hid) outlines nothing and opens no fold.
  A body is told where the document shows it, as Rust marks the stops (`marks`): the commit that starts a step in its
  opening, the concerns of one commit where `told` says; a follow-up, folded under its step, marks nothing, and a copy
  of a lead in the contents is no place a mark shows; a section shows the marks of the stops it holds. A commit shown alone is told by its own plan, so its marks show its own story.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`, `should tell a commit shown alone under Since Your Review by its own plan: its story, its steps and the marks back to it`)
  [@test] ../../src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

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
                                     "depth": 1, "reaches": ["isShownOnStartupAllowed"]}]}],
               "callers": [], "reached": 1, "unreached": [], "unfollowed": 1, "within": 3, "scenarios": []},
     "proves": [],
     "newApi": [], "moduleLinks": [], "title": "the project open shows the Project view only when …", "takenOutBy": null}],
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
- "No test reaches" is as good as the call graph of accio: a call through reflection, a dependency injection or a
  lambda passed far away is no edge, and a call only the name ties is not walked (the calls inside a lambda with a
  receiver, `withFixture { … }`, JetBrains/accio#213). A walk stopped at 3 calls with callers left names nothing: a
  test of the change deeper than that (an end-to-end test through an RPC, 17 calls away) is not seen until the
  review asks the tests of the change themselves ([#179](https://github.com/develar/branch-deck/issues/179)).

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
