# Proposal

## Why

"Copy Pull Request Link" of the details of the pull request chip said "Copied" in place, which is narrower: both
actions of the details then fit on one line, the popover shrank by that line from under the pointer that clicked, and
the hover of the chip closed it on the next move of the pointer (Chromium's own move after the layout too). The user
lost the details, and the E2E test of the copy failed under load when the close came before its check (4 of 150 runs
on the Studio).

## What Changes

- `CopyLinkButton` (`src/components/branch/PullRequestChip.tsx`) holds the width of its longer words, as the live
  values of the rows do (docs/design/ui-theme.md, "Live values hold their own width").
- Capability `branch/pull-requests`: the requirement "Chip: Hovering the chip opens the details" says that "Copied"
  keeps the width of the button and the details stay open, with its scenario.
