## MODIFIED Requirements

### Requirement: Mechanical Changes: A file of the build model is buildModel

A file MUST be `buildModel` when its name is of the build model: `*.iml`, a `*.lock` that is no lock file of a package
manager, `.idea/modules.xml`. A `BUILD` or `BUILD.bazel` MUST be none by its name: most hold targets a person writes,
so its content says what it is.

#### Scenario: A file named as the build model
- **WHEN** the commits change an `*.iml`, a `*.lock` that is no lock file of a package manager, or `.idea/modules.xml`
- **THEN** the file is `buildModel`
  [@test] crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`)

#### Scenario: A Bazel build file
- **WHEN** the commits change a `BUILD` or a `BUILD.bazel`
- **THEN** its name makes it nothing: it is `generated`, `importsOnly`, `buildModel` or real work by its content
  [@test] crates/review-plan/src/files_test.rs (`a_bazel_build_file_is_mechanical_by_its_content_not_its_name`)

### Requirement: Mechanical Changes: A manifest whose dependency entries alone changed is buildModel

A plugin descriptor, a `Cargo.toml`, a `package.json` or a Bazel `BUILD` file MUST be `buildModel` when only its
dependency entries changed ("Dependency Changes"). One that changed beside them, a new one, and one that a side does
not parse MUST stay real work. One whose entries and remainder did not change is read as any other file. An `*.iml`
that does not parse MUST stay real work too.

#### Scenario: Only the dependency entries changed
- **WHEN** only the dependency entries of an `*.iml`, a `Cargo.toml`, a `package.json`, a plugin descriptor or a Bazel `BUILD` file changed
- **THEN** the file is `buildModel`
  [@test] crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`, `the_dependencies_of_a_package_json_are_its_dependency_changes`, `the_dependencies_and_the_content_of_a_plugin_descriptor_are_its_dependency_changes`, `the_labels_of_the_targets_of_a_bazel_build_file_are_its_dependency_changes`)
  [@test] crates/review-plan/src/files_test.rs (`a_bazel_build_file_is_mechanical_by_its_content_not_its_name`)

#### Scenario: A manifest that changed beside its entries
- **WHEN** a manifest changed beside its dependency entries, is new, or a side does not parse, or an `*.iml` does not parse
- **THEN** it stays real work
- **AND** one whose entries and remainder did not change is read as any other file
  [@test] crates/review-plan/src/dependencies_test.rs (`a_new_version_with_new_features_keeps_the_manifest_in_the_tree`)

### Requirement: Mechanical Changes: An import statement runs until its brackets close

An import statement MUST start with an `import`, `use` or `package` line, a C# `using` directive or a `namespace`
line, or, in a Starlark file, a `load(` line of the top level, and run until the brackets it opens close, read in the
whole file. A `pub use` re-exports, so it MUST be none, and a C# `using` statement (`using (var x = …)`,
`using var x = …;`) MUST be code.

#### Scenario: A line inside an import block
- **WHEN** a line is inside a Rust `use a::{` block or a TypeScript `import {` block over several lines
- **THEN** it is a line of an import statement
  [@test] crates/review-plan/src/files_test.rs (`a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)

#### Scenario: A C# using directive and a using statement
- **WHEN** a C# file changes a `using` directive, or a `using` statement (`using (var x = …)`, `using var x = …;`)
- **THEN** the directive is an import and the statement is code
  [@test] crates/review-plan/src/files_test.rs (`a_csharp_using_directive_is_an_import_and_a_using_statement_code`)

#### Scenario: The load of a Bazel file
- **WHEN** the commits change only the `load` lines of a `BUILD` file
- **THEN** they are import statements, and the file is `importsOnly`
  [@test] crates/review-plan/src/files_test.rs (`a_bazel_build_file_is_mechanical_by_its_content_not_its_name`)

#### Scenario: A pub use
- **WHEN** a hunk changes a `pub use`
- **THEN** it re-exports, so it is no import statement

### Requirement: In the Review Window: A mechanical change starts collapsed and says why

The diffs MUST be the story of the units ([Review Units](../review-units/spec.md#in-the-review-window)): a mechanical
change starts collapsed, its header saying why ("Build file", "Imports only", "Whitespace only", "Moved only",
"Renamed only"; a generated file what its name makes it, "Lock file", "Minified", "Source map", "Generated (.gitattributes)", the line
its tool wrote, or the name of the region a tool writes that holds its change, cut at 100 characters). The chevron of
the header opens it.

#### Scenario: A generated file
- **WHEN** the diffs show a generated file
- **THEN** it starts collapsed, its header saying why: what its name makes it ("Lock file", "Minified", "Source map"), "Generated (.gitattributes)", the line its tool wrote, "Generated by tools/render.mjs; edit src/guide.md", or the name of its region, "auto-generated section `build intellij.a`", cut at 100 characters
- **AND** a rendering stands at the end of the concern of its source
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)
  [@test] src/presenters/review-units.test.ts (`fold each with why it is generated: its name, its attributes, the line its tool wrote`)
  [@test] src/presenters/review-plan.test.ts (`says why a generated file is folded: its name, its attributes, the line its tool wrote, its region, a long one cut`)

#### Scenario: Another mechanical change
- **WHEN** the diffs show a build file, a change of the imports or of whitespace only, declarations that moved, or a rename
- **THEN** it starts collapsed, its header saying "Build file", "Imports only", "Whitespace only", "Moved only" or "Renamed only"
- **AND** the declarations that moved are one group over their files, headed "Moved" with the count of its files
- **AND** the chevron of the header opens it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] src/presenters/review-plan.test.ts (`names the Mechanical group and its groups by kind`)
  [@test] src/presenters/review-units.test.ts (`folds the declarations that moved as one group over their files, headed Moved with their count, each file saying Moved only`)

## ADDED Requirements

### Requirement: Mechanical Changes: A change inside the regions a tool writes is generated

A file a person writes MUST be `generated`, with the name of its region as why (`GeneratedBy::Region`), when its change
is a tool's: each hunk of each change lies between the markers of a region a tool writes (`diff_marks::regions`: a
comment line naming it generated that ends with `start` or `begin`, and its line with `end`), of one name on both
sides, a hunk of a side of no lines where such a region holds that place; or removes and adds only blank lines and
import statements (the `load` lines a generator of Bazel keeps too), one hunk of a region at least. A file the commits
add or delete MUST be `generated` so when its text holds such a region and, outside its regions, only blank lines and
import statements. A hunk that touches a marker of a file still there MUST keep the file real work: a region that
comes, goes or changes its name is read.

#### Scenario: A section a generator writes
- **WHEN** the commits change only the lines of a section a generator writes into a `BUILD.bazel` (`### auto-generated section \`build intellij.a\` start` … `end`), a line added to its dependencies
- **THEN** the file is `generated`, its region `auto-generated section \`build intellij.a\`` as why
  [@test] crates/review-plan/src/files_test.rs (`a_bazel_build_file_is_mechanical_by_its_content_not_its_name`)

#### Scenario: A change beside the section
- **WHEN** the commits change a section a generator writes and code a person wrote beside it
- **THEN** the file is real work, and the lines of the section are a quiet run of its diff
  [@test] crates/review-plan/src/files_test.rs (`a_bazel_build_file_is_mechanical_by_its_content_not_its_name`)

### Requirement: Mechanical Changes: A Bazel BUILD file is mechanical by its content

A `BUILD` or `BUILD.bazel` MUST be mechanical by its content alone: `generated` when its change lies in the regions a
tool writes, `importsOnly` when only its `load` lines changed, `buildModel` when only the labels its targets write
changed; a new target, another change of a target, a deletion that is no tool's and a list whose labels the reader
cannot see MUST keep it real work.

#### Scenario: The kinds of a change of a build file
- **WHEN** the commits change the section a generator writes, the `load` lines, the dependency list of a target a person wrote, add a target, change beside a section, delete a package, or add to a list the file computes
- **THEN** the file is `generated`, `importsOnly`, `buildModel`, or real work for the last four
  [@test] crates/review-plan/src/files_test.rs (`a_bazel_build_file_is_mechanical_by_its_content_not_its_name`)

### Requirement: Dependency Changes: The entries of a Bazel BUILD file

The entries of a `BUILD` or `BUILD.bazel` MUST be the labels each named target outside the regions a tool writes
writes by attribute, of the kind `target`, read by the reader of a build file of psi-syntax, which accio reads too (the
variables of the file followed, a label as written). The scope is `test` for a test rule or a `testonly` target, else
by the attribute (`runtime_deps` `runtime`), else `main`. Its module is the name of its directory. A file whose
targets all stand in regions a tool writes MUST be no manifest: the `*.iml` it is generated from names its entries. A
label the file computes, or a statement the reader cannot read, MUST leave it unparsed.

#### Scenario: The labels of the targets of a build file
- **WHEN** the commits add a label to the `deps` of a target a person wrote, remove one from its `runtime_deps`, or add a label to a list the file computes
- **THEN** the first two are its entries, `added` and `removed`, of the kind `target`, and the third leaves the file unparsed
  [@test] crates/review-plan/src/dependencies_test.rs (`the_labels_of_the_targets_of_a_bazel_build_file_are_its_dependency_changes`)
