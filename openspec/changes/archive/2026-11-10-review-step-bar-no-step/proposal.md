# Proposal

## Why

The bar above the diffs said "Step 2 / 1 · Other changes" at Other changes of a review of one step: Other changes
has no number in the overview and in its opening, the count of the bar leaves it out, and the bar still gave it its
place in the list of the chapters. At a group the story shows after its steps (an import alone of a file in parts,
a lock file) the bar fell back to the first step, "Step 1 / 1 · probe", and the footer said "Step 1" there. A fold
over a block of such a group said "1 change in Imports only (1)", and its row "Shown in Imports only (1)": the count
of the heading of the group leaked into the name of the place. The hover of the map named a file of Other changes
"Step 2 · Other changes".

## What Changes

- The bar names what the story tells as no step alone: "Other changes" at its opening and its cards, a group after
  the story by its name ("Imports only"), the footer naming no step there; a click shows the opening of Other changes,
  the first card of the group. The count of the steps is the steps the story tells.
- A row and a fold name a group by its name on each of its files, never the count of its heading: "Shown in Imports
  only", "1 change in Imports only".
- The hover of the map names Other changes without a number.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-tour`: the bar of the step at the top, a fold, the number of Other changes.
- `branch/review-units`: the row of a block another section shows.

## Impact

- `src/presenters/review-units.ts` (`presentStepBar`, `NarrativeSection.groupName`), `src/presenters/review-steps.ts`
  (`formatStepDivider`), `src/components/commit/CommitReview.tsx` (`StepBar`); the e2e of the parts of a file and of
  the row of the imports.
