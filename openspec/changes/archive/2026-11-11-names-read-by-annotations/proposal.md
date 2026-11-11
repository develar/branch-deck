# Proposal

## Why

`commit/name-bindings` said that no code outside its body names a parameter of Java. A framework that reads
annotations does: Spring binds a `@PathVariable String id` and a `@RequestParam` by the name the compiler keeps
(`-parameters`), Spring Data the `:name` of a `@Query`, Jackson and `@ConfigurationProperties` the parameters of a
constructor. A reader of renames took `get(@PathVariable String id)` → `get(@PathVariable String userId)` for the
rename of one local, though the request then fails. It is the class the first round fixed for Rust (an attribute
macro reads the parameters of its function).

The same round made the right side of a C# `is` a type alone; it is a constant where no type has the name, and then
only the other files tell which, so the use says so.

## What Changes

- Java: the parameter of a method or a constructor says that other code may name it where the parameter, its method
  or a class around it has an annotation that is not one of the language, of nullability or of the static checks
  (`java::navigate::is_read_by_annotation`, `INERT_ANNOTATIONS`).
- C#: a local that the right side of an `is` reads is a `namesake` there.
- The readings of a rename hold for the cases the second round of the review found.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/name-bindings`: the locals of Java, the sites of C#, the readings of a rename.

## Impact

- `crates/code-symbols/src/java/navigate.rs`, `csharp/navigate.rs`, `navigate.rs` (the words of `LocalBinding` and
  `Site::Namesake`).
- No type the page receives changed.
