# Tasks

- [x] 1.1 The tokens of a psi-syntax tree (Kotlin, Java, C#, Starlark), of rust-analyzer's tree and of oxc's tokens and
  tree, each with its shape, its text, its class and its place in the view.
- [x] 1.2 `equal_by_tokens`, `compare` and `texts_equal_by_tokens`; the tokens of a range of lines; no tokens for a text
  whose tree does not hold every token.
- [x] 1.3 The tables of `tokens_test.rs`, the negative cases first, and the checks of every fixture formatted again.
- [x] 1.4 The example `tokens` run over this repository (2 474 files), 7 388 Kotlin and Java files of a monorepo, the
  test data of its Java and Kotlin plugins (61 447 files, broken code among them), psi-syntax's C#, Java and Starlark
  test data (8 124 files) and 4 460 files of JavaScript and TypeScript packages: no failure.
- [x] 1.5 After the review: the errors of every parser in the tokens; the blanks between two tokens checked, `is_blank`
  and `same_without_blanks`; no tokens for a C# text Roslyn gave up nesting in or cut a character of; the end of a
  token that ends with a line break; the rows of a Kotlin script and its fixture; `indentation_matters` for every file
  of Starlark; `tokens_version` pinned.
