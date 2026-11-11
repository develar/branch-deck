---
name: Name Bindings
description: Requirements for the bindings of the names of a file as a reader of a change asks them, many places from one parse; what a body declares a local as and whether a call may name it, every name bound to it with what its site makes of the name beside the binding (a shorthand property, a named argument, a label, a namesake, a spelled name, the tokens of a macro, a destructuring), and what another name for the local would clash with.
style: plain-1
targets:
  - crates/code-symbols/src/navigate.rs
  - crates/code-symbols/src/psi_navigate.rs
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
- A local says what a body declares it as, and whether a call may still write its name.
- Every use of a local says what its site makes of the name beside the binding.
- Another name for a local says what it would change the meaning of, by the scopes the navigation reads.

## Non-goals

- No types: a call of a Kotlin value (`total()`, its `invoke` or a function of that spelling) and a member reached
  without a qualifier are told as far as the syntax tells them, as a `namesake`, never resolved.
- No other file: a name that is no local is answered by its text (the names no local binds), as Find Usages reads it;
  its declaration and its uses in a repository are the index's ([Symbol Navigation](../symbol-navigation/spec.md)).
- No policy: no answer says "quiet" or "safe".

## Concepts

- **A local** is a declaration inside code, which no other file sees: the local of Find Usages. Its **kind** says what
  declares it; `other` is everything that is no value of its body alone (a local function, a local class and its
  members, a type parameter, a label).
- **A use** is a name the scopes bind to the declaration, the declaration itself among them, with how it uses the name
  (declared, read, written, called) and its **site**.
- **A clash** is a place whose meaning another name for the local would change: a use that a nearer declaration of the
  new name would take (`shadowed`), a name already spelled so that the local would take (`captured`), or the
  declaration itself where the language binds the new name without one (`implicit`).
- The scopes are those of the navigation, read from the syntax alone. Where a language reads a name by more than its
  scopes, the site says so; where the scopes of the navigation are narrower than the language's, a use is missing and
  the reader sees a name of the rename that is no use: never the reverse.

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
- **WHEN** a local and its uses, a parameter of Java, a second `let` of Rust that shadows, a loop variable, a value under a property or a named argument that keeps its name are renamed
- **THEN** both sides read as one local
  [@test] crates/code-symbols/src/names_at_test.rs (`a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike`)

#### Scenario: Changes of meaning
- **WHEN** the value of a variable reads another parameter after the change, a statement after a loop or an `if let` another field or parameter, a default or a `return` another name of the file, the new name takes a read of a member, or the change renames a shorthand property, a named argument, the argument of `nameof`, a called Kotlin value or a parameter a call may name
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

A parameter that a call may write as a named argument MUST say so, as its uses are then not all that spell its name:
the parameter of a function with a name, a method or a constructor of Kotlin, of anything C# calls by a name (a method,
a constructor, a local function, an indexer, a delegate) and of a `def` of Starlark but `*args` and `**kwargs`. The
parameter of a lambda, an anonymous function or a setter, and every parameter of Java, Rust, TypeScript and JavaScript
MUST NOT.

#### Scenario: Named by calls
- **WHEN** a parameter of a Kotlin function, of a constructor of a local class, of a C# method or local function, or of a `def` is asked
- **THEN** it says a call may name it
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Named by no call
- **WHEN** the parameter of a Kotlin setter, lambda or function without a name, of a C# lambda, `*args` of a `def`, or a parameter of Java, Rust, TypeScript or JavaScript is asked
- **THEN** it says no call names it
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_tells_what_a_local_is`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

### Requirement: Sites: A use says what its site makes of the name beside its binding

Each use MUST say what its site makes of the name beside the binding the scopes give it, the first of: `shorthand`
(the name is also a property there), `named argument`, `label`, `namesake` (the site reads another kind of thing than
the binding is), `spelled` (the spelling is a value), `macro` (among the tokens of a macro), `destructured` (a
pattern that takes a value apart declares or assigns it); else it is plain, a string template that reads it too.

#### Scenario: Shorthand
- **WHEN** a local is written as `{ total }` in an object, a pattern or the target of an assignment of TypeScript, TSX or JavaScript, as `Ledger { total }` of Rust, or as `new { total }` and `(total, scale)` of C#
- **THEN** the use is a shorthand
  [@test] crates/code-symbols/src/names_at_test.rs (`typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A named argument and a label
- **WHEN** a name the scopes bind to a local is the name of a named argument of Kotlin or Starlark, or a name is a label of Java or the name of its jump
- **THEN** the use is a named argument, or a label
- **AND** the name of a named argument of C# and of an element of a Java annotation is bound to no local
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: A namesake
- **WHEN** a name bound to a variable or a parameter is called in Kotlin or Java, read as a type in any language that keeps types apart, written after `::` in Kotlin, the name of a member an initializer of C# sets, the name of a field of Rust before its colon or the name of a macro
- **THEN** the use is a namesake
- **AND** a call of a local of C#, Rust, Starlark, TypeScript or JavaScript, where a call names a value, is plain
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `kotlin_tells_what_a_local_is`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

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
be another declaration's, and `captured`, a name already spelled so that would become the local's. The name of a
named argument and a label MUST be neither. No clash MUST be an empty list, and a place asked without a new name, or
of no local, MUST have none.

#### Scenario: Shadowed
- **WHEN** a parameter is asked with the name of a loop variable, a use of it inside the loop
- **THEN** that use is shadowed, in every language
  [@test] crates/code-symbols/src/names_at_test.rs (`kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `java_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`, `javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes`)

#### Scenario: Captured
- **WHEN** a local is asked with the name of a member, a constant or a name of the file that its body reads
- **THEN** that read is captured, in every language; a named argument of the new name is not
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

- A Rust pattern that is a lower-case name alone is read as a binding; where a constant of that name is in sight it
  is a constant pattern, which only the types tell.
- A nested Rust `fn` does not see the locals around it; the scopes give its names to them.
- The uses of a name inside a string are known only for the format strings of Rust.

## References

- [Symbol Navigation](../symbol-navigation/spec.md): the scopes and the names of each language
- [Quiet Changes](../quiet-changes/spec.md): the marks of a diff
- [Diff View](../../../../docs/design/diff-view.md) ("The bindings of many names")
