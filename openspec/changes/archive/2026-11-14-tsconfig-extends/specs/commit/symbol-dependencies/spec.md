## MODIFIED Requirements

### Requirement: Summaries: Module roots, crate names and path aliases come from HEAD's tree

The module roots (the nearest directory with an `*.iml`, a Bazel, Gradle or Maven build file, a `Cargo.toml` or a
`package.json`), the crate names and the TypeScript path aliases MUST come from HEAD's tree, the directories with one
`cat-file --batch`, never a command line that lists them. The aliases of a `tsconfig*.json` MUST be those tsc gives it
with its `extends` chain; a configuration a chain names that the batch of the summaries lacks is read in one more
`cat-file --batch` a level.

#### Scenario: The nearest directory with a build file
- **WHEN** a file lies below several directories with build files
- **THEN** its module root is the nearest
  [@test] crates/trial-check/src/module_roots_test.rs (`the_nearest_directory_with_a_build_file_is_the_root`)

#### Scenario: The directories above the files
- **WHEN** the roots of the files of the commits are read
- **THEN** the directories above them are read at HEAD in one `cat-file --batch`, their entries with full ids
  [@test] crates/trial-check/src/module_roots_test.rs (`reads_the_directories_above_the_files_at_head`)
  [@test] crates/git-ops/src/object_texts_test.rs (`trees_list_their_entries_with_full_ids`)

#### Scenario: Crate names and path aliases
- **WHEN** a `Cargo.toml` names its crate and a tsconfig its path aliases
- **THEN** the scan takes them from HEAD's tree
  [@test] crates/trial-check/src/symbol_scan_test.rs (`the_name_of_a_crate`, `reads_the_summaries_through_the_cache`)

#### Scenario: A tsconfig that extends another
- **GIVEN** a `tsconfig.json` with comments and a trailing comma that extends `../tsconfig.base`, one that extends a
  configuration of a directory above none of the changed files, and one that extends `./base` beside them
- **WHEN** the scan reads their aliases
- **THEN** each has the `paths` and `baseUrl` of its chain as tsc applies them, from the root
- **AND** the first costs no process beside a configuration that extends none, since the `tsconfig*.json` files of
  the directories above the files are in that batch and a name tried that those directories lack (without `.json`)
  is no read, and each of the others one `cat-file` more
  [@test] crates/trial-check/src/symbol_scan_test.rs (`the_aliases_of_a_tsconfig_with_the_chain_it_extends`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`commits_read_the_configuration_a_tsconfig_extends_in_one_batch_more_but_a_tsconfig_above_the_files`)

### Requirement: Edges: A use of another commit's declaration with evidence is an edge

A commit MUST depend on an older one by symbols when a reference on its added lines means a declaration that commit
adds, or a form of it that commit gives (see the entries below), with evidence.

#### Scenario: A Kotlin import
- **WHEN** a commit calls a function that an older commit adds in another file, through an import of it
- **THEN** it depends on that commit by symbols
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_kotlin_import_of_a_function_another_commit_adds_is_an_edge`)

#### Scenario: A Java member by its class and a static import
- **WHEN** a Java commit uses a member another commit adds by its class, or through a static import
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_member_by_its_class_and_a_static_import_are_edges`)

#### Scenario: A Rust use of a crate path and a path of another crate
- **WHEN** a Rust commit uses an item another commit adds by a `use` of a crate path, or by a path of another crate
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_use_of_a_crate_path_and_a_path_of_another_crate_are_edges`)

#### Scenario: A TypeScript import
- **WHEN** a TypeScript commit imports what another commit adds, by a relative path or a path alias
- **THEN** it is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_typescript_import_by_a_relative_path_or_an_alias_is_an_edge`)

#### Scenario: A TypeScript import under baseUrl or by the longest pattern
- **WHEN** a TypeScript commit imports what another commit adds by a specifier no pattern takes, which names the file
  under `baseUrl`, or by one two patterns take
- **THEN** it is an edge, of the targets of the pattern with the longer prefix
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_typescript_import_under_the_base_url_or_by_the_longest_pattern_is_an_edge`)
