# Tasks

- [x] 1.1 `equal_by_tokens` with `same_before` and `same_after`; `Tokens::places_in_lines`; the table of lines without
  a token (Starlark's `\`, a `#!` of Kotlin, Rust and C#, blank lines that change nothing).
- [x] 1.2 `Builder::irregular`: U+0085 and U+200B of the dialects of oxc are errors in the tokens; the blanks each
  parser skips pinned.
- [x] 1.3 `whitespace::TextLines`: the lines of the diffs for the hunks, the blocks and the text of a fingerprint; the
  declarations and the imports of a summary translated to them; `Tokens::newline_lines` removed.
- [x] 1.4 A paired declaration is whitespace with every line of its blocks.
- [x] 1.5 Tests of the plan: a blank line after the `\` of Starlark and before a `#!`, a zero width space, a change
  below a lone `\r` and in a file of `\r\r\n`, a block below a lone `\r`, a block that reaches past its declaration.
