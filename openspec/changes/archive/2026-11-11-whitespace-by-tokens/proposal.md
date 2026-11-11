# Proposal

## Why

The review plan folded a change as `whitespace` when its text without whitespace was the same. So a change of the
spaces inside a string, `a - -b` turned into `a--b` and a `return` joined with the call of the next line were folded
into the mechanical group at the end of a review, and the fingerprint of a unit, which keeps a reviewer's mark across
plans, stayed the same when only a string's spaces changed after the review. A fold that hides real work is the worst
defect of the review. A Starlark file was never `whitespace`, not even for a call wrapped inside its brackets.

## What Changes

- `whitespace` of a file of a grammar is told by its tokens ([Equal by Tokens](../../specs/commit/equal-by-tokens/spec.md)):
  a hunk, a block of the pair and a paired declaration change whitespace alone when their lines are equal by tokens,
  code and comments. A file of no grammar keeps the rule of characters; a file of a grammar without tokens has no
  change of whitespace.
- Starlark, read by its grammar, can be `whitespace`: a line continued inside its brackets; the indent of a logical
  line stays real work.
- The fingerprint of a declaration and of the blocks of a slice reads the content of the tokens
  (`code_symbols::tokens::content`: their texts and the tree between them, without what opens before the first and
  closes after the last), so a change of the spaces inside a string, or a line break that ends a statement, after a
  review is another change, and the same change of a function that became the last of its file is not.
- The tokens of a blob are read once per plan. A hunk asks them only when its lines are the same without their blanks
  (`same_without_blanks`, which lines equal by tokens always are); the fingerprints read the tokens of both sides of
  every source file that has a unit.
- A file of a grammar on one side alone (a rename from `.txt` to `.kt`) has no change of whitespace.
- `brd review-plan` prints `"schema": 32`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: Mechanical Changes: whitespace by the tokens of a grammar; the schema version.
- `branch/review-units`: a block and a pair of whitespace by their tokens; the fingerprint by the content of the
  tokens.
- `commit/equal-by-tokens`: the content of a run of tokens.

## Impact

- `crates/review-plan/src/whitespace.rs` (new), `files.rs`, `units.rs`, `lib.rs`;
  `crates/bd-rewrite/src/review_plan_command.rs` (`REVIEW_PLAN_SCHEMA_VERSION` 32).
- `docs/design/change-review.md`, `docs/design/git-operations.md`, `docs/contributing/agent-cli.md`.
