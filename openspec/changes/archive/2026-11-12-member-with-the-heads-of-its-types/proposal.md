# Proposal

## Why

A second review of the proof of a renamed local ("a rename is proven by its member whole") found the class of the
first round again where a function has no statement around it. The member of a local was the outermost statement
around the name that declares no type; for a function that is a value in the head of a type (a lambda given to a
constant of a Java enum, to the constructor of a Kotlin supertype, to a decorator of a TypeScript class) and for
the value of an `export default`, that statement is one statement inside the body of the function. The line beside
it could give the local another value (`load(request)` that became `load(backup)` above `for (const row of rows)`),
and the lines that changed in the name alone were quiet.

The head of a type was no part of a member either. A parameter of Kotlin's primary constructor is code the value of
a property and an `init` run with: `class Report(rows: List<Row>)` that became `List<Other>` left `rows.map { entry
-> entry.name }` quiet, though `entry.name` reads another property. The same holds of a C# primary constructor, of
the components of a Java record in its compact constructor and of the bounds of the type parameters of a class or
an `impl`: the case of the type in the head of a function, one level up.

The test of a copy over a quiet run selected its lines again until the copy took them: an answer that came after
the rows were shown (the scopes of a file, what the index proved) painted every row again and dropped the selection
of the reader, in files the answer said nothing of too. Two causes: a new model cleared the rows without putting
the selection on the new ones, and the ref of the rows of a section was another function with every height of the
rows, so React detached and attached the host, which dropped its rows once more.

## What Changes

- The statements of a local are the heads of the types around its member, each up to its members, and the member.
  A member stands in the file, in a type or in the body of one; a local whose outermost statement that declares no
  type stands elsewhere has no member and says the whole text. `export default` of a value is a statement. The body
  of a constant of a Java enum holds members, so the statement of the constant is without them.
- A renamed local is proven only where the heads of the types around its member changed in names alone too, and a
  local without a member only where its file did.
- A view keeps the selection through a new model: each end is found again by the line of its row and the text the
  row shows; the rows of a section stay attached across its renders.
- `navigate::RULES_VERSION` is 2 (the archived change `names-statements` said so and the constant stayed 1).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/name-bindings`: the group Answers.
- `commit/quiet-changes`: the groups Renames and Views.

## Impact

- `crates/code-symbols/src/psi_navigate.rs` (`member_of`, `head_of`), `kotlin/navigate.rs`, `java/navigate.rs`
  (`is_member`), `rust/navigate.rs` (`member_of`), `js/navigate.rs` (`member_of`, `holds_members`, `is_statement`),
  `navigate.rs` (`RULES_VERSION`).
- `src/components/diff/diff-painter.ts` (`heldSelection`, `restoreHeld`), `src/components/diff/DiffView.tsx` (the
  ref of the rows), `src/lib/diff/rows.ts` (`placeOfRow`, `rowOfRowPlace`).
- `docs/design/diff-view.md` ("A rename is proven by the member around it", "Measured on real history", the copy
  and the selection).
