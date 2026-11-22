## ADDED Requirements

### Requirement: Edges: A use needs the plain JVM import another commit adds to its file

A bare use of a name that a plain Kotlin or Java import binds (a name without an alias, Java's `import static` too)
MUST be an edge to the commit of the range that added that import to the file, or changed the path it imports the name
from, whatever declares the name. A use is a type, a value or a call by the bare name, or the head of a qualified name
with a capital (`JBUI.scale`), on an added line. It MUST be no edge when the commit of the use adds the import itself,
the baseline or a file the commit adds has the import, or the file gives the name another way: a declaration of the
name, a wildcard import of the import's own package, an import of the file's own package, and for a function by a
lower-case name a lambda's receiver that may take the call. `brd check` MUST report the branch of the use as
`missing_declaration` when the branch lacks the commit of the import.

#### Scenario: A use of a name the import of another commit binds
- **WHEN** a commit adds the plain import of `JBPanelWithEmptyText`, a type outside the range, to a Kotlin file with a
  use, and a later commit adds a bare use in the same file; or the uses are `JBUI.scale(2)`, a Java
  `TimeUnit.SECONDS` and a function of `import static`
- **THEN** each is an edge to the commit of the import, evidence `import`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_use_of_a_name_a_plain_jvm_import_of_another_commit_binds_is_an_edge`)

#### Scenario: A use that the import of another commit does not decide
- **WHEN** the baseline has the import, the commit of the use adds it itself after another commit removed it, a
  wildcard import of the same package gives the name, the import is of the file's own package, the file declares the
  name, or the use is a call in a lambda with a receiver
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_use_that_a_plain_jvm_import_of_another_commit_does_not_decide_is_no_edge`)

#### Scenario: The branch without the commit of the import
- **WHEN** branch `a2ui` adds the import of `JBPanelWithEmptyText` with a use, and branch `flakes` adds a bare use in
  another hunk of the file, which git merges onto the baseline
- **THEN** `brd check` reports `flakes` as `missing_declaration`, `declared_by` the commit of `a2ui`, evidence `import`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_uses_a_plain_import_another_branch_adds_is_a_missing_declaration`)
