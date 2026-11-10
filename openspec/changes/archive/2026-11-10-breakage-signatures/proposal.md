# Proposal

## Why

The review of `develar/virtual/jna-one-copy` (idea-1, d4913cec04b2a) showed the constructor of `PluginClassLoader` as
`PluginClassLoader(classPath: ClassPath, parents, pluginDescriptor: PluginDescriptor, …, libDirectories)`: every
generic type dropped, as the summary keeps a type only as its last names (`List/Path`), and nothing said which
parameter the change added. The line of its place wrapped and left a `·` at its end, and "No use left outside found,
the search stopped early" read unclear.

## What Changes

- The summary of a Kotlin and a Java parameter keeps its type as the source writes it (`List<Path>`, `String?`,
  `suspend (Int) -> Unit`, `String...`, `int[][]`), without annotations, for a review to show; the forms still compare
  by the last names, so a type written in other words is no change. The rules of the summaries change: every summary
  is made again once.
- A form of the API is a structure, a line per constructor, each parameter apart; of a change each parameter is marked
  `added`, `removed` or `typeChanged` (Kotlin by name, Java by place), an overload without a pair marked whole.
- The card highlights what the change did: an added parameter tinted, a removed one struck through, a changed type in
  the warning tone, the words for a screen reader; Java parameters read as Java writes them (`String... names`); a
  form wraps between its parameters, never between a name and its type (`pluginDescriptor:` / `PluginDescriptor`).
- The parts of the line under an entry are apart by `·`, none of which ends or begins a line; "No use found outside
  the change; the search stopped early".
- `brd review-plan` keeps `before` and `after` as text, now with the types as written (`"schema": 31`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: the forms show the types as written and mark what a change did to each parameter; the line
  under an entry and the words of an incomplete count of none.

## Impact

- `crates/code-symbols` (`Parameter::shown_type`, `psi_tree::written_type`, the Kotlin and Java summaries,
  `RULES_VERSION` 29), `crates/trial-check/src/declaration_checks.rs` (`ParameterForm`, `ConstructorForm`,
  `ShownType`), `crates/review-plan/src/shown.rs`, `api.rs`, `crates/bd-rewrite/src/review_plan_command.rs`.
- `src/presenters/review-plan.ts`, `src/components/review/ReviewPrologue.tsx`, `src/lib/bindings.ts`.
