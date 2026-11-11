# Proposal

## Why

A reviewer reads every line a rename touched: a local `total` that became `sum` is five changed lines of a function,
each read to see that nothing else changed. The diff already says which lines changed in their formatting alone; it
said nothing of a renamed name, though the tokens and the bindings of the file can prove that a line changed in the
name of one local and in nothing else.

## What Changes

- A diff names the renames within its file (`TextDiff.marks.renames`): one binding on each side with the same places,
  read from the names the alignment of the tokens put at one place and proven by the bindings of both texts
  (`code_symbols::navigate::names_at`), each side asked with the other name.
- The blocks that hold a place of a rename are cut again with the two names of each place read as one: lines that
  changed in the name alone are a run of the kind `renamed`, wrapped or not.
- Such a run is proven only for a value of its body alone, every name of which the scopes bind; a parameter other
  code may name, a declaration that is no value, a local of a text the scopes read in part and a name that is not
  local are labels. A place that makes more of its name than its binding, the declaration of a name that is not
  local and a place on a line with another change are in no run. Whether a proven run is quiet its kind says
  (`renames-as-labels`).
- A name the file does not declare marks nothing and is handed out for the pass over the files of a change
  (`PairMarks::undeclared`).
- A block is asked one more question before the tokens: whether a side holds a word the other has not.
- `Rename.declared` names the places that declare the name. `MODEL_VERSION` 10.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the group Renames, the limit of the names by work, what has no rename, the shape of a
  rename, the question before the tokens.

## Impact

- `crates/file-diffs/src/marks_renames.rs` (new), `marks.rs` (`pair_marks`, `PairMarks`, `MAX_RENAME_WORK`),
  `marks_runs.rs` (the alignment hands out the tokens of its gaps), `service.rs` (`MODEL_VERSION`),
  `benches/marks_bench.rs`.
- `crates/text-diff/src/model.rs` (`Rename.declared`), `crates/code-symbols/src/tokens/mod.rs` (`Tokens::with_texts`,
  `Tokens::place_at`), `src/lib/bindings.ts`.
- `docs/design/diff-view.md` ("Marks"), `docs/design/text-diff.md` ("Model"), `docs/design/change-review.md`.
