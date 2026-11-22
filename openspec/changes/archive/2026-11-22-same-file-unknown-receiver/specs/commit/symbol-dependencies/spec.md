## ADDED Requirements

### Requirement: Edges: A name on a receiver its file doesn't show is no use of its file's declaration

In the file of a Kotlin or Java declaration that is no extension, a name on a receiver of a type the summary doesn't
know MUST be no use of it: an expression or a local (`JBPanelWithEmptyText().emptyText.text`, `panel().label`), or a
name that the file neither declares nor imports. A bare name, a receiver of the file's own type, and a qualifier that
names the package, a declaration of the file or an imported value MUST stay a use.

#### Scenario: A name on an expression
- **WHEN** a file declares `fun text(value: String)` in its class and a top-level `val label`, and a later commit
  writes `JBPanelWithEmptyText().emptyText.text = "x"` and `panel().label` there
- **THEN** neither is an edge
- **AND** a bare `text(label)` and `test.text("x")` on a `val test = RendererTest()` are edges, evidence `same_file`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_name_on_an_expression_receiver_of_an_unknown_type_in_its_file_is_no_use`)
