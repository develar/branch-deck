# Proposal

## Why

"Copied" of Copy Pull Request Link held the width of its words, but the other buttons that say "Copied" in place
(Copy Prompt of Start in Air, Copy Prompt for Agent of the conflict fix and of the blocked update) shrank, and the footer
of their dialog moved under the pointer that clicked.

## What Changes

- `WidestLabel` (`src/components/shared/WidestLabel.tsx`): a label that changes in place in the width of the widest it
  shows, the trick of `RunLabel` and of the pull request chip in one place; every Copy button with words and the
  percent of Update use it.
- Capability `repository/update-project`: "Copied" holds the width of "Copy Prompt for Agent".
