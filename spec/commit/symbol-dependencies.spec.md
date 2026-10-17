---
name: Symbol Dependencies
description: Requirements for the syntax summaries of the source files the commits change, their cache, and the dependencies by symbols between commits that they give; the measurement that decides whether they order branches.
style: plain-1
targets:
  - ../../crates/code-symbols/src/extract.rs
  - ../../crates/code-symbols/src/summary.rs
  - ../../crates/code-symbols/src/language.rs
  - ../../crates/sync-core/src/symbol_scan.rs
  - ../../crates/sync-core/src/symbol_dependencies.rs
  - ../../crates/sync-core/src/module_roots.rs
  - ../../crates/object-cache/src/blob_summary.rs
  - ../../crates/git-ops/src/object_texts.rs
  - ../../crates/sync-core/src/diff_scan.rs
  - ../../crates/bd-cli/src/symbol_stats.rs
---

# Symbol Dependencies

Status: Draft
Date: 2026-10-17

## Summary

A commit that calls a function another commit added in another file merges cleanly into a branch without that commit,
and the branch fails only when it is compiled. The line dependencies don't see it. Branch Deck reads the source files
the commits change with tree-sitter (Kotlin, Java, Rust, TypeScript and JavaScript), and a commit that uses a name
another commit of the range adds, with evidence that the use means that declaration, depends on it by symbols. Such an
edge orders branches; it never joins them. See [Symbol Dependencies](../../docs/design/symbol-dependencies.md).

## Goals

- See the compile dependencies between commits that git's merges don't see, without building the project.
- Cost the blobs the commits change, not the repository: a monorepo of a million files stays as cheap as a small one.
- Read each blob once: a summary is cached by blob id.

## Non-goals

- Exact name resolution: no build, no language server, no index of the tree. A use without evidence is no edge.
- Python, Go, Groovy and Scala: their files keep the line dependencies only.

## Requirements

### Summaries

- The summary of a file must hold its package, its imports (Rust `use` and `pub use`, Kotlin aliases, TypeScript
  imports and re-exports), its declarations with their kind, containers and reach, and its references, without the
  words of comments and strings (but with the names of interpolations), locally bound names, labels and keys.
  [@test] ../../crates/code-symbols/src/rust_test.rs (`a_rust_use_binds_the_name`)
  [@test] ../../crates/code-symbols/src/kotlin_test.rs (`a_word_in_kdoc_is_no_reference`, `a_local_val_shadows_and_is_no_reference`)
  [@test] ../../crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)
- A rule must name only node kinds its grammar has.
  [@test] ../../crates/code-symbols/src/summary_test.rs (`every_spec_kind_exists_in_its_grammar`)
- The commits' changes must carry the full ids of the blobs before and after each file, from the same `log --raw`
  process (`--no-abbrev`); a side without a file, a symlink and a submodule have none.
  [@test] ../../crates/sync-core/src/diff_scan_test.rs (`parses_raw_and_patch_parts`, `read_commit_diffs_gives_the_full_blob_ids`)
- The summaries must be read through the object cache, keyed by the extractor version, the language and the blob id,
  with one `cat-file --batch` for the blobs the cache misses; a write of another extractor version must clear the
  store once. A summary cut by the parse budget must not be cached.
  [@test] ../../crates/sync-core/src/symbol_scan_test.rs (`reads_the_summaries_through_the_cache`)
  [@test] ../../crates/object-cache/src/cache_test.rs (`blob_summaries_are_read_by_the_extractor_version_that_wrote_them`)
- The module roots (the nearest directory with an `*.iml`, a Bazel, Gradle or Maven build file, a `Cargo.toml` or a
  `package.json`), the crate names and the TypeScript path aliases must come from HEAD's tree, the directories with
  one `cat-file --batch`, never a command line that lists them.
  [@test] ../../crates/sync-core/src/module_roots_test.rs (`the_nearest_directory_with_a_build_file_is_the_root`, `reads_the_directories_above_the_files_at_head`)
  [@test] ../../crates/sync-core/src/symbol_scan_test.rs (`the_name_of_a_crate_and_the_aliases_of_a_tsconfig`)
  [@test] ../../crates/git-ops/src/object_texts_test.rs (`trees_list_their_entries_with_full_ids`)

### Edges

- A commit must depend on an older one by symbols when a reference on its added lines means a declaration that commit
  adds, with evidence: an import or alias of it, its qualified name or `Type.member` with a resolved type, the same
  file, the same Rust module, a wildcard import of its package or module for a type, or a type of the same JVM package
  in the same module.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_kotlin_import_of_a_function_another_commit_adds_is_an_edge`, `a_java_member_by_its_class_and_a_static_import_are_edges`, `a_rust_use_of_a_crate_path_and_a_path_of_another_crate_are_edges`, `a_typescript_import_by_a_relative_path_or_an_alias_is_an_edge`)
- A call of a function of the same package without an import must be no edge, nor a type of the same package in
  another module.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`the_same_package_is_evidence_for_a_type_in_the_same_module_only`)
- A commit that removes a declaration must depend on the older commits that removed its uses.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_removal_needs_the_commit_that_removed_the_uses`)
- A declaration that moves within its namespace in one commit must be neither added nor removed; a use that two
  commits' declarations could mean must be no edge; a commit must not depend on itself; a file with a twentieth of its
  bytes in parse errors must make no edge.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_commit_needs_no_edge_to_itself_a_moved_declaration_is_no_addition_and_two_owners_are_ambiguous`)
- Each commit must name the functions and types around its changed lines (its entities).
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`the_entities_are_the_functions_and_types_around_the_changed_lines`)

### Mode and measurement

- `branchdeck.symbols` must be `off`, `report` (the default) or `on`; another value is an error.
  [@test] ../../crates/sync-core/src/symbol_scan_test.rs (`the_mode_comes_from_git_config_and_is_report_without_it`)
- `bd commits --symbol-stats` (hidden) must print what the scan read and found: blobs and bytes per language, parse
  errors, the times, the line, symbol, shared and symbols-only edges, the links by evidence, and a deterministic sample
  of the symbols-only edges per evidence for a human to label. It must write the object cache only.
