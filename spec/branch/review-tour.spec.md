---
name: Review Tour
description: Requirements for the story of a review (experimental), what a reviewer who sees a change for the first time reads before any diff; the brief (the issue the commits name, the rules of the specs and the prose of the docs the change adds, with the tests their test links name) and the steps, a step per commit whose body tells it, its follow-ups and the commits without a body where their lines landed, else the concerns, each with the parts of the units that land in master in its order, a declaration it adds first and its uses after it, and the tests that reach its code through its callers, the change's and the others; every block of the diff in one stop; the story of each commit once, its code names resolved to the units and the files.
style: plain-1
targets:
  - ../../crates/review-plan/src/steps.rs
  - ../../crates/review-plan/src/story.rs
  - ../../crates/review-plan/src/brief.rs
  - ../../crates/review-plan/src/interdiff.rs
  - ../../crates/review-plan/src/lib.rs
  - ../../crates/bd-rewrite/src/review_plan_command.rs
  - ../../src/presenters/review-steps.ts
  - ../../src/components/review/ReviewBrief.tsx
  - ../../src/components/review/ReviewPlanCard.tsx
  - ../../src/components/commit/CommitReview.tsx
  - ../../src/components/diff/CommitReviewCode.tsx
  - ../../src/components/review/RangeReview.tsx
  - ../../src/components/files/FileTree.tsx
  - ../../src/lib/file-tree.ts
  - ../../src/lib/range-review.ts
  - ../../src/presenters/review-units.ts
  - ../../src/lib/diff/rows.ts
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
code, those the change adds or changes and those it did not touch. Reviews differ: where the commits tell no story, the concerns of the range are the steps. The vision
and the rounds: [Change Review](../../docs/design/change-review.md).

## Goals

- The words of the author before any diff: the bodies of the commits, the rules of the specs, the docs.
- Each change once, as it lands: a step shows the units of the range, never a state a later commit replaced.
- Every block of the diff in one stop, checked: the coverage is a field of the plan.
- No git process of its own: the plan read every commit and its hunks already.

## Non-goals

- A tour a language model writes, or one the author writes with `bd tour`: the next round
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
  joins the step of the commit that rewrote the most of it). A chain of follow-ups ends at a step.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`, `a_commit_rewritten_whole_follows_the_step_of_the_commit_that_rewrote_it`)
- A commit without a body tells no step: it is listed in the step that holds most of its lines (`noStory`), and the
  units only such commits wrote are the step `no-story`, last.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)
- The commits of a step must be the one that starts it, then those that follow it up, then those of no story, each
  with its subject; its body is told once, in the story of the commits ([The Story of a Commit](#the-story-of-a-commit)).
- A mechanical part must follow the step of the commit that wrote it, at its end: the imports a step needs, the
  generated files of its sources.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_mechanical_change_follows_the_step_that_wrote_it`)

### Steps of the Concerns
- A range of one commit and a range no commit of which tells a step must have the concerns as its steps, each with the
  commits of its lines, the most lines first (`source`: `concerns` with its `reason`, `oneCommit` or `noStory`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_concerns_are_the_steps_when_the_commits_tell_no_story`)
- A long branch is told by its commits too: its concerns were no shorter a list ([Measurements](#measurements)).

### Order of a Step
- The stops of a step must come in the order of the story, its mechanical parts last; a declaration the step adds or
  changes the form of comes before the stops of the step whose changed lines name it (in its file or in a file linked
  to it by that name), and those follow it right after, each with `uses`, unless more than 6 stops name it.
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_declaration_a_step_adds_comes_first_and_its_uses_right_after_it`)
- A step that holds parts of two concerns or more that a use or a group joined must mark each stop with its concern.

### Tests That Reach It
- The code of a step is each declaration of a parsed language whose body or form changed or that it adds, in no test
  file and no test code (a test, a fixture of its class or of its Rust `mod tests`: the unit is `test`). A name two
  such declarations share is led by its owner (`Owner.run`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of`)
- After the plan, with the usages of its API, every declaration of the code of the steps must be asked in one
  `accio tests --at <path>:<line> --ref <head> --depth 3 --resolved` (`TEST_DEPTH`): the tests that reach it through
  its callers, only by calls a type or an import ties. A declaration a test reaches is `reached` with its tests, each
  with the name it reads as, its depth and the callers on the way, and its unit when the range adds or changes it
  (the test whose lines hold the line accio gives); one no test reaches is `unreached`, `within: 3` when accio stopped
  at the depth with callers left. `answered` says accio answered; without accio, or without a usable index, no step
  is reached or unreached, and `index_note` says why.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_tests_that_reach_the_code_of_a_step_and_the_code_none_reaches`, `a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of`)
- A step that adds or changes a rule of a spec must list the tests its test links name (`scenarios`): a scenario
  that names no declaration proves it.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`)

### The Story of a Commit
- Every commit of the range whose body has a block must have its story once (`commits`, oldest first), also when
  several steps list it: its id, its subject without the `(prefix)`, its body by the grammar of a body
  ([Commit Row → Message body](../commit/commit-row.spec.md)), its code names (`mentions`) and, of the concerns of one
  commit, where each block is told (`toldIn`). A commit of a step names no body of its own.
  [@test] ../../crates/review-plan/src/story_test.rs (`the_body_of_one_commit_is_told_by_the_concerns_its_sections_name_the_lead_and_the_rest_in_the_prologue`)
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`, `a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are`)
- A code name of a block that is no preformatted text must resolve to a unit or a changed file of the range, with no
  git process and no query of the index: a span in backticks that names code (a path, a file name, identifiers joined
  by `.`, `::` or `#`; a trailing `(…)` or `<…>` and a leading `@` aside), and a bare compound name (`toolWindowId`,
  `snake_case`), `Owner.member` with a capitalised owner, a path or a file name outside backticks. A plain lowercase
  word is no name, in backticks neither, unless its call (`open()`); a span with a space, its arguments aside
  (`open(path, mode)`), is a command, no name.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)
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
  ones (`toldIn`, by the place of the step); the lead and a part that names nothing are told before the steps
  (`null`). A story of several commits keeps the order of its author: `toldIn` is empty.
  [@test] ../../crates/review-plan/src/story_test.rs (`the_body_of_one_commit_is_told_by_the_concerns_its_sections_name_the_lead_and_the_rest_in_the_prologue`, `a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name`)
- A step must list the public declarations it adds (`newApi`, the keys of the `added` entries of `api` whose
  declaration or new file its stops show) and the links it adds between two modules (`moduleLinks`, by their place in
  `links`, in the step whose stop shows the first use of the link, else the first step of the file that uses), each
  in one step, the first that holds it.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_step_lists_the_new_api_and_the_new_links_between_modules_it_holds`)

### Coverage
- Every block of the diff must be in one stop, else in a mechanical unit no stop shows; a file of no blocks counts as
  one, a file changed and changed back as none (`coverage`: `blocks`, `inStops`, `mechanical`, `missing`, `twice`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Brief
- `issue` must be the issue key the subjects of the commits name: one key, named by half of them at least, and none
  naming another.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_issue_is_one_key_half_the_commits_name_and_none_contradicts`)
- `contract` must hold the prose the change adds, removes or changes, by the rank of its kind, then in the order of
  the story: a spec (`*.spec.md`, a file below `spec/` or `specs/`), a record of a decision (below `decisions/`, `adr/`
  or `adrs/`), a guide of agents (`AGENTS.md`, `CLAUDE.md`, below `.ai/`), a doc (below `docs/` or `doc/`, a README);
  of Markdown, text, reStructuredText and AsciiDoc only. An entry per changed block: its paragraphs added and removed,
  each its lines joined without the marker of an item, the heading of its section of Markdown, the paths that the
  test links it adds name (`provenBy`); a new spec its `description:` and the count of the items of its
  "Requirements"; a deleted file `deleted`.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`, `a_file_of_prose_is_a_spec_a_decision_a_guide_or_a_doc_by_its_place`)

### In `bd`
- `bd review-plan` must print `steps` and `brief` (`"schema": 14`), the ids of the commits of the steps and of the
  story as short as the others, every mention of a name in backticks with its `target` (`null` where it resolves
  nothing), and run no git process for them.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### In the Review Window
- With the review plan, the Review Summary must open on the brief (`ReviewBrief`): the issue key, a link with the
  popover of its issue; What Changes, each entry its kind ("Spec rule", "Decision", "Agent guide", "Doc"), its file and
  the heading of its section, which open the file, its paragraphs and "Proven by restart.json"; then the Steps, each its
  number and its title (the subject of the commit that starts it, without the issue key and the `(prefix)`), the first
  paragraph of its body, the rest under More, its rounds ("Follows up: …", "Without a story: …"), "Tests of this change
  reach allowed", "1 test it did not touch reaches focus", "Proven by …", and "No test reaches show" muted; a step
  accio answered for whose code no test reaches and no scenario proves says so in the warning tone ("No test reaches
  paint within 3 calls" when the walk stopped with callers left, else "No test reaches paint"). A click of the title of a
  step must show its first section at the top of the diffs, its row selected. Steps of the concerns say why ("One
  commit: the steps are its tasks"); a range whose only step is what no evidence joined lists no step.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its brief, a step per commit with its why, its rounds and the tests that reach its code`)
  [@test] ../../src/presenters/review-steps.test.ts (`words the brief: the why of each step its first paragraph, the rest folded, its rounds, the tests that reach its code, a step no test proves`, `names a step by the subject of the commit that starts it, without the issue the brief names, a step of no story by what it is`)
- The tree must list the steps, each a group "2 · The open keeps a stored layout" with its files by their path, a file
  of two steps with its row in the step of its most lines and a row "also in" each other; a step of the concerns is
  named by its concern and keeps its main rows ([Review Concerns](review-concerns.spec.md#in-the-review-window)). The
  story of the diffs must follow the steps: the first section of each starts with its opening on the canvas right above
  its card, "Step 2 · The open keeps a stored layout" (`review-concern-heading`), its number muted, its title bold and a
  rule across the rest of the width, a change of step being a wider break than a change of file; a click on it must
  show the step as the bar of the step does, the opening at the top, the bar and the tree following. The header of a file names the file alone (no step, no group), so the name of every file
  stands at one x.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its brief, a step per commit with its why, its rounds and the tests that reach its code`)
  [@test] ../../src/presenters/review-steps.test.ts (`names a step by the subject of the commit that starts it, without the issue the brief names, a step of no story by what it is`, `names a step of a concern by its concern and keeps its main rows`)
  [@test] ../../src/lib/diff/layout.test.ts (`puts the opening of a step at its measured height and the divider of a group between the gap and its card: a scroll to the file shows it, a y in it is the file below`)
- The story must be the one list of the sidebar: with the plan, a scope of a range of two commits or more ("All
  Changes", "Since Your Review" of a branch, of a stack, of a pull request) lists its commits in the tree, each group of
  a step its commit rows first, then its files: the commit that starts it, those that follow it up ("follows up"),
  those without a story ("no story"), the commits of a concern the most lines first. A commit several concerns list
  is listed once, in the one that lists it first, the first of equal ones; a step that lands nothing keeps its group
  for its commits; the commits no group lists (of no step, of what no evidence joined) close the tree in "Other
  commits" ("Commits" when the tree has no step), as the range lists them. The plan names a commit short, the range in
  full: they match by prefix. A scope lists only its own commits: "Since Your Review" of a pull request the commits
  after the reviewed one, not those the user saw. A commit row shows its subject, its role, "New"/"Rewritten" since
  the review, "Your review" on the reviewed commit of a pull request (the boundary of the column: the commits after it
  are those of Since Your Review), its branch (of the whole stack, and of a part of several branches, which the
  column heads by branch) and its hash on hover; it is no file (no position, no count, no owner mark of its own: it is
  muted with its step). The column above the tree then lists only the scopes; without the plan, for a range of one
  commit and for a commit shown alone the column lists every commit as before.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should list the commits in their steps as one list, show the step of a commit on a click, and the commit alone on Enter, Show Commit Alone and Option+Down`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`, `should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`, `should list in the tree of Since Your Review only the commits after the user's review, and mark the reviewed one in All Changes`)
  [@test] ../../src/presenters/review-steps.test.ts (`places each commit of the range in the step it tells or follows up, a commit of the concerns once, the rest in Other commits`)
  [@test] ../../src/lib/file-tree.test.ts (`lists the commits of each step above its files, and those of no group (what no evidence joined, no step) last in Other commits, as the range lists them`, `keeps the group of a step that lands nothing for its commits, and lists every commit under Commits without a step`)
  [@test] ../../src/lib/range-review.test.ts (`lists in the story of Since Your Review only the commits after the review, in All Changes every commit, the reviewed one marked`, `lists all changes, then the commits of the reviewed branch first, each branch headed by its name`)
- While the search of the files has text, the tree is what it found: only a group with a file it kept lists its
  commits, and no "Other commits". While the tree lists no commit (the search kept none of their groups or no file,
  the range has no changed file, the plan is still read) the column must list every commit again, so a commit can
  still be chosen.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should give the commits back to the column while the search keeps none of their rows, and copy a commit row by the copy key and its menu`)
  [@test] ../../src/lib/file-tree.test.ts (`lists under a search only the commits of the steps whose files it found, no Other commits, and says when it lists none`)
- A click on a commit row (and the focus, as ↑/↓ move it) must select it and show the first section of its step at
  the top of the diffs, as the title of the step in the brief; Enter on it, its menu item Show Commit Alone and ⌥↑/⌥↓
  must show the commit alone (its heading, its own plan), and "All Changes" of the column goes back. Its right-click
  menu is Show Commit Alone, then the menu of the commit of the column; ⌘C copies its hash. The heading of a commit of
  the whole stack shown alone keeps "Review <branch>".
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should list the commits in their steps as one list, show the step of a commit on a click, and the commit alone on Enter, Show Commit Alone and Option+Down`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show the plan of a commit shown alone from its row in the tree: its card and the story of its diffs @webkit`, `should give the commits back to the column while the search keeps none of their rows, and copy a commit row by the copy key and its menu`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)
- A declaration of a step whose block another step shows must be one row in the section of its step, "hooks —
  removed · shown in step 2", which opens that section, and its heading must join the heading of that block there
  ("allowed — new · hooks — removed"); a section of no change of its own shows only such rows.
  [@test] ../../src/presenters/review-steps.test.ts (`lays a declaration without a block of its own out in its step as the row of the block another step shows`)
  [@test] ../../src/lib/diff/rows.test.ts (`makes a section of no change of its own the rows of the blocks other sections show, alone`)
- Above the diffs the bar of the step of the section at the top (`review-step-bar`) must show "Step 3 / 4", its title,
  the first paragraph of its body and the warning of a step no test reaches; it follows the diffs as they scroll and as
  J, K, a row of the tree and a step of the brief move them, and a click shows the first section of the step.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its brief, a step per commit with its why, its rounds and the tests that reach its code`)

## User Experience

```json
"steps": {
  "source": {"kind": "commits"},
  "steps": [
    {"id": "8fd70458",
     "commits": [{"id": "8fd70458", "subject": "IJAI-4673 the project open shows the Project view only when …",
                  "role": "starts"}],
     "concern": null,
     "stops": [
       {"unit": "declaration:…/ToolWindowManagerImpl.kt:…isShownOnStartupAllowed:id:String", "path": "…/ToolWindowManagerImpl.kt",
        "blocks": null, "uses": null, "also": [], "concern": null},
       {"unit": "declaration:…/IdeProjectFrameAllocator.kt:…openProjectViewIfNeeded:", "path": "…/IdeProjectFrameAllocator.kt",
        "blocks": null, "uses": "isShownOnStartupAllowed", "also": [2], "concern": null}],
     "tests": {"answered": true,
               "reached": [{"name": "isShownOnStartupAllowed",
                            "tests": [{"unit": "…", "path": "…/DeferredToolWindowRegistrationTest.kt", "line": 171,
                                       "name": "the project open may show only the tool windows that the stored layout shows",
                                       "depth": 1, "via": []},
                                      {"unit": null, "path": "…/DeferredToolWindowRegistrationTest.kt", "line": 186,
                                       "name": "the first-run policy activates the project view", "depth": 2,
                                       "via": ["keepingStoredLayout"]}]}],
               "unreached": [{"name": "openProjectViewIfNeeded", "within": 3}], "scenarios": []},
     "newApi": [], "moduleLinks": []}],
  "coverage": {"blocks": 76, "inStops": 76, "mechanical": 0, "missing": 0, "twice": 0},
  "commits": [
    {"id": "8fd70458", "subject": "IJAI-4673 the project open shows the Project view only when …",
     "body": {"blocks": [{"type": "paragraph", "lines": ["Three startup steps showed the Project view at each open: …"]},
                         {"type": "label", "text": "Tests", "kind": "verification", "runIn": false},
                         {"type": "list", "ordered": false, "start": 1, "items": ["DeferredToolWindowRegistrationTest …"]}],
              "trailers": ["Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"], "lead": true, "shown": 3},
     "mentions": [{"block": 0, "item": 0, "start": 41, "end": 66, "text": "isShownOnStartupAllowed",
                   "target": {"kind": "unit", "unit": "declaration:…/ToolWindowManagerImpl.kt:…isShownOnStartupAllowed:id:String",
                              "path": "…/ToolWindowManagerImpl.kt", "side": "new", "line": 171, "step": 0}},
                  {"block": 0, "item": 0, "start": 80, "end": 94, "text": "LayoutPolicy", "target": null}],
     "toldIn": []}]},
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
  lambda passed far away is no edge, and a call only the name ties is not walked. "Within 3 calls" says the walk
  stopped with callers left; a deeper test is not seen. A constant or a property is reached only by calls, not by the
  declarations that read it.
- A step whose commit lands nothing (a file added and removed again, a revert) has no stop.

## Measurements

`bd review-plan` (schema 10, a debug build) on 2026-10-31, every range read without the object cache:

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
- The tests that reach the code (`bd review-plan` schema 13, 2026-11-01, the 5 virtual branches of `develar` in
  idea-1 then, 18 steps, 11 of them with code): a test reaches code of 10 of the 11, and in 8 of those a test the
  change did not touch does; no step is marked unproven. The text match of the changed lines of the range's tests it
  replaced had marked 20 of 116 steps of 14 branches, most of them reached through an entry. 9 steps list code no test
  reaches, among it constants and properties, whose readers accio does not walk yet.
