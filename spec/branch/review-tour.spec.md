---
name: Review Tour
description: Requirements for the story of a review (experimental), what a reviewer who sees a change for the first time reads before any diff; the brief (the issue the commits name, the rules of the specs and the prose of the docs the change adds, with the tests their test links name) and the steps, a step per commit whose body tells it, its follow-ups and the commits without a body where their lines landed, else the concerns, each with the parts of the units that land in master in its order, a declaration it adds first and its uses after it, and the tests of the range that name its code; every block of the diff in one stop.
style: plain-1
targets:
  - ../../crates/review-plan/src/steps.rs
  - ../../crates/review-plan/src/brief.rs
  - ../../crates/review-plan/src/interdiff.rs
  - ../../crates/review-plan/src/lib.rs
  - ../../crates/bd-rewrite/src/review_plan_command.rs
  - ../../src/presenters/review-steps.ts
  - ../../src/components/review/ReviewBrief.tsx
  - ../../src/components/review/ReviewPlanCard.tsx
  - ../../src/components/commit/CommitReview.tsx
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
units that land in master, a declaration it adds first and the uses of it after it, and the tests of the range that
name its code. Reviews differ: where the commits tell no story, the concerns of the range are the steps. The vision
and the rounds: [Change Review](../../docs/design/change-review.md).

## Goals

- The words of the author before any diff: the bodies of the commits, the rules of the specs, the docs.
- Each change once, as it lands: a step shows the units of the range, never a state a later commit replaced.
- Every block of the diff in one stop, checked: the coverage is a field of the plan.
- No git process of its own: the plan read every commit and its hunks already.

## Non-goals

- A tour a language model writes, or one the author writes with `bd tour`: the next round
  ([Change Review → Next](../../docs/design/change-review.md#next)).
- A test that reaches a declaration through its callers: accio answers it once JetBrains/accio#107 lands; until then a
  test names a declaration in its own changed lines.
- Whether a use left outside breaks by the arguments it passes: JetBrains/accio#106.

## Requirements

### Where a Line Comes From
- The lines of each changed file must be carried through the hunks of the commits that change it, oldest first, so
  that each line at the head names the commit that wrote it last and each line of the base the commit that removed it,
  across a rename too. A block of the range belongs to the commits of its changed lines; a blank line counts for none.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote`)

### Steps of the Commits
- A commit whose message has a body of 3 words or more after its subject, and no `fixup!`, `squash!` or `amend!`
  subject, tells a step. When a range of two commits or more has one, the steps must be the commits: each unit of the
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
  with its subject and its body.
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

### Tests That Name It
- A declaration a test framework runs is a test ([Symbol Dependencies](../commit/symbol-dependencies.spec.md)). A
  declaration of the code of a step, of a parsed language, whose body or form changed or that it adds, and in no test
  file, must be `named` by the tests of the range whose changed lines within their declaration name it as a whole word
  (a member with its owner, a type, named in the file of the test too), else `unnamed`; the words of a Kotlin test name
  in backticks name nothing. A name two such declarations share is led by its owner (`Owner.run`).
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_tests_that_name_the_code_of_a_step_and_the_code_none_names`, `a_test_reads_as_its_words`)
- A step that adds or changes a rule of a spec must list the tests its test links name (`scenarios`): a scenario
  that names no declaration proves it.
  [@test] ../../crates/review-plan/src/steps_test.rs (`the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests`)

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
- `bd review-plan` must print `steps` and `brief` (`"schema": 10`), the ids of the commits of the steps as short as
  the others, and run no git process for them.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### In the Review Window
- With the review plan, the Review Summary must open on the brief (`ReviewBrief`): the issue key, a link with the
  popover of its issue; What Changes, each entry its kind ("Spec rule", "Decision", "Agent guide", "Doc"), its file and
  the heading of its section, which open the file, its paragraphs and "Proven by restart.json"; then the Steps, each its
  number and its title (the subject of the commit that starts it, without the issue key and the `(prefix)`), the first
  paragraph of its body, the rest under More, its rounds ("Follows up: …", "Without a story: …"), "Tests that name it:
  …", "Proven by …", and "Not named by a test: …" muted; a step no test of the range names any code of, and that no
  scenario proves, says so in the warning tone ("No test in this change names paint"). A click of the title of a
  step must show its first section at the top of the diffs, its row selected. Steps of the concerns say why ("One
  commit: the steps are its tasks"); a range whose only step is what no evidence joined lists no step.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its brief, a step per commit with its why, its rounds and the tests that name its code`)
  [@test] ../../src/presenters/review-steps.test.ts (`words the brief: the why of each step its first paragraph, the rest folded, its rounds, the tests that name its code, a step no test proves`, `names a step by the subject of the commit that starts it, without the issue the brief names, a step of no story by what it is`)
- The tree must list the steps, each a group "2 · The open keeps a stored layout" with its files by their path, a file
  of two steps with its row in the step of its most lines and a row "also in" each other; a step of the concerns is
  named by its concern and keeps its main rows ([Review Concerns](review-concerns.spec.md#in-the-review-window)). The
  story of the diffs must follow the steps: the first section of each under its heading (`review-concern-heading`).
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its brief, a step per commit with its why, its rounds and the tests that name its code`)
  [@test] ../../src/presenters/review-steps.test.ts (`names a step of a concern by its concern and keeps its main rows`)
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
  the first paragraph of its body and the warning of a step no test names; it follows the diffs as they scroll and as
  J, K, a row of the tree and a step of the brief move them, and a click shows the first section of the step.
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its brief, a step per commit with its why, its rounds and the tests that name its code`)

## User Experience

```json
"steps": {
  "source": {"kind": "commits"},
  "steps": [
    {"id": "8fd70458",
     "commits": [{"id": "8fd70458", "subject": "IJAI-4673 the project open shows the Project view only when …",
                  "body": "Three startup steps showed the Project view at each open: …", "role": "starts"}],
     "concern": null,
     "stops": [
       {"unit": "declaration:…/ToolWindowManagerImpl.kt:…isShownOnStartupAllowed:id:String", "path": "…/ToolWindowManagerImpl.kt",
        "blocks": null, "uses": null, "also": [], "concern": null},
       {"unit": "declaration:…/IdeProjectFrameAllocator.kt:…openProjectViewIfNeeded:", "path": "…/IdeProjectFrameAllocator.kt",
        "blocks": null, "uses": "isShownOnStartupAllowed", "also": [2], "concern": null}],
     "tests": {"named": [{"name": "isShownOnStartupAllowed",
                          "tests": [{"unit": "…", "path": "…/DeferredToolWindowRegistrationTest.kt",
                                     "name": "the project open may show only the tool windows that the stored layout shows"}]}],
               "unnamed": ["openProjectViewIfNeeded", "settleStartupFocus"], "scenarios": []}}],
  "coverage": {"blocks": 76, "inStops": 76, "mechanical": 0, "missing": 0, "twice": 0}},
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
- "Unnamed" is no proof of an untested change: a test may reach the code through an entry it names
  (JetBrains/accio#107). The window marks a step only when no test of the range names any of its code and no scenario
  proves it.
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
- No test of the range names any code of 20 of 116 steps (17 %); 66 (57 %) have a declaration no test names, mostly a
  helper the tests reach through an entry: the window marks the first, and lists the second muted.
