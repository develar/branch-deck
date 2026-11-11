# Proposal

## Why

A user's Files tree in 1.8.0 put every short directory at the right of its row, by its counts: `daemon`,
`applications/src`, `main/kotlin` far from their folder icons. The path of a directory is cut from its start
(`dir="rtl"`, so that `…src/main` keeps its last folder), and the start of a rtl box is its right edge; the box of the
path runs to the counts (`flex-1`), so a path shorter than it stood at its right. It broke with 4cd79a389 (1.7.0), in
the tree of a commit and in the tree of the review of a branch alike. Five other paths of the page are cut from their
start by hand the same way, right only because their box is as wide as their text.

## What Changes

- A path cut from its start is one component, `StartTruncatedText` (`src/components/shared/`), which aligns it left:
  a short path stands right after the icon of its row, a long one still gives way from its start, its last folder kept.
  The tree, `FilePathLabel`, the breakages and the unbroken API of a review and the blocked Update use it.
- `pnpm lint:arch` refuses a direction of rtl outside it (`rtl-cut`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: Files section: a directory row starts its path at its icon, a long one cut from its start.

## Impact

- `src/components/shared/StartTruncatedText.tsx`, `src/components/files/FileTree.tsx`, `src/components/files/FilePathLabel.tsx`,
  `src/components/review/ReviewPrologue.tsx`, `src/components/review/UnbrokenApiList.tsx`,
  `src/components/repository/UpdateBlockedDialog.tsx`, `scripts/lint-arch.mjs`, `docs/contributing/frontend.md`
  ("Paths"), `playwright.webkit.config.ts`.
