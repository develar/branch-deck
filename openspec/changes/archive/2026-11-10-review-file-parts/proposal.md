# Proposal

## Why

The story shows a file in two cards or more when other changes come between its own (PluginClassLoader.kt of
idea-1 jna-one-copy in two), and each card said the counts of the whole file, also a card of one removal. The footer said "File 1 / 7" of either card, a file counted as seen at its first card, and a fold at the
top of the second card said "63 unchanged lines" over an added import that the first card shows: the import was in
the fold as a line of context, the old side 62 lines, the new one 63. Under Ignore Whitespace and Blank Lines the
tree said "+196 −2" where git says "+196 −3", and nothing said why.

## What Changes

- A card of a file the story shows in parts says which part it is and its own lines: "part 1 of 3 +5 −0", from the
  engine's counts of each block, which Rust gives (`UnitBlock::additions`); the footer says "File 2 / 7 · part 2 of
  3", the row of the file shows its first part, and a file is seen once each of its parts was.
- A fold counts only its unchanged lines and names the changes another section shows that it hides, with where, a
  click there showing that section: "⋯ 9 unchanged lines · 1 change in part 2"; a block the whitespace policy ignores
  is "1 line hidden by Whitespace" there.
- A click on a row or a fold that shows another section makes it the section at the top: the tree and the footer
  follow it.
- The counts of a file in the tree mark the lines the policy ignores once its diff is read, "1 line hidden by
  Whitespace" in the hover and in the description of the row.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-tour`: the parts of a file, its footer, a fold, the files seen, which surface counts as git.
- `branch/review-units`: a row and a fold show another section at the top; a file of the tree its first part.
- `commit/commit-files`: the counts mark the lines the policy ignores.

## Impact

- `crates/review-plan/src/units.rs`, `pair.rs`, `brief.rs`; `src/presenters/review-units.ts`, `src/lib/diff/rows.ts`,
  `src/components/diff/diff-painter.ts`, `DiffView.tsx`, `CommitReviewCode.tsx`, `src/components/commit/CommitReview.tsx`,
  `src/stores/review-seen.ts`, `src/lib/review-position.ts`, `src/lib/diff/view-files.ts`, `FileTree.tsx`; the template
  `review_parts`.
