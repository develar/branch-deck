## MODIFIED Requirements

### Requirement: Tokens: Only blanks stand between two tokens

What stands between two tokens MUST be blank (`is_blank`): whitespace, a byte order mark, or another character a
parser skips as whitespace. A text where anything else stands there MUST have no tokens, so that texts equal by
tokens are the same without their blanks (`same_without_blanks`), what a reader asks before it reads tokens.

#### Scenario: What stands between two tokens
- **WHEN** two texts differ by a byte order mark at their start, by a line separator (U+2028) between two tokens of JavaScript, by a left-to-right mark of Rust or a Ctrl-Z of C#
- **THEN** their tokens are the same, and the texts are the same without their blanks
- **AND** the `\` that continues a line of Starlark is a token, a line joined without it another text
- **AND** a C# text whose token Roslyn cut inside a character or dropped from its tree, and a Java text whose whitespace is a Unicode escape, have no tokens
  [@test] crates/code-symbols/src/tokens_test.rs (`what_stands_between_two_tokens_is_blank`)

### Requirement: Tokens: A token has the lines and the columns of the view

A token MUST name its start and its end by the lines of the view (`\n`, `\r\n`, a lone `\r`) and their UTF-16
columns, and by its bytes, and MUST be in every line it names. Its `shape` and its `text` MUST be hashes of names and
bytes, the same in every run and on every machine, and `tokens_version` MUST change with the rules and with the
parsers.

#### Scenario: A surrogate pair, a lone carriage return, a comment over two lines
- **WHEN** a text has a character of two UTF-16 units, a line that a lone `\r` ends and a comment over two lines
- **THEN** each token has its line and its column of the view at its start and at its end
  [@test] crates/code-symbols/src/tokens_test.rs (`the_lines_and_the_columns_are_those_of_the_view`)

#### Scenario: A token that ends with a line break
- **WHEN** the bytes of a token end with a line break: a line break of a raw string of Kotlin, the text of a branch of `#if` not taken, the `\` that continues a line of Starlark
- **THEN** the token ends at the end of the line of that break, the last line it has a byte in
- **AND** a place inside a `\r\n` is the end of its line
  [@test] crates/code-symbols/src/tokens_test.rs (`the_lines_and_the_columns_are_those_of_the_view`)

#### Scenario: The same hashes in every run
- **WHEN** the tokens of a text are read again, in another run
- **THEN** each has the same `shape` and the same `text`
  [@test] crates/code-symbols/src/tokens_test.rs (`the_keys_are_the_same_in_every_run`)

#### Scenario: The version of the tokens
- **WHEN** the rules of the tokens, the rules of the summaries or the revision of the parsers change
- **THEN** `tokens_version` is another value, and it is neither that of the summaries nor that of the highlighting
  [@test] crates/code-symbols/src/tokens_test.rs (`tokens_version_is_stable`)

#### Scenario: Any file formatted again
- **WHEN** a file of a grammar gets other whitespace where no token is: an indent, spaces at the end of a line, a blank line
- **THEN** its tokens are the same, its content and its text without blanks too, and a name of it renamed is another token
- **AND** each token is in the lines it names, with only blanks before it
  [@test] crates/code-symbols/src/tokens_test.rs (`every_fixture_formatted_again_has_the_same_tokens`)

### Requirement: Equality: Two ranges of lines are the same, the same code or different

`equal_by_tokens` MUST answer for two ranges of lines of two texts: `Same` when their tokens are the same in the same
order, `CodeOnly` when they are once their comments are left out, else `Different`. The tokens of a range MUST be
those with a byte in its lines, and of no lines the token that runs across that place. `compare` MUST answer the
same for two runs of tokens alone.

#### Scenario: A line inside a text block
- **WHEN** a range is a line inside a text block, or the place between two of its lines
- **THEN** its token is the text block, so a blank line the text block gains is `Different`
- **AND** a changed line of a block comment is `CodeOnly`
- **AND** two runs without a token are the same runs, and as two ranges of lines `Same` only by the code around them
  [@test] crates/code-symbols/src/tokens_test.rs (`the_tokens_of_lines_are_those_with_a_byte_in_them`)

## ADDED Requirements

### Requirement: Tokens: A blank that is no whitespace of its language is an error

A blank a parser skips though its language calls it no whitespace MUST be an error of the text at its place, part of
the token of the code after it, of the last one when none follows: U+0085 and a zero width space (U+200B) of
JavaScript, TypeScript and TSX, which oxc skips as the scanner of TypeScript does and an engine of JavaScript,
esbuild and Babel call a syntax error. A text with such a blank and no token of the code MUST have no tokens.

#### Scenario: A blank that is no whitespace of the language
- **WHEN** a text of JavaScript, TypeScript or TSX has a zero width space or U+0085 between two tokens, before its first token or after its last one, and the other text has a space or nothing there
- **THEN** their tokens differ, and so do those of two texts with such a blank at different places
- **AND** two texts with the same such blank between the same two tokens, formatted differently, have the same tokens: broken alike
- **AND** the same character inside a string or the text of a JSX element is content
- **AND** a text with such a blank and no token of the code has no tokens
  [@test] crates/code-symbols/src/tokens_test.rs (`what_stands_between_two_tokens_is_blank`)

#### Scenario: What each parser skips
- **WHEN** each blank stands between two tokens inside brackets, in a text of each grammar
- **THEN** the parser skips it only where its language calls it whitespace: the space, the tab, the form feed and the line ends in Kotlin and Java, without the form feed in Starlark, `Pattern_White_Space` in Rust, the whitespace and the line ends of C#, `WhiteSpace` and `LineTerminator` of ECMAScript in JavaScript, TypeScript and TSX
  [@test] crates/code-symbols/src/tokens_test.rs (`each_parser_skips_between_two_tokens_what_its_language_calls_whitespace`)

### Requirement: Equality: A range of lines is asked with the code around it

The two ranges of `equal_by_tokens` stand between lines that did not change (a block of a diff), and the code around
them MUST read the same too, else the answer is `Different`: the last token of the code before each range closes the
same nodes (`same_before`), and the first one after each is the same token with the same nodes opening before it
(`same_after`); a side without such a token is the same only as a side without one. A comment around a range MUST
NOT be asked: it is outside the tree.

#### Scenario: Lines without a token
- **WHEN** blank lines are added between two statements, before the first token of a file, after its last one, inside the brackets of Starlark, or between a call of Kotlin and the lambda on the line after it
- **THEN** the range of the blank lines and the place they were added at are `Same`
- **AND** a blank line after the `\` that continues a line of Starlark is `Different`: the statement ends there, and the tokens of the lines around it are others
- **AND** a blank line before the `#!` a Kotlin script, a Rust file or a C# file starts with is `Different`: the `#!` is code there
- **AND** a comment that changed before a range, or between it and the code after it, leaves the range `Same`
  [@test] crates/code-symbols/src/tokens_test.rs (`lines_without_a_token_are_the_same_only_where_the_code_around_them_reads_the_same`)

#### Scenario: A part of a changed block
- **WHEN** statements are wrapped into the braces of an `if` and indented, and a reader asks of the lines of the statements alone
- **THEN** their runs of tokens are the same (`compare`)
- **AND** as two ranges between lines that did not change they are `Different`: the code before them is the `{` of the `if`
  [@test] crates/code-symbols/src/tokens_test.rs (`a_statement_wrapped_into_a_block_keeps_its_tokens`)
