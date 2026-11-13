# Proposal

## Why

A reviewer opens Your Turn in the morning with 12 to 30 waiting pull requests, often one author's chain, and wants to
drop the refactorings and the layers that change nothing first. What each leaves to read is proven already
(`RangeReading`), but only once its review window opens and fetches it. Read ahead, every row and every stack header
says it before any window opens: "Nothing to read · formatting", "Tests only", "3 changes to read in 2 files",
"Labelled refactoring · 1 change to read".

## What Changes

- **The read ahead** (`app_api::reads::review_prefetch::read_ahead_requests`): each listing of the review requests asks
  the host to read the waiting requests the session has no last reading of (`SessionHost::review_requests_listed`).
  One read for all of them, a phase a process: one `cat-file` and one `rev-parse` of the merge bases of those that are
  local (`sync_core::review_fetch::local_review_pairs`), those memory or the store has told at once and the others
  `Checking`; one fetch of every missing head (two for a mirror, `fetch_review_pairs`) into
  `refs/branch-deck/review/<n>/ahead/`; one `diff-tree --stdin` and one `cat-file --batch` of every diff
  (`review_plan::reading::ahead_readings`). The index is not asked (JetBrains/accio#777): a rename across files says
  "checking renames" until a review window asks the index itself.
- **The part `facts.reviewReadings`** of the session (`repository_model::review_readings`), derived by
  `domain_policy::range_reading::request_readings`: each request checking or read, with its verdict and the word of its
  author checked, each stack the sum of its layers. No frame keeps it.
- **The rows**: a reading chip before the age, the warning chip of the author's word after the labels, the sum of a
  stack in its header; no row moves by its verdict.
- **Pruning** keeps the review refs of the waiting requests with those of the review windows, and runs on a listing too.
- **The readings are kept on disk** (`object_cache::range_reading`): a final reading that no index could prove more of,
  by the version of the readings, so the next start shows it without reading a diff.
