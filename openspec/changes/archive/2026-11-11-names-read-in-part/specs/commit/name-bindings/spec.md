## ADDED Requirements

### Requirement: Locals: A local says when the scopes read its code in part

A local MUST say that the scopes read only a part of the code that may name it: in a text with an error of its
parser, in a C# file with an `#if`, in a Rust function or closure with a `#[cfg]` inside it, and in a script that
calls `eval` by its name. The names bound to such a local are those of one build of the text, or of a part of what
runs: a reader MUST NOT take them for every use of the declaration.

#### Scenario: A C# file compiled by conditions
- **WHEN** a local of a C# method is asked whose use stands in a branch of `#if` the parser does not take, whose declaration stands in a branch it does take, or whose file has an `#if` around a `using` alone
- **THEN** the local says its code is read in part; with `#nullable` and `#region` alone it does not
  [@test] crates/code-symbols/src/names_at_test.rs (`a_local_says_when_the_scopes_read_its_code_in_part`)

#### Scenario: A Rust body with a condition inside
- **WHEN** a local of a Rust function is declared twice under two `#[cfg]`, a parameter stands under one, a statement of a closure of a `static` does, or an attribute is a `cfg_attr` that adds a `cfg`
- **THEN** the local says its code is read in part
- **AND** a `#[cfg]` on the function itself or in another function, a `cfg_attr` that adds no `cfg` and `cfg!` say nothing of it
  [@test] crates/code-symbols/src/names_at_test.rs (`a_local_says_when_the_scopes_read_its_code_in_part`)

#### Scenario: A script that evaluates text
- **WHEN** a function of JavaScript or TypeScript calls `eval` by its name, in parentheses and inside an arrow function too
- **THEN** the locals of the script say their code is read in part: the text names the variables around the call
- **AND** a call of a member named `eval` and the word in a string say nothing
  [@test] crates/code-symbols/src/names_at_test.rs (`a_local_says_when_the_scopes_read_its_code_in_part`)

## MODIFIED Requirements

### Requirement: Answers: A text with a syntax error answers the names its tree holds

A text with a syntax error MUST be answered by the tree its parser left: a local declared and read around the error
has its uses. Every local of such a text MUST say that the scopes read its code in part: what stands where the
parser gave up is no name of the tree.

#### Scenario: A broken statement between a declaration and its use
- **WHEN** a statement between the declaration of a local and its use is broken, in Kotlin, Java, C#, Starlark, Rust, TypeScript, TSX and JavaScript
- **THEN** the local has its declaration and its use, the clash of a new name is found, and the local says that its code is read in part
- **AND** an early error of a script (a `break` outside a loop, a name declared twice) leaves the tree whole: its locals say nothing of it
  [@test] crates/code-symbols/src/names_at_test.rs (`a_text_with_a_syntax_error_still_answers_the_names_its_tree_holds`)

### Requirement: Answers: A change of names reads as one local only where both sides bind alike

The answers MUST tell the rename of one local from a change of what the code means: read on each side of a change
(every changed name one entry, a value of its body alone that no call names and whose code the scopes read whole,
its uses the changed names alone and each plain, no clash with the other name), a rename of one local holds on both sides, and a change that reads another
declaration, sets another property or lets other code spell the name fails on one.

#### Scenario: Renames of one local
- **WHEN** a local and its uses, a parameter of Java under no annotation of a framework, a second `let` of Rust that shadows, a loop variable, a local of the top-level statements of C#, the variable of a Java pattern read after the statement that leaves without it, a resource of a Java `try` read in its block, or a value are renamed, the value under a property, a named argument, a field of a Rust struct or a member of a C# initializer that keeps its name, or beside the method or the function of its old name
- **THEN** both sides read as one local
  [@test] crates/code-symbols/src/names_at_test.rs (`a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

#### Scenario: Changes of meaning
- **WHEN** the value of a variable reads another parameter after the change, a statement after a loop, an `if let` or a statement without a block another field or parameter, a default, a `return`, a closure, a query or the condition of a `do … while` another name, the new name takes a read of a member, a later assignment of a `def` or a later `var` of a function writes another variable, or the change renames a shorthand property, a named argument, the argument of `nameof`, a called Kotlin value, a namespace, a parameter a call or an attribute macro names, or a name only the types tell from an inherited member or a constant of an enum
- **THEN** one side does not read as one local
  [@test] crates/code-symbols/src/names_at_test.rs (`a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

#### Scenario: A name that is a value, and a text read in part
- **WHEN** the change renames a Kotlin local declared with a delegate (of a map, of `registering` in a build script, `by lazy`), a variable of TypeScript or JavaScript that is given a function or a class without a name, a local of a C# file with an `#if` (a use left in the branch not taken, a read of the new name in both branches, a declaration under the condition), a local of a Rust function declared twice under two `#[cfg]`, a local of a script with a direct `eval`, a use inside a `with`, or a local of a Java or a Kotlin text with a syntax error
- **THEN** one side does not read as one local
  [@test] crates/code-symbols/src/names_at_test.rs (`a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

#### Scenario: Changes of meaning the scopes of a statement tell
- **WHEN** the change renames the variable of a Java pattern with a read in the branch or the operand its test did not match in, a resource of a Java `try` with a read in its `catch` or its `finally`, the variable of a C# `lock` but not its read after the `lock`, a local constant that the right side of a C# `is` reads, a range variable with the source of a `join` after it, a variable of a clause of a query with a read in a later clause, or a parameter of a Java method that an annotation lets a framework bind by its name
- **THEN** one side does not read as one local
  [@test] crates/code-symbols/src/names_at_test.rs (`a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

### Requirement: Locals: A local says what declares it

The answer of a name bound to a declaration inside code MUST say what declares it: a variable, a parameter of a
function with a name, a parameter of a lambda or a function without a name, the variable of a `for`, the parameter of
a `catch`. Any other declaration inside code MUST be `other`: a local function, a local type and its members, a
parameter that declares a member, a type parameter, a label, a parameter of a function type.

#### Scenario: Kotlin
- **WHEN** the locals of a Kotlin function are asked
- **THEN** a `val`, the subject of a `when` and an entry of a destructuring are variables, the variable of a `for` a loop variable, the parameter of a `catch`, of a lambda and of a function without a name what they are
- **AND** a `val` of the constructor of a local class, a property of it, a local function and the parameter of a function type are `other`
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`)

#### Scenario: Java
- **WHEN** the locals of a Java method are asked
- **THEN** a local, a resource and a pattern variable are variables, the variable of either `for` a loop variable, the parameter of a `catch`, of a lambda and of the method what they are, a label `other`
  [@test] crates/code-symbols/src/names_at_test.rs (`java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: C#
- **WHEN** the locals of a C# method are asked
- **THEN** a local, an `out` variable and a variable of a deconstruction are variables, the variable of a `for` and a `foreach` a loop variable, the variable of a `catch` and the parameter of a lambda what they are, a local function `other`
  [@test] crates/code-symbols/src/names_at_test.rs (`csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Starlark
- **WHEN** the locals of a `def` are asked
- **THEN** a name its body assigns is a variable, the name a `for` or a comprehension binds a loop variable, the parameter of a `lambda` a lambda parameter, a `def` inside it `other`
  [@test] crates/code-symbols/src/names_at_test.rs (`starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Rust
- **WHEN** the locals of a Rust function are asked
- **THEN** the binding of a `let`, of an `if let` and of an arm is a variable, the one of a `for` a loop variable, of a parameter of a closure a lambda parameter, of the function a parameter
  [@test] crates/code-symbols/src/names_at_test.rs (`rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: TypeScript, TSX and JavaScript
- **WHEN** the locals of a function of TypeScript, TSX or JavaScript are asked
- **THEN** a `let`, `const` or `var` is a variable, the one of the head of a `for` a loop variable, the parameter of a `catch`, of an arrow or a function expression and of a method or a function declaration what they are
- **AND** a function inside a function is `other`
  [@test] crates/code-symbols/src/names_at_test.rs (`typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A parameter that declares a property
- **WHEN** a parameter of a TypeScript constructor with a modifier (`public total`, `private readonly total`, `readonly total = 1`) is asked
- **THEN** it is no local: it declares a property of its class, and its uses are the names of its spelling that no local binds, the reads in the constructor and `this.total` among them
- **AND** the parameter beside it without a modifier is a parameter, and in a class declared in a function the property is `other`, as the class is
  [@test] crates/code-symbols/src/names_at_test.rs (`a_parameter_that_declares_a_property_is_a_member_of_its_class`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

### Requirement: Sites: A use says what its site makes of the name beside its binding

Each use MUST say what its site makes of the name beside the binding the scopes give it, the first of: `shorthand`
(the name is also a property there), `named argument`, `label`, `namesake` (only the types, or what runs, tell
whether the name is the binding), `spelled` (the spelling is a value where the code runs), `macro` (among the tokens
of a macro), `destructured` (a pattern that takes a value apart declares or assigns it); else it is plain, a string
template that reads it too.

#### Scenario: Shorthand
- **WHEN** a local is written as `{ total }` in an object, a pattern or the target of an assignment of TypeScript, TSX or JavaScript, as `Ledger { total }` of Rust, or as `new { total }` and `(total, scale)` of C#
- **THEN** the use is a shorthand
  [@test] crates/code-symbols/src/names_at_test.rs (`typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A named argument and a label
- **WHEN** a name is the name of a named argument of Kotlin, Starlark or C#, of an element of a Java annotation, or a label of Java or C# or the name of its jump
- **THEN** the use is a named argument, or a label
- **AND** the name of a named argument is a use of the names of its text that no local binds, never of a local
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A namesake
- **WHEN** a name bound to a variable or a parameter is called in Kotlin or written after `::` there, is all of a `case` label of Java, stands in an anonymous class of Java inside the body of the variable, or is all of the right side of a C# `is`
- **THEN** the use is a namesake
- **AND** a call of a local of C#, Rust, Starlark, TypeScript or JavaScript, where a call names a value, is plain
- **AND** a name inside the body of a `with` of JavaScript is a namesake: a property of the object comes before the variable, and a name the body reads would be the variable's under that name
  [@test] crates/code-symbols/src/names_at_test.rs (`a_name_that_is_handed_on_as_text_is_spelled_where_it_is`, `kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Spelled, and the tokens of a macro
- **WHEN** a local is the argument of `nameof` of C#, or stands among the tokens of a Rust macro or in a format string of one
- **THEN** the use is spelled, or of a macro
  [@test] crates/code-symbols/src/names_at_test.rs (`csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A name handed on as text
- **WHEN** a Kotlin property is declared with a delegate (`val total by map`, `val integrationTest by registering(…)`, a member `by lazy`), or a variable or a parameter of TypeScript or JavaScript is given a function or a class written without a name: as its value (`const total = () => 1`, also in parentheses and under `as`), by an assignment that names it (`total = () => 1`, `total ??= function () {}`), as its default
- **THEN** the name is spelled there: the delegate gets it, the function takes it as its own `name`; the other uses of the name are plain
- **AND** a value after `=` of Kotlin, a function with a name of its own, a call that gives a function and `total += 1` spell nothing
  [@test] crates/code-symbols/src/names_at_test.rs (`a_name_that_is_handed_on_as_text_is_spelled_where_it_is`)

#### Scenario: Destructured
- **WHEN** a local is declared by an entry of a destructuring of Kotlin, a record pattern of Java, a deconstruction of C#, a tuple of targets of Starlark, a pattern of Rust, or a pattern or a destructuring assignment of TypeScript and JavaScript that names no property by it
- **THEN** the use is destructured
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A string template is plain
- **WHEN** a local is read in a string template of Kotlin, a template literal of TypeScript or an interpolation of C#
- **THEN** the use is plain
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)
