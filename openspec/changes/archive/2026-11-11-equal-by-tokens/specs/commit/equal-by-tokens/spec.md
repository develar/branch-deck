## ADDED Requirements

### Requirement: Tokens: A token is a leaf of the tree its parser made

The tokens of a file of a grammar (Kotlin, a Kotlin script, Java, C#, Starlark, Rust, TypeScript, TSX, JavaScript)
MUST be the leaves of the tree its parser made, in the order of the text, never the roles of the highlighting nor the
tokens of a lexer where the parser decides what a token is. Whitespace MUST be no token.

#### Scenario: Operators that join or split
- **WHEN** two texts differ as `a - -b` and `a--b`, as `> >` and `>>` of a shift, as `?:` and `? :` of Kotlin, as `=>` and `= >`
- **THEN** their tokens differ
- **AND** the two `>` that close two lists of type arguments are the same tokens with a space between them and without
  [@test] crates/code-symbols/src/tokens_test.rs (`operators_that_join_or_split_are_other_tokens`)

#### Scenario: A script of Kotlin
- **WHEN** two texts of a Kotlin script differ by a line break between two of its statements, by a space inside a string, or by a call wrapped inside its brackets
- **THEN** the first two have other tokens and the wrapped call the same, as in a file of Kotlin
  [@test] crates/code-symbols/src/tokens_test.rs (`a_line_break_that_ends_a_statement_is_no_formatting`, `a_call_wrapped_inside_its_brackets_and_code_indented_again_are_formatting`, `the_whitespace_inside_a_literal_is_content`)

#### Scenario: The class of a token
- **WHEN** the tokens of a text are read
- **THEN** each is a comment, a name, a keyword, a literal or another token (`TokenClass`), a word the parser read as a name a name
  [@test] crates/code-symbols/src/tokens_test.rs (`a_token_has_its_class_and_a_renamed_name_keeps_its_shape`)

### Requirement: Tokens: A token carries the nodes that open before it and close after it

A token MUST carry its place in the tree, its `shape`: the grammar's kind of it, the nodes that open before it and
those that close after it, by their names, so that two texts with the same tokens are the same tree. No rule of line
breaks or of brackets MUST stand beside it: what a parser reads from whitespace is in the shapes.

#### Scenario: A line break that ends a statement
- **WHEN** two texts differ as `return⏎foo()` and `return foo()` of Kotlin or JavaScript, as `val x = a⏎+ b` and `val x = a + b`, as a label with and without a space before its `@`
- **THEN** their tokens differ
- **AND** a call wrapped inside its brackets, a chain continued on the next line and an operator at the end of a line are the same tokens
  [@test] crates/code-symbols/src/tokens_test.rs (`a_line_break_that_ends_a_statement_is_no_formatting`, `a_call_wrapped_inside_its_brackets_and_code_indented_again_are_formatting`)

#### Scenario: Statements wrapped into a block
- **WHEN** statements are wrapped into the braces of an `if` and indented
- **THEN** each has the tokens it had: a statement carries its own nodes whole
- **AND** a statement that becomes the body of an `if` without braces has not: the `if` closes after it
  [@test] crates/code-symbols/src/tokens_test.rs (`a_statement_wrapped_into_a_block_keeps_its_tokens`)

#### Scenario: A name renamed
- **WHEN** a name is renamed at its declaration and its uses
- **THEN** each token of the two texts has the shape it had, and only the names have another text
  [@test] crates/code-symbols/src/tokens_test.rs (`a_token_has_its_class_and_a_renamed_name_keeps_its_shape`)

#### Scenario: A text with a syntax error
- **WHEN** a text does not parse
- **THEN** its tokens are those of the tree the parser left, its error nodes and its errors in their shapes: two such texts with the same tokens are broken alike
  [@test] crates/code-symbols/src/tokens_test.rs (`a_text_with_a_syntax_error_compares_by_the_tree_the_parser_left`)

### Requirement: Tokens: A literal compares by its bytes

A string, a character, a text block, a raw string, the text of a template and of an interpolated string and a regular
expression MUST compare by their bytes: the whitespace inside them is content. The code of a hole of a template MUST
be code.

#### Scenario: A space and a blank line inside a literal
- **WHEN** two texts differ by a space inside a string, by a blank line inside a raw string of Kotlin, a text block of Java or a template of JavaScript, by a space inside a regular expression
- **THEN** their tokens differ
- **AND** the spaces inside `${ … }` of a template change no token
  [@test] crates/code-symbols/src/tokens_test.rs (`the_whitespace_inside_a_literal_is_content`)

### Requirement: Tokens: The text of a JSX element compares as JSX reads it

The text of a JSX element MUST compare as JSX reads it: every line but the first without the spaces and tabs it starts
with, every line but the last without those it ends with, the lines left empty dropped, the rest joined by one space.
A text that reads as nothing MUST be no token, and where Babel and TypeScript read a text differently its lines MUST
stay apart.

#### Scenario: The text of an element on lines of its own
- **WHEN** the text of an element moves to lines of its own, is indented again, or its elements each to a line
- **THEN** the tokens are the same
- **AND** two spaces inside a line in place of one, a space lost between a text and an element, and a line of a text joined to the next without a space are other tokens
  [@test] crates/code-symbols/src/tokens_test.rs (`the_text_of_a_jsx_element_compares_as_jsx_reads_it`)

### Requirement: Tokens: An indent is part of its line where the indentation is the syntax

In Starlark the indent of each logical line MUST be part of its first token, byte for byte. A line continued inside
brackets or after a `\`, a comment and a blank line MUST have no indent that counts. `indentation_matters` MUST name,
by its path, a file whose indentation has a meaning, for the reader of a file of no grammar, and every file the
grammar of Starlark reads among them.

#### Scenario: A statement indented again
- **WHEN** a statement of Starlark leaves its block, or its block is indented by another width or by a tab
- **THEN** its tokens differ
- **AND** an argument on a line of its own inside the brackets of a call, a comment and a blank line indented again change no token
- **AND** `WORKSPACE.bzlmod` and `third_party/zlib.BUILD` are files whose indentation has a meaning, as `BUILD` and `defs.bzl` are
  [@test] crates/code-symbols/src/tokens_test.rs (`where_the_indentation_is_the_syntax_an_indent_is_no_formatting`)

#### Scenario: A brace language indented again
- **WHEN** the code of Kotlin, Java, C#, Rust or TypeScript is indented again, gets blank lines or other line ends
- **THEN** its tokens are the same
  [@test] crates/code-symbols/src/tokens_test.rs (`a_call_wrapped_inside_its_brackets_and_code_indented_again_are_formatting`)

### Requirement: Tokens: What a parser calls trivia and another build compiles is code

A directive of C# MUST be its tokens, and the text of a branch of `#if` the configuration does not take, the message
of a directive and a conflict marker MUST each be one token that compares by its bytes. Inside the input of a macro of
Rust a punctuation mark with another right after it MUST be another token than one before a space.

#### Scenario: A directive and a branch not taken
- **WHEN** two texts of C# differ in the symbol of an `#if`, in a `#pragma`, or by a space in the text of a branch of `#if` the configuration does not take
- **THEN** their tokens differ
- **AND** the spaces between `#if` and its symbol change no token
  [@test] crates/code-symbols/src/tokens_test.rs (`what_a_parser_calls_trivia_and_another_build_compiles_is_code`)

#### Scenario: The input of a macro
- **WHEN** the input of a macro of Rust differs as `a => b` and `a = > b`
- **THEN** its tokens differ
- **AND** a space after a comma, around an operator, and an input wrapped over lines change no token
  [@test] crates/code-symbols/src/tokens_test.rs (`what_a_parser_calls_trivia_and_another_build_compiles_is_code`)

### Requirement: Tokens: A token has the lines and the columns of the view

A token MUST name its start and its end by the lines of the view (`\n`, `\r\n`, a lone `\r`) and their UTF-16
columns, and by its bytes, and MUST be in every line it names; `newline_lines` MUST give the lines of the view for
lines as git counts them. Its `shape` and its `text` MUST be hashes of names and bytes, the same in every run and on
every machine, and `tokens_version` MUST change with the rules and with the parsers.

#### Scenario: A surrogate pair, a lone carriage return, a comment over two lines
- **WHEN** a text has a character of two UTF-16 units, a line that a lone `\r` ends and a comment over two lines
- **THEN** each token has its line and its column of the view at its start and at its end
- **AND** the lines git counts are those of the view but for the line the lone `\r` ends
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

### Requirement: Comments: A comment is no code and compares by its text

A comment, a doc comment whole, MUST be one token outside the tree. It MUST compare by its text without the
whitespace that ends its lines and, at the start of its other lines, without the indent of the line it starts on.

#### Scenario: The text of a comment
- **WHEN** two texts differ as `//go:build` and `// go:build`, in the words of a comment, or by a comment that one of them has or has at another place
- **THEN** their code is the same and their comments are not
  [@test] crates/code-symbols/src/tokens_test.rs (`a_comment_compares_by_its_text_and_is_no_code`)

#### Scenario: A block comment that moved with its code
- **WHEN** a block comment is indented again with the code around it, each of its lines by what its first line moved
- **THEN** it is the same comment
- **AND** one whose other lines alone moved is another comment
  [@test] crates/code-symbols/src/tokens_test.rs (`a_comment_compares_by_its_text_and_is_no_code`)

### Requirement: Equality: Two ranges of lines are the same, the same code or different

`equal_by_tokens` MUST answer for two ranges of lines of two texts: `Same` when their tokens are the same in the same
order, `CodeOnly` when they are once their comments are left out, else `Different`. The tokens of a range MUST be
those with a byte in its lines, and of no lines the token that runs across that place; two ranges without tokens MUST
be `Same`.

#### Scenario: A line inside a text block
- **WHEN** a range is a line inside a text block, or the place between two of its lines
- **THEN** its token is the text block, so a blank line the text block gains is `Different`
- **AND** a changed line of a block comment is `CodeOnly`, and lines of whitespace alone are `Same`
  [@test] crates/code-symbols/src/tokens_test.rs (`the_tokens_of_lines_are_those_with_a_byte_in_them`)

### Requirement: Equality: A text without tokens is never the same

A text MUST have no tokens when it is over 1 MiB, when it nests deeper than its parser reads (Kotlin, Java, C#,
Starlark; Rust, TypeScript and JavaScript are read as deep as they nest), when it is a script the parser read only
after a recovery, and when its tokens are not those of the text: a tree that dropped a token, tokens out of the
order of the text, not on the boundaries of characters, or with more than blanks between two of them. A comparison
with such a text MUST be `Different`.

#### Scenario: A text over the limit, a text nested too deeply
- **WHEN** a text is over 1 MiB, or a text of Kotlin, Java, C# or Starlark nests its brackets deeper than its parser reads
- **THEN** it has no tokens, and it is `Different` from any text, itself too
- **AND** a text of Rust, TypeScript or JavaScript nested as deeply has its tokens
  [@test] crates/code-symbols/src/tokens_test.rs (`a_text_over_the_limit_or_nested_too_deeply_has_no_tokens`)

#### Scenario: A script read only after a recovery
- **WHEN** oxc reads a script only with the region of an error blanked
- **THEN** the script has no tokens: the blanked region is in none
  [@test] crates/code-symbols/src/tokens_test.rs (`a_text_with_a_syntax_error_compares_by_the_tree_the_parser_left`)
