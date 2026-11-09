---
name: Symbol Dependencies
description: Requirements for the syntax summaries of the source files the commits change, their cache, and the dependencies by symbols between commits that they give; the measurement that decides whether they order branches.
style: plain-1
targets:
  - crates/code-symbols/src/extract.rs
  - crates/code-symbols/src/summary.rs
  - crates/code-symbols/src/language.rs
  - crates/code-symbols/src/summary_builder.rs
  - crates/code-symbols/src/jvm.rs
  - crates/code-symbols/src/java/summary.rs
  - crates/code-symbols/src/kotlin/summary.rs
  - crates/code-symbols/src/csharp/summary.rs
  - crates/trial-check/src/dotnet_resolution.rs
  - crates/code-symbols/src/starlark/summary.rs
  - crates/trial-check/src/bazel_resolution.rs
  - crates/code-symbols/src/rust/summary.rs
  - crates/code-symbols/src/js/mod.rs
  - crates/code-symbols/src/js/summary.rs
  - crates/trial-check/src/symbol_scan.rs
  - crates/sync-core/src/config_snapshot.rs
  - crates/trial-check/src/symbol_dependencies.rs
  - crates/trial-check/src/module_roots.rs
  - crates/trial-check/src/dangling_references.rs
  - crates/trial-check/src/declaration_checks.rs
  - crates/object-cache/src/blob_summary.rs
  - crates/git-ops/src/object_texts.rs
  - crates/trial-check/src/diff_scan.rs
  - crates/bd-rewrite/src/symbol_stats.rs
---

# Symbol Dependencies

Status: Implemented
Date: 2026-10-18

## Purpose

A commit that calls a function another commit added in another file merges cleanly into a branch without that commit,
and the branch fails only when it is compiled. The line dependencies don't see it. Branch Deck reads the source files
the commits change (Kotlin, Java and C# by ports of the Kotlin compiler's, IntelliJ's and Roslyn's own parsers,
TypeScript and JavaScript by oxc's, Rust by rust-analyzer's), and a commit that uses a name another commit of the range adds, with evidence that the use
means that declaration, depends on it by symbols. An override of a member another commit adds, a call of the overload
its arguments select, and a use that only a new form of a member or a constructor accepts (a wider reach, `open`, a
default, a new or renamed parameter) count as uses of that commit's declaration. Such an edge orders branches; it never joins them. See [Symbol
Dependencies](../../../../docs/design/symbol-dependencies.md).

## Goals

- See the compile dependencies between commits that git's merges don't see, without building the project.
- Cost the blobs the commits change, not the repository: a monorepo of a million files stays as cheap as a small one.
- Read each blob once: a summary is cached by blob id.

## Non-goals

- Exact name resolution: no build, no language server, no index of the tree. A use without evidence is no edge.
- Python, Go, Groovy and Scala: their files keep the line dependencies only.
- The preprocessor configurations of a C# project: a file is read without symbols defined, and a `global using` of
  another file is not seen.

## Requirements

### Requirement: Summaries: A summary holds the package, imports, declarations and references

The summary of a file MUST hold its package, its imports (Rust `use` and `pub use`, Kotlin aliases, TypeScript
imports and re-exports), its declarations with their kind, containers and reach, and its references.

#### Scenario: The summaries of the fixtures
- **WHEN** a fixture of each language is summarized
- **THEN** its package, imports, declarations and references are those of its golden file
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

#### Scenario: A Rust use is an import
- **WHEN** a Rust file has a `use`
- **THEN** the summary holds the import, and it binds the name
  [@test] crates/code-symbols/src/rust_test.rs (`a_rust_use_binds_the_name`)

#### Scenario: TypeScript imports and re-exports
- **WHEN** a TypeScript file imports and re-exports names
- **THEN** the summary holds them as imports
  [@test] crates/code-symbols/src/js/summary_test.rs (`imports`)

### Requirement: Summaries: References leave out comments, strings, local names, labels and keys

The references of a summary MUST be without the words of comments and strings (but with the names of
interpolations), locally bound names, labels and keys.

#### Scenario: A word in a comment
- **WHEN** a word of a KDoc comment is the name of a declaration
- **THEN** it is no reference
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_word_in_kdoc_is_no_reference`)

#### Scenario: Locally bound names
- **WHEN** a name is bound locally (a local `val`, a parameter, a pattern's variable, a local function, a query's
  range variable, a local of top-level statements, the name of a class expression)
- **THEN** a use of it is no reference
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_local_val_shadows_and_is_no_reference`)
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`locals_parameters_patterns_and_query_variables_are_bound`, `top_level_statements_bind_their_locals`)
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_class_expression_binds_its_name`)

#### Scenario: Interpolations, labels and keys
- **WHEN** a string has an interpolation, or the code has a label or a key
- **THEN** the names of the interpolation are references
- **AND** the label and the key are none

### Requirement: Summaries: A Java file is read as IntelliJ's parser reads it

A Java file MUST be read as IntelliJ's own parser reads it, at the highest language level: an annotated vararg
(`Object @NotNull ... params`), `case null, default`, a module import, a guarded record pattern, comments inside an
import, `Outer.super::m`, a compact source file, an annotated receiver parameter, an annotated supertype and a vararg
component of a record (a public field).

#### Scenario: Annotated parameters and supertypes
- **WHEN** a Java file has an annotated vararg (`Object @NotNull ... params`), an annotated receiver parameter or an
  annotated supertype
- **THEN** the vararg is a parameter, the receiver parameter is none, and the supertype is a supertype
  [@test] crates/code-symbols/src/java/summary_test.rs (`an_annotated_vararg_is_a_parameter`, `an_annotated_receiver_parameter_is_no_parameter`, `an_annotated_supertype_is_a_supertype`)

#### Scenario: Case labels
- **WHEN** a `switch` has `case null, default` or a guarded record pattern
- **THEN** the `default` names nothing and a case label is a pattern or a constant
  [@test] crates/code-symbols/src/java/summary_test.rs (`the_default_of_a_case_label_is_no_name`, `a_case_label_is_a_pattern_or_a_constant`)

#### Scenario: Imports
- **WHEN** a Java file has a module import or comments inside an import
- **THEN** the module import imports no name, and the import with comments is read as IntelliJ reads it
  [@test] crates/code-symbols/src/java/summary_test.rs (`a_module_import_imports_no_name`, `an_import_with_comments_inside`)

#### Scenario: A qualified super, a compact source file and a record's vararg
- **WHEN** a Java file has `Outer.super::m`, is a compact source file, or has a vararg component of a record
- **THEN** the method reference is read, the compact source file declares at the top level, and the vararg component
  is a public field
  [@test] crates/code-symbols/src/java/summary_test.rs (`a_method_reference_of_a_qualified_super`, `a_compact_source_file_declares_at_the_top_level`, `a_vararg_record_component_is_a_public_field`)

### Requirement: Summaries: A Java member of an expression has an empty qualifier

A member of a call's result (`a.b().c()`) MUST have an empty qualifier, as of any expression, and so MUST a member of a
chain of names longer than any path (64 segments), so that the qualifiers of a chain take linear space.

#### Scenario: A member of a call's result
- **WHEN** a Java file calls `a.b().c()`
- **THEN** `c` has an empty qualifier
  [@test] crates/code-symbols/src/java/summary_test.rs (`a_member_of_a_call_has_an_empty_qualifier`)

#### Scenario: A chain longer than any path
- **WHEN** a chain of names is longer than 64 segments
- **THEN** its members have an empty qualifier and the qualifiers of the chain take linear space
  [@test] crates/code-symbols/src/java/summary_test.rs (`a_long_chain_of_names_takes_linear_space`)

### Requirement: Summaries: A C# file is read as Roslyn's parser reads it

A C# file MUST be read as Roslyn's own parser reads it, at `LanguageVersion.Latest` and without preprocessor symbols:
a branch of `#if` it does not take is no code.

#### Scenario: A branch of `#if` without its symbol
- **WHEN** a C# file has an `#if` of a symbol
- **THEN** the branch the parser does not take without preprocessor symbols is not read
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_branch_of_if_the_configuration_does_not_take_is_not_read`)

#### Scenario: The C# fixtures
- **WHEN** the C# fixtures are summarized
- **THEN** each matches its golden file
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: The package of a C# file is the namespace of its first type

The package of a C# file MUST be the namespace of its first type, nested block namespaces joined.

#### Scenario: Nested block namespaces
- **WHEN** the first type of a file lies in a block namespace inside another
- **THEN** the package is the two namespaces joined
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`the_package_is_the_first_namespace_with_the_namespaces_around_it`)

### Requirement: Summaries: C# usings import their path, an alias its target

`using N;` and `using static N.T;` MUST import everything of their path, an alias `using X = N.T;` its target (an
`Import` reference to it), and `global using` MUST import as any.

#### Scenario: A using, a static using, an alias
- **WHEN** a file has `using N;`, `using static N.T;` and `using X = N.T;`
- **THEN** the first two import everything of their path, and the alias imports its target, an `Import` reference to it
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`usings_import_everything_of_their_path_and_an_alias_its_target`)

#### Scenario: A global using
- **WHEN** a file has a `global using`
- **THEN** it imports as any using does

### Requirement: Summaries: A C# file declares its types, members and record parameters

A C# file MUST declare its types (a nested one static), methods, properties, events, the declarators of a field (a
`const` and a `static readonly` one constants), enum members and a record's positional parameters (public properties).

#### Scenario: A nested type and the members of a type
- **WHEN** a type holds a nested type, methods, properties, events, fields and enum members
- **THEN** each is declared, the nested type static, a `const` and a `static readonly` field constants
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_nested_type_is_static_and_a_member_is_private_without_a_modifier`)

#### Scenario: A record's positional parameters
- **WHEN** a record has positional parameters
- **THEN** each is a public property
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_record_has_its_positional_parameters_as_properties_and_its_primary_constructor`)

### Requirement: Summaries: A C# declaration has the reach of C#'s modifiers and defaults

A C# declaration MUST have the reach of C#'s modifiers and defaults: a member private, a type of a namespace
internal, a member of an interface or an enum public, `file` of the file.

#### Scenario: A member without a modifier
- **WHEN** a member of a class has no modifier
- **THEN** it is private
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_nested_type_is_static_and_a_member_is_private_without_a_modifier`)

#### Scenario: The defaults of a type, an interface, an enum and `file`
- **WHEN** a type of a namespace, a member of an interface or an enum, or a `file` type has no other modifier
- **THEN** the type is internal, the member public, the `file` type of the file

### Requirement: Summaries: A C# extension has its receiver

An extension method (`this T` of a static method) and a member of an extension block MUST be declared with the
receiver `T`.

#### Scenario: An extension method and an extension block
- **WHEN** a static method takes `this T`, or an extension block of `T` has a member
- **THEN** each is declared with the receiver `T`
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`an_extension_method_and_a_member_of_an_extension_block_have_their_receiver`)

### Requirement: Summaries: A C# type declares its constructors

A C# type MUST declare its constructors: a primary one, public, its declared ones, the implicit public `()` of a class
or a struct; none of a static class, an interface or an enum.

#### Scenario: The primary constructor of a record
- **WHEN** a record has positional parameters
- **THEN** it has a public primary constructor of them
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_record_has_its_positional_parameters_as_properties_and_its_primary_constructor`)

#### Scenario: A static class, an interface and an enum
- **WHEN** a static class, an interface or an enum is declared
- **THEN** it has no constructor
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_static_class_an_interface_and_an_enum_have_no_constructors`)

#### Scenario: A class without a declared constructor
- **WHEN** a class or a struct declares no constructor
- **THEN** it has the implicit public `()`

### Requirement: Summaries: C# overrides, tests, the entry point and unstable API

A C# `override` and an explicit implementation of an interface's member MUST be overriding; xUnit's, NUnit's and
MSTest's test attributes MUST make a test, a static `Main` the entry point, and `[Experimental]` MUST make a
declaration unstable.

#### Scenario: The marks of C# declarations
- **WHEN** a C# file has an `override`, an explicit implementation, a method under a test attribute of xUnit, NUnit
  or MSTest, a static `Main` and an `[Experimental]` declaration
- **THEN** the first two are overriding, the method a test, `Main` the entry point, the declaration unstable

### Requirement: Summaries: A C# attribute names its class too

An attribute's name MUST be a reference of its class too (`[Obsolete]`, `ObsoleteAttribute`).

#### Scenario: An attribute without its suffix
- **WHEN** a declaration has `[Obsolete]`
- **THEN** its references name `Obsolete` and `ObsoleteAttribute`
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`an_attribute_names_its_class_too`)

### Requirement: Summaries: C# names that name nothing, and names by their value

A documentation comment, a label, a named argument, `var`, `nameof` and `_` MUST name nothing, and a name MUST compare
by its value (`@event` is `event`).

#### Scenario: A documentation comment
- **WHEN** a declaration has a documentation comment
- **THEN** its words name nothing, and the declaration starts at it
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_documentation_comment_names_nothing_and_starts_its_declaration`)

#### Scenario: A verbatim identifier
- **WHEN** a file names `@event`
- **THEN** it is the name `event`
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_verbatim_identifier_is_its_value`)

#### Scenario: Labels, named arguments, `var`, `nameof` and `_`
- **WHEN** a file has a label, a named argument, `var`, `nameof` or `_`
- **THEN** none of them names anything

### Requirement: Summaries: A C# token the parser skipped or found missing is an error

A token the parser skipped or found missing MUST count as an error.

#### Scenario: A missing token
- **WHEN** the parser finds a token missing
- **THEN** it counts as an error and marks its declaration
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_missing_token_counts_as_an_error_and_marks_its_declaration`)

#### Scenario: A deep chain and nesting past Roslyn's
- **WHEN** a file holds a deep chain of calls, or nesting past what Roslyn's parser reads
- **THEN** the chain takes no stack per level, and the nesting past Roslyn's is a skipped token, read as an error
  [@test] crates/code-symbols/src/csharp/summary_test.rs (`a_deep_chain_and_deep_nesting_take_no_stack_per_level`)

### Requirement: Summaries: A TypeScript or JavaScript file is read in the dialect of its extension

A TypeScript, TSX or JavaScript file MUST be read as oxc's parser reads it in the dialect of its extension: `.ts`,
`.mts` and `.cts` as TypeScript, where `<T>x` asserts a type, `.tsx` as TSX, `.js`, `.mjs`, `.cjs` and `.jsx` as
JavaScript with JSX; a summary MUST be kept per dialect.

#### Scenario: The extension chooses the dialect
- **WHEN** the same text is read as `.ts` and as `.tsx`
- **THEN** in `.ts` `<T>x` asserts a type, in `.tsx` it is JSX, and each dialect has its own summary
  [@test] crates/code-symbols/src/js/summary_test.rs (`the_extension_chooses_the_dialect`)

#### Scenario: JavaScript with JSX
- **WHEN** a `.js`, `.mjs`, `.cjs` or `.jsx` file is read
- **THEN** it is read as JavaScript with JSX
  [@test] crates/code-symbols/src/js/summary_test.rs (`javascript`)

#### Scenario: The TypeScript and JavaScript fixtures
- **WHEN** the fixtures are summarized
- **THEN** each matches its golden file
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: TypeScript declares its types, namespaces and modules

A TypeScript file MUST declare classes, interfaces, enums and their members, type aliases with the members of the
object types they hold, and a namespace or an ambient module (`namespace a.b.c` one module named `c`).

#### Scenario: Types and their members
- **WHEN** a file declares classes, interfaces, enums and type aliases of object types
- **THEN** each is declared with its members
  [@test] crates/code-symbols/src/js/summary_test.rs (`declarations`)

#### Scenario: A dotted namespace
- **WHEN** a file declares `namespace a.b.c`
- **THEN** it is one module named `c`
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_dotted_namespace_is_one_module_of_its_last_name`)

#### Scenario: An ambient module
- **WHEN** a file declares an ambient module
- **THEN** it is a module, named by its string
  [@test] crates/code-symbols/src/js/summary_test.rs (`an_ambient_module_is_named_by_its_string`)

### Requirement: Summaries: TypeScript declares functions, variables and class members

A TypeScript or JavaScript file MUST declare functions and their signatures, the names a variable declarator binds
at the top level or in a namespace (an arrow or a function value a function, an `UPPER_CASE` `const` a constant), the
members of a class (a getter and a setter values, no constructor, as signatures too) and a constructor's parameter
property.

#### Scenario: Functions, variables and parameter properties
- **WHEN** a file declares functions, top-level variables of arrow, function and other values, an `UPPER_CASE`
  `const`, and a constructor with a parameter property
- **THEN** the functions and the arrow or function values are functions, the `const` a constant, the parameter
  property a member
  [@test] crates/code-symbols/src/js/summary_test.rs (`declarations`)

#### Scenario: Signatures of a constructor and an accessor
- **WHEN** an interface has a constructor signature and an accessor signature
- **THEN** the constructor signature declares nothing and the accessor signature is a value
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_constructor_signature_declares_nothing_and_an_accessor_signature_is_a_value`)

### Requirement: Summaries: A TypeScript member is named by an identifier, never by a computed key

A member MUST be named by an identifier, a private name, a string or a computed identifier (`[k]`), never by another
computed key.

#### Scenario: Computed keys
- **WHEN** a class has members of `[k]` and of another computed key
- **THEN** only the computed identifier names a member
  [@test] crates/code-symbols/src/js/summary_test.rs (`only_a_computed_identifier_names_a_member`)

### Requirement: Summaries: A TypeScript export makes a declaration public

`export`, `export declare` and `export { name }` MUST make a declaration public.

#### Scenario: An exported ambient declaration
- **WHEN** a declaration is under `export declare`
- **THEN** it is public
  [@test] crates/code-symbols/src/js/summary_test.rs (`an_exported_ambient_declaration_is_public`)

#### Scenario: An export of a name
- **WHEN** a file has `export { name }` of a declaration
- **THEN** the declaration is public

### Requirement: Summaries: TypeScript calls, decorators and import types are references

A call with type arguments MUST be a call, the references of a decorator MUST belong to the declaration it decorates,
and a type of `import("m")` MUST be a type.

#### Scenario: The references of a file
- **WHEN** a file uses names
- **THEN** its references are those uses
  [@test] crates/code-symbols/src/js/summary_test.rs (`references`)

#### Scenario: A call with type arguments
- **WHEN** a file calls `f<T>()`
- **THEN** it is a call of `f`
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_call_with_type_arguments_is_a_call`)

#### Scenario: A decorator
- **WHEN** a decorator stands before a declaration
- **THEN** its references belong to that declaration
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_decorator_is_of_the_declaration_it_decorates`)

#### Scenario: An import type
- **WHEN** a file names a type of `import("m")`
- **THEN** it is a reference of a type
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_type_of_an_import_type_is_a_reference`)

### Requirement: Summaries: TypeScript names that name nothing, and the scope of `infer`

The name of an index signature, the label of a named tuple member, a type predicate's parameter, `as const` and the
alias of `import x = a.b` MUST name nothing; what `infer` declares MUST be seen in its conditional type and a function
type's parameters only in it.

#### Scenario: Names of no declaration
- **WHEN** a file has an index signature, a named tuple member, a type predicate, `as const` and `import x = a.b`
- **THEN** their names, the label, the parameter, `const` and the alias are no references
  [@test] crates/code-symbols/src/js/summary_test.rs (`names_that_name_no_declaration_are_no_references`)

#### Scenario: `infer` and a function type's parameters
- **WHEN** a conditional type has `infer U`, or a function type has parameters
- **THEN** `U` is seen in its conditional type, and the parameters only in the function type
  [@test] crates/code-symbols/src/js/summary_test.rs (`infer_is_seen_in_its_conditional_type`)

### Requirement: Summaries: A TypeScript declaration starts at its doc comment and has the parse's header

A declaration MUST start at its doc comment (`/**`, not a `///` directive), its decorators or its `export`, and its
header MUST be the parse's tokens before its body or initializer, a type parameter's default kept.

#### Scenario: The start of a declaration
- **WHEN** a declaration has a `/**` comment, decorators or `export` before it, or a `///` directive
- **THEN** it starts at the comment, the decorators or the `export`, never at the directive
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_declaration_starts_at_its_doc_comment_and_decorators`)

#### Scenario: The header
- **WHEN** a function has a type parameter with a default and a body
- **THEN** its header is the parse's tokens before the body, the default kept
  [@test] crates/code-symbols/src/js/summary_test.rs (`a_header_is_the_tokens_of_the_parse`)

### Requirement: Summaries: TypeScript errors are what the parse recovered from

The errors MUST be what a recovery of the parse blanked and the labels of its syntax errors, never of an early error,
which leaves the tree whole (`js_syntax::is_syntax_error`, as accio reads them).

#### Scenario: A recovered syntax error
- **WHEN** the parse recovers from a syntax error
- **THEN** the errors are what the recovery blanked and the labels of the error
  [@test] crates/code-symbols/src/js/summary_test.rs (`errors_are_what_the_parse_recovered_from`)

#### Scenario: An early error
- **WHEN** the file has an early error only
- **THEN** it is no error of the file
  [@test] crates/code-symbols/src/js/summary_test.rs (`an_early_error_is_no_error_of_the_file`)

### Requirement: Summaries: A TypeScript file no recovery saves, or nested deep, is read safely

A file no recovery saves MUST be read with every byte in error, one nested deeper than the parse reads MUST be
truncated, and any nesting below that MUST be read on a 2 MiB thread.

#### Scenario: Deep nesting
- **WHEN** a file is nested deeper than the parse reads, or nested deep below that
- **THEN** the first is truncated, and the second is read on a 2 MiB thread
  [@test] crates/code-symbols/src/js/summary_test.rs (`deep_nesting_is_read_on_a_small_stack`)

#### Scenario: A file no recovery saves
- **WHEN** no recovery of the parse saves a file
- **THEN** it is read with every byte in error

### Requirement: Summaries: A Rust file is read as rust-analyzer's parser reads it

A Rust file MUST be read as rust-analyzer's parser reads it (psi-syntax's `rust-syntax`, edition 2021), the parse
accio reads it with.

#### Scenario: The types of paths
- **WHEN** a Rust function names `Vec<_>` and calls `Vec::<u8>::new()`
- **THEN** the references are those of rust-analyzer's tree, and `_` names nothing
  [@test] crates/code-symbols/src/rust/summary_test.rs (`the_types_of_paths`)

#### Scenario: The Rust fixtures
- **WHEN** the Rust fixtures are summarized
- **THEN** each matches its golden file
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: A Rust header is the tokens of the item as lexed

The header of a function, a field, a constant or a static MUST be the tokens before its body or its `=` as
rust-analyzer lexes them (`'a` one token, the `;` of `[T; N]`, the `=` of `Iterator<Item = T>` and an ABI string kept,
the `,` that ends a `where` and the `;` that ends the item dropped), without attributes, comments and visibility.

#### Scenario: The header of an item
- **WHEN** a function, a field, a constant or a static has a lifetime, an array type, an associated type binding, an
  ABI string, a `where` clause, attributes, comments and a visibility
- **THEN** its header is the tokens before its body or its `=` as lexed, `'a` one token, the `;` of `[T; N]`, the `=`
  of `Iterator<Item = T>` and the ABI string kept
- **AND** the `,` that ends the `where`, the `;` that ends the item, the attributes, the comments and the visibility
  dropped
  [@test] crates/code-symbols/src/rust/summary_test.rs (`a_header_is_the_tokens_of_the_item_as_lexed`)

### Requirement: Summaries: The tokens of a Rust macro are read as outside it

Among the tokens of a macro, a member after a group or a literal (`f(x).unwrap()`) MUST have an empty qualifier, as
outside a macro, and keywords, lifetimes, `_`, literals and metavariables (`$x`) MUST name nothing; the names of a
macro in a pattern MUST bind there.

#### Scenario: A member after a group
- **WHEN** a macro holds `f(x).unwrap()`
- **THEN** `unwrap` has an empty qualifier
  [@test] crates/code-symbols/src/rust/summary_test.rs (`a_member_after_a_group_in_a_macro_is_of_an_expression`)

#### Scenario: Tokens that are no names
- **WHEN** a macro holds keywords, lifetimes, `_`, literals and metavariables (`$x`), or sits in a pattern
- **THEN** none of those tokens names anything, and the names of the macro in the pattern bind there
  [@test] crates/code-symbols/src/rust/summary_test.rs (`the_tokens_of_a_macro_that_are_no_names`)

### Requirement: Summaries: Rust arm patterns, typed `self` and root paths

A capitalized name alone as the pattern of a match arm (`None`, `MAX`) MUST be a reference, a typed `self`
(`self: Box<Self>`) MUST make an instance member, and a path from the crate root (`::core::mem`) MUST keep its
segments.

#### Scenario: A capitalized pattern of an arm
- **WHEN** a match arm's pattern is `None` or `MAX` alone
- **THEN** it is a reference
  [@test] crates/code-symbols/src/rust/summary_test.rs (`a_capitalized_pattern_of_an_arm_is_a_reference`)

#### Scenario: A typed `self`
- **WHEN** a method takes `self: Box<Self>`
- **THEN** it is an instance member
  [@test] crates/code-symbols/src/rust/summary_test.rs (`a_typed_self_makes_an_instance_member`)

#### Scenario: A path from the root
- **WHEN** a file names `::core::mem`
- **THEN** the reference keeps its segments
  [@test] crates/code-symbols/src/rust/summary_test.rs (`a_path_from_the_root_keeps_its_segments`)

### Requirement: Summaries: The references of a Rust attribute are of the declaration around its item

An attribute's references MUST be of the declaration around its item.

#### Scenario: An attribute of an item
- **WHEN** an item inside a declaration has an attribute that names something
- **THEN** that reference is of the declaration around the item
  [@test] crates/code-symbols/src/rust/summary_test.rs (`the_references_of_an_attribute_are_of_the_container_around_its_item`)

### Requirement: Summaries: A Rust error marks the declarations it lies before the name of

A declaration MUST be `in_error` when an error lies between the start of its node, after its attributes, and its
name; the error bytes MUST be those of the outermost `ERROR` nodes and one per error of no bytes.

#### Scenario: An error before a name
- **WHEN** an error lies between the start of a declaration's node, after its attributes, and its name
- **THEN** the declaration is `in_error`
- **AND** the error bytes are those of the outermost `ERROR` nodes and one per error of no bytes
  [@test] crates/code-symbols/src/rust/summary_test.rs (`an_error_marks_the_declarations_it_lies_before_the_name_of`)

### Requirement: Summaries: A Rust file nested too deep is truncated

A file nested deeper than `rust_syntax::nesting::MAX_LEVELS` MUST be `truncated`, never overflow the stack.

#### Scenario: Deep nesting
- **WHEN** a file is nested deeper than `rust_syntax::nesting::MAX_LEVELS`
- **THEN** its summary is `truncated`, read on a small stack without an overflow
  [@test] crates/code-symbols/src/rust/summary_test.rs (`deep_nesting_is_summarized_on_a_small_stack`)

### Requirement: Summaries: A Java member's reach follows the modifiers of IntelliJ's PSI

The reach of a Java member and whether a subtype may override it MUST follow the modifiers IntelliJ's PSI gives it,
the implicit ones included (`PsiModifierListImpl.hasModifierProperty`): a member of an interface is public unless it
is `private` (a private method of an interface reaches its container alone), an enum none of whose constants has a
body is final as a record is (none of its methods may be overridden), and a class is an annotation type only by the
`@` right after its modifiers.

#### Scenario: Implicit modifiers
- **WHEN** an interface has a member without `private` and a `private` method, an enum none of whose constants has a
  body and a record have methods, and a class has `@` right after its modifiers
- **THEN** the member of the interface is public, the private method reaches the interface alone, the methods of the
  enum and the record may not be overridden, and the class is an annotation type
  [@test] crates/code-symbols/src/java_test.rs (`the_implicit_modifiers_give_the_reach_and_whether_a_member_may_be_overridden`)

### Requirement: Summaries: A Kotlin or Java function has its parameter types

The summary of a Kotlin or Java file MUST hold the parameter types of each function (the last names of each type, `?`,
a default value, `vararg`, the dimensions of an array with a vararg counted as one; a type parameter as any type).

#### Scenario: A signature
- **WHEN** a function has qualified, nullable, defaulted, vararg, array and type-parameter parameters
- **THEN** its signature holds the last names of each type, `?`, the default, `vararg` and the dimensions, a vararg
  counted as one, and a type parameter as any type
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_signature_has_the_parameter_types`)
  [@test] crates/code-symbols/src/java_test.rs (`a_signature_has_the_parameter_types`)

### Requirement: Summaries: A Kotlin or Java member knows how it overrides and how the JVM sees it

The summary MUST hold whether a member overrides, whether a subtype may override it and whether a Kotlin member is
`@JvmStatic`, `@JvmField`, `lateinit`, renamed by `@JvmName` (also `@get:` and `@set:`, and a name in backticks,
`` @`JvmName` ``) or `@JvmSynthetic`, the reach of the setter of a `var` (`private set`), and the `@file:JvmName` of a
Kotlin file.

#### Scenario: Overriding
- **WHEN** a member is `override`, `open`, `abstract` or final, of a class, an interface or an object
- **THEN** the summary says whether it overrides and whether a subtype may override it
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_member_knows_whether_it_overrides_and_can_be_overridden`)
  [@test] crates/code-symbols/src/java_test.rs (`a_member_knows_whether_it_overrides_and_can_be_overridden`)

#### Scenario: The JVM marks of a Kotlin member
- **WHEN** a Kotlin member is `@JvmStatic`, `@JvmField`, `lateinit`, `@JvmName` (by `@get:`, `@set:` or
  `` @`JvmName` `` too) or `@JvmSynthetic`, a `var` has a `private set`, or the file has `@file:JvmName`
- **THEN** the summary holds each
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_member_knows_whether_it_is_jvm_static`)

### Requirement: Summaries: A Kotlin or Java type has its direct supertypes

The summary MUST hold the direct supertypes of each type without type arguments, a Kotlin function type as an empty
path.

#### Scenario: Supertypes
- **WHEN** a type extends a generic type and a Kotlin function type
- **THEN** its supertypes are the generic type without type arguments and an empty path
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: An override is a reference per direct supertype

An `override` or `@Override` member MUST be one `Override` reference per direct supertype of the class around it (an
object expression, an anonymous class and the body of an enum entry included, also on one line), qualified by the
path of the supertype, with its kind, receiver and parameter types as its shape.

#### Scenario: An override in a class of two supertypes
- **WHEN** a class of two supertypes has an `override` or `@Override` member
- **THEN** it is an `Override` reference per supertype, qualified by its path, with its kind, receiver and parameter
  types as its shape
  [@test] crates/code-symbols/src/kotlin_test.rs (`an_override_is_a_reference_per_supertype`)
  [@test] crates/code-symbols/src/java_test.rs (`an_override_is_a_reference_per_supertype`)

#### Scenario: An override in a one-line object expression
- **WHEN** an object expression on one line overrides a member of its supertype
- **THEN** the override is a reference of that supertype
  [@test] crates/code-symbols/src/kotlin_test.rs (`an_override_in_a_one_line_object_expression_overrides_its_supertype`)

### Requirement: Summaries: A function a Kotlin or Java test framework runs is a test

A function a test framework runs MUST be a test (`test`), for the tests a review names: of Kotlin and Java one
annotated `@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory` or `@TestTemplate` by the last name of the
annotation, whatever its qualifier, or a JUnit 3 `testName()` (`test` and a capital or `_`) without parameters in a
class named `…Test`, `…Tests` or `…TestCase` or that extends a `…TestCase`.

#### Scenario: A Kotlin test
- **WHEN** a Kotlin function is annotated `@Test` or another test annotation of any qualifier, or is a JUnit 3
  `testName()` of a test class
- **THEN** it is a `test`
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_function_a_test_framework_runs_is_a_test`)

#### Scenario: A Java test
- **WHEN** a Java method is annotated `@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory` or
  `@TestTemplate`, or is a JUnit 3 `testName()` without parameters in a class named `…Test`, `…Tests` or `…TestCase`
  or that extends a `…TestCase`
- **THEN** it is a `test`
  [@test] crates/code-symbols/src/java_test.rs (`a_method_a_test_framework_runs_is_a_test`)

### Requirement: Summaries: A function a Rust test attribute marks is a test, a TypeScript call none

A Rust function MUST be a test under `#[test]`, `#[tokio::test]`, `#[test_log::test]` or `#[rstest]`, also below a doc
comment or another attribute, never under `#[cfg(test)]` alone. A TypeScript `test("…")` is a call, no declaration,
and so MUST be no test.

#### Scenario: Rust test attributes
- **WHEN** a Rust function is under `#[test]`, `#[tokio::test]`, `#[test_log::test]` or `#[rstest]`, below a doc
  comment or another attribute, or under `#[cfg(test)]` alone
- **THEN** it is a test under the first four and none under `#[cfg(test)]` alone
  [@test] crates/code-symbols/src/rust_test.rs (`a_function_under_a_test_attribute_is_a_test`)

#### Scenario: A TypeScript test
- **WHEN** a TypeScript file calls `test("…")`
- **THEN** it is a call, no declaration, and no test

### Requirement: Summaries: A Kotlin top-level `main` is an entry point

A function the JVM starts a program with MUST be an entry point (`entry_point`), for the API a review names: a Kotlin
top-level `fun main` without a receiver or type parameters that takes nothing, one `Array<String>` or a `vararg` of
`String`, `suspend` or not, returning `Unit` or `kotlin.Unit` when its return type is declared. A member `main`, a local
one, another return type or other parameters MUST be none.

#### Scenario: Kotlin forms of `main`
- **WHEN** a Kotlin file declares `fun main` at the top level of each form, and a member, a local or another `main`
- **THEN** the top-level forms are an `entry_point`, the others none
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_top_level_main_is_an_entry_point`)

### Requirement: Summaries: A Java `public static void main` of strings is an entry point

A Java `public static void main` of one `String[]` (`String...`, `String args[]`) of a type MUST be an entry point,
with `public` implied for a non-private interface method. Another return type or other parameters MUST be none.

#### Scenario: Java forms of `main`
- **WHEN** a Java type declares `public static void main` of `String[]`, `String...` or `String args[]`, an interface
  a non-private `static void main`, and a `main` of other parameters
- **THEN** the first ones are an `entry_point`, the last none
  [@test] crates/code-symbols/src/java_test.rs (`a_public_static_void_main_of_strings_is_an_entry_point`)

### Requirement: Summaries: A Kotlin or Java declaration marked as no stable API is unstable

A Kotlin or Java declaration of any kind marked as no stable API MUST be `unstable`, for the API a review names:
annotated `@ApiStatus.Internal` or `@ApiStatus.Experimental` qualified by `ApiStatus` or by its full name, or a bare
`@Internal`/`@Experimental` that an import of the file brings in from `org.jetbrains.annotations.ApiStatus` (by its
name, an alias of it, or `ApiStatus.*`), never another `Internal`.

#### Scenario: An annotation of `ApiStatus`
- **WHEN** a declaration is annotated `@ApiStatus.Internal`, `@ApiStatus.Experimental`, or a bare `@Internal` or
  `@Experimental` that an import brings in from `org.jetbrains.annotations.ApiStatus`
- **THEN** it is `unstable`
  [@test] crates/code-symbols/src/kotlin_test.rs (`an_unstable_annotation_marks_a_declaration`)
  [@test] crates/code-symbols/src/java_test.rs (`an_unstable_annotation_marks_a_declaration`)

#### Scenario: A bare `Internal` without the import
- **WHEN** a declaration is annotated `@Internal` and no import brings in `ApiStatus`
- **THEN** it is stable
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_bare_internal_without_the_import_of_api_status_is_stable`)

### Requirement: Summaries: Test-only API, Java fields and Kotlin file annotations are unstable

`@VisibleForTesting` or `@TestOnly` MUST mark a declaration `unstable` by the last name, whatever the qualifier; a Java
field by the annotation of its statement. A top-level declaration of a Kotlin file annotated so
(`@file:ApiStatus.Internal`, the import after it counted) MUST be `unstable` too; a member of a marked type MUST NOT be
marked itself, and an import MUST bind no API.

#### Scenario: `@VisibleForTesting`
- **WHEN** a member is annotated `@VisibleForTesting` on the line above
- **THEN** it is `unstable`
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_member_knows_whether_it_overrides_and_can_be_overridden`)

#### Scenario: A file annotation
- **WHEN** a Kotlin file is annotated `@file:ApiStatus.Internal`, its import after the annotation
- **THEN** its top-level declarations are `unstable`, the members of its types not themselves
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_file_annotation_marks_the_top_level_declarations_unstable`)

#### Scenario: A Java field and an import
- **WHEN** a Java field statement is annotated `@TestOnly`, and a file imports a marked declaration
- **THEN** the field is `unstable`, and the import binds no API

### Requirement: Summaries: A Kotlin or Java property has its type

The summary of a Kotlin or Java file MUST hold the type of each property and field (`value_type`), as written without
type arguments (a Kotlin nullable one as its inner type, past its annotations: `(@A Foo)?` is `Foo`, as is the
receiver of an extension) or the type its initializer constructs (a capitalized Kotlin call, a Java `new` without a
body), and whether a constructor call gave the type.

#### Scenario: A property's type
- **WHEN** a property is declared with a type, or initialized by a constructor call
- **THEN** its `value_type` is the type without type arguments, and whether the constructor call gave it
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_property_knows_its_type`)

#### Scenario: An annotated nullable type
- **WHEN** a property or the receiver of an extension is of `(@A Foo)?`
- **THEN** the type is `Foo`
  [@test] crates/code-symbols/src/kotlin_test.rs (`an_annotated_nullable_type_is_its_inner_type`)

#### Scenario: The Kotlin and Java fixtures
- **WHEN** the fixtures are summarized
- **THEN** the types of properties and receivers are those of the golden files
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: A Kotlin or Java member reference has the type of its receiver

A member reference MUST hold the type of its receiver (`receiver_type`): of a parameter, a local or a typed lambda
parameter whose name is the whole receiver (`m.foo()`, `m?.foo()`, `m!!.foo()`), or of a property or
field `m` (`m!!` too) or `this.m` of a class around a reference that is not enclosed, not past a class with a
supertype, a nested class that isn't inner, a companion or an extension of another receiver; a parameter of a primary
constructor only in the initializers of the class.

#### Scenario: A bound name as the receiver
- **WHEN** a parameter, a local or a typed lambda parameter is the whole receiver of a call
- **THEN** the reference has its declared type as its `receiver_type`
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_bound_name_carries_its_declared_type`)
  [@test] crates/code-symbols/src/java_test.rs (`a_bound_name_carries_its_declared_type`)

#### Scenario: A not-null assertion
- **WHEN** the receiver is `m!!`
- **THEN** it names its receiver as a safe call `m?.` does
  [@test] crates/code-symbols/src/kotlin/summary_test.rs (`a_not_null_assertion_names_its_receiver_as_a_safe_call_does`)

#### Scenario: An extension of another receiver, and a parameter of a primary constructor
- **WHEN** a property `m` is used in an extension of another receiver, and a parameter of the primary constructor in
  an initializer, in `init` and in a function
- **THEN** the use in the extension has no `receiver_type`, an extension of the class itself keeps it
- **AND** the parameter has its type in the initializer and in `init` only
  [@test] crates/code-symbols/src/kotlin_test.rs (`an_extension_receiver_or_a_function_hides_the_type`)

#### Scenario: A context parameter of a property
- **WHEN** a property has a context parameter `c: Ctx` and uses a member on `c`
- **THEN** that reference has the type `Ctx` of its receiver, and `c` is a name of the property alone, not of the
  class around it
  [@test] crates/code-symbols/src/kotlin/summary_test.rs (`a_context_parameter_of_a_property_is_a_name_of_the_property_alone`)

### Requirement: Summaries: Kotlin and Java names of no single type give no type

A type parameter, a function type, a vararg, a Java array after the name (`Foo a[]`), a delegate, a destructuring
declaration, an inferred expression, a Java anonymous class and a name that its scope or function casts (Kotlin `is`,
`!is`, `as`, an `is` branch of its `when`, Java `instanceof` and casts; not `in`) MUST give none.

#### Scenario: A smart cast
- **WHEN** a name is cast by `is`, `!is`, `as`, an `is` branch of its `when`, `instanceof` or a Java cast in its scope
  or function
- **THEN** it gives no type, but `in` keeps it
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_smart_cast_drops_the_type`)
  [@test] crates/code-symbols/src/java_test.rs (`a_smart_cast_drops_the_type`)

#### Scenario: An anonymous class initializer
- **WHEN** a Java field is initialized by an anonymous class
- **THEN** it has no type
  [@test] crates/code-symbols/src/java_test.rs (`an_anonymous_class_initializer_has_no_type`)

#### Scenario: An array after the name
- **WHEN** a Java field is declared `Foo a[]`
- **THEN** it has no type
  [@test] crates/code-symbols/src/java/summary_test.rs (`an_array_after_the_name_has_no_type`)

#### Scenario: Type parameters, function types, varargs, delegates and inferred expressions
- **WHEN** a name is of a type parameter, a function type or a vararg, a delegate, a destructuring declaration or an
  inferred expression
- **THEN** it gives no type

### Requirement: Summaries: A Kotlin or Java call has the shape of its arguments on its callee

The summary of a Kotlin or Java call (not an infix call), Java's `new` and a Kotlin constructor invocation of a
supertype included, MUST hold the shape of its arguments on the reference of its callee (the last name of the type of
`new` and of an invocation, `Base` of `: a.Base(…)`, whose qualifiers are types).

#### Scenario: A call
- **WHEN** a Kotlin or Java function is called with arguments, and a Kotlin function by an infix call
- **THEN** the reference of the callee holds the shape of the arguments, the infix call none
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_call_has_its_argument_shape`)
  [@test] crates/code-symbols/src/java_test.rs (`a_call_has_its_argument_shape`)

#### Scenario: A constructor invocation of a supertype
- **WHEN** a Kotlin class invokes `: a.Base(…)`
- **THEN** the shape is on the reference of `Base`, whose qualifiers are types
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_constructor_invocation_of_a_supertype_has_its_argument_shape`)

#### Scenario: The Kotlin and Java fixtures
- **WHEN** the fixtures are summarized
- **THEN** the shapes of their calls are those of the golden files
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: The shape of a call holds each argument in order and by its kind

The shape MUST hold the arguments in source order, a named one with its name, a Kotlin trailing lambda last, whether
one spreads an array, and each as a text (a string literal or template), a number (an integer without a suffix,
signed or not), a boolean, a lambda or another expression (`null`, a real, a character, `1L`, `1u`, a name, a call).

#### Scenario: Arguments of every kind
- **WHEN** a call passes a string literal, a template, an integer, a negative integer, a boolean, a lambda, `null`, a
  real, a character, `1L`, `1u`, a name, a call, a named argument, a trailing lambda and a spread array
- **THEN** its shape holds them in source order, the named one with its name, the trailing lambda last, the spread
  marked, each as a text, a number, a boolean, a lambda or another expression
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_call_has_its_argument_shape`)
  [@test] crates/code-symbols/src/java_test.rs (`a_call_has_its_argument_shape`)

### Requirement: Summaries: The shape of a call says whether it is enclosed

The shape MUST say whether the call is enclosed: in a Kotlin lambda or anonymous function, an object expression, a
Java anonymous class or a local declaration inside the declaration around it, whose receiver may be what a bare name
or `this` means; a Java lambda, an initializer and a getter are not, and the constructor invocation of an object
expression's supertype and a `new` of an anonymous class are of the code around the object.

#### Scenario: Enclosed and not
- **WHEN** calls stand in a Kotlin lambda, an anonymous function, an object expression, a Java anonymous class, a local
  declaration, a Java lambda, an initializer and a getter
- **THEN** the first five are enclosed, the last three are not
- **AND** the supertype invocation of the object expression and the `new` of the anonymous class are of the code
  around them
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_call_has_its_argument_shape`)
  [@test] crates/code-symbols/src/java_test.rs (`a_call_has_its_argument_shape`)

### Requirement: Summaries: An annotation with arguments stays a type

An annotation (`@Suppress("x")`) MUST stay a type, no call.

#### Scenario: An annotation with an argument
- **WHEN** a declaration is annotated `@Suppress("x")`
- **THEN** `Suppress` is a reference of a type

### Requirement: Summaries: `this(…)` and `super(…)` are calls of the constructors they mean

`this(…)` of a constructor MUST be a call of the class around it, and `super(…)` one of its superclass: in Java the
type of `extends`, in Kotlin the only supertype of the class, named without arguments or a delegate (`: Base`); Java's
`super(…)` without `extends` and Kotlin's beside a second supertype MUST be no reference.

#### Scenario: `this(…)` of a Java constructor
- **WHEN** a Java constructor calls `this(…)`
- **THEN** it is a call of the class around it, and the name of a constructor is no reference
  [@test] crates/code-symbols/src/java_test.rs (`a_class_knows_its_constructors`)

#### Scenario: `super(…)` of a Kotlin constructor of one supertype or two
- **WHEN** a secondary constructor of a Kotlin class of the only supertype `: Base` calls `super(…)`, and one of a
  class `: Base, Runnable`
- **THEN** the first is a call of `Base`, the second no reference
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_super_call_that_needs_a_new_constructor_parameter_is_an_edge`, `a_kotlin_super_call_with_two_supertypes_is_no_edge`)

#### Scenario: `super(…)` of a Kotlin class without a supertype
- **WHEN** a secondary constructor of a Kotlin class without a supertype calls `super()`
- **THEN** it is no reference
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_constructor_invocation_of_a_supertype_has_its_argument_shape`)

#### Scenario: Java `super(…)` without `extends`
- **WHEN** a Java class without `extends` calls `super(…)`
- **THEN** it is no reference

### Requirement: Summaries: A Kotlin or Java class has its constructors

The summary of a Kotlin or Java class (not an interface, an object, a companion, an enum, an annotation, a type alias,
nor a class with a parse error in it, which may have swallowed one) MUST hold its constructors, each with its
parameters (as a function's) and the reach of its modifiers.

#### Scenario: Kotlin constructors
- **WHEN** a Kotlin class has a primary constructor (class parameters, a default value, `vararg`) and secondary ones,
  and an interface, an object, a companion, an enum, an annotation and a class with a parse error in it are declared
- **THEN** the summary holds each constructor of the class with its parameters and reach, and the others none
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_class_knows_its_constructors`)

#### Scenario: Java constructors and types without them
- **WHEN** a Java class declares constructors, and an interface, an enum and an annotation are declared
- **THEN** the class holds its constructors, and the others none
  [@test] crates/code-symbols/src/java_test.rs (`a_class_knows_its_constructors`)

#### Scenario: The fixtures
- **WHEN** the fixtures are summarized
- **THEN** the constructors are those of the golden files
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: A record has its canonical constructor, a class without any the implicit one

A record MUST hold its canonical constructor (declared, compact or of its components, with the record's reach) with
its others; a class without any MUST hold the implicit `()`, public in Kotlin and of the reach of its class in Java. A
local class MUST add none to the class around it.

#### Scenario: A record and an implicit constructor
- **WHEN** a Java record has a compact or a declared canonical constructor or none, and a class declares none
- **THEN** the record holds the canonical constructor with the record's reach and its others, and the class `()` of
  its reach
  [@test] crates/code-symbols/src/java_test.rs (`a_class_knows_its_constructors`)

#### Scenario: A Kotlin class without constructors, and a local class
- **WHEN** a Kotlin class declares no constructor, and a function declares a local class
- **THEN** the class holds a public `()`, and the local class adds none to the class around it
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_class_knows_its_constructors`)

### Requirement: Summaries: Only a record's canonical constructor replaces the one of its header

A constructor MUST NOT replace another unless it is the canonical constructor of a record, compact or declared again
with the names and types of its components (every type known), which replaces the one of its header: two that the
summary can't tell apart (`Stamp(java.util.Date)` beside `Stamp(java.sql.Date)`, `Box(value: T)` beside
`Box(list: List<T>)`) MUST both be there.

#### Scenario: Two constructors the summary can't tell apart
- **WHEN** a class has `Stamp(java.util.Date)` beside `Stamp(java.sql.Date)`, or `Box(value: T)` beside
  `Box(list: List<T>)`
- **THEN** both are there, neither replaces the other
  [@test] crates/code-symbols/src/java_test.rs (`a_class_knows_its_constructors`)
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_class_knows_its_constructors`)

### Requirement: Summaries: A function, a property, a field or a constant carries its header

A function, a property, a field or a constant MUST carry its header: the tokens before its body or initializer,
without comments, annotations, the modifiers that the reach and the overriding say or that no use depends on (Kotlin
`inline`, `lateinit`, Java `final`, `synchronized`), and the parameters a signature holds. A type has none.

#### Scenario: The header of a member
- **WHEN** a Kotlin or Java member has comments, annotations, modifiers and parameters before its body
- **THEN** its header is the tokens before the body without them
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_header_is_the_form_before_the_body`)
  [@test] crates/code-symbols/src/java_test.rs (`a_header_is_the_form_before_the_body`)

#### Scenario: The fixtures
- **WHEN** the fixtures are summarized
- **THEN** the headers are those of the golden files
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: A header changes with the form, not with the body

A change of the body alone MUST keep the header, and `suspend`, a receiver or a return type MUST change it; Java MUST
take the type of a field from the declaration around its declarator.

#### Scenario: A body changes, the form changes
- **WHEN** a commit changes the body of a function alone, or its `suspend`, its receiver or its return type
- **THEN** the header stays in the first case and changes in the others
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_header_is_the_form_before_the_body`)

#### Scenario: A Java field of several declarators
- **WHEN** a Java statement declares several fields of one type
- **THEN** the header of each holds the type of the statement
  [@test] crates/code-symbols/src/java_test.rs (`a_header_is_the_form_before_the_body`)

### Requirement: Summaries: A reference knows whether a suspend call is allowed where it is

A Kotlin or Java reference MUST know whether a suspend call is allowed where it is: yes in a `suspend` function with
no lambda between and in the lambda of a coroutine builder (`runBlocking`, `launch`, `withContext`, …); no in another
function, an initializer, an accessor, the lambda of `sequence` or `iterator`, and in Java; unknown in the lambda of
another function.

#### Scenario: The coroutine contexts
- **WHEN** references stand in a `suspend` function, a lambda of `runBlocking`, another function, an initializer, an
  accessor, a lambda of `sequence` and of another function
- **THEN** a suspend call is allowed in the first two, not in the next four, and unknown in the last
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_reference_knows_its_coroutine_context`)

#### Scenario: Java
- **WHEN** a Java reference is read
- **THEN** a suspend call is not allowed there

### Requirement: Summaries: The name of an override is no use for the dangling-reference check

The dangling-reference check MUST take the name of an `override` member for no use: it declares one.

#### Scenario: An override line
- **WHEN** a commit removes a declaration and a line of another file has an `override` member of its name
- **THEN** that line is no use of the removed declaration
  [@test] crates/trial-check/src/dangling_references_test.rs (`an_override_line_is_no_use`)

### Requirement: Summaries: A Rust name after a single `.` is no use of a removed item

The dangling-reference check MUST take a Rust name after a single `.` (not `..`) for no use of a removed item: it is
a field or a method (`.collect()`). The rule is of the lines, so it MUST hold with `branchdeck.symbols` `off` too.

#### Scenario: A method call of the name of a removed free function
- **WHEN** a commit removes a free function and another file calls `.collect()` of its name
- **THEN** that call is no use of it, also with `branchdeck.symbols` `off`
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_method_call_is_no_use_of_a_removed_free_function`)

### Requirement: Summaries: A removed `pub` Rust item is searched in another crate through its crate

With `branchdeck.symbols on`, which knows the crate of each path of the commits (the nearest `Cargo.toml`,
`ModuleRoots::read_crate_of`, its name from `SymbolInput::crates`), the dangling-reference check MUST count a use of a
removed `pub` Rust item in a file of another crate only through the item's crate: a path that starts with the crate's
name or with a name a `use` of the crate binds, or a bare name where such a `use` names the item or a glob.

#### Scenario: A free call in another crate without a use
- **WHEN** a commit removes a `pub` function and a file of another crate calls its bare name without a `use` of the
  crate
- **THEN** that call is no use
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_free_call_in_another_crate_without_a_use_is_no_use`)

### Requirement: Summaries: A use in the item's own crate counts as before, and an unknown crate searches all

A use in the item's own crate MUST count as before. A file in a directory HEAD lacks (a module a commit deletes) MUST
be in the crate of its nearest directory HEAD has. Where the crate is not known (`report`, `off`, a path of no
`Cargo.toml`, a path of the baseline after the base), every file of the language MUST be searched.

#### Scenario: A free call in the same crate
- **WHEN** a commit removes a `pub` function and a file of its own crate calls it
- **THEN** that call is a use
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_free_call_in_the_same_crate_is_a_use_with_removed_by_and_declared_in`)

#### Scenario: An item of a deleted module
- **WHEN** a commit deletes a module and its `pub(crate)` item, so that its directory is gone at HEAD
- **THEN** the file is in the crate of its nearest directory HEAD has, and the item is not searched in another crate
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_crate_item_of_a_deleted_module_is_not_searched_in_another_crate`)

#### Scenario: The crate is not known
- **WHEN** the mode is `report` or `off`, or a path has no `Cargo.toml`, or is of the baseline after the base
- **THEN** every file of the language is searched

### Requirement: Summaries: A restricted Rust item is searched in its own crate or file only

A `pub(crate)`, `pub(super)` or `pub(in …)` Rust item MUST be searched in its own crate only (with the crates known),
and a `pub(self)` one as a private one, in its own file.

#### Scenario: A `pub(crate)` function
- **WHEN** a commit removes a `pub(crate)` function and another crate calls a function of its name
- **THEN** the other crate is not searched
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_pub_crate_function_is_not_searched_in_another_crate`)

#### Scenario: A `pub(self)` item
- **WHEN** a commit removes a `pub(self)` item
- **THEN** it is searched in its own file only, as a private one

### Requirement: Summaries: A call on an expression is no use of a removed Kotlin top-level function

By the syntax trees, a call on an expression (an empty qualifier: `client.controlUrl()`) MUST be no use of a removed
Kotlin top-level function that is no extension; a call of an extension on its receiver MUST stay one. No other
language drops a qualified use: `pkg.f()` MUST be a call of `f`.

#### Scenario: A call on an expression and of an extension
- **WHEN** a commit removes a Kotlin top-level function and an extension, and another file calls
  `client.controlUrl()` of the function's name and the extension on its receiver
- **THEN** the first is no use, the second is one
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_call_on_an_expression_is_a_use_of_an_extension_only`)

#### Scenario: A qualified use in another language
- **WHEN** a file of another language calls `pkg.f()` of a removed `f`
- **THEN** it is a call of `f`

### Requirement: Summaries: A removed C# declaration is searched by the form a use takes

A removed C# type MUST be searched by its name wherever it is declared (a block namespace indents the types of a
file), a static member or a constant by the stem of its file (`Defaults.Limit`), an extension method by its name; an
instance member MUST NOT be searched by the lines.

#### Scenario: A removed constant and a removed type
- **WHEN** a commit removes a C# constant and a type of a block namespace
- **THEN** the constant is searched as `Defaults.Limit` by the stem of its file, and the type by its name
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_removed_csharp_constant_is_searched_by_its_type_and_a_type_by_its_name`)

#### Scenario: An extension method and an instance member
- **WHEN** a commit removes a C# extension method and an instance member
- **THEN** the extension method is searched by its name, and the instance member not by the lines

### Requirement: Summaries: A dangling reference names where the declaration is

A dangling reference MUST name where the declaration is (`declared_in`: its file and its 1-based line in the file
before `removed_by`, after it for a duplicate), read in the same `cat-file --batch` as the blobs of the search.

#### Scenario: A removed declaration
- **WHEN** a use dangles after a commit removes or moves its declaration
- **THEN** the dangling reference names `removed_by` and `declared_in`, the file and the line before that commit
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_free_call_in_the_same_crate_is_a_use_with_removed_by_and_declared_in`, `the_incident_a_moved_constant_whose_use_is_fixed_in_another_branch`)
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_function_made_suspend_dangles_where_another_branch_removed_its_call`)

#### Scenario: A duplicate
- **WHEN** a commit declares a type again in the same package
- **THEN** `declared_in` names the file and the line after it
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_type_declared_again_in_the_same_package_is_a_duplicate`)

#### Scenario: One batch
- **WHEN** the check of a removed Rust item runs
- **THEN** the line of the declaration is read in the same `cat-file --batch` as the blobs of the search
  [@test] crates/trial-check/src/process_budget_test.rs (`the_check_of_a_removed_rust_item`)

### Requirement: Summaries: The changes of the commits carry full blob ids

The commits' changes MUST carry the full ids of the blobs before and after each file, from the same `log --raw`
process (`--no-abbrev`); a side without a file, a symlink and a submodule have none.

#### Scenario: The raw part
- **WHEN** the raw part of a `log --raw` is parsed
- **THEN** each change has the ids of its blobs, and a side without a file, a symlink and a submodule none
  [@test] crates/trial-check/src/diff_scan_test.rs (`parses_the_raw_part`)

#### Scenario: The commit diffs of a range
- **WHEN** the commit diffs of a range are read
- **THEN** they carry the full blob ids from the same process, `--no-abbrev`
  [@test] crates/trial-check/src/diff_scan_test.rs (`read_commit_diffs_gives_the_full_blob_ids`)

### Requirement: Summaries: The summaries are read through the object cache by extractor version

The summaries MUST be read through the object cache, keyed by the extractor version, the language and the blob id,
with one `cat-file --batch` for the blobs the cache misses; a write of another extractor version MUST clear the store
once. A truncated summary (over a size limit, nested deeper than the parse reads) MUST NOT be cached.

#### Scenario: A read through the cache
- **WHEN** the summaries of the blobs are read twice
- **THEN** the second read takes them from the cache, and the first reads the misses in one `cat-file --batch`
  [@test] crates/trial-check/src/symbol_scan_test.rs (`reads_the_summaries_through_the_cache`)

#### Scenario: Another extractor version
- **WHEN** another extractor version writes a summary
- **THEN** the store is cleared once, and a summary is read by the version that wrote it
  [@test] crates/object-cache/src/cache_test.rs (`blob_summaries_are_read_by_the_extractor_version_that_wrote_them`)

#### Scenario: A truncated summary
- **WHEN** a summary is truncated, over a size limit or nested deeper than the parse reads
- **THEN** it is not cached

### Requirement: Summaries: The extractor version changes with the rules and with psi-syntax

The extractor version MUST change with the rules and with the revision of psi-syntax that reads Kotlin, Java, C#,
Starlark, TypeScript, JavaScript and Rust.

#### Scenario: The version and the revision
- **WHEN** the rules or the revision of psi-syntax change
- **THEN** the extractor version changes, and the revision is the one cargo builds
  [@test] crates/code-symbols/src/summary_test.rs (`extractor_version_is_stable`, `the_psi_syntax_revision_is_the_one_cargo_builds`)

### Requirement: Summaries: A Rust macro in item position declares its `pub` items

The summary of a Rust file MUST declare the `pub` items at the top level of the body of a macro in item position (in a
file, a module or an impl) by their tokens: an enum with its variants, a struct, a union, a trait, a type alias, a
function, a constant, a static (`lazy_static!`'s `pub static ref` too) and a module, with the reach of their
visibility (`pub(crate)`), from their doc comments and attributes to their `;` or their body.

#### Scenario: The items of a macro
- **WHEN** a macro in item position holds `pub` items of each kind with doc comments and attributes
- **THEN** each is declared with the reach of its visibility, from its doc comment to its `;` or its body
  [@test] crates/code-symbols/src/rust_test.rs (`declarations`)
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`)

### Requirement: Summaries: Other names of a macro body stay references, and a macro in a function declares nothing

The other names of the body of a macro in item position MUST stay references. A macro in a function body MUST declare
nothing.

#### Scenario: A macro in a function body
- **WHEN** a function body holds a macro with `pub` items
- **THEN** it declares nothing, and its names are references
  [@test] crates/code-symbols/src/rust_test.rs (`declarations`)

### Requirement: Summaries: A Bazel file declares its defs, assignments and targets

The summary of a Bazel file (`BUILD`, `BUILD.bazel`, `MODULE.bazel`, `WORKSPACE*`, `REPO.bazel`, `.bzl`, `.star`,
`*.BUILD`, which name the language, not an extension) MUST hold its top-level `def`s with their parameters, the names
its top-level `=` assigns (a constant in capitals, a value otherwise), and, where the plugin's own rule says its
top-level calls declare targets (`BUILD`, `BUILD.bazel`, `MODULE.bazel`), a target per call with a non-empty literal
`name`, the rule it calls as its type.

#### Scenario: A target per call
- **WHEN** a `BUILD` file calls a rule with a literal `name`, with a name that is no literal, and with an empty one
- **THEN** the first is a target of the rule it calls as its type (`jps_test`, `native.cc_library`), the others
  declare none
  [@test] crates/code-symbols/src/starlark/summary_test.rs (`a_top_level_call_with_a_literal_name_is_a_target_of_its_rule`, `a_name_that_is_no_literal_or_is_empty_declares_no_target`)

#### Scenario: Only a build file declares targets
- **WHEN** a `.bzl` file makes the same top-level calls
- **THEN** they declare no target
  [@test] crates/code-symbols/src/starlark/summary_test.rs (`only_a_build_file_declares_targets_from_its_top_level_calls`)

#### Scenario: Defs and assignments
- **WHEN** a `.bzl` file has top-level `def`s and assignments
- **THEN** each `def` is declared with its parameters, an assignment in capitals as a constant, another as a value
  [@test] crates/code-symbols/src/starlark/summary_test.rs (`a_bzl_file_declares_its_top_level_defs_and_assignments`)

#### Scenario: The file names of the language
- **WHEN** a file is named by one of the Bazel names
- **THEN** it is of the language, though it has no extension of it
  [@test] crates/code-symbols/src/summary_test.rs (`every_fixture_matches_its_golden_file`, `language_by_extension`)

### Requirement: Summaries: A Bazel `load` is an import, a label in a string no reference

Each name a `load` binds MUST be an import of the label as written, the bound name its alias where it differs. A label
in a string (`"//pkg:x"`, `":x"`) MUST be no reference: a reference is a name of the tree, and a label needs a package
model the summary does not have. The rules of the tree MUST be those of `starlark-syntax`, which accio's extractor
reads too, so that one answer serves both.

#### Scenario: A load
- **WHEN** a file `load`s names of a label, one under an alias
- **THEN** each is an import of the label as written, the aliased one with its alias
  [@test] crates/code-symbols/src/starlark/summary_test.rs (`a_load_is_an_import_per_name_it_binds_under_the_label_it_names`)

#### Scenario: A call and a label in a string
- **WHEN** a file calls a rule with `"//pkg:x"` and `":x"` as arguments
- **THEN** the call is a reference of the rule, and the labels are none
  [@test] crates/code-symbols/src/starlark/summary_test.rs (`a_call_is_a_reference_of_the_rule_it_names_and_a_label_in_a_string_is_none`)

### Requirement: Summaries: Module roots, crate names and path aliases come from HEAD's tree

The module roots (the nearest directory with an `*.iml`, a Bazel, Gradle or Maven build file, a `Cargo.toml` or a
`package.json`), the crate names and the TypeScript path aliases MUST come from HEAD's tree, the directories with one
`cat-file --batch`, never a command line that lists them.

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
  [@test] crates/trial-check/src/symbol_scan_test.rs (`the_name_of_a_crate_and_the_aliases_of_a_tsconfig`)

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

### Requirement: Edges: The evidence that a use means a declaration

The evidence MUST be one of: an import or alias of it, its qualified name or `Type.member` with a resolved type, the
same file, the same Rust module, a wildcard import of its package or module for a type, a type of the same JVM package
in the same module, an override, a call inherited from the supertype, or a use on a receiver of its type (below).

#### Scenario: A use without evidence
- **WHEN** a reference on the added lines has the name of a declaration another commit adds, but none of the evidence
- **THEN** it is no edge

### Requirement: Edges: A C# type is found by C#'s lookup

A C# use MUST be an edge by C#'s lookup: a type of the use's namespace or of a namespace around it, whatever its
assembly (`same_package_type`), of a `using` (`star_import`), an alias (`import`), a qualified name, also relative to
a namespace around the use or under `global::` (`qualified`), a static member by its type or bare through
`using static`.

#### Scenario: A type by a using, an alias and its namespaces
- **WHEN** a C# commit uses a type another commit adds, of its namespace, a namespace around it, a `using`, an alias
  or a qualified name
- **THEN** each is an edge with its evidence
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_csharp_type_by_a_using_an_alias_and_its_namespaces_is_an_edge`)

#### Scenario: A static member
- **WHEN** a C# commit uses a static member another commit adds by its type, or bare through `using static`
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_csharp_static_member_by_its_type_and_by_using_static_is_an_edge`)

### Requirement: Edges: An `internal` C# declaration reaches its own assembly only

An `internal` C# declaration MUST be an edge only from its own assembly (the nearest `*.csproj`).

#### Scenario: An internal type from another assembly
- **WHEN** a use of another assembly means an `internal` type another commit adds
- **THEN** it is no edge, and one of the type's assembly is
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_internal_csharp_type_reaches_its_assembly_only`)

#### Scenario: The assembly of a file
- **WHEN** a file lies below a directory with a `*.csproj`
- **THEN** that directory is the root of its assembly
  [@test] crates/trial-check/src/module_roots_test.rs (`a_directory_with_a_csproj_is_the_root_of_its_assembly`)

### Requirement: Edges: A C# extension method needs a receiver of its type and its namespace

A C# extension method MUST be an edge only on a receiver of its type where the use sees its class's namespace
(`receiver`).

#### Scenario: An extension method on a receiver of its type
- **WHEN** a commit calls an extension method another commit adds on a receiver of its type, where its namespace is
  seen, and elsewhere
- **THEN** the first is an edge, the others are not
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_csharp_extension_method_on_a_receiver_of_its_type_where_its_namespace_is_seen_is_an_edge`)

### Requirement: Edges: A C# instance member, `base` and an override follow the JVM rules

A C# instance member MUST be an edge on a receiver of a known type, `base.Member()` as an inherited call, an
`override` by the JVM rules. The keys of a C# type MUST be only present or absent: a `partial` type has members in
several files.

#### Scenario: A member on a receiver and an override
- **WHEN** a commit uses a member another commit adds on a receiver of its type, and overrides it
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_csharp_member_on_a_receiver_of_its_type_and_an_override_are_edges`)

#### Scenario: A partial type
- **WHEN** a `partial` type has members in several files
- **THEN** its keys are only present or absent

### Requirement: Edges: A Rust item of an item macro is an edge, and a field by its owner type

A Rust item that the body of a macro in item position declares MUST be an edge like any other
(`use avl_trace::bundle::StillEncoding` of a `vocabulary! { pub enum StillEncoding { … } }`, `StillEncoding::Lossless`
by its enum), and so MUST a Rust field or method used by its owner type in another file: a field of a struct
expression or a struct pattern, shorthand too (`Manifest { stills: Stills { encoding } }`), and `Type::method(&x)`,
the type's path resolved as for a static member.

#### Scenario: An item of a macro and a field by its struct
- **WHEN** a commit names `use avl_trace::bundle::StillEncoding` and `StillEncoding::Lossless` of a
  `vocabulary! { pub enum StillEncoding { … } }` another commit adds, and a field in a struct expression or pattern
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_item_of_an_item_macro_and_a_field_by_its_struct_are_edges`)

### Requirement: Edges: A Rust member on a value is no edge

A member on a value (`manifest.stills`, `self.stills`) or through `Self` MUST be no edge: the summary knows no type of
a Rust value.

#### Scenario: A field on a value
- **WHEN** a commit reads `manifest.stills` or `self.stills` of a field another commit adds, or a member through `Self`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_field_on_a_value_is_no_edge`)

### Requirement: Edges: A Bazel `load` of a name a `.bzl` of the range declares is an edge

A commit whose Bazel file `load`s a name another commit declares in a `.bzl` of the range MUST depend on it: the label
names the file from the root of the repository (`//pkg:defs.bzl` is `pkg/defs.bzl`, `:defs.bzl` and `defs.bzl` the
package of the file that loads them).

#### Scenario: A load of a macro another commit adds
- **WHEN** a `BUILD` file `load`s a macro another commit adds to a `.bzl`
- **THEN** it is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bazel_load_of_a_macro_another_commit_adds_is_an_edge`)

#### Scenario: A label from the root and a relative label
- **WHEN** a label is `//pkg:defs.bzl`, `:defs.bzl` or `defs.bzl`
- **THEN** the first names `pkg/defs.bzl`, the others the file of the package that loads them
  [@test] crates/trial-check/src/bazel_resolution.rs (`a_label_names_a_file_from_the_root_of_the_repository`, `a_relative_label_names_a_file_of_the_package_that_loads_it`)

### Requirement: Edges: A Bazel label matches a workspace below the root by the tail of its path

Where no file of the range has the exact path of a label, a path that ends in it MUST count, as a Bazel workspace
below the root names its packages from its own root. A label of another repository (`@repo//…`) MUST name no file of
this one.

#### Scenario: A workspace below the root
- **WHEN** a label names a path that no file of the range has, but one ends in it
- **THEN** that file is the one it names
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_label_of_a_workspace_below_the_root_matches_by_the_tail_of_the_path`)
  [@test] crates/trial-check/src/bazel_resolution.rs (`a_workspace_below_the_root_of_the_repository_matches_by_the_tail_of_the_path`)

#### Scenario: Another repository
- **WHEN** a file loads `@repo//…`
- **THEN** the label names no file of this repository, and it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bazel_load_from_another_repository_is_no_edge`)
  [@test] crates/trial-check/src/bazel_resolution.rs (`a_label_of_another_repository_names_no_file_of_this_one`)

### Requirement: Edges: A Bazel file sees Bazel declarations alone, and a target is no declaration

A Bazel file MUST see the declarations of Bazel files alone: a Kotlin or Java declaration of the same name is another
family. A target MUST be no declaration a use can mean and MUST raise no dangling reference: it is a name of the build
graph, not callable code.

#### Scenario: A Kotlin function of the loaded name
- **WHEN** a build file loads a name, and another commit adds a Kotlin function of that name
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_kotlin_function_of_the_name_a_build_file_loads_is_no_edge`)

#### Scenario: A target
- **WHEN** a commit adds or removes a target, and another names it
- **THEN** it is no edge and no dangling reference
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bazel_target_is_no_declaration_a_use_can_mean`)

### Requirement: Edges: A name a private import binds is an edge where the import reaches

A name that a private import of the range binds (a Rust `use`, a TypeScript import, a Kotlin alias) MUST be an edge by
a bare use where the import reaches: its own file, and in Rust its module and the modules below it, through
`use super::*`, `use super::Path` or `super::Path` (`Path` of a `use std::path::{Path, PathBuf}` in `execute.rs`,
used in `execute/tests.rs`, also in `Path::new`).

#### Scenario: A Rust use of a name another commit imports
- **WHEN** a commit adds `use std::path::{Path, PathBuf}` in `execute.rs`, and another uses `Path` in
  `execute/tests.rs` through `use super::*`, `use super::Path` or `super::Path`, also in `Path::new`
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_use_of_a_name_another_commit_imports_is_an_edge`)

#### Scenario: The same file
- **WHEN** a commit uses a name another commit imports in the same file
- **THEN** it is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_use_of_a_name_another_commit_imports_in_the_same_file_is_an_edge`)

### Requirement: Edges: An import is needed beside the declaration it names

The import MUST be needed beside the declaration it names, never instead of it: a use of an item one commit adds,
through an import another commit adds, needs both commits.

#### Scenario: An item and its import of two commits
- **WHEN** one commit adds an item, another its import, and a third uses it
- **THEN** the third depends on both
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_use_needs_the_import_beside_the_item`)

### Requirement: Edges: A Rust path through the imports of its file ends

A Rust path through the imports of its file MUST end: the path of a `use` never starts with the name that `use` brings
in, nor with a name of a `use` the path came from, so `use stored_settings::stored_settings;` names the module
`stored_settings` and two imports of each other stop. The `mod` of the same name binds the name in the file too, and a
summary knows no namespace of Rust: such a call MUST be no edge.

#### Scenario: A function of a module of the same name
- **WHEN** a file has `use stored_settings::stored_settings;` and the `mod` of that name, and calls it
- **THEN** the path names the module `stored_settings`, the resolution ends, and the call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_use_of_a_function_of_a_module_of_the_same_name_ends`)

#### Scenario: Two imports of each other
- **WHEN** two imports name each other
- **THEN** the path through them stops

### Requirement: Edges: A use that needs no import of another commit is no edge

A use that needs no import of another commit MUST be no edge: in Rust a name the module of the use binds itself (an
item or an import of its file), which wins over a glob import; an import of the parent reached through a glob import
beside a second glob import, which may bring the same item; a qualified path (`std::path::Path`), which names the item
itself; and a private import from a module that is not its own or below it.

#### Scenario: Rust uses that need no import of another commit
- **WHEN** a Rust use names what its module binds itself, an import of the parent beside a second glob import, a
  qualified path `std::path::Path`, or a private import of another module
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_use_that_needs_no_import_of_another_commit_is_no_edge`)
  [@test] crates/code-symbols/src/rust_test.rs (`references`)

### Requirement: Edges: The same package is evidence for a type in the same module only

A call of a function of the same package without an import MUST be no edge, nor a type of the same package in another
module.

#### Scenario: A function of the package and a type of another module
- **WHEN** a commit calls a function of its package without an import, or uses a type of its package in another
  module
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`the_same_package_is_evidence_for_a_type_in_the_same_module_only`)

### Requirement: Edges: A static member is used by the path of its type

A static member MUST be used by the path of its type: in full, or from its head, which an import binds (an alias, a
nested type: `import a.Owner.Factory` and `Factory.create()`) or, without an import of its name, the same package does
when the file declares nothing of that name; a nested type is no name of its package.

#### Scenario: The path of a static member
- **WHEN** a commit calls a static member another commit adds by the full path of its type, through an import or an
  import alias of its type, by `Factory.create()` after `import a.Owner.Factory`, or by its type of the same package
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_companion_member_through_its_owner_is_an_edge`, `a_named_companion_needs_its_name`)

#### Scenario: No import, another package or a name the file declares
- **WHEN** a file of another package calls `Owner.create()` without an import, or a file of the package declares a
  type `Owner` of its own
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_companion_member_through_its_owner_is_an_edge`)

### Requirement: Edges: A member of an unnamed Kotlin companion is used through the class around it

A member of a Kotlin companion without a name of its own MUST be used through the class around it too
(`Owner.create()`, `Owner.Companion.create()`), from Java only a `@JvmStatic` function or a `const val`
(`Owner.create()`, `import static a.Owner.create`).

#### Scenario: Through its owner
- **WHEN** a Kotlin commit calls `Owner.create()` or `Owner.Companion.create()` of a companion member another commit
  adds
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_companion_member_through_its_owner_is_an_edge`)

#### Scenario: From Java
- **WHEN** a Java commit calls `Owner.create()` or imports `import static a.Owner.create` of a companion function
- **THEN** it is an edge only when the function is `@JvmStatic` or a `const val`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_call_of_a_companion_member_needs_jvm_static`)

#### Scenario: `@JvmStatic` added later
- **WHEN** one commit adds a companion function and a later one makes it `@JvmStatic`, and a Java call goes through
  the owner
- **THEN** the call depends on the later commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_jvm_static_added_later_is_an_edge_for_a_java_call_through_the_owner`)

### Requirement: Edges: A Java use of a Kotlin object goes through `INSTANCE`

Java's `Registry.INSTANCE.register()` MUST be a use of a function of the Kotlin object `Registry`, and the top-level
object's `Registry.register()` only of a `@JvmStatic` one.

#### Scenario: Through `INSTANCE` and through the object
- **WHEN** Java calls `Registry.INSTANCE.register()` and `Registry.register()` of a function another commit adds to
  the object `Registry`
- **THEN** the first is an edge, the second only for a `@JvmStatic` function
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_call_through_instance_is_an_edge`)

### Requirement: Edges: A named companion and a nested object are used by their path

A companion with a name of its own, which the summary doesn't tell from a nested object, MUST be used only by its path
(`Owner.Factory.create()`), and a member of a nested object never through the class around it.

#### Scenario: A named companion
- **WHEN** a commit calls `Owner.create()` and `Owner.Factory.create()` of a member of a companion named `Factory`
- **THEN** only the second is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_named_companion_needs_its_name`)

#### Scenario: A nested object
- **WHEN** a commit calls a member of a nested object through the class around it
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_member_of_a_nested_object_through_the_owner_is_no_edge`)

### Requirement: Edges: A name another type qualifies is no use of its file's declaration

In the same file, a name that a type other than the containers or the receiver of the file's declaration qualifies
(`Registry.register(id)` beside a `register` of `Plugin`, Rust `Registry::create()` in the `create` of another `impl`)
MUST be no use of that declaration.

#### Scenario: Qualified by another type
- **WHEN** a file declares `register` of `Plugin` and calls `Registry.register(id)`, or a Rust file calls
  `Registry::create()` in the `create` of another `impl`
- **THEN** the call is no use of the file's declaration
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_name_qualified_by_another_type_is_no_use_of_a_member_of_its_own_file`)

### Requirement: Edges: A type is a capitalized name, unless the file declares a value of it

A type MUST be a name with a capital, unless the file declares a value of that name (`DEFAULT.register(id)` of a
`val DEFAULT`, an `INSTANCE`, an enum constant), also one in capitals that the file imports (`UI`) or that another file
of its package declares (`JBUI`); a name in small letters, an imported one too, MUST be a value.

#### Scenario: A value in capitals
- **WHEN** a file calls `DEFAULT.register(id)` of a `val DEFAULT`, a member of an `INSTANCE` or of an enum constant,
  of an imported `UI` or of a `JBUI` of the package
- **THEN** the qualifier is a type only where the file declares no value of its name
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_value_in_capitals_is_no_other_type`)

#### Scenario: A name in small letters
- **WHEN** a name in small letters qualifies a use, an imported one too
- **THEN** it is a value

### Requirement: Edges: A Kotlin extension in its file is used by a bare name or its receiver's type

In the same file, a Kotlin extension (a declaration with an extension receiver) MUST be a use only by a bare name,
`this.name`, `super.name` or its receiver's type (`AirAuthoredScope::openGlobalPrompt`), of every key, one the range
adds too. A bare call in a lambda or a function with the extension's receiver as `this`
(`with(scope) { openGlobalPrompt() }`) MUST stay a use.

#### Scenario: A bare call with the receiver as `this`
- **WHEN** a file calls `openGlobalPrompt()` in `with(scope) { openGlobalPrompt() }` of its
  `fun AirAuthoredScope.openGlobalPrompt()`, or names `AirAuthoredScope::openGlobalPrompt`
- **THEN** each is a use of the extension
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_extension_called_on_a_local_in_its_file_is_no_edge`)

### Requirement: Edges: A Kotlin extension called on a local in its file is no use of it

In the same file, a call or reference of a Kotlin extension on a local, an expression or another name
(`prompt.openGlobalPrompt(frame)` on a `val prompt = AirComposerInIdeDriver(…)` beside a
`fun AirAuthoredScope.openGlobalPrompt()`) MUST be no use of it, since a member of the receiver's type wins over an
extension and only a compiler knows that type.

#### Scenario: A call on a local
- **WHEN** a file calls `prompt.openGlobalPrompt(frame)` on a `val prompt = AirComposerInIdeDriver(…)` beside its
  `fun AirAuthoredScope.openGlobalPrompt()`
- **THEN** it is no use of the extension, and no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_extension_called_on_a_local_in_its_file_is_no_edge`)

### Requirement: Edges: A private member in its file is a use only where the compiler lets it be seen

In the same file, a private member MUST be a use only where the compiler lets the file see it, in every form: in Java
the whole top-level class, nested classes included, for a qualified use (`Inner.helper()`), but only its own type for a
bare name, `this.name` or `super.name`, which outside it mean a member of the class around it (its own `helper`, an
inherited one, a static import). Another top-level class of the Java file MUST see none.

#### Scenario: A Java private member
- **WHEN** a Java top-level class calls `Inner.helper()` of a private member of a nested class, and a bare `helper()`
  outside the nested class, and another top-level class of the file uses the private member
- **THEN** the qualified call is a use, the bare one and the other class's are not
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_private_member_is_a_use_in_the_top_level_java_class_and_the_class_of_its_companion`)

### Requirement: Edges: A Kotlin private member is seen in its type, a companion's in the class around it

In Kotlin a private member MUST be a use in its own type, but in the class around a companion for a member of the
companion. The class around a nested Kotlin object MUST see none of the object's.

#### Scenario: A private member of a companion and of a nested object
- **WHEN** the class around a companion uses its private member, and the class around a nested object uses one of the
  object
- **THEN** the first is a use, the second is not
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_private_member_is_a_use_in_the_top_level_java_class_and_the_class_of_its_companion`)

### Requirement: Edges: An override of a member another commit adds is a use of it

A Kotlin `override` or Java `@Override` member MUST be a use (evidence `override`) of the member of the same name,
kind, receiver and parameter types (a type parameter fitting any) in the supertype it names, when another commit adds
that member: in a subclass, an object expression, an anonymous class or the body of an enum entry, in the same file
too.

#### Scenario: Overrides of a new member
- **WHEN** a commit overrides a member another commit adds, in a subclass, an object expression, an anonymous class or
  the body of an enum entry, in another file or the same one
- **THEN** each is an edge with the evidence `override`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_a_member_another_commit_adds_is_an_edge`)

#### Scenario: A Java anonymous class inside a subclass
- **WHEN** a Java anonymous class of a type, inside a subclass of it, has an `@Override` of a member another commit adds
- **THEN** it is an edge with the evidence `override`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_override_in_an_anonymous_class_inside_a_subclass_is_an_edge`)

### Requirement: Edges: The supertype of an override resolves by its file, path, import or package

The supertype of an override MUST resolve by the same file, its full path, an import or import alias (of the type, or
of the type it is nested in), or the same package in the same module without a type of that name in the file nor an
import of another one; a wildcard import or another package without an import is none.

#### Scenario: A supertype that does not resolve
- **WHEN** an override names its supertype through a wildcard import, or of another package without an import
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_whose_supertype_does_not_resolve_is_no_edge`)

#### Scenario: A supertype that resolves
- **WHEN** the supertype is of the same file, named by its full path, an import or an import alias, or of the same
  package in the same module
- **THEN** the override is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_a_member_another_commit_adds_is_an_edge`)

### Requirement: Edges: An override is no edge where the member it overrides is not certain

An override MUST be no edge when the member it overrides overrides one itself (the supertype of its type has it
already), when the member is one every class has from `Any` or `Object` (`equals`, `hashCode`, `toString`, `clone`,
`finalize`), when the class around it has several supertypes (a function type among them), when the member is a Java
method of a class with supertypes (it may override without `@Override`), or when it is `internal` to another module.

#### Scenario: An override of an override
- **WHEN** the member overrides one of the supertype of its type
- **THEN** an override of it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_an_override_is_no_edge`)

#### Scenario: A member of `Any`
- **WHEN** a commit overrides `equals`, `hashCode`, `toString`, `clone` or `finalize`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_a_member_of_any_is_no_edge`)

#### Scenario: Two supertypes
- **WHEN** the class around the override has two supertypes, a function type among them
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_with_two_supertypes_is_no_edge`)

#### Scenario: A Java method of a class with supertypes
- **WHEN** the member is a Java method of a class with supertypes
- **THEN** an override of it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_method_of_a_class_with_supertypes_is_no_edge_for_an_override`)

#### Scenario: An internal member of another module
- **WHEN** the member is `internal` to another module
- **THEN** an override of it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_in_another_module_of_an_internal_member_is_no_edge`)

### Requirement: Edges: An override of another receiver, kind or a shadowed supertype is no edge

An override MUST be no edge when its extension receiver or its kind (a property for a function) differs, or when a
type of the supertype's name that a scope around it may mean exists (one nested in a declaration around the override,
or a member type that a Java class around the overriding class inherits).

#### Scenario: Another receiver or kind
- **WHEN** an override differs from the member in its extension receiver, or is a property of a function's name
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_another_receiver_or_kind_is_no_edge`)

#### Scenario: A supertype a scope may shadow
- **WHEN** a type of the supertype's name is nested in a declaration around the override, or a Java class around the
  overriding class inherits a member type of that name
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_a_supertype_a_scope_may_shadow_is_no_edge`)

### Requirement: Edges: An override of a member of a type with supertypes needs its exact form

Where the member's type has supertypes, whose members are unknown, the override MUST have its exact form: no type
parameter, each type resolved by both files to the same one, a name that neither file imports or declares only when
the two files are in one package or the name is of the common default imports (`String`, `int`, `StringBuilder`,
`IllegalStateException`, Kotlin's `HashMap`). Elsewhere a type parameter MUST fit any type.

#### Scenario: The exact form
- **WHEN** the member's type has supertypes, and an override has a type parameter, a type the two files resolve to
  different ones, or the same form
- **THEN** only the same form is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_a_member_of_a_type_with_supertypes_needs_its_exact_form`)

#### Scenario: A type name neither file binds
- **WHEN** a parameter type is a name neither file imports or declares
- **THEN** it is the same type only when the two files are in one package or it is `String`, `int`,
  `StringBuilder`, `IllegalStateException` or Kotlin's `HashMap`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_exact_form_needs_one_package_for_a_type_name_neither_file_binds`)

### Requirement: Edges: Other default types and a shared wildcard import are unknown for an exact form

For the exact form of an override whose file and the member's are in two packages, another default type, and a name
each file reaches by the same wildcard import, MUST count as unknown: each file may mean a type of its own package or
of its wildcard imports.

#### Scenario: Another default type or a wildcard import
- **WHEN** the two files are in two packages, and a parameter type of the exact form is another default type, or a
  name both files reach by the same wildcard import
- **THEN** the type is unknown, and the override no edge

### Requirement: Edges: A bare call of a member the supertype gets is an inherited use

A call in a subclass, by a bare name, `this.name` or `super.name`, of an instance function that another commit adds to
the class's supertype, when it is its only one and not `Any` or `Object`, MUST be a use of it (evidence `inherited`),
in another file, when the supertype resolves as an override's does, from Kotlin and Java; the arguments MUST tell the
overloads apart (a new overload beside one of the baseline).

#### Scenario: A bare call and a super call
- **WHEN** a subclass in another file calls a function another commit adds to its only supertype, by a bare name,
  `this.name` or `super.name`
- **THEN** it is an edge with the evidence `inherited`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_call_of_a_member_the_supertype_gets_is_an_edge`, `a_super_call_of_a_new_member_is_an_edge`)

### Requirement: Edges: An inherited call is no use where another receiver or name may take it

An inherited call MUST be no use with two supertypes, in a lambda, an object expression, a Java anonymous class or a
member extension, where the file declares the name or an import may bring it (a wildcard import, a Kotlin default
import), where a receiver further out (a class around it, a companion, a Java outer class) has a supertype of its own,
or where a type of the supertype's name nested in a class around the use wins over its import.

#### Scenario: Two supertypes
- **WHEN** the subclass has two supertypes
- **THEN** the bare call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_call_with_two_supertypes_is_no_edge`)

#### Scenario: A lambda
- **WHEN** the bare call stands in a lambda
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_call_in_a_lambda_is_no_edge`)

#### Scenario: A name the subclass declares or an import may bring
- **WHEN** the file declares the name, or a wildcard import or a Kotlin default import may bring it
- **THEN** the bare call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_call_the_subclass_declares_is_no_edge`, `a_bare_call_a_default_import_may_take_is_no_edge`)

#### Scenario: A receiver further out
- **WHEN** a class around the subclass, a companion or a Java outer class has a supertype of its own
- **THEN** the bare call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_call_with_a_receiver_further_out_with_supertypes_is_no_edge`)

#### Scenario: A nested type of the supertype's name
- **WHEN** a type of the supertype's name is nested in a class around the use
- **THEN** it wins over the import, and the call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_nested_type_of_the_supertype_name_wins_over_its_import`)

#### Scenario: An object expression, an anonymous class or a member extension
- **WHEN** the bare call stands in an object expression, a Java anonymous class or a member extension
- **THEN** it is no edge

### Requirement: Edges: An inherited call is no use of an override, a private or a farther member

An inherited call MUST be no use of a member that overrides one (`override fun dispose()` of a `Base : Disposable`),
of a member extension of the supertype, of a member of a type with supertypes (Kotlin or Java), of a private member or
an `internal` one of another module, or of a member of the supertype's supertype.

#### Scenario: An override the supertype adds
- **WHEN** another commit adds `override fun dispose()` to a `Base : Disposable`, and the subclass calls `dispose()`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_of_an_override_the_supertype_adds_is_no_edge`)

#### Scenario: A supertype with supertypes
- **WHEN** the supertype, Kotlin or Java, has supertypes of its own
- **THEN** the bare call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_supertype_with_supertypes_is_no_edge`)

#### Scenario: A private member
- **WHEN** the member is private, or `internal` of another module
- **THEN** the bare call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_private_member_of_the_supertype_is_no_edge`)

#### Scenario: A member of the grandparent
- **WHEN** the member is of the supertype's supertype
- **THEN** the bare call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_call_through_a_grandparent_is_no_edge`)

#### Scenario: A member extension of the supertype
- **WHEN** the member is a member extension of the supertype
- **THEN** the bare call is no edge

### Requirement: Edges: A use on a receiver of the member's type is a use of it

A use of an instance function or property that another commit adds on a receiver of its type MUST be a use of it
(evidence `receiver`), in another file: a parameter, a local from a constructor, a property of the class, from Kotlin
and Java, or the bare call or `this.name()` in an extension function of the type that nothing else of the file may
take, whose type has no supertypes; the type MUST resolve as a supertype does, and the arguments MUST tell the
overloads apart as for a qualified call.

#### Scenario: A typed parameter, a property, a local from a constructor
- **WHEN** a commit calls a function another commit adds on a parameter of its type, a property of the class, or a
  local from a constructor
- **THEN** each is an edge with the evidence `receiver`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_on_a_typed_parameter_is_an_edge`, `a_call_on_a_property_of_the_class_is_an_edge`, `a_call_on_a_local_from_a_constructor_is_an_edge`)

#### Scenario: An extension function of the type
- **WHEN** an extension function of a type without supertypes calls a new member bare or by `this.name()`
- **THEN** it is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_call_in_an_extension_function_is_an_edge`)

### Requirement: Edges: A receiver of no certain type is no use

A use on a receiver MUST be no use with a receiver of a type parameter, of a type that doesn't resolve (a wildcard
import, another package without an import), of a smart cast, of a member that overrides one, of a member the type
inherits, of a property behind a class with supertypes, or in a lambda inside an extension function.

#### Scenario: A type parameter, an unresolved type, a smart cast
- **WHEN** the receiver is of a type parameter, of a type through a wildcard import or another package without an
  import, or smart cast
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_receiver_of_a_type_parameter_is_no_edge`, `a_receiver_whose_type_does_not_resolve_is_no_edge`, `a_smart_cast_receiver_is_no_edge`)

#### Scenario: An override or an inherited member
- **WHEN** the member overrides one, or the type inherits it
- **THEN** a use on the receiver is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_through_a_receiver_is_no_edge`, `an_inherited_member_through_a_receiver_is_no_edge`)

#### Scenario: A property behind a supertype
- **WHEN** the receiver is a property of a class with supertypes
- **THEN** a use on it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_property_behind_a_supertype_is_no_edge`)

#### Scenario: A lambda inside an extension function
- **WHEN** the bare call stands in a lambda inside an extension function of the type
- **THEN** it is no edge

### Requirement: Edges: A receiver call that a factory, no entry or an extension may take is no use

A use on a receiver MUST be no use of a type that a constructor call gave where its file declares an `invoke` of its
companion or a factory of its name (a declared type keeps its edge), of a call that no entry of a member of a type with
supertypes accepts, or where an extension of the receiver's type of that name that the file sees may take the call.

#### Scenario: A type constructed otherwise
- **WHEN** a local's type comes from a constructor call, and its file declares an `invoke` of the companion or a
  factory of the type's name
- **THEN** a call on it is no edge, and one on a declared type keeps its edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_type_constructed_otherwise_is_no_receiver`)

#### Scenario: No entry accepts the call
- **WHEN** the member's type has supertypes, and no entry of the member accepts the call
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_receiver_call_no_entry_accepts_is_no_edge`)

#### Scenario: An extension of the receiver's type
- **WHEN** the file sees an extension of the receiver's type of that name that may take the call
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_receiver_call_an_extension_of_its_type_may_take_is_no_edge`)

### Requirement: Edges: A local shadows a property, a member wins over an extension, and the file of the member

A local MUST shadow a property with its own type, a member of the receiver's type MUST win over an extension of its
name, and in the file of the member a use on a receiver of another type MUST be no use of it, unless the type is a
subtype of the file whose supertype names the member's type.

#### Scenario: A shadowing local
- **WHEN** a local of another type shadows a property of the member's type
- **THEN** the receiver has the local's type
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_shadowing_local_takes_its_own_type`)

#### Scenario: A member of the receiver's type wins over an extension
- **WHEN** a commit adds an extension of a name the receiver's type has as a member, and a call on the receiver
- **THEN** the call is no use of the extension
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_extension_on_a_receiver_with_a_member_of_the_name_is_no_edge`)

#### Scenario: Another receiver in the file of the member
- **WHEN** the file of the member calls its name on a receiver of another type, or of a subtype of the file whose
  supertype names the member's type
- **THEN** the first is no use, the second is one
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_same_file_use_on_a_receiver_of_another_type_is_no_use_of_the_file`, `a_same_file_use_on_a_subtype_of_the_file_is_a_use`)

### Requirement: Edges: A Java accessor call of a Kotlin property is a use of the property

A Java call of an accessor of a property that another commit adds to Kotlin MUST be a use of the property by the rules
of its receiver, inheritance or static path (`m.getSize()`, `m.setLabel(text)`, `m.isReady()`, `Owner.getSize()` of a
`@JvmStatic` property of a companion, `Owner.Companion.getCount()`, `Registry.INSTANCE.getName()`,
`UtilKt.getName()`).

#### Scenario: Accessors on a receiver
- **WHEN** Java calls `m.getSize()`, `m.setLabel(text)` or `m.isReady()` of properties another commit adds
- **THEN** each is an edge to the property
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_accessor_call_of_a_kotlin_property_is_an_edge`, `a_boolean_property_has_its_own_accessor_names`)

#### Scenario: Accessors of a companion or an object
- **WHEN** Java calls `Owner.getSize()` of a `@JvmStatic` property of a companion, `Owner.Companion.getCount()` or
  `Registry.INSTANCE.getName()`
- **THEN** each is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_accessor_of_a_companion_or_object_property_is_an_edge`)

#### Scenario: An accessor through the facade
- **WHEN** Java calls `UtilKt.getName()` of a top-level property another commit adds
- **THEN** it is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_accessor_of_a_top_level_property_through_the_facade_is_an_edge`)

### Requirement: Edges: A Kotlin property access of a Java getter is a use of it

A Kotlin property access on a receiver MUST be a use of a Java getter without parameters (`m.count` of `getCount()`).

#### Scenario: A property access of a getter
- **WHEN** Kotlin reads `m.count` on a receiver of a Java type to which another commit adds `getCount()`
- **THEN** it is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_kotlin_property_access_of_a_java_getter_is_an_edge`, `a_java_field_read_needs_a_field_of_the_property`)

### Requirement: Edges: A Java use through the class of a Kotlin file is a facade use

A Java use of a top-level Kotlin function, `const` or `@JvmField` property through the class of its file MUST be one
(evidence `facade`): `UtilKt.helper()` with the class imported or of its package, `a.UtilKt.helper()`,
`import static a.UtilKt.helper` or `a.UtilKt.*` and a bare `helper()`, the class being the file's `@file:JvmName` if it
has one.

#### Scenario: The forms of a facade call
- **WHEN** Java calls `UtilKt.helper()` with the class imported or of its package, `a.UtilKt.helper()`, or a bare
  `helper()` after `import static a.UtilKt.helper` or `a.UtilKt.*`
- **THEN** each is an edge with the evidence `facade`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_call_through_the_facade_is_an_edge`)

#### Scenario: A file of another class name
- **WHEN** the Kotlin file has `@file:JvmName("Util")`, or its name starts with a digit
- **THEN** its class is `Util`, or the name of the file as the JVM gives it, and only a use of that class is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_facade_of_another_name_is_no_edge`, `a_facade_of_a_file_named_with_a_digit_or_a_script`)

### Requirement: Edges: A `@JvmField` property of a companion is a field of the class around it

A `@JvmField` property of a companion MUST be a field of the class around it (`Owner.field`).

#### Scenario: A field of the owner
- **WHEN** Java reads `Owner.field` of a `@JvmField` property of a companion another commit adds
- **THEN** it is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_accessor_of_a_companion_or_object_property_is_an_edge`)

### Requirement: Edges: A Java accessor of a property without that accessor is no use

No accessor of a `@JvmField`, `const` or `@get:JvmName` property, of an `internal` member (Java names it by a mangled
name; an `internal` top-level property keeps its accessor's name), of an extension property, nor one beside a function
of the accessor's name, no setter of a `val` or a `private set`, no `getIsReady()` of an `isReady`, and no getter
called with an argument MUST be a use.

#### Scenario: A property without the accessor
- **WHEN** Java calls `m.getField()` of a `@JvmField` property, the getter of a `@get:JvmName` one, or `m.getSize(1)`
  with an argument
- **THEN** none is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_accessor_call_of_a_property_without_that_accessor_is_no_edge`)

#### Scenario: An accessor name of a real member
- **WHEN** the type has a function of the accessor's name beside the property
- **THEN** the accessor call is no use of the property
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_accessor_name_of_a_real_member_is_no_edge`)

#### Scenario: A setter without a visible setter
- **WHEN** Java calls the setter of a `val` or of a `private set`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_setter_of_a_property_without_a_visible_setter_is_no_edge`)

#### Scenario: A boolean property
- **WHEN** Java calls `getIsReady()` of an `isReady`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_boolean_property_has_its_own_accessor_names`)

#### Scenario: An `internal` member and an `internal` top-level property
- **WHEN** Java calls `m.isSecret()` or `m.setSecret(1)` of `internal` members, and `UtilKt.getConfig()` of an
  `internal val config` at the top level of `util.kt`
- **THEN** the calls of the members are no edge, the one of the top-level property is an edge with the evidence
  `facade`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_boolean_property_has_its_own_accessor_names`, `a_java_setter_of_a_property_without_a_visible_setter_is_no_edge`, `a_java_field_read_needs_a_field_of_the_property`)

#### Scenario: A `const` and an extension property
- **WHEN** Java calls the accessor of a `const` property or of an extension property
- **THEN** it is no use

### Requirement: Edges: A Java use of a renamed, synthetic or unexposed Kotlin declaration is no use

No Java use of a `@JvmName` or `@JvmSynthetic` declaration by its Kotlin name, no Java field read of a property
without `@JvmField` or `lateinit`, no `Owner.Companion.field` of a `@JvmField`, no `Owner.getCount()` of a property
without `@JvmStatic`, no facade of a script, and no `UtilKt` beside a `@file:JvmName("Util")` MUST be a use.

#### Scenario: A renamed or synthetic declaration
- **WHEN** Java uses a `@JvmName` or `@JvmSynthetic` declaration by its Kotlin name
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_use_of_a_renamed_or_synthetic_kotlin_declaration_is_no_edge`)

#### Scenario: A field read
- **WHEN** Java reads `m.size` of a property without `@JvmField` or `lateinit`, and of a `@JvmField` and a `lateinit`
  one
- **THEN** only the last two are edges
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_field_read_needs_a_field_of_the_property`)

#### Scenario: A companion's field and a non-static accessor through the owner
- **WHEN** Java reads `Owner.Companion.field` of a `@JvmField`, or calls `Owner.getCount()` of a property without
  `@JvmStatic`
- **THEN** neither is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_accessor_of_a_companion_or_object_property_is_an_edge`)

#### Scenario: A script and a renamed facade
- **WHEN** Java calls `BuildKt.script()` of a script, or `UtilKt` of a file with `@file:JvmName("Util")`
- **THEN** neither is an edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_facade_of_a_file_named_with_a_digit_or_a_script`, `a_facade_of_another_name_is_no_edge`)

### Requirement: Edges: Kotlin reads of Java getters and Kotlin uses of a facade class are no use

No Kotlin read of a Java getter beside a field of the property's name, no Kotlin access of a Java method with
parameters, no Kotlin use of `UtilKt`, and no `UtilKt` of another package without an import MUST be a use.

#### Scenario: A method with parameters
- **WHEN** Kotlin reads `m.at` of a Java method `getAt(int index)`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_kotlin_property_access_of_a_java_getter_is_an_edge`)

#### Scenario: A getter beside a field
- **WHEN** Kotlin reads a property of a Java getter's name where the Java type has a field of that name
- **THEN** it is no edge

#### Scenario: A facade class from Kotlin
- **WHEN** Kotlin calls `UtilKt.helper()` of a top-level function another commit adds
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_facade_of_another_name_is_no_edge`)

#### Scenario: A facade class of another package without an import
- **WHEN** Java uses `UtilKt` of another package without an import
- **THEN** it is no edge

### Requirement: Edges: The declarations of a JVM key are entries with their origins

The declarations of a key of a JVM type or a member of one MUST be entries told apart by kind, extension receiver and
parameter types (with their arrays, a vararg counted), each with its origin: the baseline, or the commit that created
it or changed its form, one entry removed and one added by a commit being the same entry in a new form.

#### Scenario: Parameter types with their arrays
- **WHEN** two overloads differ by an array or a vararg of a parameter
- **THEN** they are two entries
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_entry_is_its_parameter_types_with_their_arrays`)

#### Scenario: Two overloads of a new file
- **WHEN** a commit adds a file with two overloads
- **THEN** both are entries
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`two_overloads_in_a_new_file_keep_both`)

#### Scenario: A new form of an entry
- **WHEN** a commit removes an entry and adds it in a new form
- **THEN** it is the same entry, its origin that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_new_form_of_an_entry_is_an_edge_for_the_uses_that_need_it`)

### Requirement: Edges: An override depends on what each entry it may mean needs

An override MUST depend on what each entry it may mean needs: the commit of the latest form of the entry that the form
before didn't accept, with the commits back along the forms of the entry to its creation. It MUST depend on the newest
commit before its own that all of them need, and on none when that is the baseline or when they need no commit in
common.

#### Scenario: An override of a new overload
- **WHEN** a commit adds an overload of a name the type had, and another overrides it
- **THEN** the new overload is an addition, and the override depends on its commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_a_new_overload_is_an_edge`)

#### Scenario: An override of an overload of the baseline
- **WHEN** an override means an overload of the baseline
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_override_of_a_member_the_range_does_not_add_is_no_edge`)

#### Scenario: Two overloads of a new file
- **WHEN** an override may mean either of two overloads of a new file
- **THEN** both are entries, and it depends on the commit both need
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`two_overloads_in_a_new_file_keep_both`)

#### Scenario: The cases of the report
- **WHEN** the cases of the report run
- **THEN** each override depends on the commit they name
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`the_cases_of_the_report`)

### Requirement: Edges: A use that tells no overloads apart depends on the commit that added the key

A use of a function that tells no overloads apart (a call of a member of a type with a supertype other than `Any` or
`Object`) MUST depend on the commit that added the key (the oldest origin of the entries that accept it), and on none
when one entry is of the baseline. A use of a value or a type MUST depend on the step whose reach let it in (see the
forms below).

#### Scenario: A call of a member of a type with a supertype
- **WHEN** a call means a member of a type with a supertype, whose key a commit added
- **THEN** it depends on the commit that added the key, and on none when an entry is of the baseline
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_needs_the_commit_that_added_the_key_of_its_name`)

#### Scenario: The cases of the report
- **WHEN** the cases of the report run
- **THEN** each use depends on the commit they name
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`the_cases_of_the_report`)

### Requirement: Edges: A call of a JVM member tells its overloads apart by its arguments

A call of a member of a JVM type without a supertype other than `Any` or `Object` MUST tell its overloads apart by its
arguments: it MUST depend, as an override, on the newest commit that every entry that accepts it needs.

#### Scenario: A new overload the arguments select
- **WHEN** a commit adds an overload beside one of the baseline, and a call passes what only the new one accepts
- **THEN** the call depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_of_a_new_overload_that_the_arguments_select_is_an_edge`)

#### Scenario: The old overload, or arguments that cannot tell them apart
- **WHEN** a call passes what the old overload accepts, or arguments that both accept, or an unknown argument beside
  overloads of two commits
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_of_the_old_overload_is_no_edge`, `a_call_the_arguments_cannot_tell_apart_is_no_edge`, `overloads_of_two_commits_and_an_unknown_argument_are_no_edge`)

#### Scenario: A type with supertypes
- **WHEN** the member's type has a supertype other than `Any` or `Object`
- **THEN** its calls tell no overloads apart, and a call of a new overload is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_overload_of_a_type_with_supertypes_is_no_edge_for_calls`)

#### Scenario: The key and the cases of the report
- **WHEN** a call is accepted by entries of several commits, and the cases of the report run
- **THEN** each call depends on the newest commit every accepting entry needs
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_needs_the_commit_that_added_the_key_of_its_name`, `the_cases_of_the_report`)

### Requirement: Edges: An entry accepts a call that fills its parameters

An entry MUST accept a call whose positional arguments fill its parameters in order (a vararg, the last, all of them
left), whose named arguments name parameters not filled yet, whose last lambda fills the last parameter (a Kotlin
trailing lambda) or its place, and that leaves only parameters with a default or a vararg, each literal fitting its
parameter.

#### Scenario: Named arguments, a lambda and a vararg
- **WHEN** calls of new names pass named arguments, a trailing lambda and varargs
- **THEN** each accepting entry is told apart, and the calls keep their edges
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`new_names_keep_their_edges_with_named_arguments_lambda_vararg_and_spread`)

### Requirement: Edges: A literal fits only parameters of its family

A text, a number or a boolean MUST fit no parameter of another of these families (text: `String`, `CharSequence`;
number: the Kotlin and Java number types, their boxes and `Number`, an `int` for a `double` included; boolean) nor a
class, an interface, an object or an enum that the member's file declares and doesn't shadow by an import of its name.

#### Scenario: Java widening
- **WHEN** a Java call passes an `int` literal to an overload of a `double`
- **THEN** that overload accepts it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`java_widening_accepts_a_number_for_a_double`)

#### Scenario: A literal of another family
- **WHEN** a call passes a text, beside an old overload of an `int` and a new one of a `String`
- **THEN** the old overload rejects it, and the call depends on the new one's commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_of_a_new_overload_that_the_arguments_select_is_an_edge`)

#### Scenario: A literal for a type of the member's file
- **WHEN** a call passes a literal to a parameter of a class, an interface, an object or an enum that the member's
  file declares and doesn't shadow by an import of its name
- **THEN** the entry rejects the call

### Requirement: Edges: A spread or an entry without a signature accepts any call

A spread, a named argument before a positional one, a vararg before the last parameter, and an entry without a
signature (a type, a property) MUST accept any call, and a call of a name every class has (`equals`, `hashCode`,
`toString`, `clone`, `finalize`) MUST tell no overloads apart.

#### Scenario: A spread
- **WHEN** a call spreads an array
- **THEN** every entry accepts it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`new_names_keep_their_edges_with_named_arguments_lambda_vararg_and_spread`)

#### Scenario: A name every class has
- **WHEN** a commit adds an overload of `equals`, `hashCode`, `toString`, `clone` or `finalize`, and another calls it
- **THEN** the call tells no overloads apart, and it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_of_a_name_every_class_has_is_no_edge`)

#### Scenario: Named before positional, a vararg before the last, no signature
- **WHEN** a call passes a named argument before a positional one, an entry has a vararg before its last parameter,
  or an entry is a type or a property
- **THEN** the entry accepts any call

### Requirement: Edges: A bare name of an import tells no overloads apart where its file may take it first

A bare name that an import brings in (Java's `import static`, a Kotlin import of a member of an object) MUST tell no
overloads apart when its file shows something that may take it first: an enclosed call, a declaration of its name in
the file, another import that may bring the name, a type around it (or in Kotlin the companion of one) with a
supertype other than `Any` or `Object`, or a Kotlin extension around it.

#### Scenario: What the file may take first
- **WHEN** a bare call of an imported name stands in an enclosed call, beside a declaration of its name, a second
  import of it, as `import static q.Other.add`, a wildcard import, as `import static q.Other.*`, a Kotlin function of
  the default imports, as `error`, inside a type with a supertype or a Kotlin extension
- **THEN** it tells no overloads apart, and a call of a new overload is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_bare_name_of_an_import_that_its_file_may_take_first_is_no_edge`)

### Requirement: Edges: An extension of the member's type the range doesn't show is a known limit

A call MUST be told apart by the overloads of the member's type and the extensions the range shows. An extension of
the member's type that the range doesn't show (`fun Registry.register(id: Int)` beside a new member
`register(id: Long)`) may take a call the old overloads reject; the check does not look at it, a known limit.

#### Scenario: An extension outside the range
- **GIVEN** `fun Registry.register(id: Int)`, which the range doesn't show
- **WHEN** a commit adds the member `register(id: Long)`, and another calls `register` with an argument the old
  overloads reject
- **THEN** the call depends on the new member's commit, though the extension may take it

### Requirement: Edges: A new form of an entry is a new step, a new body is none

A commit that keeps the identity of an entry and changes its form (the names of its parameters, a default value, a
vararg, its reach, whether a subtype may override it, or `@JvmStatic`) MUST make a new step of the entry, with the
commit for its origin and the old form for its predecessor; a change of its body alone MUST keep its origin.

#### Scenario: A changed body
- **WHEN** a commit changes only the body of a member another commit added
- **THEN** the entry keeps its origin, and a use depends on the commit that added it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_changed_body_is_no_new_form`)

#### Scenario: A changed form
- **WHEN** a commit changes the names of the parameters, a default, a vararg, the reach, whether a subtype may
  override, or `@JvmStatic` of an entry
- **THEN** the entry has a new step of that commit, its predecessor the old form
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_that_needs_the_new_parameter_is_an_edge_to_the_commit_that_adds_it`)

### Requirement: Edges: An older form is judged by its own attributes and the entry's present rest

Each older form MUST be judged by the attributes of its form, and by the rest (whether it is static, whether it
overrides) as the entry is now.

#### Scenario: Static and overriding as the entry is now
- **WHEN** an older form of an entry was not static or did not override, and the entry is now static or overriding
- **THEN** the older form is judged as static and as overriding as the entry is now
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_older_form_is_judged_as_static_as_the_entry_is_now`, `an_older_form_is_judged_as_overriding_as_the_entry_is_now`)

### Requirement: Edges: A use depends on the step whose form before rejected it, by reach

A use MUST depend on the step whose form before rejected it: by its reach from the file of the use (a private member
made `internal` and used by `Owner.member` from another file of its module, an `internal` one made public and used
through an import from another module, an inner class made a public nested one and named from another package or
module).

#### Scenario: A widened member
- **WHEN** a commit makes a private member `internal` and another file of its module uses `Owner.member`, or makes an
  `internal` one public and another module imports it, or makes an inner class a public nested one named from another
  package or module
- **THEN** the use depends on the commit that widened it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_widened_member_is_an_edge_for_a_use_the_old_reach_forbids`)

### Requirement: Edges: A use depends on the step that made it overridable, callable or static

A use MUST depend on the step whose form before rejected it: for an override by whether the form was overridable (a
member made `open`), for a call by its parameters (a call that leaves out an argument a new default makes optional,
passes the elements of a Java array made a vararg, or names a renamed parameter by its new name), and for a Java call
of a member of a companion by the class around it by `@JvmStatic`.

#### Scenario: A member made `open`
- **WHEN** a commit makes a member `open`, and another overrides it
- **THEN** the override depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_member_made_open_is_an_edge_for_an_override_only`)

#### Scenario: A new default, a vararg, a new parameter
- **WHEN** a call leaves out an argument a new default makes optional, passes the elements of a Java array made a
  vararg, or passes a parameter a commit adds
- **THEN** it depends on the commit of that form
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_new_default_or_vararg_is_an_edge_for_a_call_that_needs_it`, `a_call_that_needs_the_new_parameter_is_an_edge_to_the_commit_that_adds_it`)

#### Scenario: A renamed parameter
- **WHEN** a call names a renamed parameter by its new name
- **THEN** it depends on the commit that renamed it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_renamed_parameter_is_an_edge_for_a_call_by_its_new_name`)

#### Scenario: `@JvmStatic` added later
- **WHEN** a Java call of a companion member goes through the class around it, and a later commit makes the member
  `@JvmStatic`
- **THEN** it depends on the later commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_jvm_static_added_later_is_an_edge_for_a_java_call_through_the_owner`)

### Requirement: Edges: A use the older form accepted too does not depend on the new step

A use that the older form accepted too MUST NOT depend on the new step (the `internal` member from its module, a
private one in its object, a call that passes the argument or a renamed parameter by position, a call of a member made
`open`, the inner class from its package or module, an override of a member that stopped overriding, also in a Java
class that dropped its supertype).

#### Scenario: A call both forms accept
- **WHEN** a later commit adds a parameter, and a call is accepted by both forms
- **THEN** the call keeps its edge to the commit that added the member
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_parameter_added_later_keeps_the_edge_of_a_call_both_forms_accept`)

#### Scenario: A call by position and a call of a member made `open`
- **WHEN** a call passes the argument or a renamed parameter by position, or calls a member made `open`
- **THEN** it does not depend on the new step
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_new_default_or_vararg_is_an_edge_for_a_call_that_needs_it`, `a_renamed_parameter_is_an_edge_for_a_call_by_its_new_name`, `a_member_made_open_is_an_edge_for_an_override_only`)

#### Scenario: Uses the old reach allowed
- **WHEN** the `internal` member is used from its module, a private one in its object, the inner class from its
  package or module
- **THEN** the use does not depend on the widening
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_widened_member_is_an_edge_for_a_use_the_old_reach_forbids`)

#### Scenario: A member that stopped overriding
- **WHEN** an override means a member that stopped overriding, also in a Java class that dropped its supertype
- **THEN** it does not depend on that step
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_older_form_is_judged_as_overriding_as_the_entry_is_now`)

### Requirement: Edges: A step needs the commits back along the forms of its entry

A step MUST need the commits back along the forms of its entry too: a call that two overloads of one commit accept,
one of them in a form a later commit gave it, MUST depend on the first, and a call that only the form its own commit
gives accepts on the older commit of that entry whose parameter it passes.

#### Scenario: A new form needs the commit that created its entry
- **WHEN** a call is accepted by two overloads of one commit, one of them in a form a later commit gave it
- **THEN** it depends on the first commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_new_form_needs_the_commit_that_created_its_entry`)

#### Scenario: A form its own commit gives
- **WHEN** a call is accepted only by the form its own commit gives
- **THEN** it depends on the older commit of that entry whose parameter it passes
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_use_of_a_form_its_own_commit_gives_needs_the_older_steps_of_the_entry`)

### Requirement: Edges: An older form is no stricter than the compiler

An older form MUST be no stricter than the compiler: a private member MUST reach the whole top-level class in Java,
nested classes included, for a bare name too, and in Kotlin the class around a nested object or a companion (one with
a name of its own included, which the summary doesn't tell from a nested object); an `internal` one MUST reach a file
whose module no build file tells.

#### Scenario: An older private form
- **WHEN** an older form was private, and a use stands in the top-level Java class or a nested class of it, or in the
  class around a nested Kotlin object or companion
- **THEN** the older form accepts it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_older_private_form_reaches_the_top_level_java_class_and_the_class_around_a_nested_kotlin_object`)

#### Scenario: An older `internal` form
- **WHEN** an older form was `internal`, and a use stands in a file whose module no build file tells
- **THEN** the older form accepts it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_internal_older_form_reaches_a_file_of_an_unknown_module`)

### Requirement: Edges: An import of a member of a type without supertypes is judged by each form's reach

An import of a member of a type without a supertype other than `Any` or `Object`, which names all its overloads, MUST
be judged by the reach of each form, as a call by its arguments.

#### Scenario: An import of a widened member
- **WHEN** an import names a member whose forms differ in reach
- **THEN** it depends on the step whose reach lets it in
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_import_of_a_member_of_a_type_without_supertypes_is_judged_by_its_reach`)

### Requirement: Edges: A use that tells no overloads apart needs the key only

A use of a function that tells no overloads apart (a call of a type with a supertype other than `Any` or `Object`, an
import of a member of one) MUST need the key only, as an overload of the supertype may take it while the older form is
private; a use of a value or a type MUST need the step whose reach let it in.

#### Scenario: A widened member of a type with supertypes
- **WHEN** a commit widens a member of a type with supertypes, and another file calls it
- **THEN** the call needs the key only, and it is no edge to the widening
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_widened_member_of_a_type_with_supertypes_is_no_edge_for_a_call`)

#### Scenario: A value or a type
- **WHEN** a use of a widened value or type stands where only the new reach lets it in
- **THEN** it needs the step of that reach

### Requirement: Edges: A top-level function or property has no forms

A top-level function or property MUST have no forms, only presence.

#### Scenario: A changed form of a top-level declaration
- **WHEN** a commit changes the form of a top-level function or property another commit added
- **THEN** a use is no edge to the change
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_changed_form_of_a_top_level_function_or_property_is_no_edge`)

### Requirement: Edges: An extension of another file while the member is private is a known limit

The reach of a use MUST be judged by the forms of the member alone. An extension of another file
(`fun Panes.helper()`, `val Panes.size`) may take a use while the member of its name is private; the check does not
look at it, a known limit.

#### Scenario: An extension beside a private member
- **GIVEN** `fun Panes.helper()` or `val Panes.size` in another file
- **WHEN** a commit widens a private member `helper` or `size` of `Panes`, and another file uses it where only the new
  reach lets it in
- **THEN** the use depends on the step that widened the member, though the extension may have taken it before

### Requirement: Edges: In the file of an old key a use means only a member of its type

In the file of a key that was there before the range, a use MUST mean it only where it can mean only a member of its
type: a bare name, `this` or `super` whose innermost type (an enum entry's body counts as one) is that type, in no
enclosed call (`buildList { add(0, item) }`, `apply { this.add(0, item) }`, an object expression, a Java anonymous
class) and in no Kotlin member extension function or accessor of a member extension property.

#### Scenario: A call on another receiver
- **WHEN** the file of an old type calls `list.add(0, item)` of a `MutableList`, or makes a bare call at the top
  level, beside a new overload `add`
- **THEN** neither is a use of the new overload, and `this.add(0, item)` in a member of the type is one
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_on_another_receiver_in_the_same_file_is_no_edge`)

#### Scenario: Enclosed calls and member extensions
- **WHEN** the call stands in `buildList { add(0, item) }`, `apply { this.add(0, item) }`, an object expression, a
  Java anonymous class or a nested class, or a member extension whose receiver comes first
  (`fun MutableList<String>.fill() { add(0, "x") }`)
- **THEN** it is no use of the new overload
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_in_a_java_anonymous_or_nested_class_is_no_edge`, `a_call_another_receiver_may_take_is_no_edge`)

### Requirement: Edges: A Kotlin call in the file of an old key needs no other function its fallback may take

In Kotlin, which falls back to the receivers further out and then to the top level when the members reject a call, a
use in the file of an old key MUST also have no function or value of its name in a type around that type, in the
companion of one of these or of it, or at the top level of the file, no supertype other than `Any` or `Object` of any
of these receivers, no import of its name, no wildcard import and no function of its name among the common ones of the
default imports.

#### Scenario: A receiver further out
- **WHEN** `foo(1)` stands in an inner class whose outer class, or its companion, has a `foo(x: Int)`
- **THEN** it is no use of a new overload beside an old one that rejects it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_another_receiver_may_take_is_no_edge`)

#### Scenario: A nested class with a function of its own
- **WHEN** `add(0, "x")` stands in a nested class with an `add` of its own
- **THEN** it is no use of the new overload of the outer type
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_in_a_java_anonymous_or_nested_class_is_no_edge`)

#### Scenario: An import or a default import
- **WHEN** `add(0, "x")` stands in a file with `import q.add` or a wildcard import, or `error("boom")` beside an old
  `error(code: Int)`, a function of the common default imports (`error`, `println`, `require`)
- **THEN** it is no use of the new overload
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_an_import_or_a_default_import_may_take_is_no_edge`)

#### Scenario: A type with supertypes
- **WHEN** the type has a supertype other than `Any` or `Object`
- **THEN** a bare call in it is no use of the new overload, and one in a type of `Any()` or none is
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_overload_of_a_type_with_supertypes_is_no_edge_for_calls`)

#### Scenario: A function of its name at the top level
- **WHEN** a function or value of the name stands at the top level of the file or in a type around
- **THEN** the call is no use of the new overload

### Requirement: Edges: Another file's function or an unlisted default function is a known limit

In the file of an old key, a call MUST be judged by what that file shows and by the list of the common default
functions. A top-level function of the package in another file, or a default function the list leaves out, may take
such a call too; the check does not look at it, a known limit.

#### Scenario: A function the file does not show
- **GIVEN** a top-level function of the name in another file of the package, or a default function not in the list
- **WHEN** the file of an old key calls the name with arguments that only a new member overload accepts
- **THEN** the call is a use of the new member overload, though that function may take it

### Requirement: Edges: A new key and an old top-level type keep the same file as evidence

A key the range adds MUST keep the same file as its evidence elsewhere in the file (a bare call in an extension of its
type at the top level). A top-level type of the baseline MUST be meant in its file by a bare name, in no enclosed call,
that no other declaration of the file has (a function of its name beside it) and, in Kotlin, no import may bring.

#### Scenario: A constructor call in the file of an old type
- **WHEN** the file of a type of the baseline calls its constructor by a bare name, with a parameter a commit adds
- **THEN** it is an edge to that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_constructor_call_that_needs_a_new_parameter_is_an_edge`)

#### Scenario: A function of the type's name beside it
- **WHEN** the file declares a function of the type's name beside it, or a Kotlin import may bring the name
- **THEN** a bare call is no use of the type's constructors
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_kotlin_call_that_a_function_of_its_name_may_take_is_no_edge`)

#### Scenario: A new key
- **WHEN** a key the range adds is used by a bare call in an extension of its type at the top level of its file
- **THEN** the same file is its evidence
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_on_another_receiver_in_the_same_file_is_no_edge`)

### Requirement: Edges: The key of a JVM type holds the type and its constructors

The key of a JVM type MUST hold the type itself and one entry per constructor (the implicit `()` included), each with
its origin, as a member's overloads.

#### Scenario: The implicit constructor
- **WHEN** a type declares no constructor, and a commit adds one with parameters
- **THEN** the implicit `()` is an entry of the baseline that takes no arguments, and a call with arguments needs the
  new one
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_implicit_constructor_takes_no_arguments`)

### Requirement: Edges: A call of a type means its constructors, any other use the type

A type position, an import and any use but a call MUST mean the type; a call of the type, a Kotlin constructor
invocation of a supertype, Java's `new` and `this(…)` and `super(…)` included, its constructors, or the type too where
its file declares an `invoke` of its companion or the range adds a function of its name that accepts the call, judged
by the arguments that tell the constructors apart.

#### Scenario: A type reference
- **WHEN** a commit adds a constructor parameter, and another names the type in a type position or an import
- **THEN** that use means the type, and it is no edge to the new parameter
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_type_reference_is_no_edge_for_a_new_constructor_parameter`)

#### Scenario: `super(…)` and `this(…)`
- **WHEN** a `super(…)` call passes a constructor parameter a commit adds, a Kotlin `super(…)` stands beside two
  supertypes, or a `this(…)` calls a constructor of its own commit
- **THEN** the first is an edge, the second and the third are not
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_super_call_that_needs_a_new_constructor_parameter_is_an_edge`, `a_kotlin_super_call_with_two_supertypes_is_no_edge`, `a_this_call_is_no_edge_across_its_own_commit`)

#### Scenario: A function of the type's name
- **WHEN** the range adds a function of the name of a type that accepts the call
- **THEN** the call means the type too
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_that_a_function_of_the_name_of_a_new_type_may_take_needs_the_type`)

### Requirement: Edges: A constructor call depends on what every constructor that accepts it needs

A call MUST depend, as one of a member, on the newest commit that every constructor that accepts it needs, told apart
by its arguments also for a type with supertypes, which declare none of them: a call that passes a parameter a commit
adds, by position or by name, a call that only a constructor a commit adds accepts (a text for an `Int`), and one with
arguments that the implicit `()` rejects need that commit, in the file of the type too.

#### Scenario: A new parameter by name or by position
- **WHEN** a commit adds a constructor parameter, and calls pass it by name and by position, also in the file of the
  type
- **THEN** each depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_constructor_call_that_needs_a_new_parameter_is_an_edge`)

#### Scenario: A supertype invocation with a new named argument
- **WHEN** a Kotlin constructor invocation of a supertype names a parameter a commit adds
- **THEN** it depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_supertype_constructor_call_with_a_new_named_argument_is_an_edge`)

#### Scenario: A Java `new` only a new constructor accepts
- **WHEN** a Java `new` passes a text, which only a constructor a commit adds accepts beside one of an `Int`
- **THEN** it depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_java_new_that_only_a_new_constructor_accepts_is_an_edge`)

#### Scenario: Arguments the implicit constructor rejects
- **WHEN** a call passes arguments to a type whose implicit `()` a commit replaced
- **THEN** it depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_implicit_constructor_takes_no_arguments`)

### Requirement: Edges: A use the old constructors accept as well needs no new constructor

A type position, an import, a call that the constructor of the baseline accepts as well (an expression for the old
`Int` beside a new `String`), and a call that leaves a new parameter to its default MUST NOT need the commit of a new
constructor or parameter.

#### Scenario: A call both constructors accept
- **WHEN** a call passes an expression beside an old constructor of an `Int` and a new one of a `String`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_both_constructors_accept_is_no_edge`)

#### Scenario: A call that leaves the new parameter to its default
- **WHEN** a call leaves a new parameter to its default
- **THEN** it is no edge to the commit that added it
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_constructor_call_that_needs_a_new_parameter_is_an_edge`)

#### Scenario: A type position and an import
- **WHEN** a use of the type is a type position or an import
- **THEN** it needs no constructor
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_type_reference_is_no_edge_for_a_new_constructor_parameter`)

### Requirement: Edges: A constructor's form is its parameters, its reach and its type's reach

Each constructor's form MUST be its parameters, its reach and the reach of its type, judged as a member's: a private
constructor made `internal` MUST be what a call from another file of its module needs, a type made public what a use
or a call (a qualified one too) needs from another package (Java) or module (Kotlin), and a use that the old reach
allowed MUST NOT.

#### Scenario: A widened type
- **WHEN** a commit makes a type public, and a use or a call, a qualified one too, stands in another package (Java) or
  module (Kotlin), and another where the old reach allowed it
- **THEN** the first depends on that commit, the second not
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_widened_type_is_an_edge_for_a_use_the_old_reach_forbids`)

#### Scenario: A widened constructor
- **WHEN** a commit makes a private constructor `internal`, and another file of its module calls it
- **THEN** the call depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_widened_constructor_is_an_edge_for_a_call_the_old_reach_forbids`)

### Requirement: Edges: A Java use reaches an `internal` Kotlin type, and so does a file of no known module

A Java use MUST reach an `internal` Kotlin type or constructor from any module, and an `internal` constructor MUST
reach a use of a file whose module no build file tells, as its type does.

#### Scenario: An internal constructor of no known module
- **WHEN** a file whose module no build file tells calls an `internal` constructor
- **THEN** the constructor takes the call
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_internal_constructor_of_no_known_module_takes_a_call`)

#### Scenario: A Java use of an internal Kotlin type
- **WHEN** Java of another module uses an `internal` Kotlin type or constructor
- **THEN** it reaches it

### Requirement: Edges: A bare Kotlin call of a type's name is judged by its arguments only where nothing else may take it

A Kotlin call of a bare name falls back from the constructors to the functions of the name, so it MUST be judged by its
arguments only where its file declares nothing else of the name and no other import may bring one (a wildcard import
beside the import of the type), and else need the key only, as a call of a member of a type with supertypes; Java's
`new` only where its file declares nothing else of the name (a nested type of it).

#### Scenario: A function of its name may take the call
- **WHEN** the file of a call declares a function of the type's name, or has a wildcard import beside the import of
  the type
- **THEN** the call needs the key only, and it is no edge to a new parameter
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_kotlin_call_that_a_function_of_its_name_may_take_is_no_edge`)

#### Scenario: A Java `new` beside a nested type of the name
- **WHEN** a Java file declares a nested type of the name and calls `new` of it
- **THEN** the call is not judged by the constructors of the other type

### Requirement: Edges: Functions and members of a type's name outside the file are a known limit

A call of a type's name MUST be judged by the type's constructors, an `invoke` of its companion in its file and the
functions of its name that the range adds. A function of the type's name in a file of the package that the range doesn't change, an `invoke` of its companion
declared in another file, and a member of a receiver of the name may take such a call too; the check does not look at
them, a known limit.

#### Scenario: A function of the name the range does not show
- **GIVEN** a function of the type's name in an unchanged file of the package, or an `invoke` of its companion in
  another file
- **WHEN** a call of the type's name passes a new constructor parameter
- **THEN** it depends on the commit of the parameter, as a call of the constructors, though the function may take it

### Requirement: Edges: A use through a wildcard import needs only the type as it came

A use through a wildcard import MUST need the type as it came, its oldest step, never a constructor or a form: a type
of its name in the package of the use, which another file may declare, takes the name first.

#### Scenario: A call through a wildcard import
- **WHEN** a call through a wildcard import passes a parameter a later commit adds
- **THEN** it needs only the type, its oldest step
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_through_a_wildcard_import_needs_only_the_type`)

### Requirement: Edges: A call of a nested type by its qualified name tells its constructors apart

A call of a nested type by its qualified name (`Outer.Inner(…)`, `new Outer.Inner(…)`) MUST tell its constructors
apart as one of a top-level type.

#### Scenario: A nested class with a new parameter
- **WHEN** `Outer.Inner(…)` or `new Outer.Inner(…)` passes a parameter a commit adds
- **THEN** it depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_of_a_nested_class_that_needs_a_new_parameter_is_an_edge`)

### Requirement: Edges: A type the range adds keeps the edges of its uses

A type the range adds MUST keep the edge of every use that it or one of its constructors accepts, and of a call where
the range adds a function of its name that accepts it, which no evidence links from another file of the package.

#### Scenario: Calls and type positions of a new type
- **WHEN** a commit adds a type, and others call it and name it in type positions
- **THEN** each keeps its edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_new_type_keeps_its_edges_for_calls_and_type_positions`)

#### Scenario: A function of the name of a new type
- **WHEN** the range adds a type and a function of its name that accepts a call
- **THEN** the call needs the type
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_call_that_a_function_of_the_name_of_a_new_type_may_take_needs_the_type`)

### Requirement: Edges: A commit of sides that know the constructors differently is no step of them

A commit of whose sides one knows the constructors of a type and the other doesn't (a parse error in the class) MUST
make no step of its constructors (see the clean sides below).

#### Scenario: A class with a parse error in it
- **WHEN** one side of a commit has a parse error in the class
- **THEN** the commit is no step of its constructors
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_class_with_a_parse_error_in_it_is_no_step_of_its_constructors`)

### Requirement: Edges: A function of a type's name shares its key without losing the type's entries

A top-level function or property of the name of a type (a factory `fun Foo(): Foo`) MUST share its key and stay only
present or absent beside the entries of the type, in the type's file too: a commit that adds or removes it MUST keep
the type's entries, one that changes the type MUST keep its entries, and one that removes the type from its file MUST
remove the type's entries though the function keeps the key.

#### Scenario: A factory beside its type
- **WHEN** a file declares a type and `fun Foo(): Foo`, and commits change the type
- **THEN** the key keeps both, the type's entries kept
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_factory_function_and_its_type_share_a_key_without_losing_either`)

#### Scenario: The function comes and goes
- **WHEN** commits add and remove the function beside the type
- **THEN** the type's entries stay
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_function_of_the_name_of_a_type_comes_and_goes_beside_it`)

#### Scenario: The type is removed
- **WHEN** a commit removes the type from its file and keeps the function
- **THEN** the type's entries are gone, and the function keeps the key
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_type_removed_beside_a_function_of_its_name_leaves_no_entries`)

### Requirement: Edges: A function of a type's name takes a call its parameters accept now

A function of a type's name MUST count as a taker of a call of the name that its parameters, as its file has them now,
accept, judged as an `invoke` of the companion by the arguments that tell the constructors apart.

#### Scenario: By its parameters now
- **WHEN** a commit changes the parameters of the function
- **THEN** a call is judged by its parameters as its file has them now
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_function_of_the_name_of_a_type_takes_a_call_by_its_parameters_now`)

### Requirement: Edges: A call a function of a type's name may take depends on what its takers need

A call that a constructor and such a function accept MUST depend on what both need, one that only the function
accepts on the function's commit, also from another file of the package, which only the type links, and one that it
rejects (a parameter a commit adds to the constructor, passed by name) on what the constructors need.

#### Scenario: Only a call its parameters accept
- **WHEN** a call is accepted by a constructor and the function, by the function only, or rejected by the function
- **THEN** it depends on what both need, on the function's commit, or on what the constructors need
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_function_of_the_name_of_a_type_takes_only_a_call_its_parameters_accept`)

### Requirement: Edges: Only a function of a type's name that the use sees takes the call

Only a function that the use sees MUST take the call: by an import, its qualified name or its file, or of the package
in the use's module where its reach lets it in, never a private one of another file, one of another module or
language, nor any for a Java `new`. No type position MUST mean it.

#### Scenario: A function the use does not see
- **WHEN** the function of the type's name is private in another file, of another module or language, or the call is
  a Java `new`
- **THEN** the function takes no call
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_function_of_the_name_of_a_type_that_the_use_does_not_see_takes_no_call`)

#### Scenario: A type position
- **WHEN** a use names the type in a type position
- **THEN** it does not mean the function

### Requirement: Edges: A call beside an unchanged type of the baseline has no evidence, a known limit

A call that a function of a type's name takes from another file of the package MUST be linked only through the type.
A type of the baseline that the range doesn't change is no entry, so such a call has no evidence; that is a known
limit.

#### Scenario: An unchanged type and a new function of its name
- **GIVEN** a type of the baseline that the range doesn't change, and a function of its name a commit adds
- **WHEN** another file of the package calls the name
- **THEN** the call has no evidence and is no edge

### Requirement: Edges: The key of a top-level function holds the entries of each file

The key of a top-level function MUST hold the entries of each file of the range that adds it, so that a call of an
overload two files add is no edge.

#### Scenario: Overloads in two files
- **WHEN** two files of the range add overloads of one top-level function, and a call means either
- **THEN** both are entries, and the call is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`top_level_overloads_in_two_files_are_both_entries`)

### Requirement: Edges: The entries of a top-level function follow their declarations

The entries MUST follow their declarations to the file a commit renames or moves them to, each with its origin to a
declaration of its identity, so that such a call stays no edge when a commit moves both overloads into one file; where
the parser can't read the declarations there (a degraded file, a parse error), the entries MUST keep their forms and
origins in that file until a commit reads it again.

#### Scenario: A move into one file
- **WHEN** a commit moves both overloads into one file
- **THEN** they keep their origins, and the call stays no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`overloads_a_move_brings_into_one_file_keep_their_origins`)

#### Scenario: A version the parser cannot read
- **WHEN** a commit moves the entries into a file the parser cannot read
- **THEN** they keep their forms and origins in that file until a commit reads it again
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`entries_go_with_their_file_into_a_version_the_parser_cannot_read`)

### Requirement: Edges: A removal of a top-level function drops the entries of its file only

A removal MUST drop the entries of its file only, also the removal of an overload of the baseline.

#### Scenario: A removal in one of two files
- **WHEN** a commit removes a top-level function, or an overload of the baseline, from one of two files
- **THEN** the entries of the other file stay
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_removal_of_a_top_level_function_drops_the_entries_of_its_file_only`)

### Requirement: Edges: The entries of a key change only by clean sides

The entries of a key MUST change only by clean sides: when a side of a file that declares the key is degraded, a
declaration of the key on either side is inside a parse error, or two declarations of one side are the same entry (two
constructors of a type included), the key MUST only be present or absent, as a top-level function, a Rust or a
TypeScript key always is.

#### Scenario: A misread side before
- **WHEN** the side before of a commit is degraded or has the declaration inside a parse error
- **THEN** the commit changes no state of the key's entries
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_misread_before_side_changes_no_state`)

#### Scenario: Two declarations of one entry
- **WHEN** one side has two declarations that are the same entry, two constructors of a type included
- **THEN** the key is only present or absent
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`an_entry_is_its_parameter_types_with_their_arrays`, `constructors_the_summary_cannot_tell_apart_are_no_form`)

### Requirement: Edges: A type whose constructors one side doesn't know takes any call

A type whose constructors one side of a commit knows and the other doesn't MUST only be present or absent too; the type
then loses the constructors the range gave it, which might miss what the commit gave them, and MUST take any call, also
from its first commit; a constructor that a later commit on clean sides finds MUST be as old as the type, which it
needs.

#### Scenario: Sides a commit cannot match
- **WHEN** one side of a commit knows the constructors of a type and the other doesn't
- **THEN** the type loses the constructors the range gave it, and takes any call, also from its first commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_type_whose_sides_a_commit_cannot_match_takes_any_call`)

#### Scenario: A constructor found later
- **WHEN** a later commit on clean sides finds a constructor of such a type
- **THEN** the constructor is as old as the type, and a call of it needs the type
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_constructor_that_comes_while_its_type_takes_any_call_is_as_old_as_the_type`)

### Requirement: Edges: A removal depends on the commits that removed its uses

A commit that removes a declaration MUST depend on the older commits that removed its uses, an override of it
included.

#### Scenario: A removal after its uses are gone
- **WHEN** a commit removes the uses of a declaration, and a later one removes the declaration
- **THEN** the later depends on the earlier
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_removal_needs_the_commit_that_removed_the_uses`)

#### Scenario: A removal after its override is gone
- **WHEN** a commit removes an override of a declaration, and a later one removes the declaration
- **THEN** the later depends on the earlier
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_removal_needs_the_commit_that_removed_its_override`)

### Requirement: Edges: A move is no addition, an ambiguous use and a use of its own commit are no edge

A declaration that moves within its namespace in one commit MUST be neither added nor removed; a use that two commits'
declarations could mean MUST be no edge, its own commit's included; a commit MUST NOT depend on itself.

#### Scenario: A move, two owners and the commit itself
- **WHEN** a commit moves a declaration within its namespace, a use may mean declarations of two commits, and a commit
  uses its own declaration
- **THEN** the move is no addition, the ambiguous use no edge, and the commit no edge to itself
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_commit_needs_no_edge_to_itself_a_moved_declaration_is_no_addition_and_two_owners_are_ambiguous`)

#### Scenario: A use its own commit may mean
- **WHEN** a use may mean a declaration of its own commit and one of another
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_use_its_own_commit_may_mean_is_no_edge`)

### Requirement: Edges: A member of its own commit counts only for a use in its type

A member its own commit adds or changes MUST NOT count for a bare name outside the member's type in its file, which
only an implicit receiver makes a use of it.

#### Scenario: A bare name outside the type
- **WHEN** a commit adds a member and, in the same file, a bare name of it outside the member's type
- **THEN** the member is no origin of that use, and another commit's declaration may still be its edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_member_of_its_own_commit_is_no_origin_of_an_unqualified_use_outside_its_type`)

### Requirement: Edges: A file with a twentieth of its bytes in parse errors makes no edge

A file with a twentieth of its bytes in parse errors MUST make no edge.

#### Scenario: A file mostly in error
- **WHEN** a twentieth of the bytes of a file are in parse errors
- **THEN** its uses make no edge

### Requirement: Edges: `brd` names an override of another commit's overload

`brd commits` MUST name an override of another commit's overload as an edge by symbols, `brd check` as a missing
declaration with the evidence `override` and the branch that adds it, and `brd suggest` MUST stack the branch of the
override on that branch.

#### Scenario: `brd commits`
- **WHEN** a commit overrides an overload another commit adds
- **THEN** `brd commits` names it as an edge by symbols, with the symbol, and it joins no unit
  [@test] crates/bd-rewrite/src/commits_test.rs (`an_edge_by_symbols_names_the_symbol_and_joins_no_unit`, `an_override_of_a_new_overload_is_an_edge_by_symbols`)

#### Scenario: `brd check`
- **WHEN** the branch of the override lacks the branch of the overload
- **THEN** `brd check` names a missing declaration with the evidence `override` and the branch that adds it
  [@test] crates/bd-rewrite/src/check_test.rs (`an_override_of_another_branchs_overload_is_a_missing_declaration`)

#### Scenario: `brd suggest`
- **WHEN** the symbols are on and the override is of another branch's overload
- **THEN** `brd suggest` stacks the branch of the override on that branch
  [@test] crates/bd-rewrite/src/suggest_test.rs (`with_the_symbols_on_an_override_of_another_branch_stacks`)

### Requirement: Edges: Each commit names its entities

Each commit MUST name the functions and types around its changed lines (its entities).

#### Scenario: The entities of a commit
- **WHEN** a commit changes lines inside functions and types
- **THEN** it names those functions and types
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`the_entities_are_the_functions_and_types_around_the_changed_lines`)

### Requirement: Mode and measurement: `branchdeck.symbols` is `off`, `report` or `on`

`branchdeck.symbols` MUST be `off`, `report` or `on` (the default; Settings → Branches, Symbol dependencies: Off ·
Report Only · On, [Settings](../../repository/settings/spec.md#branches)); another value is an error.

#### Scenario: The mode from git config
- **WHEN** `branchdeck.symbols` is set to each value, to another one, or not at all
- **THEN** the mode is that value, an error, or `on`
  [@test] crates/sync-core/src/config_snapshot_test.rs (`the_mode_comes_from_git_config_and_is_on_without_it`)

### Requirement: Mode and measurement: `report` reads the summaries for `brd commits` and the dangling check only

`report` MUST read the summaries for `brd commits` and the dangling-reference check only; `off` MUST read none.

#### Scenario: `brd commits` in `report`
- **WHEN** the mode is `report`
- **THEN** `brd commits` names the edges by symbols, and they join no unit
  [@test] crates/bd-rewrite/src/commits_test.rs (`an_edge_by_symbols_names_the_symbol_and_joins_no_unit`)

#### Scenario: The dangling-reference check
- **WHEN** a commit removes a function whose name a KDoc comment of another file holds, and the check runs
- **THEN** the lines find the word, and the summaries take it for no use
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_word_in_kdoc_is_no_use`)

#### Scenario: The check of a stacked branch
- **WHEN** the check of a stacked branch that applies on the baseline runs with `on`, and again with `report`
- **THEN** the branch builds alone both times, with the symbols read under `on` and not under `report`
  [@test] crates/trial-check/src/check_test.rs (`a_stacked_branch_that_applies_on_the_baseline_builds_alone`)

#### Scenario: `off`
- **WHEN** the mode is `off`
- **THEN** no summary is read

### Requirement: Mode and measurement: `brd commits --symbol-stats` prints what the scan read and found

`brd commits --symbol-stats` (hidden) MUST print what the scan read and found: blobs and bytes per language, parse
errors, the times, the line, symbol, shared and symbols-only edges, the links by evidence, and a deterministic sample
of the symbols-only edges per evidence for a human to label. It MUST write the object cache only.

#### Scenario: The statistics of a range
- **WHEN** `brd commits --symbol-stats` runs
- **THEN** it prints the blobs and bytes per language, the parse errors, the times, the edges by kind, the links by
  evidence and a deterministic sample of the symbols-only edges per evidence
- **AND** it writes nothing but the object cache
