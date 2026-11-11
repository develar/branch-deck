# Proposal

## Why

The renames of a diff ask the names of every place where a name stands for another, before they know how many
bindings those places are. A limit on the places asked cut a file with one local of the same name in sixty functions
after its tenth function, though the sixty bindings cost sixty-one resolutions: the cost of an answer is its
resolutions, and only the navigation knows them.

## What Changes

- `names_at_within(grammar, text, questions, resolutions)` answers within a number of resolutions, one per spelling
  asked and one more per binding asked with another name; a place past them has no answer. `NamesAt::resolutions`
  says how many the answers took.
- `resolution_work(grammar, tokens)`: what a resolution costs by its grammar, the tokens of the text where it walks
  the tree and an eighth of them where it reads the names of one spelling.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/name-bindings`: the answers within a number of resolutions.

## Impact

- `crates/code-symbols/src/navigate.rs`, `crates/code-symbols/src/names_at_test.rs`.
- `crates/file-diffs/src/marks_renames.rs`: the renames of a diff ask within `MAX_RENAME_WORK`.
