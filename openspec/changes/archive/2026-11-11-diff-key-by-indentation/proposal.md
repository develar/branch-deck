# Proposal

## Why

The moves of a diff depend on whether the indentation of its file has a meaning: a line of a move at another indent
than the others is an edited line in a Python file and the same line in a text file. The caches kept a diff by its
blob pair, the policy and the grammar of each side, so the same two blobs at a `.py` and at a `.txt` path shared one
diff, with the moves of whichever path was read first.

## What Changes

- The key of a diff holds, for a side without a grammar, whether its indentation has a meaning
  (`code_symbols::tokens::indentation_matters`); a grammar decides it for its own files. In memory, in the store and
  for the scans, through the one function of all of them (`service::diff_options`).
- `MODEL_VERSION` 9: the stored diffs of the model before are not read.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: File diffs: what the caches key a diff by.

## Impact

- `crates/file-diffs/src/service.rs` (`diff_options`, `INDENTED`), `crates/object-cache/src/file_diff.rs` (the words
  of `DiffKey`).
- `docs/design/git-operations.md` ("File diffs").
