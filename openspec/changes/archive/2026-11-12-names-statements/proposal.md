# Proposal

## Why

The review of the measured renames found quiet lines whose name held another value. In each the lines between the
places of the name had changed in the name alone, and the line that gave the name its value stood outside them: the
chain a lambda is given to, before its parameter (`states.mapNotNull { it.registry }.collect { registry -> … }`
with another `mapNotNull`); the head of the function, which gives what the lambda is called on another type; the
second line of an assignment after the last place of its name, read by the lines before it in the next turn of a
loop; a declaration below the function that reads the name. The reader of the renames had no way to ask what code a
name lives in: the navigation gave the places alone.

A second class is of C# alone. `ArgumentNullException.ThrowIfNull(total)` names "total" in its exception and
`Debug.Assert(total > 0)` in its message (`[CallerArgumentExpression]`), an extension method takes the text of
what it is called on the same way, and a lambda whose body is an expression may become an expression tree that
holds the names of its parameters and of the variables around it. Only the declaration of what is called says so,
in another file: such a use was a plain one.

## What Changes

- An entry asked with another name says the lines of the statements of its names (`NameAt::statements`): of a
  local the member around its uses, whole (the outermost statement around a name that declares no type); of any
  other name the statements of its declarations, each with its bodies but the members of a type. Every language
  says what a statement and what a type is, over the tree its navigation reads.
- A name of a value of a C# body is `spelled` where the language may hand the text around it to code that keeps
  it: in an argument, in what a method is called on, in a lambda whose body is an expression, in a query.
- `RULES_VERSION` is 2: what was stored of the answers is made again.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/name-bindings`: the groups Answers and Sites.

## Impact

- `crates/code-symbols/src/navigate.rs` (`NameAt::statements`, `Names::statements_of`, `without_bodies`,
  `RULES_VERSION`), `psi_navigate.rs` (`member_of`, `statement_of`, `Reading::is_statement`, `is_type`,
  `is_member`), the `navigate.rs` of Kotlin, Java, C#, Starlark, Rust and TypeScript/JavaScript
  (`csharp::navigate::is_handed_as_text`).
- `docs/design/diff-view.md` ("A rename is proven by the member around it", "The bindings of many names").
