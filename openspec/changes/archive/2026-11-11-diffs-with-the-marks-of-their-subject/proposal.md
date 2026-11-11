# Proposal

## Why

The pass over the files of a subject (`marks-of-a-subject`) needs every pair of the subject, and its answer of the
index comes after the files are drawn. The stream of the diffs sent each content as it was known and ended; its
caches kept the diff of a pair and nothing of what the pair leaves to the other files.

## What Changes

- The stream of the diffs ends with a `marks` event for each file the others of its subject say something of, and
  one more with what the index answered. When the caches have every pair the pass runs before the first content,
  which carries its marks. A late event adds runs that are labels, words and what a reader may pass; it takes no row.
- What a pair left is kept beside its diff, in memory and in the `file_leftovers` store, under the same key and
  version; a text is known to a view with it or compared again.
- `get_file_diffs` is a command of its window: the query of the index is killed once the page that asked went away.
- The page takes the marks of a `marks` event in place of those of the content of the file and keeps what a view
  opened of it.
- A pair hands out, with the names it declares none of, the runs their blocks have were they renames.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: the stream of the diffs, its caches, the store of the page.
- `commit/quiet-changes`: what a pair hands out of a name it declares none of.
- `commit/symbol-navigation`: the queries of the index that never build one.

## Impact

- `crates/file-diffs/src/service.rs` (`FileDiffs::stream`, `cached`, `compare`), `model.rs` (`FileDiffEvent::Marks`).
- `crates/app-api/src/reads/diffs.rs`, `commands.rs`, `session/windows.rs` (`Windows::shows`).
- `src/stores/file-stream.ts`, `src/stores/file-diffs.ts`, `src/lib/bindings.ts`.
- `docs/design/git-operations.md` (File diffs).
