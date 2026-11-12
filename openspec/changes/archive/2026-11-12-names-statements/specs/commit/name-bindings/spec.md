## ADDED Requirements

### Requirement: Answers: An entry asked with another name says the statements of its names

An entry asked with another name MUST say the lines of the statements of its names (`NameAt::statements`), any other
entry none. Of a local they MUST be the member around its uses, whole: the outermost statement around a name that
declares no type, without the comments before it. Of any other name they MUST be the statements of its declarations
alone, each with its bodies but the members of a type. A name the tree gives no statement MUST say the whole text.

#### Scenario: The member around a local
- **WHEN** a local is the parameter of a lambda at the end of a chain of calls, a parameter of its function, a local of a function declared in that function or of a method of an object or of an anonymous class declared in it, the parameter of a lambda that is the value of a property, or a local of an initializer, in Kotlin, Java, C#, Starlark, Rust and TypeScript
- **THEN** its statements are the lines of the function of the class or of the file around it, from its name to its last line, of the property, or of the initializer: never those of the class, the namespace, the module or the `impl` that holds the member
- **AND** a local of a statement of a module of TypeScript has that statement
  [@test] crates/code-symbols/src/names_at_test.rs (`the_statements_of_a_local_are_the_member_around_it_and_those_of_another_name_its_declarations`)

#### Scenario: A name that is not local
- **WHEN** a constant whose value goes on over a line, a function with a block, a field, a method, a property of a constructor or a class with members is asked with another name
- **THEN** the statements are those of its declarations alone, none of its uses: the constant and the field whole, the function and the method with their blocks, the class and the property of its constructor the head of the class without its members
  [@test] crates/code-symbols/src/names_at_test.rs (`the_statements_of_a_local_are_the_member_around_it_and_those_of_another_name_its_declarations`)

#### Scenario: Asked together or alone
- **WHEN** every word of a text of each language is asked, every other one with another name, together and each alone
- **THEN** each entry says the same statements either way, and an entry of a name the text declares none of says none
  [@test] crates/code-symbols/src/names_at_test.rs (`many_places_asked_at_once_answer_as_each_asked_alone`)

## MODIFIED Requirements

### Requirement: Sites: A use says what its site makes of the name beside its binding

Each use MUST say what its site makes of the name beside its binding, the first of: `shorthand` (also a property
there), `named argument`, `label`, `namesake` (only the types, or what runs, tell whether the name is the binding),
`spelled` (the spelling is a value where the code runs, or the language may hand the text around the name on),
`macro` (among the tokens of a macro), `destructured` (a pattern that takes a value apart declares or assigns it);
else plain, in a string template too.

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

#### Scenario: The text C# may hand to what is called
- **WHEN** a variable or a parameter of a C# body stands in an argument of a call, of a `new` or of an indexer (declared there by `out var` too), in what a method is called on (after `?.` too), in a lambda whose body is an expression or as the parameter of one, or in a query
- **THEN** the use is spelled: a parameter of what is called may take the text of its argument or of its receiver (`[CallerArgumentExpression]`: the exception of `ArgumentNullException.ThrowIfNull(total)`, the message of `Debug.Assert(total > 0)`), and an expression tree holds the names of its parameters and of the variables around it
- **AND** a member read of the variable, an assignment, a `return`, an interpolation, a delegate called by its name and a lambda with a block that is no argument are plain; a name that is no value of a body is plain in an argument too
- **AND** so the change of such a name reads as the rename of one local only where none of its uses stands at such a place
  [@test] crates/code-symbols/src/names_at_test.rs (`csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`, `a_text_with_a_syntax_error_still_answers_the_names_its_tree_holds`)

#### Scenario: Destructured
- **WHEN** a local is declared by an entry of a destructuring of Kotlin, a record pattern of Java, a deconstruction of C#, a tuple of targets of Starlark, a pattern of Rust, or a pattern or a destructuring assignment of TypeScript and JavaScript that names no property by it
- **THEN** the use is destructured
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A string template is plain
- **WHEN** a local is read in a string template of Kotlin, a template literal of TypeScript or an interpolation of C#
- **THEN** the use is plain
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)
