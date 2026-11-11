# Proposal

## Why

A reader that trusts the bindings of a file (the renames of a diff, `commit/name-bindings`) found the scopes of the
navigation wider than the languages': a name was bound to a declaration that its place does not see.

- Kotlin and Rust: the value of a variable read the variable it declares. `val name = name.trim()` and
  `let total = total + 1` read the name before them; Rust shadows this way on every page.
- Kotlin: a name before the declaration of a local of its block went to that local, not to the property around.
- Rust: the binding of an `if let` or a `while let` was seen after the block it guards.
- C#: the variable of a `foreach` was declared in the block around its statement, so a name after the loop went to it.
- Java, C#, Kotlin, Rust: the collection of a loop saw the variable of the loop.
- Starlark: a comprehension declared its variable in the `def` or the file around it, and a default of a parameter
  read the parameter.
- Rust: a name a format string takes from the code around its macro (`"{total}"`) was no usage of it.

Each sends Go to Declaration to another declaration, and loses or adds usages of a local.

## What Changes

- Kotlin: a variable of code is seen after its own statement, the variable of a `for` after its range; a block of
  code that declares a name only later is no scope of a name before it.
- Rust: a declaration is seen where its scope reads it: the binding of a `let` after its statement, of an `if let`
  or a `while let` in the rest of the condition and the block it guards, of a `for` in its body; a binding shadows an
  item.
- Java and C#: the collection of a `for` over a collection is read outside the loop; the variable of a C# `foreach`
  is declared in its statement.
- Starlark: a comprehension is a scope, its variables locals; its first collection and the default of a parameter are
  read outside their scope.
- Rust: the names a string among the tokens of a macro would take as a format string are usages.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/symbol-navigation`: the scopes above.

## Impact

- `crates/code-symbols/src/kotlin/navigate.rs`, `rust/navigate.rs`, `psi_navigate.rs` (`Scoping::sees`),
  `java/navigate.rs`, `csharp/navigate.rs`, `starlark/navigate.rs`.
- Checked on 7 684 files (this repository, Patronus, the test data of psi-syntax) with `examples/facts.rs`: the
  navigation facts moved in Kotlin, Rust and Starlark files only, each to the declaration its language reads.
