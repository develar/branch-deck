# Proposal

## Why

develar/branch-deck#251: the Safe Push of `a2ui-v1` failed with `error[E0603]: constant TOO_BIG is private`. A commit of
`native-webview`, stacked on `a2ui-v1`, had only made an existing `const TOO_BIG` `pub`; a later commit of `a2ui-v1`
used it from the module of a test. `brd commits` gave no edge and `brd check` said `ok`: a key of Rust (and of
TypeScript) was only present or absent, so a commit that changes the reach of an existing item changed nothing the
uses were resolved by. Of the JVM only the members of a type had steps of their reach.

## What Changes

- A Rust item of a module, a field, and a TypeScript declaration whose reach a commit changes is a step of its key:
  a later use that the old reach forbids needs the commit, as the use of a JVM member whose reach a commit widens
  does. A member of an `impl` is no step (a trait the range doesn't show may give the type its name), nor a key with
  two declarations on one side (`#[cfg]`), nor a JVM top-level declaration (a file the range doesn't show may declare
  its name in the package).
- The old reach is judged by psi-syntax's one rule of Rust's visibility (`rust_syntax::visibility`): `pub(super)`,
  `pub(in super::super)` and `pub(in crate::a)` reach the ancestor they name. An older form is never stricter than the
  compiler: a bare name a glob would bring may come from a second glob or the preludes.
- A narrower reach is a removal for the uses it forbids: the commit needs the older commits that removed them, Rust,
  TypeScript and JVM members alike.
- `brd check`: a missing declaration names the file whose change it needs (`change_file`), and a parent that needs a
  commit of its child for one is an inverted stack with what it needs (`needs`) and the fixes a trial proves: the use
  into the child, the parent on the child, the child's commit into the parent when it changes only that file, else
  the change of the file into the commit of the use (`move_hunk`, the command of `brd move-hunk`). A trial of a
  rewritten history reads the summaries of the blobs its rewrite wrote.

Capabilities: `commit/symbol-dependencies`, `cli/agent-cli`.
