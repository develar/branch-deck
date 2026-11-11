# Proposal

## Why

A declaration that stands at another place of its file with the same code had no name of its own in the review plan:
it was a change of its behaviour, which a reviewer read line by line to find nothing changed (and before the
whitespace of a declaration was read with every line of its blocks, it was folded as `whitespace`, which it is not).
A diff names such lines "Moved" ([Quiet Changes](../../specs/commit/quiet-changes/spec.md)); the plan says the same
word, and folds the move only where the place of a declaration has no meaning: a fold is never wrong.

## What Changes

- A paired declaration of the same form whose every line was deleted in blocks of old lines alone and added in
  blocks of new lines alone, with the same content of its tokens on both sides, moved. It is asked before whitespace
  and without the code around it, which a move changes, and is never whitespace.
- Its blocks go to the unit of the mechanical kind `moved` only where its place has no meaning: the lines that moved
  are whole functions, types or type aliases of a language that names its declarations whatever their order (of
  TypeScript a hoisted function or a type alias, nothing of Starlark), and between their two places stand only
  declarations they may cross. A value with its initializer, an enum entry, a function that crossed a macro, an
  initializer block or the end of an `impl`, a class of TypeScript, a method of TypeScript or JavaScript that moved
  with a decorator (the decorators of a class run in the order of its members) stay a unit `behaviour`, read.
- The mechanical kinds come in the order build files, lists of modules, imports, whitespace, moved, renames,
  generated. `moved` is a kind of a unit alone, never of a file.
- The page names the kind "Moved", the folded section of a file "Moved only".
- `brd review-plan` prints `"schema": 33`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-units`: a pair is shape, behaviour, moved or whitespace; the order of the mechanical units.
- `branch/review-plan`: the schema version; the header of a collapsed mechanical change ("Moved only", the group
  "Moved").
- `branch/review-tour`: the kinds of the mechanical group of a step.

## Impact

- `crates/review-plan/src/files.rs` (`MechanicalKind::Moved`), `units.rs` (`SourceFile::moved_pair`,
  `moved_freely`, `MECHANICAL_ORDER`), `moved.rs` (new: `place_free`, `crossable`, `ordering_token`), `whitespace.rs`
  (`Sides::same_code`, `no_code`, `no_token`), `groups.rs`.
- `crates/bd-rewrite/src/review_plan_command.rs` (`REVIEW_PLAN_SCHEMA_VERSION` 33).
- `src/presenters/review-plan.ts` (the words), `src/presenters/review-steps.ts` (the hover of the mechanical group
  of a step), `src/lib/bindings.ts`.
- `docs/design/change-review.md`, `docs/contributing/agent-cli.md`, `docs/design/git-operations.md`.
