# Proposal

## Why

On 2026-11-23 the Safe Push of `open-url-gate` (IJAI-7014) failed with E0061. A commit of another branch gave the
method `Apps::connect_page` of `air-apps` a second parameter. A commit of `open-url-gate` called it with two arguments
on `runner.world.apps`, a field of a local. git merged cleanly, and `brd check` said `ok`. The Rust summary had no
signature, so no Rust call was judged by its arguments. And a Rust member on a value resolved nowhere, as the summary
knows no type of a value.

## What Changes

- psi-syntax's `code-symbols`: a Rust function has its parameters as its signature, without the `self` of a method. A
  Rust call of a path or a method has the shape of its arguments; a method call has `method`, as its receiver fills
  `self`.
- A call of a Rust function is judged by the count of its arguments: a method call by the parameters after `self`, a
  path call of a method with the receiver as its first argument.
- A new count of parameters or a new vararg of a Rust function is a new step of its key; of a member of an `impl` only
  in the same reach.
- A Rust method call on a value of a method of its own crate that it sees is an edge, `method_arity`, only to a step
  whose form before rejected the call and whose form accepts it. A field on a value, a method the range adds, and a
  call every form accepts stay no edge.

Capabilities: `commit/symbol-dependencies`, `cli/agent-cli`.
