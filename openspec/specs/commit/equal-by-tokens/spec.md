---
name: Equal by Tokens
description: Requirements for telling a change of formatting from a change of code by the tokens of a file; the tokens of every grammar as its parser read them, each with its place in the tree and the errors the parser reported, the text a literal, a comment and the text of a JSX element compare by, the indent of Starlark, the blanks between two tokens and those that are no whitespace of the language, the one answer for two ranges of lines with the code around them, the same, the same code with other comments, or different, and the content of a run of tokens wherever it stands.
style: plain-1
targets:
  - crates/code-symbols/src/tokens/mod.rs
  - crates/code-symbols/src/tokens/build.rs
  - crates/code-symbols/src/tokens/psi.rs
  - crates/code-symbols/src/kotlin/tokens.rs
  - crates/code-symbols/src/java/tokens.rs
  - crates/code-symbols/src/csharp/tokens.rs
  - crates/code-symbols/src/starlark/tokens.rs
  - crates/code-symbols/src/rust/tokens.rs
  - crates/code-symbols/src/js/tokens.rs
  - crates/code-symbols/examples/tokens.rs
  - crates/review-plan/src/whitespace.rs
---

# Equal by Tokens

Status: Implemented
Date: 2026-11-11

## Purpose

A reviewer passes what only changed its formatting: a call wrapped over lines, code indented again. That is safe only
when the change provably is no change of the program. The same characters without their whitespace prove nothing: a
space inside a string is content, `a - -b` is not `a--b`, and a line break ends a statement of Kotlin, JavaScript and
Starlark. This spec is the one rule both readers of a change ask, the review plan for its `whitespace`
([Review Plan](../../branch/review-plan/spec.md#mechanical-changes)) and the diffs for a block of formatting: two
pieces of two texts are the same when their tokens are, a token being a leaf of the tree the parser made with its
place in that tree and the errors the parser reported there, so that the same tokens are the same tree with the same
errors.

## Goals

- Never call a change of code formatting: the answer is a proof, or it is "different".
- One rule for every reader of a change, in Rust, with no rule of a grammar's line breaks or brackets of its own:
  what a parser reads from whitespace is in its tree.
- Tell a change of comments alone apart: the code is the same, the comments are not.
- Tokens a reader keeps per blob: no text, no tree, the same hashes in every run.
- A cheap question before the tokens are read that never disagrees with them: texts equal by tokens are the same
  without their blanks.
- The identity of a change across two states of a file, by the same tokens: the same after a change of formatting,
  wherever the change stands.

## Non-goals

- A file of no grammar (Python, Go, YAML, Markdown): its reader keeps a rule of characters.
- What a formatting changes beyond the program: the line numbers a stack trace or `line!()` reports.
- The tokens of a script that does not parse without a recovery: such a file is never the same.

## Requirements

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

### Requirement: Tokens: The errors of the parser are part of the tokens

Every error the parser reported MUST be part of the shape of the token at its place, by its message: a parser may
keep its errors beside the tree, so that two texts with the same tokens are the same tree with the same errors. An
error on a comment MUST be part of what that comment compares by, not of the code.

#### Scenario: An error that changes no node
- **WHEN** two texts differ by a line break before the `=>` of an arrow function or after a `throw` of JavaScript, or by the space before the file name of a `#line` of C# with a span
- **THEN** their tokens differ: the parser reports the error beside a tree of the same nodes
- **AND** two texts with that error both, formatted differently, have the same tokens
- **AND** a documentation comment of C# whose XML does not close and the one that closes it are the same code with other comments
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
those with a byte in its lines, and of no lines the token that runs across that place. `compare` MUST answer the
same for two runs of tokens alone.

#### Scenario: A line inside a text block
- **WHEN** a range is a line inside a text block, or the place between two of its lines
- **THEN** its token is the text block, so a blank line the text block gains is `Different`
- **AND** a changed line of a block comment is `CodeOnly`
- **AND** two runs without a token are the same runs, and as two ranges of lines `Same` only by the code around them
  [@test] crates/code-symbols/src/tokens_test.rs (`the_tokens_of_lines_are_those_with_a_byte_in_them`)

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

### Requirement: Equality: The content of a run of tokens is the same wherever the run stands

`content` MUST answer one hash for a run of tokens: each token by its kind and its text and the tree between the
tokens, without what opens before the first token of the code and what closes after the last, which the code around
the run decides. Two runs that are `Same` MUST have the same content, and two that differ between their ends MUST NOT.

#### Scenario: A function that becomes the last of its file
- **WHEN** a function is formatted again and another one follows it, where it was the last of its file
- **THEN** its lines have the same content, though their last token closes other nodes
- **AND** a line break that ends a statement, a space inside a string, an error the parser reported, another comment, a renamed name and the indent of a line of Starlark are another content
  [@test] crates/code-symbols/src/tokens_test.rs (`the_content_of_a_run_is_the_same_wherever_it_stands`)

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

### Requirement: Equality: Two names read as one compare as the same where the tree is

A reader that takes two spellings for one name MUST be able to give the tokens of its places one text on both sides,
each place found by its line and its column of the view. The texts then MUST compare as any tokens do: the same
where the tree around the names is the same, and no other token changes.

#### Scenario: A renamed local
- **WHEN** a local is renamed at its three places, one after a character of two UTF-16 units, and the tokens of the places get one text on both sides
- **THEN** the two texts are the same by tokens, which they are not without it
- **AND** no token starts inside a name, at a blank or past the text, and a place past the tokens is left out
- **AND** the name as an argument of a call, where it was an operand, is another token under the one text
  [@test] crates/code-symbols/src/tokens_test.rs (`two_names_read_as_one_compare_as_the_same_where_the_tree_is`)

## Testing / Local Run

`cargo nextest run -p code-symbols -E 'test(/tokens_test/)'`. The rule on real files, each formatted again and with a
name renamed: `cargo run --release -p code-symbols --example tokens -- <files and directories>` (`--none` lists the
files without tokens).

## Open Questions / Risks

- A `shape` and a `text` are 64-bit hashes: two different tokens share both with a probability of 2⁻⁶⁴ a pair.
- An error is in the tokens by its message: a parser that words one error two ways makes two texts different that
  are broken alike, never the reverse.
- The errors of Rust cost a quarter of its tokens (rust-analyzer's validation), though no line break of Rust is known
  to be an error of the same tree.
- The text of a JSX element is read as Babel and TypeScript read it; another reader of JSX may differ.
- A range of lines is asked with the two tokens of the code around it, not with the whole text: a parser that read
  the whitespace of a range into a token farther away would not be seen. None of the parsers does, each asks for a
  line break only before the token it stands at; the whole texts are compared by `texts_equal_by_tokens`.
- A block of formatting right before or after another changed block, with only blank lines and comments between
  them, is `Different` when that block changed the token next to it: one stop more, never one less.

## References

- [Change Review](../../../../docs/design/change-review.md) ("Equal by tokens")
- [Review Plan](../../branch/review-plan/spec.md#mechanical-changes), [Review Units](../../branch/review-units/spec.md#identity)
