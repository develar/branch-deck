---
name: Name Bindings
description: Requirements for the bindings of the names of a file as a reader of a change asks them, many places from one parse; what a body declares a local as and whether other code may name it, every name bound to it with what its site makes of the name beside the binding (a shorthand property, a label, a namesake only the types tell apart, a spelled name, the tokens of a macro, a destructuring), the names of its spelling that are no variable kept apart, and what another name for the local would clash with.
style: plain-1
targets:
  - crates/code-symbols/src/navigate.rs
  - crates/code-symbols/src/psi_navigate.rs
  - crates/code-symbols/src/kotlin/navigate.rs
  - crates/code-symbols/src/java/navigate.rs
  - crates/code-symbols/src/csharp/navigate.rs
  - crates/code-symbols/src/starlark/navigate.rs
  - crates/code-symbols/src/js/navigate.rs
  - crates/code-symbols/src/rust/navigate.rs
---

# Name Bindings

Status: Implemented
Date: 2026-11-11

## Purpose

A diff that renames a local changes many lines and nothing of the program. A reader may pass those lines only when the
rename provably is one: every name of the old text bound to the declaration is a place of the rename, every name of
the new text bound to the renamed declaration is one too, no other code can spell the name, and no site makes more of
the name than its binding. The spelling of a name proves none of it: two locals of two functions share a name, a
parameter is named by a call of another file, `{ total }` names a property, and a new name may already be taken where
the local is seen.

This spec is what the navigation of a file ([Symbol Navigation](../symbol-navigation/spec.md)) answers for such a
reader, the renames of the marks of a diff ([Quiet Changes](../quiet-changes/spec.md)): the facts of the bindings of
many places from one parse. It decides nothing: which local may be quiet, and at which sites, is the reader's.

## Goals

- One parse of a text answers every place asked of it; the places of one binding share their answer.
- A local says what a body declares it as, and whether code outside its body may still name it.
- Every use of a local says what its site makes of the name beside the binding; a name the language reads as another
  kind of thing than a variable is no use of one.
- Another name for a local says what it would change the meaning of, by the scopes the navigation reads.

## Non-goals

- No types: where only the types tell whether a name is the local the scopes give it (a call of a Kotlin value, its
  `invoke` or a function of that spelling; a name in a Java class that may inherit a member of it), the use says so,
  as a `namesake`, and is never resolved.
- No other file: a name that is no local is answered by its text (the names no local binds), as Find Usages reads it;
  its declaration and its uses in a repository are the index's ([Symbol Navigation](../symbol-navigation/spec.md)).
- No policy: no answer says "quiet" or "safe".

## Concepts

- **A local** is a declaration inside code, which no other file sees: the local of Find Usages. Its **kind** says what
  declares it; `other` is everything that is no value of its body alone (a local function, a local class and its
  members, a type parameter, a label).
- **A use** is a name the scopes bind to the declaration, the declaration itself among them, with how it uses the name
  (declared, read, written, called) and its **site**. A name its place reads as another kind of thing than a variable
  (a type, a Java call, the name of a named argument, a field before its colon) is bound to no local, whatever it
  spells.
- **A clash** is a place whose meaning another name for the local would change: a use that a nearer declaration of the
  new name would take (`shadowed`), a name already spelled so that the local would take (`captured`), or the
  declaration itself where the language binds the new name without one (`implicit`).
- The scopes are those of the navigation ([Symbol Navigation](../symbol-navigation/spec.md)), read from the syntax
  alone, as each language scopes its names. A reader proves a rename by them, so a scope that differs from the
  language's is a defect whichever way it differs: a scope too wide binds a name to a local that the language reads
  as another, and a scope too narrow loses a use, which a change that leaves it in place then passes in silence (the
  use is no place of the rename, so no side sees it). The differences known are in Open Questions.

## Requirements

### Requirement: Answers: The names of many places come from one parse of their text

`names_at` MUST answer every place asked of a text from one parse of it, each as the place asked alone is answered:
the name, the declaration inside code it is bound to, and every name of the text bound as it is, in the order of the
text, its declaration among them. A place where no name is, a place past the text and a text that has no tree MUST
have no answer.

#### Scenario: Many places at once
- **WHEN** every word of a text is asked at once, in each language
- **THEN** each place has the answer it has asked alone
  [@test] crates/code-symbols/src/names_at_test.rs (`many_places_asked_at_once_answer_as_each_asked_alone`)

#### Scenario: No name
- **WHEN** a place asked is a brace, or lies past the last line
- **THEN** it has no answer, and the places beside it have theirs
  [@test] crates/code-symbols/src/names_at_test.rs (`the_places_of_one_binding_share_an_entry_and_a_place_of_no_name_has_none`)

#### Scenario: No tree
- **WHEN** the text is one oxc reads no program of
- **THEN** no place has an answer
  [@test] crates/code-symbols/src/names_at_test.rs (`a_text_with_a_syntax_error_still_answers_the_names_its_tree_holds`)

### Requirement: Answers: The places of one binding share an answer

The places bound to one declaration and asked with the same new name MUST share one entry of the answer, as the
places of one text that no local binds; a place says which entry is its own.

#### Scenario: Three places of a local
- **WHEN** three places of one local are asked, two with a new name and one without, and a place of another name
- **THEN** the answer has one entry for the local without a new name, one for each new name, and one for the other name
  [@test] crates/code-symbols/src/names_at_test.rs (`the_places_of_one_binding_share_an_entry_and_a_place_of_no_name_has_none`)

#### Scenario: Fewer entries than names
- **WHEN** every word of a text is asked
- **THEN** the answer has fewer entries than the text has names
  [@test] crates/code-symbols/src/names_at_test.rs (`many_places_asked_at_once_answer_as_each_asked_alone`)

### Requirement: Answers: Find Usages is the same answer without its sites

The usages of a name ([Symbol Navigation](../symbol-navigation/spec.md)) MUST be the uses `names_at` answers for its
place, with their accesses: of its declaration for a local, else of every name of that text that no local binds.

#### Scenario: The same uses
- **WHEN** Find Usages and `names_at` are asked of the same place
- **THEN** they answer the same name, the same uses with the same accesses, and a local alike
  [@test] crates/code-symbols/src/names_at_test.rs (`many_places_asked_at_once_answer_as_each_asked_alone`)

### Requirement: Answers: A text with a syntax error answers the names its tree holds

A text with a syntax error MUST be answered by the tree its parser left: a local declared and read around the error
has its uses.

#### Scenario: A broken statement between a declaration and its use
- **WHEN** a statement between the declaration of a local and its use is broken, in Kotlin, Java, C#, Starlark, Rust, TypeScript, TSX and JavaScript
- **THEN** the local has its declaration and its use, and the clash of a new name is found
  [@test] crates/code-symbols/src/names_at_test.rs (`a_text_with_a_syntax_error_still_answers_the_names_its_tree_holds`)

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
- **AND** a parameter with a modifier (`private scale`), which declares a property, and a function inside a function are `other`
  [@test] crates/code-symbols/src/names_at_test.rs (`typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

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

### Requirement: Clashes: A name the language binds without a declaration clashes at the declaration

A new name that the language binds without a declaration MUST clash at the declaration of the local, `implicit`: `it`
and `field` of Kotlin, `value` and `field` of C#.

#### Scenario: it and value
- **WHEN** a loop variable of Kotlin is asked with `it`, or a local of a C# setter with `value`
- **THEN** the declaration clashes as implicit
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

## Open Questions / Risks

The scopes are read without types and without the other files, so they differ from the language's in the places
below. None is conservative by itself: a reader that hides what it proves by the bindings keeps the quiet of a rename
behind its measurement on real ranges.

- A Rust pattern that is a lower-case name alone is read as a binding; where a constant of that name is in sight
  (which Rust warns about) it is a constant pattern, and the names after it are the constant's.
- A member a class inherits is in no file the navigation reads. Java: a use in an anonymous class, or in a local
  class with a supertype, inside the body of its variable is a `namesake`; a method of a class with a supertype reads
  an inherited field by a bare name, which the scopes bind to nothing, so a local renamed to that name captures it
  in the answer only where the read is in the file. Kotlin and TypeScript find a local before a member.
- Kotlin: a property declared by a `val` of a primary constructor and read in an initializer of its class is the
  parameter there; the scopes give it the property.
- JavaScript outside strict mode: a function declared in a block is also a variable of the function around the
  block; the scopes keep it in its block.
- C#: a name before the declaration of a local of its block is the local's (an error of the compiler); the scopes
  give it the declaration around.
- Starlark: `total += 1` is read as a read of `total`, though it writes it.
- The uses of a name inside a string are known only for the format strings of Rust; a text the parser reads no tree
  of (TypeScript, TSX and JavaScript with some syntax errors) has no answer at all.

## References

- [Symbol Navigation](../symbol-navigation/spec.md): the scopes and the names of each language
- [Quiet Changes](../quiet-changes/spec.md): the marks of a diff
- [Diff View](../../../../docs/design/diff-view.md) ("The bindings of many names")
