## ADDED Requirements

### Requirement: Summaries: A Rust function has its parameters without `self`

A Rust function MUST have the parameters of its list as its signature, without the `self` of a method, each named by
its pattern when that is one name (else `_`), of no type, the `...` of a function of an `extern` block a vararg; a
function a macro declares MUST have none.

#### Scenario: The parameters of a function
- **WHEN** a method takes `&self` and two parameters, one of a tuple pattern, and a function of an `extern` block ends
  with `...`
- **THEN** the method has the two parameters, the tuple one named `_`, and the function a vararg last
- **AND** a function a macro declares has no signature
  [@test] psi-syntax:crates/code-symbols/src/rust/summary_test.rs (`a_function_has_its_parameters_without_self`)

### Requirement: Summaries: A Rust call has the shape of its arguments, a method call its receiver apart

A Rust call of a path (`f(a)`, `Type::f(x, a)`) or of a method (`x.f(a)`) MUST have the shape of its arguments on its
callee, a method call with `method`, as its receiver fills `self`; a macro and a call of a field MUST have none.

#### Scenario: The shape of a call
- **WHEN** a file calls a method on a value, a method by the path of its type, a function with a literal and a
  closure, and a macro
- **THEN** the method call has `method` and its arguments, the path call its receiver first, each literal and closure
  its kind
- **AND** the macro has no shape
  [@test] psi-syntax:crates/code-symbols/src/rust/summary_test.rs (`a_call_has_the_shape_of_its_arguments`)

### Requirement: Edges: A Rust function whose arity a commit changes is a new step

A commit that changes the count of the parameters or a vararg of a Rust function, whose key has that one declaration on
each side, MUST make a new step of its key, of a member of an `impl` only in the same reach; a call of a Rust function
MUST be judged by the count of its arguments, a method call by the parameters after `self`, a path call of a method
with the receiver as its first argument.

#### Scenario: A path call and a call through an import
- **WHEN** a commit gives a method or a top-level function another parameter, and a later commit calls it by the path
  of its type with the receiver first, or through an import, with the new count of arguments
- **THEN** the call depends on that commit
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_path_call_that_only_a_new_arity_takes_is_an_edge`)

## MODIFIED Requirements

### Requirement: Edges: A Rust member on a value is no edge

A member on a value (`manifest.stills`, `self.stills`) or through `Self` MUST be no edge: the summary knows no type of
a Rust value. Only a Rust method call on a value (`runner.world.apps.connect_page(APPS, "review:2")`) of a method of the
use's own crate that its reach lets the use see MUST be an edge, `method_arity`, to a step of the method whose form
before rejected the count of the call's arguments and whose form accepts it.

#### Scenario: A field on a value
- **WHEN** a commit reads `manifest.stills` or `self.stills` of a field another commit adds, or a member through `Self`
- **THEN** it is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_field_on_a_value_is_no_edge`)

#### Scenario: A method call on a value that only a new arity takes
- **WHEN** a commit gives a method another parameter, and a later commit calls it on a value with the new count of
  arguments
- **THEN** the call depends on that commit, by `method_arity`
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_method_call_on_a_value_that_only_a_new_arity_takes_is_an_edge`)

#### Scenario: A method call on a value that both forms accept
- **WHEN** a commit renames or retypes a parameter of a method, or a later call on a value has the old count of
  arguments, or a commit adds the method
- **THEN** a call on a value is no edge
  [@test] crates/trial-check/src/symbol_dependencies_test.rs (`a_rust_method_call_on_a_value_that_both_forms_accept_is_no_edge`)
