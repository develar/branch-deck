# Proposal

## Why

The review of the marks of a subject found that the read of a subject's diffs waited for the index of the repository
(a query of seconds, 14 s warm on the IntelliJ monorepo), that a subject read ahead of its view and a subject the
user stepped past each ran a query to its end, and that the pass over a subject added a quiet run of formatting,
after the rows of its file were drawn, that leans on a name nothing proves. It also found verdicts that were wrong:
a use without a word judged as a use of the old name, lines of another depth read as moved without a change where
the indentation has a meaning, and uses mapped to lines of a text whose lines the index counts otherwise.

## What Changes

- The diffs of a subject ask no index and end with their last file; the names say `asked` where an index is. The
  index is asked in a read of its own (`get_rename_proofs`) by the window that shows the subject, one question per
  window, superseded by the next, killed with its window; its answers are kept in memory by their two trees.
- The page asks only of a subject a view holds, one at a time, the subject shown last first.
- The pass over a subject adds labels alone: a pair keeps of its third reading the runs of the kind `renamed`, each
  with every name it leans on, the name after its block included.
- A move between files pairs a line only with a line at its depth where the indentation has a meaning.
- A use that writes no word and a use by another word that both trees hold alike ask nothing of a rename; a use in
  a text with a lone carriage return, and a declaration of a language the index proves nothing of, prove nothing.
- accio is asked again what it answers once its executable is another.
- The layout of what the stores keep is pinned with their versions.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the group Subject.
- `commit/commit-files`: the stream of the diffs, its caches, the read of the page.

## Impact

- `crates/file-diffs/src/service.rs` (`FileDiffs::rename_proofs`, `stream` without an index), `marks_subject.rs`,
  `marks_renames.rs`, `leftovers.rs`, `accio.rs`, `merge.rs`; `crates/text-diff/src/moves_across.rs`.
- `crates/app-api/src/reads/diffs.rs` (`get_rename_proofs`), `session/windows.rs`.
- `src/stores/file-diffs.ts`, `src/stores/file-stream.ts`.
- `docs/design/diff-view.md`, `docs/design/text-diff.md`, `docs/design/git-operations.md`.
