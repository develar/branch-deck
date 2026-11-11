## MODIFIED Requirements

### Requirement: A C# loop and a statement without a block end the variables of their expressions

A variable an expression of a C# `while`, `do`, `for`, `foreach`, `using` or `fixed` declares (`out var x`,
`o is int x`) MUST end with that statement, and so MUST one of a statement that is the body of another without a
block (`if (c) Use(out var y);`): a name after it goes to the declaration around. A variable of the condition of an
`if` or of the expression of a `lock` is of the statements around it.

#### Scenario: A while, a do, a lock, a body without a block, an if
- **WHEN** a method declares `x` in the condition of a `while`, `y` in `if (c) Use(out var y);`, `z` in the condition of a `do … while`, `w` in the expression of a `lock`, `s` in the condition of an `if` and `v` in `lock (o) Use(out var v);`, beside fields `x`, `y`, `z`, `w`, `v`, and reads each after its statement
- **THEN** the body of the `while` goes to its variable, the names after the `while`, the `do` and the two bodies without a block to the fields
- **AND** `w` after its `lock` and `s` after its `if` go to the variables of their expressions
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_loop_and_an_embedded_statement_end_the_variables_of_their_expressions`)

### Requirement: A C# query reads its clauses in their order

In a C# query the source of a `from`, the value of a `let` and the left key of a `join` MUST be read before their
own range variable, the source of a `join` before every range variable of its query, and its right key MUST see the
variable of the `join` alone; the variable of a `join … into` MUST end with its clause, and an `into` MUST end every
variable before it. The collection of a `foreach` MUST be read before the variables of its deconstruction too.

#### Scenario: A from, a let, a join into, an into, a foreach of a deconstruction
- **WHEN** queries `from item in item.Parts let part = part + item select part`, `from x in xs join item in item.Parts on item equals item into g select g`, `from x in xs group x by x into g select item` and `from item in xs join y in item.Parts on item equals y select item` stand beside fields `item` and `part`, and a `foreach (var (item, n) in item.Pairs)`
- **THEN** the sources, the value of the `let` and the left key go to the fields, the right key to the variable of the `join`, the names after the clauses to their range variables
- **AND** the source of the `join` after `from item` goes to the field, its left key and the `select` to the variable of the `from`
- **AND** after an `into` a variable before it is gone: `item` goes to the field, an `x` to no declaration
- **AND** the collection of the `foreach` goes to the field, its body to the variable
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_query_reads_its_clauses_in_their_order`)

### Requirement: A Rust name is of its namespace

A Rust name MUST be bound to a declaration of the namespace its place reads: a type and a path segment before `::`
to a type, a module or a type parameter, never to a binding (`total::run(total)` calls the module); the name of a
macro call and of an attribute to a macro; any other name to no macro. An argument of a generic that is one name
(`Array<N>`) is written as a type and MUST be bound to a constant or a const parameter too. The name of a field
before its colon (`Point { total: 1 }`) MUST be a member, which no scope binds.

#### Scenario: A module, a macro and a field of the name of a binding
- **WHEN** a function declares `let total` and `let first`, then `S { total: total }`, `let typed: total::T = first!()` and `total::run(total) + first`, beside a module `total` and a macro `first` of the file
- **THEN** the segments before `::` go to the module, `first!` to the macro, the field before its colon is a member with no declaration
- **AND** the value of the field, the argument and `first` go to the bindings, and the usages of `total` are its declaration, that value and that argument
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_name_is_of_its_namespace_and_an_item_sees_no_binding_around_it`)

#### Scenario: A constant and a const parameter as the argument of a generic
- **WHEN** a file has `impl<const N: usize> Array<N>`, `fn f() -> Array<CAP>` beside `const CAP`, `fn g<const M: usize>(a: Array<M>) -> usize { M }` and `let a: Array<n>` beside a parameter `n`
- **THEN** `N`, `CAP` and `M` go to their declarations, `n` to none
- **AND** the usages of `M` are its declaration, the argument and its read
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`an_argument_of_a_generic_is_a_constant_or_a_const_parameter`)

## ADDED Requirements

### Requirement: The right side of a C# is is a type or a constant

A name that is all of the right side of a C# `is` (`x is Max`) MUST be bound to a type or a type parameter of the
scopes where one has the name, else to the constant, the field or the local of that name: C# reads a constant
pattern where no type is.

#### Scenario: A constant of the class, a local constant, a class
- **WHEN** a method with the local constants `Limit` and `Max` reads `x is Top`, `x is Limit`, `x is Max`, `x is not Limit` and `x is Other`, beside a class `Max` of the file and a constant `Top` of its class
- **THEN** `Top` and `Limit` go to the constants, `Max` to the class, `Other` to no declaration
- **AND** the usages of `Limit` are its declaration and both patterns
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_right_side_of_an_is_is_a_constant_where_no_type_of_the_name_is_declared`)

### Requirement: A variable a C# expression declares ends where the compiler ends it

A variable an expression declares MUST be scoped as C# scopes it: one of the initializer of a field or a property
or of a body after `=>` ends with it; one of an expression of a query that is a lambda's body (a `where`, a `let`, a
`select`, an `orderby`, a `group`, a key of a `join`, a `from` after the first) ends with that expression; one of an
incrementor of a `for` is not seen by its body; one of the source of the first `from` or of a `join` is of the
statements around the query.

#### Scenario: The readings of the compiler
- **WHEN** Go to Declaration runs on each of the 141 reads of `testdata/navigate/Scopes.cs`: a field `s` and a variable `s` in every statement, member and clause that may declare one, each read given to a method that takes the field or the variable alone, so that csc compiles the file only as it is tagged
- **THEN** each read goes to the declaration the compiler binds it to
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_variable_of_an_expression_is_seen_where_the_compiler_sees_it`)

### Requirement: A parameter of a Kotlin primary constructor is seen by the initializers of its class

A parameter of a Kotlin primary constructor that declares no property MUST be seen only by what runs as the
constructor does: a default, the call of a supertype, the initializers and the delegates of the properties and the
`init` blocks of its class, with the lambdas and the objects inside them, where it comes before a member of its
name. A function, an accessor, a secondary constructor and a class of the body MUST NOT see it.

#### Scenario: A property of the name of the parameter, and a property of the file
- **WHEN** `class C(total: String, val other: String = total) : Base(total)` reads `total` in a function, an initializer, a getter, an `init`, an inner class, an object, a function and a lambda of initializers and a secondary constructor, beside a property `total` of the file; and `class Greeter(name: String)` declares `fun greet() = "Hello, " + name` before `private val name = name.trim()`
- **THEN** the default, the call of the supertype, the initializers, the `init` and what stands inside initializers go to the parameter, the function, the getter, the inner class and the secondary constructor to the property of the file
- **AND** `name` in `greet` goes to the property, `name.trim()` to the parameter
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_parameter_of_a_primary_constructor_is_seen_by_the_initializers_of_its_class`)

### Requirement: A Kotlin companion is seen in its class, an instance in no class nested in it

The members of a Kotlin companion MUST be seen in all of the class it is the companion of. A class that is not
`inner` and an object declared in the body of a class MUST NOT see the properties and the functions of the
instances of the classes around it: a name there goes to a declaration further out.

#### Scenario: The readings of the compiler
- **WHEN** Go to Declaration runs on each of the 75 reads of `testdata/navigate/Scopes.kt`: a property `s` of the file and an `s` a function, a lambda, a loop, a `when`, a class, a companion, an object or an interface declares, each read given to a function that takes one of the two alone, so that kotlinc compiles the file only as it is tagged
- **THEN** each read goes to the declaration the compiler binds it to: a function of a class to the `s` of its companion, an inner class to the property of the class around it, a class nested in it to the property of the file
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_name_is_bound_where_the_compiler_binds_it`)

### Requirement: A Java pattern variable is seen where its test is known to have matched

The variable of the pattern of a Java `instanceof` MUST be seen only where the test is known to have matched (JLS
6.3): the operands after it of an `&&` it is true in and of an `||` it is false in, the branch of an `if` and the
operand of a `? :` of its outcome, the body of a loop it is true in, and the statements after an `if` or a loop
that goes on only when it matched. Elsewhere the name MUST go to the declaration around.

#### Scenario: The readings of the compiler
- **WHEN** Go to Declaration runs on each of the 140 reads of `testdata/navigate/Scopes.java`: a field `s` and a pattern variable, a resource or a local `s` per method, under `!`, `&&`, `||`, `? :`, in the branches of an `if`, in loops, guards and labels, after statements that can and cannot complete normally, each read given to a method that takes the field or the variable alone, so that javac compiles the file only as it is tagged
- **THEN** each read goes to the declaration the compiler binds it to
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_pattern_variable_and_a_resource_are_seen_where_java_sees_them`)

#### Scenario: The else of an if
- **WHEN** a method reads `name` in both branches of `if (o instanceof String name) … else …` beside a field `name`
- **THEN** the usages of the pattern variable are its declaration and the read of the first branch, and the read of the `else` is a usage of the field
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_pattern_variable_and_a_resource_are_seen_where_java_sees_them`, `a_pattern_variable_is_seen_where_java_sees_it_and_the_field_elsewhere`)

### Requirement: A resource of a Java try is seen by its resources and its block

A resource of a Java `try` MUST be seen by the resources after it and by the block of the `try`, never by a
`catch` or the `finally` of that `try`: a name there goes to the declaration around.

#### Scenario: A catch and a finally of the name of a resource
- **WHEN** a `try (Res s = new Res(); Res q = r(s)) { r(s); } catch (Exception e) { f(s); }` and a `try` with a `finally` that reads `s` stand beside a field `s`
- **THEN** the second resource and the block go to the resource, the `catch`, the `finally` and the statement after the `try` to the field
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_pattern_variable_and_a_resource_are_seen_where_java_sees_them`)
