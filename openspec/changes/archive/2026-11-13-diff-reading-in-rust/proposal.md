# Proposal

## Why

The count "N to read" of a file was made by the page (`readingOf` of `src/lib/diff/block-parts.ts`, the `toRead` of
`withEngineCounts`): its stops among its blocks of changed lines. Rust decides every start the count reads, and a sum
of what a pull request, a layer or a stack leaves to read (the next phases of the review of requests) has to be the
same count in Rust, for the window, the list of requests and `brd`. One count, in Rust, beside the marks it follows from.

## What Changes

- `file_diffs::reading::DiffReading { changes, toRead, toReadPastImports }`, counted from `DiffMarks::starts`, the
  blocks of the diff and `TextDiff::import_blocks`; sent with the `content` of a text, its `scopes`, the `marks` of its
  subject and the marks the index answers (`FileMarks`). Computed when sent, never stored: no version of a store
  changes.
- The proven starts of a measurement (`proven_runs`, `proven_starts`) move from `marks_measure` into the same module.
- The page takes the count of a file from `FileDiff.reading`; `readingOf` is gone. Where N and P stop (`stopsOf`) and
  the count of the section of a card (`FileRows.changeStarts`) stay the page's, from the same starts.
- What the user sees does not change. The role of a file (`review_plan::modules::role_of`) is not part of the reading:
  `review-plan` depends on `file-diffs`, so the reading of a range with its roles belongs to the review plan.
