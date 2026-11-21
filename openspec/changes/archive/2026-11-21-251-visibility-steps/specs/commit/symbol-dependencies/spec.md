## ADDED Requirements

### Requirement: Edges: A Rust or TypeScript declaration whose reach a commit changes is a new step

A commit that changes the reach of a Rust item of a module or field, or of a TypeScript declaration (by `export` or an
`export { … }` of its file), whose key has that one declaration on each side, MUST make a new step of its key, with the
commit for its origin and the old reach for its predecessor; a use MUST depend on the latest step whose reach before
rejected it, by psi-syntax's rule of Rust's visibility (`rust_syntax::visibility`) between the module of the item and
that of the use.

#### Scenario: An item made `pub`
- **WHEN** a commit makes a private Rust item `pub`, and a later commit uses it from a module outside the item's
- **THEN** the use depends on that commit, and a use inside the item's module depends on none
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_item_made_pub_is_an_edge_for_a_use_its_old_reach_forbids`)

#### Scenario: `pub(crate)` made `pub`
- **WHEN** a commit makes a `pub(crate)` item `pub`
- **THEN** a use of another crate depends on it, and one of the item's crate on none
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_item_made_pub_from_pub_crate_is_an_edge_for_another_crate_only`)

#### Scenario: A field made `pub`
- **WHEN** a commit makes a field `pub`, and a struct expression of another module names it
- **THEN** the use depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_field_made_pub_is_an_edge_and_an_impl_member_is_none`)

#### Scenario: A TypeScript declaration exported later
- **WHEN** a commit exports a TypeScript declaration, by `export` or by an `export { … }` of its file, and another
  file imports it
- **THEN** the import depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_typescript_declaration_exported_later_is_an_edge`)

#### Scenario: `pub` taken away and given again
- **WHEN** one commit makes an item `pub`, a later one takes the `pub` away, a third gives it again
- **THEN** a use after them depends on the third
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_use_needs_the_latest_step_that_let_it_see_the_item`)

### Requirement: Edges: A restricted Rust reach is the ancestor it names

The reach of a Rust declaration MUST be the module that `pub(super)`, `pub(in super::super)`, `pub(in crate::a)` or
`pub(crate)` names and the modules inside it, as rustc reads it.

#### Scenario: `pub(super)` and `pub(in crate::a)` made `pub(crate)`
- **WHEN** a commit makes `pub(crate)` an item of `a::b` that was `pub(super)` or `pub(in crate::a)`
- **THEN** a use from `a::d` depends on no commit, and a use from `c` depends on that one
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_restricted_rust_reach_is_the_ancestor_it_names`)

### Requirement: Edges: A reach that something else may stand in for is no step

A member of a Rust `impl` (a trait the range doesn't show may give the type a function of its name, which rustc takes
while the inherent one can't be seen), a key with two declarations on one side (a `#[cfg]` pair) and a JVM top-level
declaration MUST make no step of their reach; an older Rust form MUST accept a bare name that a glob of the item's
module would bring where a second glob of the file or the preludes (std's, the primitive types, the crates) may give
the name.

#### Scenario: A member of an `impl` made `pub`
- **WHEN** a commit makes a function of an `impl` `pub`, and another module calls `Type::function()`
- **THEN** the call depends on no commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_field_made_pub_is_an_edge_and_an_impl_member_is_none`)

#### Scenario: A `#[cfg]` pair made `pub`
- **WHEN** a commit makes both declarations of a `#[cfg]` pair `pub`
- **THEN** a use from another module depends on no commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_cfg_pair_made_pub_is_no_step`)

#### Scenario: A glob beside another glob or a prelude name
- **WHEN** a use of an item made `pub` comes through a glob of its module
- **THEN** it depends on the commit where the glob is the file's only one and the name no prelude's, else on none
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_glob_use_of_an_item_made_pub_needs_it_only_where_nothing_else_may_give_the_name`)

### Requirement: Edges: A narrower reach depends on the commits that removed the uses it forbids

A commit that narrows the reach of a declaration (a Rust or TypeScript step, a JVM member's new form) MUST depend on
the older commits that removed a use that the old reach took and the new one forbids, as a removal depends on the
commits that removed its uses.

#### Scenario: `pub` taken away after the use is gone
- **WHEN** a commit removes the use of a `pub` Rust item from another module, and a later one takes the `pub` away
- **THEN** the later depends on the earlier
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_narrowed_rust_item_needs_the_commit_that_removed_the_uses_it_forbids`)

#### Scenario: A JVM member made private after the use is gone
- **WHEN** a commit removes the use of an `internal` member from another file, and a later one makes it private
- **THEN** the later depends on the earlier
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_narrowed_jvm_member_needs_the_commit_that_removed_the_uses_it_forbids`)

## MODIFIED Requirements

### Requirement: Edges: A top-level function or property has no forms

A JVM top-level function or property MUST have no forms, only presence: a file the range doesn't show may declare an
overload of the function or a property of its name in the package, which takes the use while the one of the range
can't be seen.

#### Scenario: A changed form of a top-level declaration
- **WHEN** a commit changes the form of a top-level function or property another commit added
- **THEN** a use is no edge to the change
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_changed_form_of_a_top_level_function_or_property_is_no_edge`)
