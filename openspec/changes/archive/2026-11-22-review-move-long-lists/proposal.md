# Proposal

## Why

The heading of a new file that several moves came into named the file they came from once per declaration ("parseTotal
— moved from Orders.kt · parseSum — moved from Orders.kt") and grew without a bound: the measured top case moves ten
declarations in one commit. The row of the place they left cut its list ("Moved a, b, c and 7 more to Parsing.kt") with
no way to read the rest.

## What Changes

- The heading of a file taken whole names each other file once, its declarations a list cut as every list of the app
  is ("parseA, parseB, parseC and 7 more — moved from Orders.kt"), each name in the code font, the rest in the hover of
  the count and to a screen reader after it (`HeadingPart.names`).
- The row of the place a move left, and its place in a fold, keep their words in full where they cut a list: their
  hover and their name to a screen reader (`SectionUnits.details`).
- Words of the page only: no fact of Rust, no mark of a diff changes.

Capabilities: `branch/review-units`.
