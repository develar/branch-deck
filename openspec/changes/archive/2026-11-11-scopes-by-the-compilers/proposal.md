# Proposal

## Why

The second round of the review of the bindings (`commit/name-bindings`) found places where the scopes of the
navigation still differed from the languages', some of them made by the fixes of the first round, so that a change of
meaning read as the rename of one local, or Go to Declaration went to another declaration than the compiler's:

- C#: a `lock` ended the variable its expression declares, though C# gives it to the statements around, as an `if`
  does; a name that is all of the right side of an `is` (`x is Max`) was a type alone, never the constant C# reads
  where no type has the name; the source of a `join` saw the range variables before it.
- Java: the variable of the pattern of an `instanceof` was seen in all of its statement, also where the test did not
  match (the `else`, the operand after `||`, the third operand of `? :`); a resource of a `try` in its `catch` and
  `finally`.
- Kotlin: a parameter of a primary constructor that declares no property was seen by the functions of the class and
  came before a property of its name.
- Rust: an argument of a generic that is one name (`Array<N>`, `Array<CAP>`), which the parser reads as a type, was
  bound to no const parameter and no constant.

The rules are now checked against the compilers: a fixture per language (`crates/code-symbols/testdata/navigate/`)
gives every read of a name to a function of what it reads, typed so that the file compiles only as it is tagged
(javac 26, csc 3.9, kotlinc 2.4.20, rustc, tsc 7.0; the Starlark one runs as Python 3). They showed more of the same
class, fixed here too:

- C#: a variable an expression declares in the initializer of a field or a property, or in a body after `=>` of a
  property, was seen by the whole type; one of a clause of a query by the clauses after it; one of an incrementor of
  a `for` by its body; one of the source of the first `from` or of a `join` not by the statements after the query.
- Kotlin: the members of a companion were not seen in its class; a class that is not `inner` and an object in the
  body of a class saw the properties of the instances around them.
- Java: whether a statement can complete normally was read from its last statement alone, so a `synchronized`, a
  `try` with a `finally`, a labeled block, a `switch` and a loop of the condition `true` did not introduce a pattern
  variable to the statements after an `if` that leaves through them; a `for` introduced none.

## What Changes

- C#: `opens_scope` without `lock`, with the initializers of members and the lambdas of a query; `declares_in` for the
  sources of a query; `Scoping::sees` for the incrementors of a `for`; `Scoping::else_reads` for the right side of an
  `is`, a `Site::Namesake` where it binds to a local; `query_sees` for the source of a `join`.
- Java: `java/pattern_scope.rs` holds where a pattern variable is seen (`PatternScope`), with what the highlighting
  and the frames of the navigation share of it (`is_pattern_scope`, `completes_normally`, `jumps_to`); `Scoping::sees`
  asks it, and scopes a resource to its resources and its block.
- Kotlin: `Names::is_seen` for the parameters of a primary constructor, a member gives way to one only where it is
  seen; the members of a companion are filed under its class; `is_nested`, `is_of_instance`.
- Rust: `Space::Type` holds a constant and a const parameter.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/symbol-navigation`: the scopes above.

## Impact

- `crates/code-symbols/src/csharp/navigate.rs`, `java/navigate.rs`, `java/pattern_scope.rs` (new),
  `java/highlight.rs` (the helpers of the pattern scopes moved out), `kotlin/navigate.rs`, `rust/navigate.rs`,
  `psi_navigate.rs` (`Scoping::else_reads`).
- `crates/code-symbols/testdata/navigate/` and `test_support::readings_unlike_the_compiler`.
- The highlighting of Java reads the same `is_pattern_scope`, now with the fuller reading of what can complete
  normally; its tests are unchanged.
