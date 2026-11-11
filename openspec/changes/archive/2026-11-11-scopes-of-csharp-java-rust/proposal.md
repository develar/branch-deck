# Proposal

## Why

The second half of `scopes-of-the-languages`: the places where the scopes of the navigation differed from C#, Java
and Rust, so that a reader of the bindings (`commit/name-bindings`) took a change of meaning for the rename of one
local. Each was checked against the compiler (csc, javac, rustc).

- C#: a section of a `switch` was a scope of its own, though its statements share the block of the `switch`; a
  variable of the condition of a `while`, of a `lock` and of a statement without a block lived on after it; a
  top-level statement was a scope of its own; the source of a query clause saw the range variable of its own clause.
- Java: a call and a type of the spelling of a variable were bound to the variable; the variable of a pattern of a
  `case …:` was seen under the next label.
- Rust: a path segment before `::`, a type, the name of a macro and the field of a struct before its colon were
  bound to a binding of their spelling; a `fn` inside a function saw the bindings of that function; a width after
  the flag `0` of a format string (`{:0width$}`) was no usage.

## What Changes

- C#: the scopes of the locals are the language's (`opens_scope`, `declares_in`, `query_sees`); a name where a type
  is read and the member an initializer names are of no variable.
- Java: a name is of the kind of thing its place reads; a method reference names a member; the variable of a `case`
  pattern ends at the next label.
- Rust: a name is of its namespace; an item sees no binding of the code around it; a digit starts no word of a
  format string.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/symbol-navigation`: the scopes above.

## Impact

- `crates/code-symbols/src/csharp/navigate.rs`, `java/navigate.rs`, `rust/navigate.rs`, `psi_navigate.rs`
  (`Scoping::declares_in`).
- The highlighting keeps `csharp::is_block_scope` and `is_code` as they are: the scopes of the navigation are its
  own.
