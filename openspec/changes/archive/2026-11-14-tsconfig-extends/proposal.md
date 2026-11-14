# Proposal

## Why

The dependencies by symbols read the path aliases of a `tsconfig*.json` by a reader of their own
(`trial_check::symbol_scan::ts_aliases` with `without_comments`): `compilerOptions.paths` of the file itself, every
pattern that takes a specifier. A configuration whose aliases come from the one it extends (`"extends":
"../tsconfig.base"`, the common layout of a monorepo) gave none, a specifier under `baseUrl` named no file, and two
patterns that take a specifier both gave targets where tsc takes the one with the longer prefix. psi-syntax now reads
a configuration as tsc does (`js_syntax::tsconfig`, accio's decision 2026-11-13-tsconfig-read-by-js-syntax), the reader
accio's modules use too.

## What Changes

- `symbol_scan` reads each `tsconfig*.json` of the directories above the changed files with `js_syntax::tsconfig`:
  JSON with comments and trailing commas, its `extends` chain, `baseUrl` and `paths` as tsc applies them. The reader of
  trial-check and its JSON clean-up are deleted; `SymbolInput::aliases` holds the `Aliases` of each configuration of a
  directory.
- A `tsconfig*.json` of a directory above the files is in the batch of the summaries, so a chain of those costs
  nothing, and a name tried that those directories lack (a name without `.json`) costs nothing either
  (`ModuleRoots::file_at`: the files of those directories by name). Any other configuration a chain names is read in
  one more `cat-file --batch` per level of the chains: by its blob when those directories list it, else at the
  revision by its path.
- A TypeScript import names the targets of the best pattern of each configuration, then the specifier under its
  `baseUrl` (`Aliases::candidates`).
- Capability `commit/symbol-dependencies`: the aliases with their `extends` chain, their reads, and the edge of an
  import under `baseUrl` or by the longest pattern.
