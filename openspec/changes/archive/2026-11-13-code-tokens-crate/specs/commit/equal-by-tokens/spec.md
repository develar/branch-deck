## MODIFIED Requirements

### Requirement: Tokens: A token has the lines and the columns of the view

A token MUST name its start and its end by the lines of the view (`\n`, `\r\n`, a lone `\r`) and their UTF-16
columns, and by its bytes, and MUST be in every line it names. Its `shape` and its `text` MUST be hashes of names and
bytes, the same in every run and on every machine, and `tokens_version` MUST change with the rules of the tokens,
the grammars of the build and the revision of the parsers its consumer names, and with nothing else.

#### Scenario: A surrogate pair, a lone carriage return, a comment over two lines
- **WHEN** a text has a character of two UTF-16 units, a line that a lone `\r` ends and a comment over two lines
- **THEN** each token has its line and its column of the view at its start and at its end
  [@test] crates/code-tokens/src/tokens_test.rs (`the_lines_and_the_columns_are_those_of_the_view`)

#### Scenario: A token that ends with a line break
- **WHEN** the bytes of a token end with a line break: a line break of a raw string of Kotlin, the text of a branch of `#if` not taken, the `\` that continues a line of Starlark
- **THEN** the token ends at the end of the line of that break, the last line it has a byte in
- **AND** a place inside a `\r\n` is the end of its line
  [@test] crates/code-tokens/src/tokens_test.rs (`the_lines_and_the_columns_are_those_of_the_view`)

#### Scenario: The same hashes in every run
- **WHEN** the tokens of a text are read again, in another run
- **THEN** each has the same `shape` and the same `text`
  [@test] crates/code-tokens/src/tokens_test.rs (`the_keys_are_the_same_in_every_run`)

#### Scenario: The version of the tokens
- **WHEN** the rules of the tokens, the grammars of the build or the revision of the parsers change
- **THEN** `tokens_version` is another value, whatever the rules of the summaries are
  [@test] crates/code-tokens/src/tokens_test.rs (`tokens_version_is_stable`)

#### Scenario: Any file formatted again
- **WHEN** a file of a grammar gets other whitespace where no token is: an indent, spaces at the end of a line, a blank line
- **THEN** its tokens are the same, its content and its text without blanks too, and a name of it renamed is another token
- **AND** each token is in the lines it names, with only blanks before it
  [@test] crates/code-symbols/src/tokens_fixture_test.rs (`every_fixture_formatted_again_has_the_same_tokens`)
