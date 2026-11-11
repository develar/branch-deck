# Proposal

## Why

A reader that trusts the bindings of a file (the renames of a diff, `commit/name-bindings`) is told "one local" for a
change of meaning wherever the scopes of the navigation differ from the language's. The review of the lane found such
places in Starlark, TypeScript, JavaScript and Kotlin, each checked against the language itself (Bazel, node,
kotlinc):

- Starlark: every assignment of a name in a `def` was a declaration of its own, and a closure that reads the name
  before its assignment went to the file. Starlark has one variable of a name in a `def`.
- JavaScript and TypeScript: a second `var` of a name, and a `var` of the name of a parameter, were other variables;
  the default of a parameter was bound to a `let` of the body.
- Kotlin: a property of an object or a local class won over a local of the function around it, where Kotlin finds
  the local first; the condition of a `do … while` did not see the variables of its body.
- The name of a named argument of Kotlin and Starlark, and a type of the spelling of a variable in Kotlin and
  TypeScript, were bound to the variable.

Each also sent Go to Declaration to another declaration and lost or added usages of a local.

## What Changes

- Starlark: a name a `def` or a comprehension binds is one variable in all of it; a later assignment writes it.
- TypeScript and JavaScript: a function has one variable of a name (a parameter, a `var`, a function it declares);
  the signature of a function is read before its body; a type and the namespace of one are of no variable; the
  members of an enum are seen in its body.
- Kotlin: a local is found before a member of a class inside its body; the condition of a `do … while` sees the
  block of its body; a name where a type is written is of no value.
- Kotlin, Starlark: the name of a named argument is a member no scope binds, as in C#.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/symbol-navigation`: the scopes above.

## Impact

- `crates/code-symbols/src/starlark/navigate.rs`, `js/navigate.rs`, `kotlin/navigate.rs`, `psi_navigate.rs`
  (`Reading::rebinds`, `Scoping::sees` per declaration).
- The navigation facts of `examples/facts.rs` on the 7 684 files of the lane's corpus: 2 076 of 147 516 lines moved in
  915 files, read by class: the names of Kotlin arguments (most of them), the fields of Rust structs before their
  colon, types and path segments of the spelling of a binding, the assignments of a `def`.
