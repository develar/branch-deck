# Proposal

## Why

A check on a real Kotlin stack renamed a private member (`sumOf` to `totalOf`) in a repository with no accio index. The
reading named the file not proven (`renamesUnanswered`) and still counted its changes against the author's word:
"Labelled refactoring · 4 to read" flagged a label that may well be true. `author_claim::check` stood back only while a
rename waited for the index (`RangeReading::pending`), and summed every change to read of the roles outside the word,
those of the files no proof can be made of too. The rule of the design is that only a proven contradiction is flagged.

## What Changes

- Each role of a reading sums the changes to read of the files it proves apart (`RoleReading::to_read_proven`,
  `files_to_read_proven`), computed with the rest in `review_plan::reading::range_reading`.
- `author_claim::check` counts only those: a change of a file not proven, for any reason (a rename pending or
  unanswered, no grammar, binary, too large, a submodule, missing), proves nothing; the summary already says "N files
  not proven". The guard on a pending rename goes: a pending file is a file not proven.
- The kept readings are of another version (`READING_RULES` 2), and `brd review-plan` prints `"schema": 36`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `review/range-reading`: the proven changes of a role are summed apart; the author's word is flagged only by them.
- `branch/review-plan`: schema version 36.

## Impact

- `crates/domain-types/src/range_reading.rs`, `crates/review-plan/src/reading.rs`,
  `crates/domain-policy/src/author_claim.rs`, `crates/bd-rewrite/src/review_plan_command.rs`, `src/lib/bindings.ts`;
  `docs/contributing/agent-cli.md`, `docs/design/git-operations.md`.
