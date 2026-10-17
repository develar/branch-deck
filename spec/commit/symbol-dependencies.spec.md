---
name: Symbol Dependencies
description: Requirements for the syntax summaries of the source files the commits change, their cache, and the dependencies by symbols between commits that they give; the measurement that decides whether they order branches.
style: plain-1
targets:
  - ../../crates/code-symbols/src/extract.rs
  - ../../crates/code-symbols/src/summary.rs
  - ../../crates/code-symbols/src/language.rs
  - ../../crates/code-symbols/src/kotlin.rs
  - ../../crates/code-symbols/src/java.rs
  - ../../crates/sync-core/src/symbol_scan.rs
  - ../../crates/sync-core/src/symbol_dependencies.rs
  - ../../crates/sync-core/src/module_roots.rs
  - ../../crates/sync-core/src/dangling_references.rs
  - ../../crates/object-cache/src/blob_summary.rs
  - ../../crates/git-ops/src/object_texts.rs
  - ../../crates/sync-core/src/diff_scan.rs
  - ../../crates/bd-cli/src/symbol_stats.rs
---

# Symbol Dependencies

Status: Implemented
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
- The summary of a Kotlin or Java file must hold the parameter types of each function (the last names of each type,
  `?`, a default value, `vararg`, the dimensions of an array with a vararg counted as one; a type parameter as any
  type), whether a member overrides and whether a subtype may override it, and the direct supertypes of each type
  without type arguments, a Kotlin function type as an empty path. An `override` or `@Override` member must be one
  `Override` reference per direct supertype of the class around it (an object expression, an anonymous class and the
  body of an enum entry included), qualified by the path of the supertype, with its kind, receiver and parameter types
  as its shape; an override in a lambda (a one-line object expression the grammar reads as a call) must have none.
  [@test] ../../crates/code-symbols/src/kotlin_test.rs (`an_override_is_a_reference_per_supertype`, `a_signature_has_the_parameter_types`, `a_member_knows_whether_it_overrides_and_can_be_overridden`, `an_override_in_a_lambda_gets_no_supertype`)
  [@test] ../../crates/code-symbols/src/java_test.rs (`an_override_is_a_reference_per_supertype`, `a_signature_has_the_parameter_types`, `a_member_knows_whether_it_overrides_and_can_be_overridden`)
  [@test] ../../crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)
- The dangling-reference check must take the line of an `override` member for no use of its name: it declares one.
  [@test] ../../crates/sync-core/src/dangling_references_test.rs (`an_override_line_is_no_use`)
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
- In the same file, a name that a type other than the containers or the receiver of the file's declaration qualifies
  (`Registry.register(id)` beside a `register` of `Plugin`, Rust `Registry::create()` in the `create` of another
  `impl`) must be no use of that declaration.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_name_qualified_by_another_type_is_no_use_of_a_member_of_its_own_file`)
- A Kotlin `override` or Java `@Override` member must be a use (evidence `override`) of the member of the same name,
  kind, receiver and parameter types (a type parameter fitting any) in the supertype it names, when another commit
  adds that member: in a subclass, an object expression, an anonymous class or the body of an enum entry, in the same
  file too. The supertype must resolve by the same file, its full path, an import or import alias (of the type, or of
  the type it is nested in), or the same package in the same module without a type of that name in the file nor an
  import of another one; a wildcard import or another package without an import is none.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`an_override_of_a_member_another_commit_adds_is_an_edge`, `an_override_whose_supertype_does_not_resolve_is_no_edge`)
- An override must be no edge when the member it overrides overrides one itself (the supertype of its type has it
  already), when the member is one every class has from `Any` or `Object` (`equals`, `hashCode`, `toString`, `clone`,
  `finalize`), when the class around it has several supertypes (a function type among them), when the member is a Java
  method of a class with supertypes (it may override without `@Override`), when it is `internal` to another module,
  when its extension receiver or its kind (a property for a function) differs, or when a type of the supertype's name
  that a scope around it may mean exists (one nested in a declaration around the override, or a member type that a
  Java class around the overriding class inherits). Where the member's type has supertypes, whose members are
  unknown, the override must have its exact form: no type parameter, each type resolved by both files to the same
  one; elsewhere a type parameter fits any type.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`an_override_of_an_override_is_no_edge`, `an_override_of_a_member_of_any_is_no_edge`, `an_override_with_two_supertypes_is_no_edge`, `a_java_method_of_a_class_with_supertypes_is_no_edge_for_an_override`, `an_override_in_another_module_of_an_internal_member_is_no_edge`, `an_override_of_another_receiver_or_kind_is_no_edge`, `an_override_of_a_supertype_a_scope_may_shadow_is_no_edge`, `an_override_of_a_member_of_a_type_with_supertypes_needs_its_exact_form`)
- The declarations of a key of a member of a JVM type must be entries told apart by kind, extension receiver and
  parameter types (with their arrays, a vararg counted), each with its origin: the baseline, or the commit that
  created it or changed its form, one entry removed and one added by a commit being the same entry in a new form. An
  override must depend on the commit of the latest form of each entry it may mean that the form before didn't accept,
  and only when that is one older commit for all of them: a new overload of a name the type had is an addition, an
  override of an overload of the baseline is no edge, and two overloads of a new file are both entries. Until a call
  carries its arguments it can't tell the overloads apart, so a use without a shape must depend on the commit that
  added the key (the oldest origin of the entries that accept it), and on none when one is of the baseline.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`an_override_of_a_new_overload_is_an_edge`, `an_override_of_a_member_the_range_does_not_add_is_no_edge`, `two_overloads_in_a_new_file_keep_both`, `a_new_form_of_an_entry_is_an_edge_for_the_uses_that_need_it`, `an_entry_is_its_parameter_types_with_their_arrays`, `a_call_needs_the_commit_that_added_the_key_of_its_name`, `the_cases_of_the_report`)
- The key of a top-level function must hold the entries of each file of the range that adds it, so that a call of an
  overload two files add is no edge; a removal must drop the entries of its file only.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`top_level_overloads_in_two_files_are_both_entries`)
- The entries of a key must change only by clean sides: when a side of a file that declares the key is degraded, a
  declaration of the key on either side is inside a parse error, or two declarations of one side are the same entry,
  the key must only be present or absent, as a top-level function, a Rust or a TypeScript key always is.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_before_side_in_error_changes_no_state`, `an_entry_is_its_parameter_types_with_their_arrays`)
- A commit that removes a declaration must depend on the older commits that removed its uses, an override of it
  included.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_removal_needs_the_commit_that_removed_the_uses`, `a_removal_needs_the_commit_that_removed_its_override`)
- A declaration that moves within its namespace in one commit must be neither added nor removed; a use that two
  commits' declarations could mean must be no edge, its own commit's included; a commit must not depend on itself; a
  file with a twentieth of its bytes in parse errors must make no edge.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_commit_needs_no_edge_to_itself_a_moved_declaration_is_no_addition_and_two_owners_are_ambiguous`, `a_use_its_own_commit_may_mean_is_no_edge`)
- `bd commits` must name an override of another commit's overload as an edge by symbols, `bd check` as a missing
  declaration with the evidence `override` and the branch that adds it, and `bd suggest` with stacked branches must
  stack the branch of the override on that branch.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`an_edge_by_symbols_names_the_symbol_and_joins_no_unit`, `an_override_of_a_new_overload_is_an_edge_by_symbols`)
  [@test] ../../crates/bd-cli/src/check_test.rs (`an_override_of_another_branchs_overload_is_a_missing_declaration`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`with_the_symbols_on_an_override_of_another_branch_stacks`)
- Each commit must name the functions and types around its changed lines (its entities).
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`the_entities_are_the_functions_and_types_around_the_changed_lines`)

### Mode and measurement

- `branchdeck.symbols` must be `off`, `report` or `on` (the default; Settings → Repository, Symbol dependencies: Off ·
  Report Only · On, [Settings](../repository/settings.spec.md#repository)); another value is an error. `report` must
  read the summaries for `bd commits` and the dangling-reference check only; `off` must read none.
  [@test] ../../crates/sync-core/src/symbol_scan_test.rs (`the_mode_comes_from_git_config_and_is_on_without_it`)
  [@test] ../../crates/bd-cli/src/commits_test.rs (`an_edge_by_symbols_names_the_symbol_and_joins_no_unit`)
  [@test] ../../crates/sync-core/src/dangling_references_test.rs (`a_word_in_kdoc_is_no_use`)
  [@test] ../../crates/sync-core/src/check_test.rs (`a_stacked_branch_that_applies_on_the_baseline_builds_alone`)
- `bd commits --symbol-stats` (hidden) must print what the scan read and found: blobs and bytes per language, parse
  errors, the times, the line, symbol, shared and symbols-only edges, the links by evidence, and a deterministic sample
  of the symbols-only edges per evidence for a human to label. It must write the object cache only.
