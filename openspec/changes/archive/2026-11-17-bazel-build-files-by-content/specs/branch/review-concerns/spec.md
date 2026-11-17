## ADDED Requirements

### Requirement: The Rest: A region a tool writes follows the file it is generated from

A file `generated` by a region a tool writes (`GeneratedBy::Region`) MUST be a consequence of the changed file whose
name without its extension is a word of the name of the region (the `*.iml` `intellij.a.iml` of the section
`build intellij.a`): it follows the concern of that file, or, when that file is itself a consequence (a build file of
its dependency entries alone), the concern it follows. A region whose name names no changed file MUST stay in the
Mechanical group.

#### Scenario: A section of a BUILD file and its iml
- **WHEN** the commits add a dependency to the `*.iml` of a module, its code that needs it, and the line the generator
  writes into the section of the module in its `BUILD.bazel`
- **THEN** the `BUILD.bazel` follows the concern the `*.iml` follows, the concern of that code
  [@test] crates/review-plan/src/concerns_test.rs (`a_section_a_tool_writes_follows_the_concern_of_the_iml_it_is_generated_from`)
