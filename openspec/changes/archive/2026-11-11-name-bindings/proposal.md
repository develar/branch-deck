# Proposal

## Why

The marks of a diff are to tell a rename of a local name from a change of the code (stage 4 of the plan of the diff
marks). That is safe only by the bindings: every name of the old text bound to the declaration is a place of the
rename, and so on the new side. The navigation answered this for one place per parse, as Find Usages asks it, with
nothing of what a rename must also know: whether other code may spell the name (a named argument), whether a site makes
more of the name than its binding (`{ total }`, `nameof(total)`), and whether the new name is already taken where the
binding is seen.

## What Changes

- `code_symbols::navigate::names_at(grammar, text, questions)` answers many places of a text from one parse: per
  place the name, what a body declares it as (`LocalBinding`: a variable, a parameter, a lambda parameter, a loop
  variable, a catch parameter, else `Other`; `named_by_calls`), every name bound as it is with its access and its
  site (`Site`), and for a new name asked with the place what it would clash with (`Clash`). The places of one binding
  share an entry.
- `usages_at` and `usages_of` are built on the same resolution (`navigate::Names`, one implementation per parser):
  nothing of them is written twice.
- All of it is data: which local is quiet, and which site is, is the decision of the reader of the answer.

## Capabilities

### New Capabilities

- `commit/name-bindings`: the answers of `names_at`.

### Modified Capabilities

None (`commit/symbol-navigation` changes with `scopes-in-reading-order`, in the same commit).

## Impact

- `crates/code-symbols/src/navigate.rs`, `psi_navigate.rs`, `kotlin/navigate.rs`, `java/navigate.rs`,
  `csharp/navigate.rs`, `starlark/navigate.rs`, `js/navigate.rs`, `rust/navigate.rs`.
- No type of the page, no command, no stored value.
