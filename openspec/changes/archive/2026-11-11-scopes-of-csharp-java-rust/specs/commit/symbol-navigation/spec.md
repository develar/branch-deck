## ADDED Requirements

### Requirement: The statements of the sections of a C# switch share its block

The statements of every section of a C# `switch` MUST share the block of the `switch`: a local one section declares
is the one another assigns, and a local function of a section is seen in all of them. A variable of a label
(`case int n when n > 0`) MUST be of its section alone, and a variable the governing expression declares of the
statements around the `switch`.

#### Scenario: A local of one section assigned by another
- **WHEN** a `switch (Next(out var seen))` declares `int total` in its first section beside a field `total`, assigns `total` in its second, whose label is `case int n when n > 0`, and declares a local function in its third
- **THEN** the assignment of the second section goes to the local of the first, a `total` after the `switch` to the field
- **AND** `n` goes to the variable of the label in its section and to the field in the third, the call of the local function to it, `seen` after the `switch` to the variable of the governing expression
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_sections_of_a_switch_share_its_block`)

### Requirement: A C# loop and a statement without a block end the variables of their expressions

A variable an expression of a C# `while`, `do`, `for`, `foreach`, `lock`, `using` or `fixed` declares (`out var x`,
`o is int x`) MUST end with that statement, and so MUST one of a statement that is the body of another without a
block (`if (c) Use(out var y);`): a name after it goes to the declaration around. A variable of the condition of an
`if` is of the statements around the `if`.

#### Scenario: A while, a do, a lock, a body without a block, an if
- **WHEN** a method declares `x` in the condition of a `while`, `y` in `if (c) Use(out var y);`, `z` in the condition of a `do … while`, `w` in the expression of a `lock` and `s` in the condition of an `if`, beside fields `x`, `y`, `z`, `w`, and reads each after its statement
- **THEN** the bodies of the `while` and the `lock` go to the variables, the names after the statements to the fields
- **AND** `s` after its `if` goes to the variable of the condition
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_loop_and_an_embedded_statement_end_the_variables_of_their_expressions`)

### Requirement: The top-level statements of a C# file share one scope

The top-level statements of a C# file MUST share one scope: a local of one is read by those after it and by a local
function of the file, which they call, and it stays a local, whose usages are the names bound to it.

#### Scenario: A local of the first statement
- **WHEN** a file of top-level statements declares `var total`, reads it in the next statement and in a local function, and calls that function
- **THEN** each read goes to the declaration and the call to the local function
- **AND** the usages of `total` are its declaration and the three reads
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_top_level_statements_of_a_file_share_a_scope`)

### Requirement: A C# query reads its clauses in their order

In a C# query the source of a `from`, the value of a `let` and the source and the left key of a `join` MUST be read
before their own range variable, the right key of a `join` MUST see that variable alone, the variable of a
`join … into` MUST end with its clause, and an `into` MUST end every variable before it. The collection of a
`foreach` MUST be read before the variables of its deconstruction too.

#### Scenario: A from, a let, a join into, an into, a foreach of a deconstruction
- **WHEN** queries `from item in item.Parts let part = part + item select part`, `from x in xs join item in item.Parts on item equals item into g select g` and `from x in xs group x by x into g select item` stand beside fields `item` and `part`, and a `foreach (var (item, n) in item.Pairs)`
- **THEN** the sources, the value of the `let` and the left key go to the fields, the right key to the variable of the `join`, the names after the clauses to their range variables
- **AND** after an `into` a variable before it is gone: `item` goes to the field, an `x` to no declaration
- **AND** the collection of the `foreach` goes to the field, its body to the variable
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_query_reads_its_clauses_in_their_order`)

### Requirement: A C# type and the member an initializer names are of no variable

A name where C# reads a type MUST be bound to a type or a type parameter the scopes declare, never to a variable of
its spelling. The member an object initializer or a `with` sets (`new Box { Size = size }`) and the name of a
`Name =` MUST be members without a qualifier, which no scope binds.

#### Scenario: A type, a member of an initializer and of an anonymous type
- **WHEN** a method declares `var total`, then `total typed`, `new Box { Size = Size, total = total }` and `new { Size = total }`, beside a class `total` of the file and a parameter `Size`
- **THEN** the type goes to the class, the members the initializers name to no declaration, their values to the parameter and the local
- **AND** the usages of the local are its declaration and the two values
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_type_and_a_member_an_initializer_names_are_of_no_variable`)

### Requirement: A Java call, type and method reference are of no variable

As Java keeps methods, types and variables apart, a called name MUST be bound to a method of the scopes (the accessor
of a record component too), a name read as a type to a class or a type parameter, and a value to no method. The name
after `::` of a method reference MUST be a member of what stands before the colons.

#### Scenario: A variable of the name of the method that gives it
- **WHEN** a method declares `int size = size()` and `int total`, then `total typed` and `this::size`, beside a field and a method `size` and a class `total`
- **THEN** the call goes to the method, the type to the class, `size` after `this::` is a member of `this`
- **AND** the usages of each variable are its declaration and its reads
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_call_a_type_and_a_method_reference_are_of_no_variable`)

### Requirement: The variable of a Java case pattern ends at the next label

The variable of a pattern of a `case …:` label of Java MUST be seen in the guard of its label and in the statements
after it up to the next label; the statements of another label go to the declaration around the `switch`.

#### Scenario: A pattern variable read under the next label
- **WHEN** a `switch` has `case Integer i when i > 0: use(i); break;` and `default: use(i);` beside a field `i`
- **THEN** the guard and the first `use(i)` go to the variable of the pattern, the one under `default` to the field
  [@test] crates/code-symbols/src/java/navigate_test.rs (`the_variable_of_a_case_pattern_ends_at_the_next_label`)

### Requirement: A Rust name is of its namespace

A Rust name MUST be bound to a declaration of the namespace its place reads: a type and a path segment before `::`
to a type, a module or a type parameter, never to a binding (`total::run(total)` calls the module); the name of a
macro call and of an attribute to a macro; any other name to no macro. The name of a field before its colon
(`Point { total: 1 }`) MUST be a member, which no scope binds.

#### Scenario: A module, a macro and a field of the name of a binding
- **WHEN** a function declares `let total` and `let first`, then `S { total: total }`, `let typed: total::T = first!()` and `total::run(total) + first`, beside a module `total` and a macro `first` of the file
- **THEN** the segments before `::` go to the module, `first!` to the macro, the field before its colon is a member with no declaration
- **AND** the value of the field, the argument and `first` go to the bindings, and the usages of `total` are its declaration, that value and that argument
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_name_is_of_its_namespace_and_an_item_sees_no_binding_around_it`)

### Requirement: A Rust item sees no binding of the code around it

A Rust item inside a function (a `fn`, a `const`, a `static`, an `impl`, a module, a type) MUST see the items around
it and none of the bindings: a name in a `fn` inside a function goes past the parameters and the `let`s of that
function.

#### Scenario: A fn inside a function reads a static
- **WHEN** a function with the parameter `sum` declares `fn g() -> u32 { total::run(sum) }` beside a static `sum` of the file, and reads `sum` itself
- **THEN** the `sum` of `g` goes to the static, the `sum` of the function to its parameter
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_name_is_of_its_namespace_and_an_item_sees_no_binding_around_it`)

## MODIFIED Requirements

### Requirement: A Rust format string uses the names it takes from the code around it

A string among the tokens of a Rust macro may be a format string, so a name it would take from the code around the
macro MUST be a usage of that name: a word right after the `{` of a placeholder (`{total}`, `{total:>8}`) or before a
`$` (`{:width$}`, and `{:0width$}` after a flag: a digit starts no word). A doubled brace, the braces of a code point
and a string outside a macro MUST name nothing, and a name inside a string is no place to go from.

#### Scenario: Placeholders, a width, a doubled brace
- **WHEN** Find Usages runs on a parameter that `format!("{total} {{total}} {total:>8} {:width$} …", total)` and `format!("{total:0width$} {:#0width$x} {:08.width$} {width:0>5} …", total)` read, a string outside a macro spelling `{total}` too
- **THEN** its usages are its declaration, the placeholders and the arguments; `width` has its declaration, the width of the first string and the three widths and the placeholder of the second
- **AND** Go to Declaration finds no name inside the string
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_format_string_uses_the_names_it_takes_from_the_code_around_it`)
