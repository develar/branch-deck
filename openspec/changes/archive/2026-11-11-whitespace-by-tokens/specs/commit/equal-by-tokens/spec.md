## ADDED Requirements

### Requirement: Tokens: Only blanks stand between two tokens

What stands between two tokens MUST be blank (`is_blank`): whitespace, a byte order mark, or another character a
parser skips as whitespace. A text where anything else stands there MUST have no tokens, so that texts equal by
tokens are the same without their blanks (`same_without_blanks`), what a reader asks before it reads tokens.

#### Scenario: What stands between two tokens
- **WHEN** two texts differ by a byte order mark at their start, by a zero width space between two tokens of JavaScript, by a left-to-right mark of Rust or a Ctrl-Z of C#
- **THEN** their tokens are the same, and the texts are the same without their blanks
- **AND** the `\` that continues a line of Starlark is a token, a line joined without it another text
- **AND** a C# text whose token Roslyn cut inside a character or dropped from its tree, and a Java text whose whitespace is a Unicode escape, have no tokens
  [@test] crates/code-symbols/src/tokens_test.rs (`what_stands_between_two_tokens_is_blank`)

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

### Requirement: Equality: The content of a run of tokens is the same wherever the run stands

`content` MUST answer one hash for a run of tokens: each token by its kind and its text and the tree between the
tokens, without what opens before the first token of the code and what closes after the last, which the code around
the run decides. Two runs that are `Same` MUST have the same content, and two that differ between their ends MUST NOT.

#### Scenario: A function that becomes the last of its file
- **WHEN** a function is formatted again and another one follows it, where it was the last of its file
- **THEN** its lines have the same content, though their last token closes other nodes
- **AND** a line break that ends a statement, a space inside a string, an error the parser reported, another comment, a renamed name and the indent of a line of Starlark are another content
  [@test] crates/code-symbols/src/tokens_test.rs (`the_content_of_a_run_is_the_same_wherever_it_stands`)
