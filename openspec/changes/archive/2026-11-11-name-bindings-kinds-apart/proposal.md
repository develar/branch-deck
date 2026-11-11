# Proposal

## Why

The review of the name bindings (`commit/name-bindings`) found answers a reader of renames cannot use, and claims the
code did not keep.

- A name the language never reads as a variable (a type, a Java call, the field of a Rust struct before its colon,
  the member a C# initializer names, the name of a named argument, the name of a macro) stayed among the uses of a
  local of its spelling, as a `namesake`, and came back as a `captured` clash: `S { total: total }` → `S { total: sum }`
  never read as the rename of one local, while its TypeScript twin did.
- A parameter of a Starlark `lambda` said that no call names it, though `key(item = 2)` does; a parameter of a Rust
  function under an attribute macro (`#[tauri::command]`, `#[instrument]`) said so too, though the macro names it in
  what it generates.
- The spec promised that a scope narrower than the language's only ever costs a stop. A use that a change leaves in
  place is no place of the rename, so such a use is silent: the promise was wrong, and the scopes that broke it are
  fixed by `scopes-of-the-languages` and `scopes-of-csharp-java-rust`.

## What Changes

- A name its place reads as another kind of thing than a variable is bound to no local: it is no use and no clash of
  one, and stands among the names of its text that no local binds.
- `namesake` is left for what only the types tell: a called Kotlin value, a Kotlin name after `::`, a name that is
  all of a Java `case` label, a name in a Java class with a supertype inside the body of the variable.
- The parameter of a Starlark `lambda` and of a Rust function under an attribute that is not the language's own say
  that other code may name them.
- The reference reading of a rename drops nothing from the uses: named arguments and labels are no uses any more.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/name-bindings`: the sites, the parameters other code names, the reading of a rename.

## Impact

- `crates/code-symbols/src/navigate.rs` (`clashes`, `Site`), every `navigate.rs` of a language (`site_of`,
  `local_of`, the kinds of the names).
- `crates/code-symbols/benches/names_bench.rs`: the cost of many places, which the design document quoted from a run
  nobody could repeat.
