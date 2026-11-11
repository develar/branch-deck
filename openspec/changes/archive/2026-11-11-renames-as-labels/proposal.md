# Proposal

## Why

The lines that changed in the name of a renamed local are proven by the bindings of both texts, and the plan of the
marks lets a reader pass a kind of run only after a measurement: a sample of its proven runs of real ranges, read by
a person, with no change of behaviour among them. The proof reads scopes without types and without the other files,
and its first review found changes it had passed (a delegate that gets the name, a branch of `#if`). So the kind
lands as a label. And the tokens of a text got what a reader of renames needs: one text for the two spellings of a
name.

## What Changes

- `is_quiet` says no for the kind `renamed`: every run of it is a label, and every block of it a stop. `has_proof`
  names the kinds that can be proven; the marks of a pair name the proven runs that are labels (`PairMarks::proven`),
  and `PairMarks::as_proven` gives the marks with those runs quiet: what the measurement reads and what the tests of
  a proof ask. The marks of a view are those but for the quiet of such runs and the starts that follow.
- The blocks of a rename are cut again within the work the first reading of the blocks left of the file's
  (`MAX_FILE_TOKEN_WORK` is the work of both readings); a block it does not pay for keeps the runs it has.
- `Tokens::with_texts` gives the tokens at some places another text, `Tokens::place_at` finds the token that starts
  at a line and a column of the view.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the runs of a renamed name are labels until their kind is switched on; the one work of a
  file for both readings of its blocks.
- `commit/equal-by-tokens`: two names read as one.

## Impact

- `crates/file-diffs/src/marks.rs` (`is_quiet`, `has_proof`, `PairMarks`, `marks_of`), `marks_runs.rs`
  (`runs_of_block` says when a block is over a limit), `marks_renames.rs` (`Renames::cut`).
- `crates/code-symbols/src/tokens/mod.rs`, `crates/code-symbols/src/tokens_test.rs`.
- The `commit_marks` template keeps its count: `src/Ledger.kt` says "8 to read" of its eleven changes, the three
  places of its renamed local among them, until the kind is switched on.
