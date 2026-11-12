## MODIFIED Requirements

### Requirement: Answers: An entry asked with another name says the statements of its names

An entry asked with another name MUST say the lines of the statements of its names (`NameAt::statements`), any other
entry none: of a local the member around its uses, whole, after the head of each type around that member; of any
other name the statements of its declarations alone, each with its bodies but the members of a type. A local without
a member (of a function no statement stands around) and a name the tree gives no statement MUST say the whole text.

#### Scenario: The member around a local
- **WHEN** a local is the parameter of a lambda at the end of a chain of calls, a parameter of its function, a local of a function declared in that function or of a method of an object or of an anonymous class declared in it, the parameter of a lambda that is the value of a property, or a local of an initializer, in Kotlin, Java, C#, Starlark, Rust and TypeScript
- **THEN** its statements are the lines of the function of the class or of the file around it, from its name to its last line, of the property, or of the initializer, and the head of the class, the namespace, the module or the `impl` that holds the member: never another member of it
- **AND** a local of a statement of a module of TypeScript has that statement
  [@test] crates/code-symbols/src/names_at_test.rs (`the_statements_of_a_local_are_the_member_around_it_and_those_of_another_name_its_declarations`)

#### Scenario: The head of a type
- **WHEN** a local is of the value of a property or of an initializer of a Kotlin class with a primary constructor, of a member of an entry of an enum, of the compact constructor of a Java record, of a method of a C# class with a primary constructor, of a function of a Rust `impl` with type parameters, of a method of a TypeScript class with type parameters or of a class of a namespace
- **THEN** its statements are the head of each type around its member, from the first token of the type to its members, and the member: the parameters of the constructor, the components, the type parameters with their bounds
- **AND** no line of another member of the type is among them
  [@test] crates/code-symbols/src/names_at_test.rs (`the_member_of_a_local_holds_the_heads_of_its_types_and_a_function_in_the_head_of_a_type_has_none`)

#### Scenario: A function in the head of a type
- **WHEN** a local is of a lambda given to the constructor of a supertype of a Kotlin class or to a constant of a Java enum, of a function in a decorator of a TypeScript class or in what the class extends, or of a closure in a block of the head of a Rust `impl`
- **THEN** its statements are every line of the text: the statement of the body of that function that holds the name is no member
  [@test] crates/code-symbols/src/names_at_test.rs (`the_member_of_a_local_holds_the_heads_of_its_types_and_a_function_in_the_head_of_a_type_has_none`)

#### Scenario: The value of an `export default`
- **WHEN** a local is of a function that is the value of an `export default` of TypeScript, or of a method of an object that is such a value in JavaScript
- **THEN** its statements are the lines of the export, whole
- **AND** a local of a method of a class and of a function that an `export default` declares has the member it has without the export
  [@test] crates/code-symbols/src/names_at_test.rs (`the_member_of_a_local_holds_the_heads_of_its_types_and_a_function_in_the_head_of_a_type_has_none`)

#### Scenario: A name that is not local
- **WHEN** a constant whose value goes on over a line, a function with a block, a field, a method, a property of a constructor or a class with members is asked with another name
- **THEN** the statements are those of its declarations alone, none of its uses: the constant and the field whole, the function and the method with their blocks, the class and the property of its constructor the head of the class without its members
  [@test] crates/code-symbols/src/names_at_test.rs (`the_statements_of_a_local_are_the_member_around_it_and_those_of_another_name_its_declarations`)

#### Scenario: A constant of an enum with a body
- **WHEN** a constant of a Java enum with arguments and a body is asked with another name
- **THEN** its statements are its name and its arguments, without the members of its body
  [@test] crates/code-symbols/src/names_at_test.rs (`the_statement_of_a_constant_of_an_enum_is_without_the_members_of_its_body`)

#### Scenario: Asked together or alone
- **WHEN** every word of a text of each language is asked, every other one with another name, together and each alone
- **THEN** each entry says the same statements either way, and an entry of a name the text declares none of says none
  [@test] crates/code-symbols/src/names_at_test.rs (`many_places_asked_at_once_answer_as_each_asked_alone`)
