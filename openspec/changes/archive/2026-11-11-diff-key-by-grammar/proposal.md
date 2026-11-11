# Proposal

## Why

The caches of the diffs keyed a pair by its two blobs, the whitespace policy and the language of each side. The
language joins what the diff of a pair depends on: `.kt` and `.kts`, `.ts`, `.tsx` and `.js`, `BUILD` and `.bzl` are
one language each and several grammars. The scopes of the same bytes already differed between a `BUILD` and a `.bzl`
file and between a `.ts` and a `.tsx` one, and the marks of a diff are read from the tokens of the grammar: with the
language in the key, a pair shown at a `.ts` path answered for the same blobs at a `.tsx` path.

## What Changes

- The key of a diff, in memory and in the `file_diff` store, holds the grammar of each side beside the policy
  (`diff_options`, the one function of the views and of the scans of `blocks_of`). The keys of the merges stay.
- The store keeps the diffs under a version made of `MODEL_VERSION` and the version of the tokens, so a stored diff is
  never read with marks of other parsers.
- The marks of a text come with its content: the stream sends a text with its marks, then its scopes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: File diffs: the key of the caches by the grammars; the marks with the content of the stream.

## Impact

- `crates/file-diffs/src/service.rs` (`DiffOptions`, `diff_options`, `model_version`, `store_version`), `blocks.rs`,
  `merge.rs`.
- `crates/object-cache/src/file_diff.rs` (`DiffKey::grammars`).
- `docs/design/git-operations.md` ("File diffs").
