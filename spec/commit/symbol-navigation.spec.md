---
name: Symbol Navigation
description: Requirements for going to the declaration of a name in the diffs of a review and finding its usages, as in IntelliJ: ⌘-click, ⌘B, ⌥F7 and the menu of the code, a choice of declarations, a list of usages, Back and Forward, and what lies outside the review, found by accio, in a view of its file.
style: plain-1
targets:
  - ../../src/components/diff/DiffView.tsx
  - ../../src/components/diff/diff-painter.ts
  - ../../src/components/diff/SymbolChooser.tsx
  - ../../src/components/diff/SymbolSearchStatus.tsx
  - ../../src/components/diff/DeclarationFileView.tsx
  - ../../src/components/diff/CommitReviewCode.tsx
  - ../../src/hooks/useSymbolNavigation.ts
  - ../../src/lib/diff/code-menu.ts
  - ../../crates/code-symbols/src/navigate.rs
  - ../../crates/file-diffs/src/declarations.rs
  - ../../crates/file-diffs/src/usages.rs
  - ../../crates/file-diffs/src/accio.rs
  - ../../crates/app-api/src/reads/symbols.rs
  - ../../crates/app-api/src/session/windows.rs
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
and the declarations from the summaries the sync already made, from the blobs the diffs read: a jump costs no git. A
name the review does not declare is looked for in the index of the repository that
[accio](https://github.com/JetBrains/accio) keeps, when accio is installed and has indexed the repository, at the
commit of the side; its file opens over the review at that commit, coloured and navigable as the diffs. ⌥F7 lists the
usages of a name, as IntelliJ's "Show Usages": those the syntax trees of the review show, then those the index knows
outside it.

## Goals

- Follow a call or a use to what it means without leaving the review.
- Never guess silently: an ambiguous name is a choice, a name without a declaration in the review is said.
- Cost nothing the diffs have not read already.

## Non-Goals

- Building an index: a repository accio has not indexed is searched in the review only.
- Resolution as the compiler does it: what the syntax and the declarations of the review tell, not the types. The usages
  of a name that is not a local are the names of that text, as the index of accio answers them too.

## Requirements

- A ⌘-click (Ctrl-click off macOS, where Meta works too) on a name of a diff must show its declaration; with the
  modifier held, the name under the pointer is underlined as a link and the pointer is a hand.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)
- ⌘B (Ctrl+B off macOS) on the selection and the item "Go to Declaration" of the menu of the code must do the same; the
  item is disabled, with the reason, where no name is.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code`)
- One Go to Declaration or Find Usages is in flight per view: the same name asked again before the answer joins it (two
  ⌘B open one file, one Back returns), another name replaces it, and an answer that arrives after the view shows
  another commit (⌥↓) does nothing.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should open one file for a name clicked twice before the answer, so one Back returns to the review`)
  [@test] ../../src/lib/navigation-gate.test.ts (`joins a repeat of the same name: two ⌘B open one view`, `drops an answer whose subject changed, and one another name replaced`)
- No deadline cuts a search: the index is waited for as long as accio runs, since a loaded machine stretches a query of
  0.03–3 s. While the answer has not come, the name shows "Looking for the declaration…" or "Finding usages…" (faded in
  late, so a quick answer shows none) with Cancel; Escape or Cancel stops it, kills its accio, and its answer, if one
  still comes, does nothing. A newer search of the window, ⌥↓ to another subject and the close of the window kill it
  too; a killed search answers nothing (the code `canceled`), never a toast.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should say that it searches while the backend answers, and stop the search on Escape and on Cancel`)
  [@test] ../../src/lib/navigation-gate.test.ts (`drops the answer of a canceled navigation, and says whether one was in flight`)
  [@test] ../../crates/file-diffs/src/accio_test.rs (`waits_for_a_slow_index_as_long_as_it_runs`, `a_canceled_search_kills_its_query`)
  [@test] ../../crates/file-diffs/src/usages_test.rs (`a_canceled_search_answers_nothing`)
  [@test] ../../crates/app-api/src/session/windows_test.rs (`a_window_supersedes_only_its_own_request`, `a_closed_window_supersedes_its_request`)
- The declaration shown has its file open, the fold around it expanded, the view scrolled to it and its name selected
  once its row is painted with its text, however long its text takes, and again when a new layout paints the row anew,
  until the user presses or types in the view or selects elsewhere.
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
  declaration of Any found").
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code`, `should say when the review has no declaration of a name`)
- Back (⌘[, Ctrl+Alt+← off macOS) must return to the name a jump started from, and Forward (⌘], Ctrl+Alt+→) to the
  declaration again.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)
- A name that the files of the review do not declare must be looked for in the index of accio, at the commit of its
  side (the commit or the parent for a commit, the head or the base for a range, the working tree for uncommitted
  changes), `Owner.name` first when the owner is known; only declarations of the language family and of the kind of
  the name (a call: a function, a constructor or a type; a type: a type), ranked by the imports of the file, the
  owner, then the top directory. A declaration in a file of the review is shown there. One `cat-file --batch-check` names the blobs of the
  other files and one `cat-file --batch` reads them; nothing runs when accio is missing, and without a usable index of
  the repository accio answers at once (`--no-build`): never an index build, a wait for one or a refresh.
  [@test] ../../crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)
  [@test] ../../crates/file-diffs/src/accio_test.rs (`an_index_accio_cannot_use_answers_no_index_with_its_reason`)
- A declaration outside the review must open its whole file over the review: its path, "at <commit>, outside the
  review" ("the working tree, outside the review" for uncommitted changes), coloured as the diffs, its name selected,
  and Go to Declaration in it as in the review (a declaration of the review closes the file and shows it there).
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should open a declaration outside the review that the index of the repository knows, go back to the review, and from that file to a declaration of the review`)
- Back (the key or the button "Back" of the file) must close the file and return to the name the jump started from; a
  file of the tree, J and K, or N and P show in the review and close the files over it.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should open a declaration outside the review that the index of the repository knows, go back to the review, and from that file to a declaration of the review`)
- ⌥F7 (Alt+F7 off macOS) on the selection and the item "Find Usages" of the menu of the code must list the usages of
  the name at the name: "N usages of X" (the declarations not counted), the usages in the review, then those outside
  it, each with how it uses the name (declaration, import, call, write, read), its line with the name in bold, and its
  file and line; ↑ and ↓ and Enter or a click show one, in the review or in the view of its file, and Back returns to
  the name; Escape gives the focus back to the view. A name with no usage but its declarations says so in a toast ("No
  usages of version found"); the item is disabled, with the reason, where no name is.
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should list the usages of a name in the review and outside it, from ⌥F7 and from the menu of the code, and show the one chosen`, `should say when a name has no usage but its declaration`)
- The usages of a local (a parameter, a local of a function) are the names its scopes bind to its declaration, in its
  file; those of any other name are the names of that text that no local binds, in its file and in the same side of
  the other files of the review in a language that sees it, from the texts the diffs read: no git process.
  [@test] ../../crates/code-symbols/src/navigate_test.rs (`finds_the_usages_of_a_kotlin_parameter_apart_from_the_property_it_shadows`, `tells_a_java_write_an_increment_and_an_import`, `finds_the_usages_of_a_rust_binding_and_of_an_imported_function`, `finds_the_usages_of_a_name_in_another_file_without_the_locals_that_shadow_it`)
  [@test] ../../crates/file-diffs/src/usages_test.rs (`finds_a_word_in_the_files_of_the_review_without_git_and_a_local_in_its_file_only`)
- The usages outside the review must come from the index of accio at the commit of the side (100 at most), only in the
  files the review lacks and in a language that sees the name; one `cat-file --batch-check` and one `cat-file --batch`
  read their files, for their lines and the view of each. A cut answer says "The index has more usages outside the
  review"; nothing runs when accio is missing or has no index of the repository.
  [@test] ../../crates/file-diffs/src/usages_test.rs (`adds_the_usages_outside_the_review_that_the_index_knows_at_the_revision_of_the_side`)
- An index that fails must take nothing from what the review knows: the usages of the review stay, with "accio did not
  answer: <why>" under them, and a name without a declaration in the review says why in its toast.
  [@test] ../../crates/file-diffs/src/usages_test.rs (`an_index_that_fails_takes_nothing_from_the_review_and_says_why`)
  [@test] ../../tests/e2e/commit-files/symbol-navigation.spec.ts (`should keep what the review knows and say why when the index of the repository fails`)
