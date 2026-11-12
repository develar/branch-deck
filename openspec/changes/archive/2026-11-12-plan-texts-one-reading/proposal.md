# Proposal

## Why

The plan of one range was not always the same plan. The lab of the review concerns ran `brd review-plan` on 739
ranges; one run gave other groups than the reruns in 3 of them (`commons-lang-0008`: 2 groups → 3, `commons-lang-0017`:
18 → 24, `idea1-IJAI-3760`: 22 → 21). The run was the first of those ranges in its clone: the diffs of step 6 read the
blobs in that process, so the text of the hunks came from the memory of the diffs, which keeps a blob up to 8 MiB and
replaces an invalid UTF-8 sequence. A later plan found the diffs in the object cache and read the same blobs with the
summaries, which kept a blob only up to the 1 MiB of a parsed source file and dropped one that is not UTF-8. A Java
file in Windows-1252 was then `unparsed` (one unit in place of a behaviour and a mechanical import), a 1.1 MB
`MODULE.bazel` had no text, so no token of it joined a concern. The plan depended on what the cache held, not on the
commits.

The word diff of two paragraphs of the brief had a budget of 50 ms of wall time: on a loaded machine a pair ran out
and was `removed` and `added` in place of `changed` (`bd-0177` of the lab, under 8 parallel plans and a release build).

## What Changes

- The text of a blob that the plan reads with the summaries is the text the memory of the diffs would hold: up to the
  8 MiB of the diffs (`MAX_BLOB_BYTES`), an invalid sequence replaced (`into_text`). A source blob or a manifest over
  the 1 MiB of a parsed file is still none to the summaries, as before.
- The word diff of two paragraphs has no time limit: the work limit of the comparison (`FilesTooBig`, the changes it
  counts) bounds it, as the length of a paragraph does.
- A test pins it: the plan of a range from git equals the plan from the object cache with a new memory of the diffs,
  with a Java file not in UTF-8 and a text over 1 MiB.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: a blob has the same text wherever it is read.
- `branch/review-tour`: the word diff of a changed paragraph is bounded by its work, not by time.

## Impact

- `crates/trial-check/src/symbol_scan.rs` (`read_symbol_input_with`: the extra blobs under `MAX_BLOB_BYTES`).
- `crates/review-plan/src/lib.rs` (`review_plan_of`: `into_text`), `crates/review-plan/src/prose.rs` (`word_change`).
- `docs/design/git-operations.md` (Review plan, step 12).
- The lab (`~/projects/review-concerns-lab`): the three ranges give the groups of their first run on a cold and on a
  warm cache, under load.
