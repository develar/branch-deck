# Proposal

## Why

"Equal by tokens" proved the lines of a hunk and the plan read the proof as one of the change. Three ways it was
none, each a fold of real work as `whitespace`:

- Lines without a token have no tokens to differ, so a blank line was always the same, while it can change the tokens
  of the lines around it, which no hunk holds: after the `\` that continues a line of Starlark it ends the statement
  (a syntax error), before the `#!` of a Kotlin script, a Rust or a C# file it makes code of the line.
- A hunk of the plan is a block of the diffs, whose lines end at a lone `\r` too, and was read as lines of git. Below
  a lone `\r`, or in a file whose lines end with `\r\r\n`, the plan compared the text and the tokens of the line after
  the change: a blank line, the same on both sides.
- oxc skips U+0085 and a zero width space (U+200B) between two tokens as the scanner of TypeScript does and reports
  nothing. ECMAScript calls neither whitespace: V8, esbuild and Babel refuse the file, a TypeScript one too. A text
  with one had the tokens of the text without it.

## What Changes

- `equal_by_tokens` asks the code around its two ranges: the last token of the code before each closes the same nodes
  (`same_before`) and the first one after each is the same token with the same nodes opening before it
  (`same_after`), else `Different`. Lines without a token are the same only then. `compare` answers for two runs of
  tokens alone, what a reader asks of a part of a changed block (a body wrapped into `if {}`).
- A blank a parser skips though its language calls it no whitespace is an error of the text at its place, in the
  tokens: U+0085 and U+200B of JavaScript, TypeScript and TSX. What each parser skips between two tokens is pinned
  against the rule of its language.
- The plan reads a hunk and a block by the lines of the diffs, their text and their tokens alike, and the lines of a
  declaration and of an import statement, which a summary counts as git does, as the same lines: a block below a
  lone `\r` goes to the declaration of its lines.
- A paired declaration is whitespace only with every line its blocks change: a block may reach past the declaration
  that holds most of it, into a comment that changed.
- `Tokens::newline_lines` is gone: the tokens know the lines of the view alone, the plan translates the lines of git
  from the text, which a side without tokens needs too.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/equal-by-tokens`: a range with the code around it; a blank that is no whitespace of its language; no
  lines of git.
- `branch/review-plan`: whitespace of a hunk with the code around it, by the lines of the diffs.
- `branch/review-units`: the declarations by the lines of the diffs; a pair with every line of its blocks.

## Impact

- `crates/code-symbols/src/tokens/mod.rs`, `tokens/build.rs`, `js/tokens.rs`.
- `crates/review-plan/src/whitespace.rs` (`TextLines`), `files.rs`, `units.rs`.
- `docs/design/change-review.md` ("Equal by tokens", decision 25).
