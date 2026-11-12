# Proposal

## Why

The review of the measured marks read the quiet runs of a renamed name again and found lines whose name held
another value, of three classes.

- **The value of a local came from outside the lines between its places.** `users.filter { … }.forEach { user ->
  send(user) }` became `admins…forEach { admin -> send(admin) }`; `.mapNotNull { it.registry }` before `.collect {
  registry -> … }` became `.mapNotNull { it.catalogOrNull }`; `fun Collection<String>.seeds()` became `fun
  Collection<SessionId>.seeds()` above a renamed parameter of a lambda; `last = describe(item,` kept its line while
  `mode);` below it changed, in a loop whose earlier lines read `last`; a blank line was put into the raw string a
  renamed `val` is given. Each was passed.
- **A name that is not local got another value with its other name.** `const val TIMEOUT = 5` became `const val
  LIMIT = 10`, the index confirmed both uses, and `pause(LIMIT)` was passed. Nine of the 149 runs the index had
  confirmed on real commits were such lines.
- **C# hands the text of an argument to what is called** (`[CallerArgumentExpression]`), so a renamed local given
  to `ArgumentNullException.ThrowIfNull` changes the text of an exception.

## What Changes

- A rename of a local is proven only where nothing else of the member around it changed: the function it lives in,
  whole, may differ in formatting, in comments, in other locals proven the same way and in the name the member
  itself is known by. A narrower rule (the lines between the places, then the statements that hold a place) left a
  value given elsewhere in the function each time; what a function reads from outside it is to a renamed line what
  it is to a line the change left alone.
- A line without a token of code is no change of a name: a blank line and a comment. A blank line inside a string
  of several lines is a line of that string.
- The pair says of a name that is not local whether the code that declares it changed in more than names
  (`redeclared`). Such a name is another thing: the index is not asked of it, and its proof is `otherDeclaration`
  wherever it is named, with an index or without one.
- A name of a value of a C# body that stands where the text around it may be handed on is a place that makes more
  of its name: in no run, read ([Name Bindings](../../specs/commit/name-bindings/spec.md)).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the groups Renames, Subject and Views.

## Impact

- `crates/file-diffs/src/marks_renames.rs` (`Renames::prove`, `with_declarations`, `read_declarations`,
  `Found::code`, `Changed::is_quiet`), `marks.rs` (`PairMarks::redeclared`), `leftovers.rs`
  (`Declarations::redeclared`), `marks_subject.rs` (no question of a redeclared name), `service.rs`
  (`MODEL_VERSION` 17).
- `crates/text-diff/src/model.rs` (`RenameProof::OtherDeclaration`), `src/lib/bindings.ts`,
  `src/presenters/diff-marks.ts` (a comment).
- `tests/e2e/commit-files/quiet-changes.spec.ts`: the stops of the renamed local are looked for as the text a row
  shows.
- `docs/design/diff-view.md` ("A rename is proven by the member around it", "Measured on real history").
