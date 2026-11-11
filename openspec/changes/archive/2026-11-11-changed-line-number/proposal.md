# Proposal

## Why

The line numbers have IntelliJ's `LINE_NUMBERS_COLOR`, made for the editor background. The colour of a changed line
fills its number too, and there the number is lost: 1.4:1 on a deleted line of the light scheme, 1.1:1 on a deleted and
1.04:1 on a modified line of the dark one. Inline, the default layout, shows a block it cannot pair as deleted and
added rows, so the old line number of a deleted row could not be read in the dark scheme.

## What Changes

- The number of a changed line has its own token, `--diff-line-number-changed`, at least 3:1 against every line colour
  of its scheme. The number of an unchanged line keeps `LINE_NUMBERS_COLOR`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: the number of a changed line reads on the colour of its line.

## Impact

- `src/index.css`, `src/components/diff/diff-view.css`, `docs/design/ui-theme.md`; `tests/e2e/commit-files/diff-font.spec.ts`.
