## MODIFIED Requirements

### Requirement: Answers: A change of names reads as one local only where both sides bind alike

The answers MUST tell the rename of one local from a change of what the code means: read on each side of a change
(every changed name one entry, a value of its body that no call names, its uses the changed names alone and each
plain, no clash with the other name), a rename of one local holds on both sides, and a change that reads another
declaration, sets another property or lets other code spell the name fails on one.

#### Scenario: Renames of one local
- **WHEN** a local and its uses, a parameter of Java, a second `let` of Rust that shadows, a loop variable, a local of the top-level statements of C#, or a value are renamed, the value under a property, a named argument, a field of a Rust struct or a member of a C# initializer that keeps its name, or beside the method or the function of its old name
- **THEN** both sides read as one local
  [@test] crates/code-symbols/src/names_at_test.rs (`a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

#### Scenario: Changes of meaning
- **WHEN** the value of a variable reads another parameter after the change, a statement after a loop, an `if let` or a statement without a block another field or parameter, a default, a `return`, a closure, a query or the condition of a `do … while` another name, the new name takes a read of a member, a later assignment of a `def` or a later `var` of a function writes another variable, or the change renames a shorthand property, a named argument, the argument of `nameof`, a called Kotlin value, a namespace, a parameter a call or an attribute macro names, or a name only the types tell from an inherited member or a constant of an enum
- **THEN** one side does not read as one local
  [@test] crates/code-symbols/src/names_at_test.rs (`a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

### Requirement: Locals: A parameter a call may name says so

A parameter that code outside its body may name MUST say so: a call names the parameter of a function with a name, a
method or a constructor of Kotlin, of anything C# calls by a name and of a `def` or a `lambda` of Starlark (but
`*args`, `**kwargs`); an attribute macro reads those of a Rust function under an attribute that is not the language's
own. The parameter of a Kotlin or C# lambda, an anonymous function, a setter or a closure, and every one of Java,
TypeScript and JavaScript MUST NOT.

#### Scenario: Named by calls
- **WHEN** a parameter of a Kotlin function, of a constructor of a local class, of a C# method or local function, or of a `def` or a `lambda` of Starlark is asked
- **THEN** it says a call may name it
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Named by no call
- **WHEN** the parameter of a Kotlin setter, lambda or function without a name, of a C# lambda, `*args` of a `def`, or a parameter of Java, Rust, TypeScript or JavaScript is asked
- **THEN** it says no call names it
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_tells_what_a_local_is`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Read by an attribute macro
- **WHEN** a parameter of a Rust function under `#[instrument(skip(amount))]` is asked, and one of a function under `#[inline]` and a `cfg_attr` of `allow`
- **THEN** the first says other code may name it, with the name among the tokens of the attribute a use of it; the second does not
  [@test] crates/code-symbols/src/names_at_test.rs (`rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

### Requirement: Sites: A use says what its site makes of the name beside its binding

Each use MUST say what its site makes of the name beside the binding the scopes give it, the first of: `shorthand`
(the name is also a property there), `named argument`, `label`, `namesake` (only the types tell whether the name is
the binding), `spelled` (the spelling is a value), `macro` (among the tokens of a macro), `destructured` (a pattern
that takes a value apart declares or assigns it); else it is plain, a string template that reads it too.

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
- **WHEN** a name bound to a variable or a parameter is called in Kotlin or written after `::` there, is all of a `case` label of Java, or stands in an anonymous class of Java inside the body of the variable
- **THEN** the use is a namesake
- **AND** a call of a local of C#, Rust, Starlark, TypeScript or JavaScript, where a call names a value, is plain
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Spelled, and the tokens of a macro
- **WHEN** a local is the argument of `nameof` of C#, or stands among the tokens of a Rust macro or in a format string of one
- **THEN** the use is spelled, or of a macro
  [@test] crates/code-symbols/src/names_at_test.rs (`csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Destructured
- **WHEN** a local is declared by an entry of a destructuring of Kotlin, a record pattern of Java, a deconstruction of C#, a tuple of targets of Starlark, a pattern of Rust, or a pattern or a destructuring assignment of TypeScript and JavaScript that names no property by it
- **THEN** the use is destructured
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A string template is plain
- **WHEN** a local is read in a string template of Kotlin, a template literal of TypeScript or an interpolation of C#
- **THEN** the use is plain
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

### Requirement: Clashes: A new name for a local is tried against the scopes

A place of a local asked with a new name MUST answer what the name would change the meaning of, by the scopes asked
of the names of the new spelling and the names of the local as one name: `shadowed`, a name of the local that would
be another declaration's, and `captured`, a name already spelled so that would become the local's. No clash MUST be
an empty list, and a place asked without a new name, or of no local, MUST have none.

#### Scenario: Shadowed
- **WHEN** a parameter is asked with the name of a loop variable, a use of it inside the loop
- **THEN** that use is shadowed, in every language but Starlark, where the loop variable of a `def` and the parameter would be one variable: the names of the loop variable are captured
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Captured
- **WHEN** a local is asked with the name of a member, a constant or a name of the file that its body reads
- **THEN** that read is captured, in every language; a named argument of the new name and a call of a Java method of the new name are not
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: No clash, and none asked
- **WHEN** a local is asked with a name that changes no binding, with its own name, or without a new name
- **THEN** the clashes are an empty list, an empty list, and none
  [@test] crates/code-symbols/src/names_at_test.rs (`the_places_of_one_binding_share_an_entry_and_a_place_of_no_name_has_none`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

## ADDED Requirements

### Requirement: Sites: A name its place reads as another kind of thing is no use of a local

A name its place reads as another kind of thing than a variable MUST be no use of a local of its spelling and no
clash of one: a type and the namespace before one, a call and a method reference of Java, the name of a named
argument, the member an initializer or a `Name =` of C# names, the field of a Rust struct before its colon, the name
of a macro and a path segment before `::`. It is a use of the names of its text that no local binds.

#### Scenario: Not among the uses of the local
- **WHEN** a local is asked beside a type of its spelling in Kotlin, Java, C#, Rust or TypeScript, a call of it in Java, the name of an argument it is given as in Kotlin or Starlark, the member a C# initializer names, or the field of a Rust struct before its colon and the name of a macro
- **THEN** none of them is among its uses
- **AND** each is among the uses of the name of that text that no local binds, which has no clashes
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)
