# Proposal

## Why

In the file of a declaration, a name on an expression receiver was a use of the declaration by the same-file rule. The
test of `genui-flakes` (2026-11-22) showed a false edge: `JBPanelWithEmptyText().emptyText.text = "x"` linked to a
`fun text(value: String)` of the class around it, evidence `same_file`. The receiver is a type of the platform, which
only a compiler knows. Across files such a use needs a receiver of a known type (`receiver`); the same file took any.

## What Changes

- `on_unknown_expression` of `symbol_dependencies.rs`: in the file of a JVM declaration that is no extension, a name
  on a receiver of a type the summary doesn't know is no use of it. This is an expression or a local (an empty
  qualifier), or a name that the file neither declares nor imports.
- A bare name, `this` and `super`, a receiver of a known type (`of_another_receiver`), and a qualifier that names the
  package, a declaration of the file, an imported value or a container stay uses.
- Decision 3 of docs/design/symbol-dependencies.md.

Capabilities: `commit/symbol-dependencies`.
