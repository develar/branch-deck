# N and P start from the selected file

## Why

N and P step from the last change they showed, else from the file at the top of the diffs, which the view learns from
a scroll; with neither, from the first file. A chip that showed one file, then Show All Files, keeps that file
selected (the footer says "File 4 / 4") with the files above it shown, and nothing scrolled: P went back from the first
file, found nothing before it, and did nothing. In branch-review.spec.ts:598 ("should filter tree, diffs and navigation
to the files of an owner chip", a loaded full run: "File 4 / 4" for "File 3 / 4") N had shown a change of README.md
before the chip went back to All, so P went on from there; a run in which that N found no change of the file yet
(its diff still read) showed the same P doing nothing.

## What Changes

- While the diffs have shown no change and no scroll told the file at their top, N and P start from the file the tree
  selects (`DiffViewHandle.moveChange` takes it from the review), else from the first file.
- Capability `commit/commit-review`.
