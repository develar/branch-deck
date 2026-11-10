# Proposal

## Why

In the review of a branch the tree set the conventional prefix of a commit subject (`cleanup platform:`) dimmed in the
code font, as the main window does, while the contents, the opening of a step and the bar above the diffs wrote it
in the font of the text: one title, two looks. The hunk heading of a block named the declarations it holds in the
order of their names (`JNA_NATIVE_CLASS_FILE` before `JNA_PACKAGE`), not as the source declares them. A click on a
name of a body (`PluginClassLoader.tryLoadingClass`) whose file the review cuts into two sections selected the name in
the section that shows its line beside another change, not in the section of its own change.

## What Changes

- Rust splits the title of a commit (`ReviewTitle`: `prefix`, `text`) for `ReviewStep.title`, `StepCommit.title` and
  `StoryCommit.title`; a label, a sentence or a name has no prefix. One component (`ReviewTitleText`) sets it in the
  tree, the contents, the opening and the bar: the prefix dimmed in the code font, no link inside the button.
- The declarations of one block that hold none of it follow the unit of the block in the order of their lines.
- A place of a unit (a mention, a fact of a step) is shown in the section of that unit.
- `brd review-plan` prints `"schema": 30`: the titles are objects.

## Capabilities

### Modified Capabilities

- `branch/review-tour`: the title of a commit has its prefix apart; a click on a name shows it in the section of its
  unit.
- `branch/review-units`: the declarations without a block of one block follow in the order of their lines.

## Impact

- `crates/review-plan/src/steps.rs`, `story.rs`, `message_steps.rs`, `units.rs` and their tests; `src/lib/bindings.ts`.
- `src/presenters/review-steps.ts`, `review-story.ts`, `src/lib/file-tree.ts`, `src/lib/commit.ts`,
  `src/components/review/ReviewTitleText.tsx`, `ChapterOpening.tsx`, `ReviewPrologue.tsx`, `CommitReview.tsx`,
  `FileTree.tsx`, `CommitSubject.tsx`, `DiffView.tsx`, `src/lib/review-place.ts`.
- `REVIEW_PLAN_SCHEMA_VERSION` 30 and the docs that name it.
