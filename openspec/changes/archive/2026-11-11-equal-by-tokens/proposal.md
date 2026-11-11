# Proposal

## Why

"Only the formatting changed" was told by characters: the same text without its whitespace. That calls a space inside
a string, `a - -b` turned into `a--b`, `return` joined with the call of the next line (Kotlin, JavaScript) and an
indent of Starlark whitespace, each of which is another program. A review that folds such a change, or a diff that
shows it quietly, hides real work: the one defect a fold must not have. The diffs are about to mark a block of
formatting and to let the reader pass it (the plan of the diff marks), which needs a rule that proves its answer.

## What Changes

- `code_symbols::tokens`: the tokens of a text of every grammar of the highlighting, each a leaf of the tree its
  parser made with the nodes that open before it and close after it (`Token::shape`) and its text as it compares
  (`Token::text`); comments are tokens outside the tree. `equal_by_tokens` answers for two ranges of lines: `Same`,
  `CodeOnly` (the comments differ), `Different`.
- Every error the parser reported is part of the tokens, by its message, at its place: oxc and Roslyn keep their
  errors beside the tree, and a line break before the `=>` of an arrow function, after a `throw`, or a space lost
  before the file name of a `#line` is an error that changes no node. Two texts with the same tokens are the same tree
  with the same errors.
- Only blanks stand between two tokens (`is_blank`: whitespace, a byte order mark, the few characters a parser skips
  as whitespace), checked for every text, so texts equal by tokens are the same without their blanks
  (`same_without_blanks`): the cheap question before the tokens never disagrees with them. The `\` that continues a
  line of Starlark is a token.
- No rule names a grammar's line breaks or brackets: a line break that ends a statement changes the nodes around it,
  one inside the brackets of a call changes none. The text of a JSX element compares as JSX reads it; in Starlark the
  indent of a logical line is part of its first token; a directive of C# and the text of a branch of `#if` not taken
  are code; the marks of the input of a Rust macro compare by what follows them.
- A text over 1 MiB, nested deeper than its parser reads, a script read only after a recovery, or one whose tokens
  are not those of the text (a C# token cut inside a character) has no tokens and is never the same.
- A token is in every line it names: one whose bytes end with a line break ends on the line of that break.
- `indentation_matters` moves from `review-plan` to `code_symbols::tokens`, its one owner, and names every file the
  grammar of Starlark reads.
- `cargo run --release -p code-symbols --example tokens -- <paths>` checks the rule on real files: the same tokens once
  a file is formatted again, another for a renamed name.

## Capabilities

### New Capabilities

- `commit/equal-by-tokens`: the tokens of a file and the one rule of "the same code".

### Modified Capabilities

None.

## Impact

- `crates/code-symbols/src/tokens/` (`mod.rs`, `build.rs`, `psi.rs`), `tokens.rs` of `kotlin`, `java`, `csharp`,
  `starlark`, `rust` and `js`, `examples/tokens.rs`, `examples/shared/token_checks.rs`.
- `docs/design/change-review.md` ("Equal by tokens", decision 25), `docs/contributing/code-map.md`.
