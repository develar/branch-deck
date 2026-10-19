---
name: Symbol Navigation
description: Requirements for going to the declaration of a name in the diffs of a review, as in IntelliJ: ⌘-click, ⌘B and the menu of the code, a choice of declarations, Back and Forward.
style: plain-1
targets:
  - ../../src/components/diff/DiffView.tsx
  - ../../src/components/diff/diff-painter.ts
  - ../../src/components/diff/DeclarationChooser.tsx
  - ../../src/components/diff/CommitReviewCode.tsx
  - ../../src/hooks/useGoToDeclaration.ts
  - ../../src/lib/diff/code-menu.ts
  - ../../crates/code-symbols/src/navigate.rs
  - ../../crates/file-diffs/src/declarations.rs
  - ../../src-tauri/src/commands/file_diffs.rs
---

# Symbol Navigation

Status: Implemented
Date: 2026-10-19

## Summary

A review reads a change the way the editor reads code: a name in a diff leads to its declaration. As in IntelliJ, a
⌘-click (Ctrl-click off macOS) on a name, ⌘B on it or the item "Go to Declaration" of the menu of the code shows the
declaration: in the same file by its scopes (a parameter, a local, a member of the class around it), else in the files
of the review, on the same side of their diffs. A name with several declarations offers them, the most likely first;
Back and Forward return along the jumps. The backend reads the names from the syntax tree (`code_symbols::navigate`)
and the declarations from the summaries the sync already made, from the blobs the diffs read: a jump costs no git.

## Goals

- Follow a call or a use to what it means without leaving the review.
- Never guess silently: an ambiguous name is a choice, a name without a declaration in the review is said.
- Cost nothing the diffs have not read already.

## Non-Goals

- A declaration outside the review (the file view with `accio`) and Find Usages: later steps.
- Resolution as the compiler does it: what the syntax and the declarations of the review tell, not the types.

## Requirements

- A ⌘-click (Ctrl-click off macOS, where Meta works too) on a name of a diff must show its declaration; with the
  modifier held, the name under the pointer is underlined as a link and the pointer is a hand.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)
- ⌘B (Ctrl+B off macOS) on the selection and the item "Go to Declaration" of the menu of the code must do the same; the
  item is disabled, with the reason, where no name is.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code`)
- The declaration shown has its file open, the fold around it expanded, the view scrolled to it and its name selected.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)
- A name declared in its own file by the scopes (a parameter, a local, a property of the class around it, a member
  declared after its use, a hoisted TypeScript function) must go there; the name of a declaration goes nowhere.
  [@test] ../../crates/code-symbols/src/navigate_test.rs (`finds_a_kotlin_local_a_parameter_that_shadows_a_property_and_a_member_declared_after_its_use`, `finds_a_java_field_a_local_and_tells_a_static_call_by_its_qualifier`, `finds_a_rust_binding_and_tells_a_method_and_a_path`, `finds_a_typescript_parameter_a_hoisted_function_and_tells_a_property`, `has_no_reference_outside_a_name`)
  [@test] ../../crates/file-diffs/src/declarations_test.rs (`goes_to_a_local_by_the_scopes_and_nowhere_from_a_declaration`)
- Else the declarations of the name in the files of the review, on the side of the name, in a language that sees it,
  ranked by what the name says of its owner: a member reached through a type, `this` or `self`, or a property of a
  declared type (`items.add` of `val items: Store`) prefers that type; then the same file, the files that import its
  package, its directory. The texts are the blobs the diffs read, the summaries those of the store: no git process.
  [@test] ../../crates/file-diffs/src/declarations_test.rs (`goes_to_a_constant_of_a_type_in_another_file_of_the_review_without_git`, `prefers_the_member_of_the_type_a_property_is_declared_with`)
- Several declarations are a choice at the name, as IntelliJ's "Choose Declaration": the member's owner, the file and
  the line of each, the first focused, ↑ and ↓ and Enter or a click to show one. None says so in a toast ("No
  declaration of Any in the review").
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code`, `should say when the review has no declaration of a name`)
- Back (⌘[, Ctrl+Alt+← off macOS) must return to the name a jump started from, and Forward (⌘], Ctrl+Alt+→) to the
  declaration again.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)
