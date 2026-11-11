# Proposal

## Why

Every diff view opened in Split, as IntelliJ's diff does. A side of Split is about 50 columns in the preview of a
commit and in a review beside its tree, so many changed lines scroll or wrap, and the eye travels between two columns
to find what a light edit changed. Inline shows such a line as one row, the deleted parts struck through beside what
replaced them, and reads top to bottom at any width.

## What Changes

- Inline is the default layout of every diff view (`diff-view-settings.ts`); the toggle and Settings → Diffs list the
  layouts from the most compact one: Inline · Unified · Split.
- Only what differs from a default is stored, so a choice of Split made while Split was the default cannot be told
  from no choice: such a user gets Inline, without a notice. A choice stored since keeps winning.
- The default layout is the one the tests run in: the keys, the find, the folds and the benchmark choose no layout; a
  test of what only Split has (its two sides, its divider, the colours of a block by its type) stores Split.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: Inline is the default, a stored choice wins.
- `commit/commit-review`: the toggle is Inline/Unified/Split, Inline by default.
- `commit/commit-row`: the toggle of the missing commits keeps a stored choice over the Inline default.
- `repository/settings`: Layout lists Inline · Unified · Split.
- `branch/branch-review`, `repository/title-bar`, `repository/review-requests`: the toggle by its new order.

## Impact

- `src/stores/settings/diff-view-settings.ts`, `src/components/shared/DiffViewToggle.tsx`; the e2e of the diffs, their
  snapshots, `tests/perf/diff-review.perf.ts` (Inline by the default, Split stored); `docs/design/diff-view.md`,
  `docs/design/ui-theme.md`.
