## ADDED Requirements

### Requirement: A name a Bazel def binds is one variable in all of it

A name a `def` or a comprehension of a Bazel file binds MUST be one variable in all of that scope, as Starlark reads
it: its declaration is the first place that binds it (the parameter, else the first assignment, `for` or `def`
inside), a later one is a write of it, and a name before its first assignment, a closure's too, is the variable's,
not a name of the file.

#### Scenario: An assignment in a branch, a parameter assigned again, a closure before the assignment
- **WHEN** a `def` assigns `total` twice, the second time in an `if`, assigns and loops over its parameter `deps`, and a `def` inside it reads `total` before the first assignment, beside a `total` of the file
- **THEN** the second assignment and the read of the closure go to the first assignment, the assignment and the `for` of `deps` to the parameter
- **AND** the usages of `total` are its declaration, the write of the branch and its reads, the closure's among them; the name of the file has none of them
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_name_a_def_binds_is_one_variable_in_all_of_it`)

### Requirement: The name of a named argument is the parameter of what is called

The name of a named argument (`report(total = 1)` of Kotlin, `rule(name = name)` of a Bazel file, `F(total: 1)` of
C#) MUST be a member without a qualifier, which no scope of its file binds: Go to Declaration from it goes to no
local of its spelling, and it is no usage of one.

#### Scenario: Kotlin
- **WHEN** a Kotlin function calls `report(total = total)` beside its parameter `total`
- **THEN** the name of the argument is a member with no declaration, its value goes to the parameter
- **AND** the usages of the parameter are its declaration and the value
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_do_while_condition_sees_its_body_and_a_type_or_a_named_argument_is_no_local`)

#### Scenario: Bazel
- **WHEN** a `def` calls `rule(name = name)` beside its parameter `name`
- **THEN** the name of the argument is a member with no declaration, its value goes to the parameter
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`the_name_of_a_named_argument_is_bound_to_no_local`)

#### Scenario: C#
- **WHEN** a C# method calls `Report(total: total)` beside its local `total`
- **THEN** the name of the argument is no usage of the local
  [@test] crates/code-symbols/src/names_at_test.rs (`csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

### Requirement: A TypeScript or JavaScript function has one variable of a name

In TypeScript and JavaScript a parameter, a `var` and a function the body of a function declares, of one name, MUST
be one variable: the first declares it, a later one is a use of it wherever its block stands, a write when it is a
`var` with a value. A `var` in a `catch` of the name of its parameter MUST write that parameter. The name of a
function expression is another variable than a parameter of its name.

#### Scenario: A var of the name of a parameter, a second var, a var in a catch
- **WHEN** a function declares `var total = 2` in a block beside its parameter `total`, declares `var later` three times, the last in the head of a `for … of`, and reads `later` before the first
- **THEN** the `var` of `total` goes to the parameter and is a write of it, every `later` goes to the first `var`, the last one as a write
- **AND** `var e = 1` in `catch (e)` writes the parameter of the `catch`, and the parameter of `function total(total)` is what its body reads
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_function_has_one_variable_of_a_name`)

### Requirement: The signature of a TypeScript or JavaScript function is read before its body

The names of the signature of a function (the defaults of its parameters, their types, its return type) MUST be bound
to its parameters and type parameters, else to a scope around the function, never to a declaration of its body:
`function f(a = total) { let total = a }` reads the `total` around `f`.

#### Scenario: A default of the name of a let and of a var of the body
- **WHEN** a function inside `outer(total, sum)` has the default `a = total` and a `let total` in its body, and an arrow function the default `c = sum` and a `var sum`
- **THEN** both defaults go to the parameters of `outer`, a default that names a parameter before it to that parameter
- **AND** the bodies read their own declarations
  [@test] crates/code-symbols/src/js/navigate_test.rs (`the_default_of_a_parameter_is_read_outside_the_body`)

### Requirement: A TypeScript name read as a type is of no variable

A name TypeScript reads as a type, and a namespace before the dot of one (`total.T`), MUST be bound to a type, a
class, an enum or a namespace the scopes declare, else to none: never to a variable, a parameter or a function of its
spelling. `typeof total` reads the value.

#### Scenario: A type and a namespace of the spelling of a constant
- **WHEN** a function declares `const total`, a `const typed: total`, a `const other: total.T` and reads `typeof total`, beside a `type total` of the file
- **THEN** the type and the namespace go to the type of the file, `typeof total` to the constant
- **AND** the usages of the constant are its declaration, the `typeof` and its reads
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_type_is_of_no_variable_and_an_enum_sees_its_members`)

### Requirement: The members of a TypeScript enum are seen in its body

A bare name in the body of a TypeScript enum MUST go to the member of that name (`twice = sum * 2`), before any
declaration around the enum; outside its body a member is reached through the enum alone.

#### Scenario: A member read by the member after it
- **WHEN** an enum declares `sum = 1, twice = sum * 2` beside a constant `sum` of the function around it
- **THEN** the `sum` of the enum goes to its member, the `sum` after the enum to the constant
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_type_is_of_no_variable_and_an_enum_sees_its_members`)

### Requirement: A Kotlin local is found before a member of a class inside its body

As Kotlin finds a local before a member of an implicit receiver, a bare name in a class or an object declared inside
code MUST go to a variable, a parameter or a function that the code around the class declares, before a member of
the class of that name (a property, a `val` of its constructor). A member gives way to no declaration of the top
level of a file.

#### Scenario: A property of an object and a val of a constructor of the name of a local
- **WHEN** a function declares `val total` and `val sum`, then an object with a property `sum` whose function reads `sum`, `total` and a property of its own, and a local class `Box(val total: Int)` whose function reads `total`
- **THEN** `sum` and `total` go to the variables of the function, in the object and in the class
- **AND** the property no local hides goes to the property, and a property of the class of the function comes before one of the file
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_member_of_a_class_inside_a_body_gives_way_to_a_local_of_the_body`)

### Requirement: The condition of a Kotlin do while sees the block of its body

The condition of a Kotlin `do … while` MUST see the variables the block of its body declares, as Kotlin reads it; a
statement after the loop does not.

#### Scenario: A variable of the body read by the condition
- **WHEN** a function loops `do { val more = … } while (more)` beside a property `more` of the file and reads `more` after the loop
- **THEN** the condition goes to the variable of the body, the name after the loop to the property
- **AND** the usages of the variable are its declaration and the condition
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_do_while_condition_sees_its_body_and_a_type_or_a_named_argument_is_no_local`)

### Requirement: A Kotlin name where a type is written is of no value

A Kotlin name where a type is written (`val x: Name`, a constraint of a `where`) MUST be bound to a class, an object,
a type alias or a type parameter the scopes declare, else to none: never to a variable, a parameter, a property or a
function of its spelling.

#### Scenario: A type of the spelling of a parameter
- **WHEN** a function with the parameter `total` declares `val typed: total`
- **THEN** the type has no declaration in the file, and is no usage of the parameter
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_do_while_condition_sees_its_body_and_a_type_or_a_named_argument_is_no_local`)

## MODIFIED Requirements

### Requirement: A write is an assignment, an increment, a C# out or ref argument or a destructuring target

A usage MUST be a write where its name ends the left side of an assignment or is the operand of `++` or `--`, where
a C# argument is `out` or `ref`, and where it is a target of a TypeScript destructuring assignment. In a Bazel file
an `=` declares its name, but one of a `def` whose name the `def` bound before, which writes, as a later `var` with a
value of a variable of a TypeScript or JavaScript function does. An argument of any other language reads (a Rust
`&mut x` too).

#### Scenario: An assignment and an increment write
- **WHEN** a name ends the left side of an assignment or is the operand of `++` or `--`
- **THEN** its usage is a write
  [@test] crates/code-symbols/src/navigate_test.rs (`tells_a_java_write_an_increment_and_an_import`)
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_write_is_the_left_side_of_an_assignment_and_the_operand_of_an_increment`)
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_call_with_type_arguments_is_a_call_and_an_increment_a_write`)

#### Scenario: A Bazel assignment declares
- **WHEN** a name of a Bazel file is assigned by `=`
- **THEN** its usage there is its declaration, no write
- **AND** a later assignment of the name in its `def`, and a `for` over it, is a write of the variable
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`the_usages_of_a_parameter_stay_inside_its_def`, `a_name_a_def_binds_is_one_variable_in_all_of_it`)

#### Scenario: A C# out or ref argument writes
- **WHEN** a C# name is passed as an `out` or `ref` argument
- **THEN** its usage is a write
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_usages_of_a_local_are_its_own_and_of_a_member_all_unbound`)

#### Scenario: An argument of any other language reads
- **WHEN** a name of a Kotlin, Java, Rust, TypeScript or JavaScript file is passed as an argument, a Rust `&mut x` too
- **THEN** its usage is a read

#### Scenario: The targets of a TypeScript destructuring assignment write
- **WHEN** a TypeScript or JavaScript name is a target of a destructuring assignment
- **THEN** its usage is a write
- **AND** a later `var` of a variable of its function is a write when it has a value, a declaration again when it has none
  [@test] crates/code-symbols/src/js/navigate_test.rs (`an_assignment_a_destructuring_and_an_increment_write`, `a_function_has_one_variable_of_a_name`)
