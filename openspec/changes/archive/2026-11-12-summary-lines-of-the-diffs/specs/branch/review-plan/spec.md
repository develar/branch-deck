## MODIFIED Requirements

### Requirement: In the Review Window: A block of the imports alone is played down

A block whose every changed line of each side is an import or a package statement or blank (`TextDiff::import_blocks`,
from the syntax of the texts) MUST be pale, as a change of whitespace alone, and N and P MUST pass it; in the story it
is in the unit of the imports, and the section of its file shows one row for it. A block with a line of code beside
them is a change as any. The statements are read at the lines of the diff, never of git.

#### Scenario: A block of the imports alone
- **WHEN** every changed line of each side of a block is an import or a package statement or blank
- **THEN** the block is pale and N and P pass it; elsewhere than a review it is a change as any
- **AND** in the story it is in the unit of the imports, and the section of its file shows one row that opens it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)
  [@test] src/lib/diff/rows.test.ts (`plays down a block of the imports alone in a review: pale, and N and P pass it; elsewhere it is a change as any`)

#### Scenario: The statements that are imports
- **WHEN** `TextDiff::import_blocks` reads the syntax of the texts
- **THEN** the import and package statements of Kotlin, Java, Rust and TypeScript and the `using` directives and the `namespace` line of C# are imports
- **AND** a Rust `pub use` and a TypeScript `export … from` are none
  [@test] crates/file-diffs/src/service_test.rs (`a_block_of_import_statements_alone_is_marked_by_the_syntax`)
  [@test] crates/code-symbols/src/summary_test.rs (`the_import_lines_are_the_import_and_package_statements`)

#### Scenario: A line of code beside the imports
- **WHEN** a block has a line of code beside the imports
- **THEN** it is a change as any

#### Scenario: A lone `\r` above the imports
- **GIVEN** a text whose comment above its statements holds a lone `\r`, and a line of code right above the first statement: a file annotation of Kotlin, a directive of TypeScript
- **WHEN** a change touches that line of code alone
- **THEN** its block is a change as any, and a block of the import below it is a block of the imports alone
  [@test] crates/file-diffs/src/service_test.rs (`the_imports_and_the_scopes_of_a_text_with_a_lone_cr_are_at_the_lines_of_the_diffs`)
