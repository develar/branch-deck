# Proposal

## Why

The renames of a diff prove a rename by the names the scopes bind to a local on both sides. The first review of that
proof found locals whose bindings say less than the proof takes them for: a name that is a value where the code runs
(a Kotlin property with a delegate gets it, a function of JavaScript written without a name takes it), and a text of
which the scopes read only a part (a branch of `#if` of C# that the parser does not take, two declarations under two
`#[cfg]` of Rust, the text a direct `eval` runs, what stands where a parser gave up). And a stored diff kept the
renames of a rule of the scopes after the rule was fixed.

## What Changes

- A local says when the scopes read its code in part (`LocalBinding::read_in_part`): a text with a parser error, a C#
  file with an `#if`, a Rust function or closure with a `#[cfg]` inside it, a script with a direct `eval`.
  `LocalBinding::is_of_body_alone` is the one question of a reader of renames: a value no code outside its body may
  name, whose code the scopes read whole.
- `Site::Spelled` also at the declaration of a Kotlin property with a delegate and where a variable or a parameter of
  TypeScript or JavaScript is given a function or a class written without a name; `Site::Namesake` for a name inside
  the body of a `with`.
- A parameter of a TypeScript constructor with a modifier is the property it declares: no local, and one of the
  names of its spelling with `this.total` (Find Usages of it lists those too).
- The rules of the names have a version (`navigate::RULES_VERSION`), part of the version the store keeps the diffs
  under.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/name-bindings`: a local read in part, the sites of a name handed on as text and of a `with`, a parameter
  that declares a property, the reading of a rename.
- `commit/commit-files`: the version of the store of the diffs.

## Impact

- `crates/code-symbols/src/navigate.rs`, `psi_navigate.rs`, the `navigate.rs` of Kotlin, Java, C#, Starlark, Rust and
  TypeScript/JavaScript, `names_at_test.rs`.
- `crates/file-diffs/src/service.rs` (`model_version`, `store_version`), `marks_renames.rs` (`confirm`).
