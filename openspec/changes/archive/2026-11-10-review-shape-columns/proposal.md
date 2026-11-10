# Proposal

## Why

Each row of the Shape was a grid of its own, its counts an `auto` column, so the bars of the rows started at
different edges whenever their counts differed in width, and the folders under a changed module read as numbers
without a name ("plugins/cl 1 · plugins 1").

## What Changes

- The rows of the Shape are subgrids of one grid: the bars start at one edge, the lines and the files stand in
  columns of their own.
- The folders of a changed module are words: "in ui/embeds, ui/markdown and ui/util"; the row counts its files.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-tour`: the columns of the rows of the Shape, the words of the folders of a changed module.

## Impact

- `src/components/review/ReviewMap.tsx`, `src/presenters/review-map.ts` and their tests; the snapshot of the rail.
