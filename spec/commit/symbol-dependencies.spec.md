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
Date: 2026-10-18

## Summary

A commit that calls a function another commit added in another file merges cleanly into a branch without that commit,
and the branch fails only when it is compiled. The line dependencies don't see it. Branch Deck reads the source files
the commits change with tree-sitter (Kotlin, Java, Rust, TypeScript and JavaScript), and a commit that uses a name
another commit of the range adds, with evidence that the use means that declaration, depends on it by symbols. An
override of a member another commit adds, a call of the overload its arguments select, and a use that only a new form of
a member or a constructor accepts (a wider reach, `open`, a default, a new or renamed parameter) count as uses of that
commit's declaration. Such an edge orders branches; it never joins them. See [Symbol
Dependencies](../../docs/design/symbol-dependencies.md).

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
- The summary of a Kotlin or Java file must hold the parameter types of each function (the last names of each type, `?`,
  a default value, `vararg`, the dimensions of an array with a vararg counted as one; a type parameter as any type),
  whether a member overrides, whether a subtype may override it and whether a Kotlin member is `@JvmStatic`, and the
  direct supertypes of each type without type arguments, a Kotlin function type as an empty path. An `override` or
  `@Override` member must be one `Override` reference per direct supertype of the class around it (an object expression,
  an anonymous class and the body of an enum entry included), qualified by the path of the supertype, with its kind,
  receiver and parameter types as its shape; an override in a lambda (a one-line object expression the grammar reads as
  a call) must have none.
  [@test] ../../crates/code-symbols/src/kotlin_test.rs (`an_override_is_a_reference_per_supertype`, `a_signature_has_the_parameter_types`, `a_member_knows_whether_it_overrides_and_can_be_overridden`, `a_member_knows_whether_it_is_jvm_static`, `an_override_in_a_lambda_gets_no_supertype`)
  [@test] ../../crates/code-symbols/src/java_test.rs (`an_override_is_a_reference_per_supertype`, `a_signature_has_the_parameter_types`, `a_member_knows_whether_it_overrides_and_can_be_overridden`)
  [@test] ../../crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)
- The summary of a Kotlin or Java call (not an infix call), Java's `new` and a Kotlin constructor invocation of a
  supertype included, must hold the shape of its arguments on the reference of its callee (the last name of the type of
  `new` and of an invocation, `Base` of `: a.Base(…)`, whose qualifiers are types): in source order, a named one with
  its name, a Kotlin trailing lambda last, whether one spreads an array, and each as a text (a string literal or
  template), a number (an integer without a suffix, signed or not), a boolean, a lambda or another expression (`null`, a
  real, a character, `1L`, `1u`, a name, a call). The shape must say whether the call is enclosed: in a Kotlin lambda or
  anonymous function, an object expression, a Java anonymous class or a local declaration inside the declaration around
  it, whose receiver may be what a bare name or `this` means; a Java lambda, an initializer and a getter are not, and
  the constructor invocation of an object expression's supertype and a `new` of an anonymous class are of the code
  around the object. An annotation (`@Suppress("x")`) must stay a type, and `this(…)` and `super(…)` no reference.
  [@test] ../../crates/code-symbols/src/kotlin_test.rs (`a_call_has_its_argument_shape`, `a_constructor_invocation_of_a_supertype_has_its_argument_shape`)
  [@test] ../../crates/code-symbols/src/java_test.rs (`a_call_has_its_argument_shape`)
  [@test] ../../crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)
- The summary of a Kotlin or Java class (not an interface, an object, a companion, an enum, an annotation, a type alias,
  a Kotlin type the grammar misread, nor a class with a parse error in it, which may have swallowed one) must hold its
  constructors, each with its parameters (as a function's) and the reach of its modifiers: a Kotlin primary constructor
  (its class parameters, a default value, `vararg`) and the secondary ones, the constructors a Java class declares, and
  the canonical constructor of a record (declared, compact or of its components, with the record's reach) with its
  others; without any, the implicit `()`, public in Kotlin and of the reach of its class in Java. Only the canonical
  constructor of a record, compact or declared again with the names and types of its components (every type known), may
  replace another, the one of its header: two that the summary can't tell apart (`Stamp(java.util.Date)` beside
  `Stamp(java.sql.Date)`, `Box(value: T)` beside `Box(list: List<T>)`) must both be there. A local class must add none
  to the class around it.
  [@test] ../../crates/code-symbols/src/kotlin_test.rs (`a_class_knows_its_constructors`)
  [@test] ../../crates/code-symbols/src/java_test.rs (`a_class_knows_its_constructors`)
  [@test] ../../crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)
- The dangling-reference check must take the name of an `override` member for no use: it declares one.
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
  adds, or a form of it that commit gives (see the entries below), with evidence: an import or alias of it, its
  qualified name or `Type.member` with a resolved type, the same file, the same Rust module, a wildcard import of its
  package or module for a type, or a type of the same JVM package in the same module.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_kotlin_import_of_a_function_another_commit_adds_is_an_edge`, `a_java_member_by_its_class_and_a_static_import_are_edges`, `a_rust_use_of_a_crate_path_and_a_path_of_another_crate_are_edges`, `a_typescript_import_by_a_relative_path_or_an_alias_is_an_edge`)
- A call of a function of the same package without an import must be no edge, nor a type of the same package in
  another module.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`the_same_package_is_evidence_for_a_type_in_the_same_module_only`)
- A static member must be used by the path of its type: in full, or from its head, which an import binds (an alias, a
  nested type: `import a.Owner.Factory` and `Factory.create()`) or, without an import of its name, the same package
  does when the file declares nothing of that name; a nested type is no name of its package. A member of a Kotlin
  companion without a name of its own must be used through the class around it too (`Owner.create()`,
  `Owner.Companion.create()`), from Java only a `@JvmStatic` function or a `const val` (`Owner.create()`,
  `import static a.Owner.create`), and Java's `Registry.INSTANCE.register()` must be a use of a function of the Kotlin
  object `Registry`. A companion with a name of its own, which the summary doesn't tell from a nested object, must be
  used only by its path (`Owner.Factory.create()`), and a member of a nested object never through the class around it.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_companion_member_through_its_owner_is_an_edge`, `a_named_companion_needs_its_name`, `a_java_call_of_a_companion_member_needs_jvm_static`, `a_java_call_through_instance_is_an_edge`, `a_member_of_a_nested_object_through_the_owner_is_no_edge`)
- In the same file, a name that a type other than the containers or the receiver of the file's declaration qualifies
  (`Registry.register(id)` beside a `register` of `Plugin`, Rust `Registry::create()` in the `create` of another `impl`)
  must be no use of that declaration. A type is a name with a capital, unless the file declares a value of that name
  (`DEFAULT.register(id)` of a `val DEFAULT`, an `INSTANCE`, an enum constant), also one in capitals that the file
  imports (`UI`) or that another file of its package declares (`JBUI`); a name in small letters, an imported one too,
  is a value.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_name_qualified_by_another_type_is_no_use_of_a_member_of_its_own_file`, `a_value_in_capitals_is_no_other_type`)
- In the same file, a Kotlin extension (a declaration with an extension receiver) must be a use only by a bare name,
  `this.name`, `super.name` or its receiver's type (`AirAuthoredScope::openGlobalPrompt`), of every key, one the range
  adds too: a call or reference on a local, an expression or another name (`prompt.openGlobalPrompt(frame)` on a `val prompt = AirComposerInIdeDriver(…)` beside a
  `fun AirAuthoredScope.openGlobalPrompt()`) must be no use of it, since a member of the receiver's type wins over an
  extension and only a compiler knows that type. A bare call in a lambda or a function with the extension's receiver
  as `this` (`with(scope) { openGlobalPrompt() }`) stays a use.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`an_extension_called_on_a_local_in_its_file_is_no_edge`)
- In the same file, a private member must be a use only where the compiler lets the file see it, in every form: in
  Java the whole top-level class, nested classes included, for a qualified use (`Inner.helper()`), but only its own
  type for a bare name, `this.name` or `super.name`, which outside it mean a member of the class around it (its own
  `helper`, an inherited one, a static import), and in Kotlin its own type, but the class around a companion for a
  member of the companion. Another top-level class of the Java file, and the class around a nested Kotlin object, must
  see none.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_private_member_is_a_use_in_the_top_level_java_class_and_the_class_of_its_companion`)
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
  one, a name that neither file imports or declares only when the two files are in one package or the name is of the
  common default imports (`String`, `int`, `StringBuilder`, `IllegalStateException`, Kotlin's `HashMap`); another
  default type, and a name each file reaches by the same wildcard import, count as unknown. Elsewhere a type parameter
  fits any type.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`an_override_of_an_override_is_no_edge`, `an_override_of_a_member_of_any_is_no_edge`, `an_override_with_two_supertypes_is_no_edge`, `a_java_method_of_a_class_with_supertypes_is_no_edge_for_an_override`, `an_override_in_another_module_of_an_internal_member_is_no_edge`, `an_override_of_another_receiver_or_kind_is_no_edge`, `an_override_of_a_supertype_a_scope_may_shadow_is_no_edge`, `an_override_of_a_member_of_a_type_with_supertypes_needs_its_exact_form`, `an_exact_form_needs_one_package_for_a_type_name_neither_file_binds`)
- The declarations of a key of a JVM type or a member of one must be entries told apart by kind, extension receiver and
  parameter types (with their arrays, a vararg counted), each with its origin: the baseline, or the commit that
  created it or changed its form, one entry removed and one added by a commit being the same entry in a new form. An
  override must depend on what each entry it may mean needs: the commit of the latest form of the entry that the form
  before didn't accept, with the commits back along the forms of the entry to its creation. It must depend on the
  newest commit before its own that all of them need, and on none when that is the baseline or when they need no
  commit in common: a new overload of a name the type had is an addition, an override of an overload of the baseline
  is no edge, and two overloads of a new file are both entries. A use of a function that tells no overloads apart (a
  call of a member of a type with a supertype other than `Any` or `Object`) must depend on the commit that added the
  key (the oldest origin of the entries that accept it), and on none when one entry is of the baseline. A use of a
  value or a type must depend on the step whose reach let it in (see the forms below).
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`an_override_of_a_new_overload_is_an_edge`, `an_override_of_a_member_the_range_does_not_add_is_no_edge`, `two_overloads_in_a_new_file_keep_both`, `a_new_form_of_an_entry_is_an_edge_for_the_uses_that_need_it`, `an_entry_is_its_parameter_types_with_their_arrays`, `a_call_needs_the_commit_that_added_the_key_of_its_name`, `the_cases_of_the_report`)
- A call of a member of a JVM type without a supertype other than `Any` or `Object` must tell its overloads apart by its
  arguments: it must depend, as an override, on the newest commit that every entry that accepts it needs. An entry
  must accept a call whose
  positional arguments fill its parameters in order (a vararg, the last, all of them left), whose named arguments name
  parameters not filled yet, whose last lambda fills the last parameter (a Kotlin trailing lambda) or its place, and
  that leaves only parameters with a default or a vararg, each literal fitting its parameter; a text, a number or a
  boolean fits no parameter of another of these families (text: `String`, `CharSequence`; number: the Kotlin and Java
  number types, their boxes and `Number`, an `int` for a `double` included; boolean) nor a class, an interface, an
  object or an enum that the member's file declares and doesn't shadow by an import of its name. A spread, a named
  argument before a positional one, a vararg before the last parameter, and an entry without a signature (a type, a
  property) must accept any call, and a call of a name every class has (`equals`, `hashCode`, `toString`, `clone`,
  `finalize`) must tell no overloads apart. Nor must a bare name that an import brings in (Java's `import static`, a
  Kotlin import of a member of an object) when its file shows something that may take it first: an enclosed call, a
  declaration of its name in the file, another import that may bring the name (a second import of it, as
  `import static q.Other.add`, a wildcard import, as `import static q.Other.*`, or in Kotlin a function of the default
  imports, as `error`), a type around it (or in Kotlin the companion of one) with a supertype other than `Any` or
  `Object`, or a Kotlin extension around it. An extension of the member's type that the range doesn't show
  (`fun Registry.register(id: Int)` beside a new member `register(id: Long)`) may take a call the old overloads reject;
  that is a known limit, no check rules it out.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_call_of_a_new_overload_that_the_arguments_select_is_an_edge`, `a_call_of_the_old_overload_is_no_edge`, `a_call_the_arguments_cannot_tell_apart_is_no_edge`, `overloads_of_two_commits_and_an_unknown_argument_are_no_edge`, `java_widening_accepts_a_number_for_a_double`, `an_overload_of_a_type_with_supertypes_is_no_edge_for_calls`, `a_call_of_a_name_every_class_has_is_no_edge`, `new_names_keep_their_edges_with_named_arguments_lambda_vararg_and_spread`, `a_bare_name_of_an_import_that_its_file_may_take_first_is_no_edge`, `a_call_needs_the_commit_that_added_the_key_of_its_name`, `the_cases_of_the_report`)
- A commit that keeps the identity of an entry and changes its form (the names of its parameters, a default value, a
  vararg, its reach, whether a subtype may override it, or `@JvmStatic`) must make a new step of the entry, with the
  commit for its origin and the old form for its predecessor; a change of its body alone must keep its origin. Each
  older form must be judged by the attributes of its form, and by the rest (whether it is static, whether it overrides)
  as the entry is now. A use must depend on the step whose form before rejected it: by its reach from the file of the
  use (a private member made `internal` and used by `Owner.member` from another file of its module, an `internal` one
  made public and used through an import from another module, an inner class made a public nested one and named from
  another package or module), for an override by whether the form was overridable (a member made `open`), and for a call
  by its parameters (a call that leaves out an argument a new default makes optional, passes the elements of a Java
  array made a vararg, or names a renamed parameter by its new name), and for a Java call of a member of a companion by
  the class around it by `@JvmStatic`. A use that the older form accepted too must not (the `internal` member from its
  module, a private one in its object, a call that passes the argument or a renamed parameter by position, a call of a
  member made `open`, the inner class from its package or module, an override of a member that stopped overriding, also
  in a Java class that dropped its supertype). A step must need the commits back along the forms of its entry too: a
  call that two overloads of one commit accept, one of them in a form a later commit gave it, must depend on the first,
  and a call that only the form its own commit gives accepts on the older commit of that entry whose parameter it
  passes. An older form must be no stricter than the compiler: a private member must reach the whole top-level class in
  Java, nested classes included, for a bare name too, and in Kotlin the class around a nested object or a companion (one
  with a name of its own included, which the summary doesn't tell from a nested object); an `internal` one must reach a
  file whose module no build file tells. An import of a member of a type without a supertype other than `Any` or
  `Object`, which names all its overloads, must be judged by the reach of each form, as a call by its arguments. A use
  of a function that tells no overloads apart (a call of a type with a supertype other than `Any` or `Object`, an import
  of a member of one) must need the key only, as an overload of the supertype may take it while the older form is
  private; a use of a value or a type must need the step whose reach let it in. A top-level function or property must
  have no forms, only presence. An extension of another file (`fun Panes.helper()`, `val Panes.size`) may take a use
  while the member of its name is private; that is a known limit, no check rules it out.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_parameter_added_later_keeps_the_edge_of_a_call_both_forms_accept`, `a_call_that_needs_the_new_parameter_is_an_edge_to_the_commit_that_adds_it`, `a_new_default_or_vararg_is_an_edge_for_a_call_that_needs_it`, `a_renamed_parameter_is_an_edge_for_a_call_by_its_new_name`, `a_changed_body_is_no_new_form`, `a_widened_member_is_an_edge_for_a_use_the_old_reach_forbids`, `a_widened_member_of_a_type_with_supertypes_is_no_edge_for_a_call`, `an_older_form_is_judged_as_static_as_the_entry_is_now`, `an_older_form_is_judged_as_overriding_as_the_entry_is_now`, `a_new_form_needs_the_commit_that_created_its_entry`, `a_use_of_a_form_its_own_commit_gives_needs_the_older_steps_of_the_entry`, `an_older_private_form_reaches_the_top_level_java_class_and_the_class_around_a_nested_kotlin_object`, `an_internal_older_form_reaches_a_file_of_an_unknown_module`, `an_import_of_a_member_of_a_type_without_supertypes_is_judged_by_its_reach`, `a_member_made_open_is_an_edge_for_an_override_only`, `a_changed_form_of_a_top_level_function_or_property_is_no_edge`, `a_jvm_static_added_later_is_an_edge_for_a_java_call_through_the_owner`)
- In the file of a key that was there before the range, a use must mean it only where it can mean only a member of its
  type: a bare name, `this` or `super` whose innermost type (an enum entry's body counts as one) is that type, in no
  enclosed call (`buildList { add(0, item) }`, `apply { this.add(0, item) }`, an object expression, a Java anonymous
  class) and in no Kotlin member extension function or accessor of a member extension property, whose extension
  receiver comes first (`fun MutableList<String>.fill() { add(0, "x") }`); in Kotlin, which falls back to the receivers
  further out and then to the top level when the members reject a call, also without a function or value of its name
  in a type around that type, in the companion of one of these or of it, or at the top level of the file, without a
  supertype other than `Any` or `Object` of any of these receivers, and without an import of its name, a wildcard
  import or a function of its name among the common ones of the default imports (`error`, `println`, `require`).
  `list.add(0, item)` of a `MutableList`, a bare call at the top level, `add(0, "x")` in a nested class with an `add`
  of its own or in a file with `import q.add`, `foo(1)` in an inner class whose outer class has a `foo(x: Int)` and
  `error("boom")` beside an old `error(code: Int)` are no use of a new overload beside an old one that rejects them. A
  top-level function of the package in another file, or a default function the list leaves out, may take such a call
  too; that is a known limit. A key the range adds must keep the same file as its evidence elsewhere in the file (a
  bare call in an extension of its type at the top level). A top-level type of the baseline must be meant in its file
  by a bare name, in no enclosed call, that no other declaration of the file has (a function of its name beside it)
  and, in Kotlin, no import may bring.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_call_on_another_receiver_in_the_same_file_is_no_edge`, `a_call_another_receiver_may_take_is_no_edge`, `a_call_an_import_or_a_default_import_may_take_is_no_edge`, `a_call_in_a_java_anonymous_or_nested_class_is_no_edge`, `an_overload_of_a_type_with_supertypes_is_no_edge_for_calls`, `a_constructor_call_that_needs_a_new_parameter_is_an_edge`, `a_kotlin_call_that_a_function_of_its_name_may_take_is_no_edge`)
- The key of a JVM type must hold the type itself and one entry per constructor (the implicit `()` included), each with
  its origin, as a member's overloads. A type position, an import and any use but a call must mean the type; a call of
  the type, a Kotlin constructor invocation of a supertype and Java's `new` included, its constructors, or the type too
  where its file declares an `invoke` of its companion or the range adds a function of its name that accepts the call,
  judged by the arguments that tell the constructors apart. A call must depend, as
  one of a member, on the newest commit that every constructor that accepts it needs, told apart by its arguments also
  for a type with supertypes, which declare none of them: a call that passes a parameter a commit adds, by position or
  by name, a call that only a constructor a commit adds accepts (a text for an `Int`), and one with arguments that the
  implicit `()` rejects need that commit, in the file of the type too; a type position, an import, a call that the
  constructor of the baseline accepts as well (an expression for the old `Int` beside a new `String`), and a call that
  leaves a new parameter to its default must not. Each constructor's form must be its parameters, its reach and the
  reach of its type, judged as a member's: a private constructor made `internal` must be what a call from another file
  of its module needs, a type made public what a use or a call (a qualified one too) needs from another package (Java)
  or module (Kotlin), and a use that the old reach allowed must not; a Java use must reach an `internal` Kotlin type or
  constructor from any module, and an `internal` constructor a use of a file whose module no build file tells, as its
  type does. A Kotlin call of a bare name falls back from the constructors to the functions of the name, so it must be
  judged by its arguments only where its file declares nothing else of the name and no other import may bring one (a
  wildcard import beside the import of the type), and else need the key only, as a call of a member of a type with
  supertypes; Java's `new` only where its file declares nothing else of the name (a nested type of it). A function of
  the type's name in a file of the package that the range doesn't change, an `invoke` of its companion declared in
  another file, and a member of a receiver of the name may take such a call too; that is a known limit. A use through a
  wildcard import must need the type as it came, its oldest step, never a constructor or a form: a type of its name in
  the package of the use, which another file may declare, takes the name first. A call of a nested type by its qualified
  name (`Outer.Inner(…)`, `new Outer.Inner(…)`) must tell its constructors apart as one of a top-level type. A type the
  range adds must keep the edge of every use that it or one of its constructors accepts, and of a call where the range
  adds a function of its name that accepts it, which no evidence links from another file of the package. A commit of
  whose sides one knows the constructors of a type and the other doesn't (a parse error in the class) must make no
  step of its constructors (see the clean sides below).
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_constructor_call_that_needs_a_new_parameter_is_an_edge`, `a_supertype_constructor_call_with_a_new_named_argument_is_an_edge`, `a_type_reference_is_no_edge_for_a_new_constructor_parameter`, `a_call_both_constructors_accept_is_no_edge`, `a_java_new_that_only_a_new_constructor_accepts_is_an_edge`, `an_implicit_constructor_takes_no_arguments`, `a_widened_type_is_an_edge_for_a_use_the_old_reach_forbids`, `a_widened_constructor_is_an_edge_for_a_call_the_old_reach_forbids`, `an_internal_constructor_of_no_known_module_takes_a_call`, `a_kotlin_call_that_a_function_of_its_name_may_take_is_no_edge`, `a_call_through_a_wildcard_import_needs_only_the_type`, `a_call_of_a_nested_class_that_needs_a_new_parameter_is_an_edge`, `a_new_type_keeps_its_edges_for_calls_and_type_positions`, `a_call_that_a_function_of_the_name_of_a_new_type_may_take_needs_the_type`, `a_class_with_a_parse_error_in_it_is_no_step_of_its_constructors`)
- A top-level function or property of the name of a type (a factory `fun Foo(): Foo`) must share its key and stay
  only present or absent beside the entries of the type, in the type's file too: a commit that adds or removes it must
  keep the type's entries, one that changes the type must keep its entries, and one that removes the type from its
  file must remove the type's entries though the function keeps the key. It may take a call of the name that its
  parameters as its file has them now accept, judged as an `invoke` of the companion by the arguments that tell the
  constructors apart: a call that a constructor and such a function accept must depend on what both need, one that
  only the function accepts on the function's commit, also from another file of the package, which only the type
  links, and one that it rejects (a parameter a commit adds to the constructor, passed by name) on what the
  constructors need. Only a function that the use sees may take the call: by an import, its qualified name or its
  file, or of the package in the use's module where its reach lets it in, never a private one of another file, one of
  another module or language, nor any for a Java `new`. No type position means it. A type of the baseline that the
  range doesn't change is no entry, so such a call from another file of the package has no evidence; that is a known
  limit.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_factory_function_and_its_type_share_a_key_without_losing_either`, `a_type_removed_beside_a_function_of_its_name_leaves_no_entries`, `a_function_of_the_name_of_a_type_takes_only_a_call_its_parameters_accept`, `a_function_of_the_name_of_a_type_comes_and_goes_beside_it`, `a_function_of_the_name_of_a_type_takes_a_call_by_its_parameters_now`, `a_function_of_the_name_of_a_type_that_the_use_does_not_see_takes_no_call`)
- The key of a top-level function must hold the entries of each file of the range that adds it, so that a call of an
  overload two files add is no edge; the entries must follow their declarations to the file a commit renames or moves
  them to, each with its origin to a declaration of its identity, so that such a call stays no edge when a commit
  moves both overloads into one file; where the parser can't read the declarations there (a degraded file, a parse
  error), the entries must keep their forms and origins in that file until a commit reads it again; and a removal must
  drop the entries of its file only, also the removal of an overload of the baseline.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`top_level_overloads_in_two_files_are_both_entries`, `a_removal_of_a_top_level_function_drops_the_entries_of_its_file_only`, `overloads_a_move_brings_into_one_file_keep_their_origins`, `entries_go_with_their_file_into_a_version_the_parser_cannot_read`)
- The entries of a key must change only by clean sides: when a side of a file that declares the key is degraded, a
  declaration of the key on either side is inside a parse error, or two declarations of one side are the same entry
  (two constructors of a type included), the key must only be present or absent, as a top-level function, a Rust or a
  TypeScript key always is, and so must a type whose constructors one side of a commit knows and the other doesn't; a
  type then loses the constructors the range gave it, which might miss what the commit gave them, and takes any call,
  also from its first commit; a constructor that a later commit on clean sides finds must be as old as the type, which
  it needs.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_before_side_in_error_changes_no_state`, `an_entry_is_its_parameter_types_with_their_arrays`, `constructors_the_summary_cannot_tell_apart_are_no_form`, `a_type_whose_sides_a_commit_cannot_match_takes_any_call`, `a_constructor_that_comes_while_its_type_takes_any_call_is_as_old_as_the_type`)
- A commit that removes a declaration must depend on the older commits that removed its uses, an override of it
  included.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_removal_needs_the_commit_that_removed_the_uses`, `a_removal_needs_the_commit_that_removed_its_override`)
- A declaration that moves within its namespace in one commit must be neither added nor removed; a use that two
  commits' declarations could mean must be no edge, its own commit's included, but a member its own commit adds or
  changes must not count for a bare name outside the member's type in its file, which only an implicit receiver makes
  a use of it; a commit must not depend on itself; a file with a twentieth of its bytes in parse errors must make no
  edge.
  [@test] ../../crates/sync-core/src/symbol_dependencies_test.rs (`a_commit_needs_no_edge_to_itself_a_moved_declaration_is_no_addition_and_two_owners_are_ambiguous`, `a_use_its_own_commit_may_mean_is_no_edge`, `a_member_of_its_own_commit_is_no_origin_of_an_unqualified_use_outside_its_type`)
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
