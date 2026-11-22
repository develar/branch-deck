# Proposal

## Why

The Safe Push of `genui-flakes` (2026-11-22) failed with `Unresolved reference 'JBPanelWithEmptyText'`. A commit of
`a2ui-v1` gave a Kotlin test the plain import of `JBPanelWithEmptyText`, a type of the platform outside the range, with
uses. A later commit of `genui-flakes`, a root branch of its own, added a bare use in another hunk of the file. git
applied it onto master without the import, and `brd check` said `ok`. The index of the edges holds only the
declarations of the range and the names a `Binding` of the summary binds (a Rust `use`, a TypeScript import, a Kotlin
alias). A plain Kotlin or Java import binds no name in the summary, so it made no edge.

## What Changes

- `trial_check::symbol_dependencies` follows the plain imports of Kotlin and Java that the commits of the range add
  (`jvm_imports.rs`, `JvmImports`): per file, the commit that added the import of each name.
- A bare use of such a name on an added line of a later commit of the same file is an edge to that commit, evidence
  `import`, whatever declares the name. Not when the commit adds the import itself, the baseline has it, the file gives
  the name another way, or the import doesn't decide a function's call.
- `brd check` reports the branch of the use as `missing_declaration`, and the landing gate refuses its start.
- Decision 30 of docs/design/symbol-dependencies.md.

Capabilities: `commit/symbol-dependencies`.
