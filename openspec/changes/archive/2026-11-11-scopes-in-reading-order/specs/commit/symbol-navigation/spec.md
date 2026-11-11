## ADDED Requirements

### Requirement: A Kotlin variable is seen after its own statement

In Kotlin code a variable MUST be seen only after its own statement, the variable of a `for` only after its range and
the subject of a `when` only after its value: the value reads the name of a scope around it. A name before the
declaration of a local of its block MUST go to the declaration of a scope around the block; the members of a class
and of a file are seen wherever they are declared.

#### Scenario: The value of a variable, the range of a for, a name before the declaration
- **WHEN** a function declares `val name = name.trim()` beside its parameter `name`, loops `for (items in items)`, reads `when (val count = count())` and reads `count` before its `val count`
- **THEN** the value of `name` goes to the parameter, the range to the parameter `items`, both early `count` to the property of the file
- **AND** after their statements the names go to the variables
- **AND** the usages of the parameter are its declaration and the value read from it
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`code_reads_in_its_order_a_variable_after_its_statement_and_a_loop_variable_after_its_range`)

### Requirement: A Rust binding is seen after its statement and in the block it guards

In Rust the binding of a `let` MUST be seen only after its statement, so `let total = total + 1` reads the `total`
before it; the binding of an `if let` or a `while let` only in the rest of its condition and the block it guards; the
binding of a `for` only in its body. An item, a parameter and the binding of an arm are seen in all of their scope,
and a binding shadows an item.

#### Scenario: A let that shadows, an if let, a while let, a for, a let else
- **WHEN** a function shadows its parameter by `let total = total + 1`, takes `found` apart by `if let Some(found) = found`, loops `while let Some(items) = next(&items)` and `for items in items.iter()`, then declares `let Some(found) = found else { … }`
- **THEN** each value and collection goes to the parameter, each block to its binding
- **AND** after the block of the `if let`, `found` is the parameter again
- **AND** the usages of the parameter `total` are its declaration and the value read from it
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_binding_is_seen_after_its_statement_and_in_the_block_it_guards`)

### Requirement: The collection of a loop is read before its variable

The collection a loop runs over MUST be read outside the loop: `for (String item : item.parts())` of Java and
`foreach (var item in item)` of C# read the `item` around the loop. The variable of the loop MUST be seen in its
statement alone: a name after the loop goes to the declaration around it.

#### Scenario: Java
- **WHEN** a Java method loops `for (String item : item.parts())` beside a field `item` and reads `item` after the loop
- **THEN** the collection and the name after the loop go to the field, the body to the variable
  [@test] crates/code-symbols/src/java/navigate_test.rs (`the_collection_of_a_for_is_read_before_its_variable`)

#### Scenario: C#
- **WHEN** a C# method loops `foreach (var item in item)` beside a field `item` and reads `item` after the loop
- **THEN** the collection and the name after the loop go to the field, the body to the variable
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_variable_of_a_foreach_is_seen_in_its_statement_alone`)

### Requirement: A Rust format string uses the names it takes from the code around it

A string among the tokens of a Rust macro may be a format string, so a name it would take from the code around the
macro MUST be a usage of that name: a word right after the `{` of a placeholder (`{total}`, `{total:>8}`) or before a
`$` (`{:width$}`). A doubled brace, the braces of a code point and a string outside a macro MUST name nothing, and a
name inside a string is no place to go from.

#### Scenario: Placeholders, a width, a doubled brace
- **WHEN** Find Usages runs on a parameter that `format!("{total} {{total}} {total:>8} {:width$} …", total)` reads, a string outside a macro spelling `{total}` too
- **THEN** its usages are its declaration, the two placeholders and the argument; `width` has its declaration and the width
- **AND** Go to Declaration finds no name inside the string
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_format_string_uses_the_names_it_takes_from_the_code_around_it`)

### Requirement: A Bazel default and the first collection of a comprehension are read outside their scope

The default of a parameter of a `def` or a `lambda` MUST be read where the function is defined, and the first
collection of a comprehension where the comprehension stands: neither sees the names their scope declares. A later
collection and the condition of a comprehension see its variables.

#### Scenario: A default and a first collection
- **WHEN** a `def` has the parameters `name, deps = name, key = lambda name = name: name` and the comprehension `[name for name in name.split() if name]`, beside a `name` of the file
- **THEN** both defaults go to the `name` of the file, the body of the `lambda` to its parameter
- **AND** the first collection goes to the parameter of the `def`, the element and the condition to the variable of the comprehension
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_comprehension_is_a_scope_and_a_default_is_read_where_its_function_is_defined`)

## RENAMED Requirements

- FROM: `### Requirement: A Bazel file has two scopes, the file and the body of a def`
- TO: `### Requirement: A Bazel file has the scopes of the file, a def and a comprehension`

## MODIFIED Requirements

### Requirement: A Bazel file has the scopes of the file, a def and a comprehension

A Bazel file has the scope of the file and, inside it, the body of a `def` or a `lambda` and a comprehension: a name
MUST go to the `def`, the assignment or the `load` of the file that binds it, and a name declared inside a `def` (a
parameter, a name the body assigns) or by a comprehension is a local, whose usages stay in that `def` or that
comprehension.

#### Scenario: A name of the file goes to what binds it
- **WHEN** Go to Declaration runs on a call of a `def` of the file, or a read of a name the file assigns, in a `BUILD` or a `.bzl` file
- **THEN** it goes to the `def` or the assignment that binds it
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_call_of_a_def_of_the_file_goes_to_the_def`, `a_read_of_an_assigned_name_goes_to_its_assignment`, `a_bzl_file_reads_the_same_names_as_a_build_file`)

#### Scenario: A qualified call is bound by no scope of the file
- **WHEN** a call is qualified by a receiver
- **THEN** it names its receiver, and no `def`, assignment or `load` of the file is its declaration
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_qualified_call_names_its_receiver_and_has_no_declaration_of_its_own`)

#### Scenario: A name declared inside a def is a local
- **WHEN** Find Usages runs on a parameter of a `def` or a name its body assigns
- **THEN** it is a local, whose usages stay in that `def`
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`the_usages_of_a_parameter_stay_inside_its_def`)

#### Scenario: The variable of a comprehension is a local
- **WHEN** a comprehension binds a name, inside a `def` or at the top level of the file, that the code around it spells too
- **THEN** the name is a local of the comprehension: past it the name is the one around again
- **AND** the usages of the name of the file hold none of the comprehension
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_comprehension_is_a_scope_and_a_default_is_read_where_its_function_is_defined`)
