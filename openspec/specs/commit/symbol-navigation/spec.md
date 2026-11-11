---
name: Symbol Navigation
description: Requirements for going to the declaration of a name in the diffs of a review and finding its usages, as in IntelliJ: ⌘-click, ⌘B, ⌥F7 and the menu of the code, a choice of declarations, a list of usages, Back and Forward, and what lies outside the review, found by accio, in a view of its file.
style: plain-1
targets:
  - src/components/diff/DiffView.tsx
  - src/components/diff/diff-painter.ts
  - src/components/diff/SymbolChooser.tsx
  - src/components/diff/SymbolSearchStatus.tsx
  - src/components/diff/DeclarationFileView.tsx
  - src/components/diff/CommitReviewCode.tsx
  - src/hooks/useSymbolNavigation.ts
  - src/lib/diff/code-menu.ts
  - src/lib/menu-key.ts
  - crates/code-symbols/src/navigate.rs
  - crates/code-symbols/src/summary_builder.rs
  - crates/code-symbols/src/kotlin/navigate.rs
  - crates/code-symbols/src/java/navigate.rs
  - crates/code-symbols/src/csharp/navigate.rs
  - crates/code-symbols/src/starlark/navigate.rs
  - crates/code-symbols/src/js/navigate.rs
  - crates/code-symbols/src/rust/navigate.rs
  - crates/file-diffs/src/declarations.rs
  - crates/file-diffs/src/usages.rs
  - crates/file-diffs/src/accio.rs
  - crates/app-api/src/reads/symbols.rs
  - crates/app-api/src/session/windows.rs
---

# Symbol Navigation

Status: Implemented
Date: 2026-10-19

## Purpose

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
- Compiler resolution inside the review: it uses the syntax and declarations the review has, not types. Outside, when
  the review has the declaration, a place-based index query uses type/import links; without one, the index is still
  queried by name.

## Requirements

### Requirement: A modifier click on a name shows its declaration

A ⌘-click (Ctrl-click off macOS, where Meta works too) on a name of a diff MUST show its declaration. With the
modifier held, the name under the pointer SHALL be underlined as a link and the pointer SHALL be a hand.

#### Scenario: The modifier underlines the name and a click goes to its declaration
- **GIVEN** the modifier is held (⌘, Ctrl off macOS, where Meta works too)
- **WHEN** the pointer is over a name of a diff and the user clicks it
- **THEN** before the click the name under the pointer is underlined as a link and the pointer is a hand
- **AND** the click shows the declaration of the name
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)

### Requirement: ⌘B and the menu of the code go to the declaration

⌘B (Ctrl+B off macOS) on the selection and the item "Go to Declaration" of the menu of the code MUST do the same as a
⌘-click. The item MUST be disabled, with the reason, where no name is.

#### Scenario: ⌘B and the item "Go to Declaration" show the declaration
- **WHEN** the user presses ⌘B (Ctrl+B off macOS) on the selection, or chooses "Go to Declaration" in the menu of the code on a name
- **THEN** the declaration of the name is shown, as on a ⌘-click
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code`)

#### Scenario: The item is disabled where no name is
- **WHEN** the menu of the code opens where no name is
- **THEN** "Go to Declaration" is disabled, with the reason
- **AND** the backend finds no reference at a place that is no name
  [@test] crates/code-symbols/src/navigate_test.rs (`has_no_reference_outside_a_name`)
  [@test] src/lib/diff/code-menu.test.ts (`offers both items for a name, and disables them with the reason where no name is`)

### Requirement: One Go to Declaration or Find Usages is in flight per view

One Go to Declaration or Find Usages MUST be in flight per view: the same name asked again before the answer MUST join
it, another name MUST replace it, and an answer that arrives after the view shows another commit (⌥↓) MUST do nothing.

#### Scenario: The same name asked again before the answer joins it
- **WHEN** the user presses ⌘B twice on the same name before the answer comes
- **THEN** one file opens
- **AND** one Back returns to the review
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open one file for a name clicked twice before the answer, so one Back returns to the review`)
  [@test] src/lib/navigation-gate.test.ts (`joins a repeat of the same name: two ⌘B open one view`)

#### Scenario: Another name replaces it, another commit drops its answer
- **WHEN** another name is asked before the answer comes, or the view shows another commit (⌥↓) before it comes
- **THEN** the answer of the first name does nothing
  [@test] src/lib/navigation-gate.test.ts (`drops an answer whose subject changed, and one another name replaced`)

### Requirement: No deadline cuts a search

No deadline MUST cut a search: the index is waited for as long as accio runs, since a loaded machine stretches a query
of 0.03–3 s.

#### Scenario: A slow index is waited for
- **WHEN** accio answers a query slowly
- **THEN** the search waits for it as long as accio runs, and no deadline cuts it
  [@test] crates/file-diffs/src/accio_test.rs (`waits_for_a_slow_index_as_long_as_it_runs`)

### Requirement: A search in flight says so and Escape or Cancel stops it

While the answer has not come, the name MUST show "Looking for the declaration…" or "Finding usages…" (faded in late,
so a quick answer shows none) with Cancel. Escape or Cancel MUST stop the search and kill its accio, and its answer, if
one still comes, MUST do nothing.

#### Scenario: The name says that it searches
- **WHEN** the backend has not answered a Go to Declaration or a Find Usages yet
- **THEN** the name shows "Looking for the declaration…" or "Finding usages…" with Cancel, faded in late
- **AND** a quick answer shows none of it
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should say that it searches while the backend answers, and stop the search on Escape and on Cancel`)

#### Scenario: Escape or Cancel stops the search and kills its accio
- **WHEN** the user presses Escape or Cancel while the search is in flight
- **THEN** the search stops and its accio is killed
- **AND** its answer, if one still comes, does nothing
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should say that it searches while the backend answers, and stop the search on Escape and on Cancel`)
  [@test] src/lib/navigation-gate.test.ts (`drops the answer of a canceled navigation, and says whether one was in flight`)
  [@test] crates/file-diffs/src/accio_test.rs (`a_canceled_search_kills_its_query`)

### Requirement: A newer search, another subject or the close of the window kill a search

A newer search of the window, ⌥↓ to another subject and the close of the window MUST kill the search in flight too. A
killed search MUST answer nothing (the code `canceled`), never a toast.

#### Scenario: A newer search of the window or ⌥↓ kills the one in flight
- **WHEN** the window starts a newer search, or ⌥↓ goes to another subject, while a search is in flight
- **THEN** the search in flight is killed, and only the one of that window
  [@test] crates/app-api/src/session/windows_test.rs (`a_window_supersedes_only_its_own_request`)

#### Scenario: The close of the window kills its search
- **WHEN** the window closes while its search is in flight
- **THEN** the search is killed
  [@test] crates/app-api/src/session/windows_test.rs (`a_closed_window_supersedes_its_request`)

#### Scenario: A killed search answers nothing
- **WHEN** a search is killed
- **THEN** it answers nothing (the code `canceled`)
- **AND** no toast is shown
  [@test] crates/file-diffs/src/usages_test.rs (`a_canceled_search_answers_nothing`)

### Requirement: The declaration shown is revealed with its name selected

The declaration shown MUST have its file open, the fold around it expanded, the view scrolled to it and its name
selected at its exact position on the line, once its row is painted with its text, however long its text takes, and
again when a new layout paints the row anew, until the user presses or types in the view or selects elsewhere. A name
of the old side on an unchanged line, which one column shows once by its new line, MUST be selected in the line shown,
at the columns the name has there.

#### Scenario: A declaration in a fold is revealed with its name selected
- **WHEN** a jump shows a declaration inside a fold
- **THEN** its file is open, the fold around it expanded and the view scrolled to it
- **AND** its name is selected at its exact position on the line once its row is painted with its text
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

#### Scenario: A new layout paints the row anew
- **GIVEN** the name of the declaration shown is selected
- **WHEN** a new layout paints its row anew, before the user presses or types in the view or selects elsewhere
- **THEN** the name is selected again at its position

#### Scenario: A name of the old side on a line one column shows by its new line
- **GIVEN** a call indented again, an unchanged line under Ignore Whitespace, and a jump from its name on the old side in Split
- **WHEN** the layout is Inline and the user goes Back
- **THEN** the name is selected in the new line of the row, four columns further than in the old line, and not the text at the columns of the old line
- **AND** its text is selected as any, though the press in Split kept the selection on the old side
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should select a name of the old side at its own columns in a line the whitespace policy ignores, which one column shows by its new line`)
  [@test] src/lib/diff/line-parts.test.ts (`places a name of the old line in the new line of a row the whitespace policy ignores, by the characters that are no whitespace`)

### Requirement: A name of the old line is selected in the Inline row that shows it

A name of the old line whose line Inline shows as the row of a similar pair MUST be selected where that row shows it:
in the text the row shares with the new line, or struck through in a deleted part, which is selectable for as long as
the selection is that name. A name found by its word is found in the line itself, never in the text of the row.

#### Scenario: A removed name the row strikes through
- **GIVEN** a declaration whose name changed, its old and its new line one row in Inline
- **WHEN** the old name is shown (a removed declaration of Possible Breakage)
- **THEN** it is selected in the struck-through part of the row, and not the name that replaced it
- **AND** after a press in the diffs the deleted part is left out of a selection again
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should select the name of a removed declaration in the Inline row that strikes it through, until a press in the diffs`)

#### Scenario: A name after a deleted part of its row
- **GIVEN** a declaration whose line lost a modifier before its name, one row in Inline with the modifier struck through
- **WHEN** the name is shown by its word
- **THEN** the name is selected, not the text as many columns further as the deleted part is long
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)

#### Scenario: A name of a pair of another indent
- **GIVEN** a whitespace policy pairs a line with the line indented again and edited
- **WHEN** a name of the old line is placed in the row
- **THEN** it stands at its own characters, whatever the indent of the new line
  [@test] src/lib/diff/line-parts.test.ts (`places an offset of the old line by the text the lines share, whatever whitespace a policy ignored in it`, `gives the place of an offset of each line in the text of the row`)

### Requirement: The selected name of the old line is copied and read by the keys of the code

While the name of the old line that a reveal selected in its Inline row is selected, a copy MUST take it, and ⌘B, ⌥F7
and Back MUST read it as the name of the old line, on the old side. A modifier alone and the copy MUST NOT let the
name go.

#### Scenario: A copy and Find Usages of the selected old name
- **GIVEN** the old name of a row is selected in its struck-through part
- **WHEN** the user copies, then presses ⌥F7
- **THEN** the copy is the old name, which stays selected after the keys of the copy
- **AND** the usages are asked at the name of the old line, on the old side, not at the new name that stands after it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should select the name of a removed declaration in the Inline row that strikes it through, until a press in the diffs`)

### Requirement: A struck-through name of an Inline row is the name of the old line

A name in a deleted part of the row of a similar pair in Inline MUST be the name of the old line, on the old side:
with the modifier held its underline is that part alone, and a ⌘-click (Ctrl-click off macOS) and the menu of the
code MUST act on it, never on the word of the new line beside it. The underline of a name of the new line MUST NOT
reach into a struck-through part before it.

#### Scenario: The old name under the pointer
- **GIVEN** a call whose name changed, its old and its new line one row in Inline, the old name struck through before the new one
- **WHEN** the pointer is on the old name with the modifier held, then on the new name
- **THEN** the underline is the old name alone, then the new name alone
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click`)

#### Scenario: A click on the old name
- **WHEN** the user ⌘-clicks the old name, then goes Back
- **THEN** the declaration of the old name is asked on the old side and shown
- **AND** Back selects the old name in its struck-through part
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click`)

### Requirement: The menu of the code acts on the name it opened on

The menu of the code opened by the pointer MUST act on the name under the pointer, whatever is selected elsewhere.
Where the pointer is on no name its items MUST be disabled, with the reason, also while a name is selected elsewhere.

#### Scenario: The menu on a struck-through name after a click elsewhere
- **GIVEN** a click left the caret in another row
- **WHEN** the user opens the menu of the code on the struck-through old name of an Inline row and chooses "Find Usages"
- **THEN** the usages listed are those of the old name
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click`)

#### Scenario: The pointer on no name while a name is selected elsewhere
- **GIVEN** a name is selected in the code
- **WHEN** the user opens the menu of the code with the pointer on the number of another line
- **THEN** "Go to Declaration" and "Find Usages" are disabled
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

### Requirement: A name is under the pointer only where the pointer is on its text

The name under the pointer MUST be a name whose text the pointer is on. Beside the text of a line (its number, the
space before its first character, the blank end of its row) the pointer is on no name, though the nearest text is
one: the modifier MUST underline nothing there, and the menu of the code MUST have no name.

#### Scenario: The pointer between the number of a line and its first word
- **GIVEN** a line that starts with a word, and the modifier held with the pointer on a name, which is underlined
- **WHEN** the pointer moves into the space between the number of that line and the word
- **THEN** nothing is underlined and the pointer is no hand
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)

#### Scenario: The menu of the code on the number of a line
- **WHEN** the user opens the menu of the code on the number of a line that starts with a word
- **THEN** its items are disabled: the word is not under the pointer
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

### Requirement: The context menu key and Shift+F10 open the menu of the code on the name at the selection

While the view has the focus, the context menu key and Shift+F10 MUST open the menu of the code on the name at the
selection (a selected name, or the name the caret is in), under the start of that name, wherever the pointer is.
Where the selection holds no name the menu MUST open at the selection, or at the start of the view, its items
disabled. A menu key on a focused control of the view (the header of a file) is that control's.

#### Scenario: The context menu key at a caret
- **GIVEN** a click left the caret in a name, nothing selected, and the focus on the view
- **WHEN** the user presses the context menu key
- **THEN** the menu of the code opens under the start of that name, not at the middle of the view, "Go to Declaration" and "Find Usages" enabled
- **AND** Escape closes it and gives the focus back to the view
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: Shift+F10 and the keys of the menu
- **WHEN** the user presses Shift+F10, then ↓, ↓ and Enter
- **THEN** the menu opens under the name and the usages of that name are listed
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: A selected name while the pointer is on another
- **GIVEN** a name is selected and the pointer is over another name
- **WHEN** the user presses the context menu key and chooses "Go to Declaration"
- **THEN** the menu opens under the selected name and acts on it
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: A selection out of the view, or none
- **WHEN** a scroll took the selection out of the view, or nothing is selected in the view
- **THEN** the menu opens inside the view: at the edge nearest to the selection, or at the start of the view
  [@test] src/lib/diff/code-menu.test.ts (`opens for a menu key under the start of the name at the selection`, `opens for a menu key inside the view when a scroll took the selection out of it`, `opens for a menu key at the start of the view where nothing is selected in it`)

### Requirement: A menu key opens one menu

A menu key that opens a menu of the app (the menu of the code, the menu of a row of pull requests) MUST open no menu
of the engine beside it: its press is prevented, and so is the next release of the context menu key, at which Windows
opens the menu of that key at whatever has the focus by then. A key held down MUST open the menu once.

#### Scenario: The keys that ask for the menu
- **WHEN** a key is pressed
- **THEN** the context menu key and Shift+F10 ask for the menu; F10 alone and a key with Alt, Ctrl or Meta do not
  [@test] src/lib/menu-key.test.ts (`takes the context menu key and Shift+F10 for the menu, and no key with another modifier`)

#### Scenario: The release of the context menu key
- **GIVEN** the context menu key opened a menu of the app, which took the focus
- **WHEN** the key goes up
- **THEN** that release is prevented, wherever it lands in the window, and no other release is
- **AND** a window that lost the focus meanwhile waits for no release
  [@test] src/lib/menu-key.test.ts (`prevents the release of the context menu key once, wherever it lands in the page, and no other release`, `stops waiting for a release when the window loses the focus, not when an element of it does`)

#### Scenario: Shift+F10
- **WHEN** Shift+F10 opens a menu of the app
- **THEN** the menu opens at the point given, as a right click there, the press is prevented and no release is
  [@test] src/lib/menu-key.test.ts (`opens the menu of the trigger at the point, as a right click there, and prevents the press`, `prevents no release after Shift+F10, whose menu the engine opens as the key goes down`)

#### Scenario: A key held down
- **WHEN** the key repeats
- **THEN** the menu opens once, and no repeat opens a menu of the engine
  [@test] src/lib/menu-key.test.ts (`opens the menu once for a key held down`)

### Requirement: The view keeps its selection while a popup of it has the focus

The selection of the code MUST be as it was once the menu of the code or a list of declarations or usages closes and
the focus returns to the view, also in an engine that clears the selection of the page as the focus leaves it
(WebKit), so that the keys of the code read the same name again.

#### Scenario: The menu closed by Escape
- **GIVEN** the caret is in a name and the context menu key opened the menu on it
- **WHEN** the user presses Escape, then a menu key again
- **THEN** the menu opens on the same name again
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: An item that jumps nowhere
- **GIVEN** a name is selected and a menu key opened the menu on it
- **WHEN** "Go to Declaration" finds no declaration and the menu has closed
- **THEN** the name is selected as it was
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

### Requirement: The caret of a field is no selection elsewhere

While the name waits for its paint, the caret of a field (the search that the reveal of a hidden file clears) MUST
NOT count as a selection elsewhere.

#### Scenario: The reveal of a file hidden by a search clears the search
- **GIVEN** a text search hides the file of the declaration and its field has the caret
- **WHEN** the reveal clears the search and shows the file
- **THEN** the name is still selected once its row is painted
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should reveal an overview file under a text search while retaining its owner chip`)

### Requirement: The name is selected where the parser of the file read it

That position MUST be the one the parser of the file read, never a word found on the line: a declaration of the review
is selected at the column its summary carries (`Declaration::name_column`, in UTF-16 units, written by every walk), one
outside it from the column to the end column accio answers, and the same declaration is at the same column on either
side.

#### Scenario: A line that writes the name twice
- **GIVEN** `#[doc(alias = "needle")] pub fn needle(…)`
- **WHEN** the function is shown as a declaration, inside the review or outside it
- **THEN** the name of the function is selected, not the word of its attribute
- **AND** it is at the same column on either side
  [@test] crates/code-symbols/src/summary_test.rs (`a_declaration_whose_line_writes_its_name_twice_is_found_at_its_own_column`)
  [@test] crates/file-diffs/src/declarations_test.rs (`finds_the_rust_impl_method_at_one_column_inside_and_outside_the_review`)

#### Scenario: A doc comment above the name
- **WHEN** a doc comment stands above the name of a declaration
- **THEN** the column of the summary is still the one of the name the parser read
  [@test] crates/code-symbols/src/summary_test.rs (`a_doc_comment_above_a_name_leaves_the_column_of_the_name`)

#### Scenario: The column of the summary counts UTF-16 units
- **WHEN** a line has characters of more than one UTF-16 unit before the name
- **THEN** `Declaration::name_column` counts the UTF-16 units of its line before it
  [@test] crates/code-symbols/src/summary_test.rs (`a_name_column_counts_the_utf16_units_of_its_line_before_it`)

#### Scenario: A declaration outside the review is selected from the column to the end column accio answers
- **WHEN** accio answers a declaration outside the review with its column and end column
- **THEN** the selection goes from that column to that end column
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

### Requirement: A place without a name or a column selects nothing

A declaration that writes no name (an anonymous class at `new`) SHALL select nothing at its place; a hit of an accio
that answers no column SHALL go to its line with no selection, and one of an accio before 0.21 SHALL select as many
units as the name has.

#### Scenario: A declaration that writes no name
- **WHEN** the declaration shown writes no name (an anonymous class at `new`)
- **THEN** nothing is selected at its place
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

#### Scenario: An accio that answers no column
- **WHEN** a hit of accio has no column
- **THEN** the view goes to its line with no selection, no word guessed on it
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

#### Scenario: An accio before 0.21
- **WHEN** a hit comes from an accio before 0.21
- **THEN** as many units are selected as the name has

### Requirement: A name declared in its own file goes there by the scopes

A name declared in its own file by the scopes (a parameter, a local, a property of the class around it, a member
declared after its use, a hoisted TypeScript function) MUST go there; the name of a declaration MUST go nowhere.

#### Scenario: Kotlin names go to their declarations by the scopes
- **WHEN** a name of a Kotlin file is a local, a parameter that shadows a property, or a member declared after its use
- **THEN** Go to Declaration goes to the declaration the scopes bind it to
  [@test] crates/code-symbols/src/navigate_test.rs (`finds_a_kotlin_local_a_parameter_that_shadows_a_property_and_a_member_declared_after_its_use`)
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`the_name_of_a_parameter_used_in_the_body_goes_to_the_parameter`, `the_parameter_of_a_catch_is_a_local_declaration`, `the_subject_of_a_where_clause_is_the_type_parameter`, `a_script_declares_its_top_level_names_for_its_functions`)

#### Scenario: Java names go to their declarations by the scopes
- **WHEN** a name of a Java file is a field, a local or a parameter its scopes declare
- **THEN** Go to Declaration goes to the declaration the scopes bind it to
  [@test] crates/code-symbols/src/navigate_test.rs (`finds_a_java_field_a_local_and_tells_a_static_call_by_its_qualifier`)
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_loop_variable_a_pattern_variable_a_resource_and_a_lambda_parameter_declare_their_name`, `a_use_before_the_declaration_of_a_local_is_the_field`, `the_members_of_an_anonymous_class_are_its_own`, `a_break_goes_to_its_label`, `an_argument_of_an_annotation_goes_to_the_element_of_its_type_in_the_file`, `var_and_this_are_no_names`, `a_call_through_super_of_an_interface_has_that_interface_for_its_qualifier`, `a_compact_constructor_reads_the_record_component_and_a_record_pattern_names_a_type`, `the_dotted_names_of_an_import_and_a_module_are_a_value_and_members_without_a_qualifier`, `a_pattern_variable_is_seen_where_java_sees_it_and_the_field_elsewhere`, `the_arguments_of_an_anonymous_class_see_the_code_around_it_not_its_members`)

#### Scenario: Rust and TypeScript names go to their declarations by the scopes
- **WHEN** a name of a Rust file is a binding, or a name of a TypeScript file is a parameter or a hoisted function
- **THEN** Go to Declaration goes to the declaration the scopes bind it to
  [@test] crates/code-symbols/src/navigate_test.rs (`finds_a_rust_binding_and_tells_a_method_and_a_path`, `finds_a_typescript_parameter_a_hoisted_function_and_tells_a_property`)

#### Scenario: C# names go to their declarations by the scopes
- **WHEN** a name of a C# file is a parameter, a field, a local, a pattern variable or a type parameter
- **THEN** Go to Declaration goes to the declaration the scopes bind it to
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_name_goes_to_its_declaration_by_the_scopes`)

#### Scenario: A C# member after a qualifier
- **WHEN** a name of a C# file is a member after a qualifier (`a.b`)
- **THEN** it has its qualifier, and the scopes bind it to no declaration
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_member_after_a_qualifier_has_its_qualifier_and_no_declaration`)

#### Scenario: A local of the review, and the name of a declaration
- **WHEN** Go to Declaration runs on a local of a file of the review
- **THEN** it goes to the local by the scopes
- **AND** on the name of a declaration it goes nowhere
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_to_a_local_by_the_scopes_and_nowhere_from_a_declaration`)

### Requirement: A Kotlin name after a receiver and `::` is its member

A Kotlin name after a receiver and `::` (`Foo::bar`, `this::bar`) MUST be a member of that receiver, as after a dot.

#### Scenario: A callable reference names a member of its receiver
- **WHEN** Go to Declaration runs on `bar` of `Foo::bar` or `this::bar`
- **THEN** the name is a member of that receiver, as after a dot
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_name_after_a_receiver_and_colons_is_its_member`)

### Requirement: A Kotlin label and a doc comment have no names but this@Outer

In Kotlin a label and a name in a doc comment MUST be no names, but the `Outer` of `this@Outer` SHALL be a type.

#### Scenario: A label, a doc comment and the label of this
- **WHEN** Go to Declaration runs on a label or a name in a doc comment
- **THEN** there is no name there
- **AND** the `Outer` of `this@Outer` is a type that goes to its class
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_label_and_a_doc_comment_have_no_names_but_the_label_of_this`)

### Requirement: A Kotlin variable is seen after its own statement

In Kotlin code a variable MUST be seen only after its own statement, the variable of a `for` only after its range and
the subject of a `when` only after its value: the value reads the name of a scope around it. A name before the
declaration of a local of its block MUST go to the declaration of a scope around the block; the members of a class
and of a file are seen wherever they are declared.

#### Scenario: The value of a variable, the range of a for, a name before the declaration
- **WHEN** a function declares `val name = name.trim()` beside its parameter `name`, loops `for (items in items)`, reads `when (val count = count())` and reads `count` before its `val count`
- **THEN** the value of `name` goes to the parameter, the range to the parameter `items`, both early `count` to the property of the file
- **AND** after their statements the names go to the variables
- **AND** the usages of the parameter are its declaration and the value read from it
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`code_reads_in_its_order_a_variable_after_its_statement_and_a_loop_variable_after_its_range`)

### Requirement: C# local functions, labels, verbatim names and using static

In C# a local function MUST be seen in all of its block, `goto` MUST go to the label of its name in the blocks around it
(the innermost first), a name MUST compare by its value (`@count` is `count`), and the target of a `using static`
written as one name (`using static Console;`) SHALL be a type; a dotted target is read as the names of any `using`, a
value and members after it (`Math` of `using static System.Math;` is a member).

#### Scenario: A local function is seen in all of its block
- **WHEN** a name calls a local function declared after the statement of the use
- **THEN** it goes to that local function
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_name_goes_to_its_declaration_by_the_scopes`)

#### Scenario: A goto goes to its label
- **WHEN** Go to Declaration runs on the label of a `goto`
- **THEN** it goes to the label of its name in the blocks around it, the innermost first
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_goto_goes_to_the_label_of_its_name_in_the_blocks_around_it`)

#### Scenario: A name compares by its value
- **WHEN** a name is written verbatim (`@count`)
- **THEN** it is the name `count`
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_verbatim_identifier_is_found_by_its_value`)

#### Scenario: The target of a using static
- **WHEN** the target of a `using static` is one name (`using static Console;`)
- **THEN** it is a type

#### Scenario: The dotted names of a using
- **WHEN** a `using` or a namespace writes a dotted name (`using System.Text;`, `using static System.Math;`)
- **THEN** its first name is a value and the names after it are members
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_names_of_a_namespace_and_a_using_are_values_and_members`)

### Requirement: C# documentation comments and directives have no names

In C# a name in a documentation comment or a directive MUST be none.

#### Scenario: A documentation comment or a directive
- **WHEN** Go to Declaration runs on a name in a documentation comment or a directive
- **THEN** there is no name there
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`a_name_in_a_documentation_comment_or_a_directive_is_none`)

### Requirement: The collection of a loop is read before its variable

The collection a loop runs over MUST be read outside the loop: `for (String item : item.parts())` of Java and
`foreach (var item in item)` of C# read the `item` around the loop. The variable of the loop MUST be seen in its
statement alone: a name after the loop goes to the declaration around it.

#### Scenario: Java
- **WHEN** a Java method loops `for (String item : item.parts())` beside a field `item` and reads `item` after the loop
- **THEN** the collection and the name after the loop go to the field, the body to the variable
  [@test] crates/code-symbols/src/java/navigate_test.rs (`the_collection_of_a_for_is_read_before_its_variable`)

#### Scenario: C#
- **WHEN** a C# method loops `foreach (var item in item)` beside a field `item` and reads `item` after the loop
- **THEN** the collection and the name after the loop go to the field, the body to the variable
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_variable_of_a_foreach_is_seen_in_its_statement_alone`)

### Requirement: A TypeScript or JavaScript file is read in the dialect of its extension

A TypeScript or JavaScript file MUST be read in the dialect of its extension: a `.ts` file's `<T>x` asserts a type, a
`.js` file's JSX is JSX.

#### Scenario: A type assertion of a .ts file
- **WHEN** a `.ts` file writes `<T>x`
- **THEN** it asserts a type, and its `x` is a name of the code
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_typescript_file_reads_a_type_assertion`)

#### Scenario: JSX of a .js file
- **WHEN** a `.js` file writes JSX
- **THEN** it is read as JSX
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_jsx_tag_goes_to_its_component`)

### Requirement: TypeScript and JavaScript declarations are seen in their scopes

In TypeScript and JavaScript a `var` MUST be seen in all of its function, a `let`, a `const`, a class and a function in
all of their block, a parameter of a function type or a signature in it alone, and what `infer` declares in its
conditional type.

#### Scenario: A var in its function, a let in its block
- **WHEN** a name is declared by a `var`, or by a `let`, a `const`, a class or a function
- **THEN** it is seen in all of its function for the `var`, in all of its block for the others
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_var_is_seen_in_its_function_and_a_let_in_its_block`, `deep_nesting_is_navigated_on_a_small_stack`)

#### Scenario: A parameter of a type, a type parameter and what infer declares
- **WHEN** a name is a parameter of a function type or a signature, or what `infer` declares
- **THEN** it is seen in that function type or signature alone, or in its conditional type
  [@test] crates/code-symbols/src/js/navigate_test.rs (`type_parameters_infer_and_a_predicate_go_to_their_declarations`)

### Requirement: A bare TypeScript name is never a member of the class around it

In TypeScript and JavaScript a bare name MUST NOT be a member of the class around it: only `this.` reaches one.

#### Scenario: A bare name in a method
- **WHEN** a method of a class uses a bare name that a member of the class also has
- **THEN** the name is not that member; only `this.` reaches it
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_bare_name_is_no_member_of_the_class_around_it`)

### Requirement: TypeScript destructurings, members and private names are declarations

In TypeScript and JavaScript the names of a destructuring, the members of a class, an interface and an enum and a
private name (`#count`) MUST be declarations.

#### Scenario: The names of members and a private name
- **WHEN** Go to Declaration runs on a member of a class, an interface or an enum, a private name (`#count`) or a name of a destructuring
- **THEN** that name is a declaration
  [@test] crates/code-symbols/src/js/navigate_test.rs (`the_names_of_members_are_declarations`)

### Requirement: TypeScript calls, qualified types, import types and imports

In TypeScript and JavaScript a call with type arguments and a `new` MUST be calls, the last name of `a.B` in a type a
type qualified by `a`, a name after `import("m")` a member of it, and a name of an import a value whose declaration is
in its module.

#### Scenario: A call with type arguments and a new
- **WHEN** a name is called with type arguments, or follows `new`
- **THEN** it is a call
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_call_with_type_arguments_and_a_new_are_calls`)

#### Scenario: A qualified name in a type
- **WHEN** `a.B` stands in a type
- **THEN** `B` is a type qualified by `a`
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_qualified_type_is_a_type_with_its_qualifier`)

#### Scenario: A name after an import type
- **WHEN** a name follows `import("m")`
- **THEN** it is a member of that module
  [@test] crates/code-symbols/src/js/navigate_test.rs (`the_names_after_an_import_type_are_members_of_its_module`)

#### Scenario: A name of an import
- **WHEN** Go to Declaration runs on a name of an import
- **THEN** it is a value whose declaration is in its module
  [@test] crates/code-symbols/src/js/navigate_test.rs (`a_name_of_an_import_goes_to_its_module`)

### Requirement: What no TypeScript name of a declaration of the file is

In TypeScript and JavaScript a JSX attribute, a lowercase JSX tag, a label and the parameter of an index signature
MUST be no names of a declaration of the file.

#### Scenario: A JSX attribute, a lowercase tag, a label, an index signature
- **WHEN** Go to Declaration runs on a JSX attribute, a lowercase JSX tag, a label or the parameter of an index signature
- **THEN** it is no name of a declaration of the file

### Requirement: Rust patterns and aliases declare their bindings

A Rust file MUST be read by rust-analyzer's parser: the binding of any pattern (`if let`, a `for`, a match arm, a
closure's parameter, a field of a struct pattern) and an alias (`as e`) are declarations, a capitalized name alone in a
pattern (`None`) a value.

#### Scenario: The binding of any pattern and an alias
- **WHEN** a name is bound by any pattern (`if let`, a `for`, a match arm, a closure's parameter, a field of a struct pattern) or is an alias (`as e`)
- **THEN** it is a declaration its uses go to
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_binding_of_any_pattern_is_a_declaration`, `deep_nesting_is_navigated_on_a_small_stack`)

#### Scenario: A capitalized name alone in a pattern
- **WHEN** a pattern is a capitalized name alone (`None`)
- **THEN** it is a value, no declaration
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_variant_in_a_pattern_is_no_declaration`)

### Requirement: The names of a Rust attribute are in the scopes of its item

The names of a Rust attribute MUST be in the scopes of its item: `#[instrument(skip(ctx))]` names the parameter.

#### Scenario: An attribute names a parameter
- **WHEN** Go to Declaration runs on `ctx` of `#[instrument(skip(ctx))]`
- **THEN** it goes to the parameter of the function
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`the_names_of_an_attribute_are_in_the_scopes_of_its_item`)

### Requirement: What no Rust name is

In Rust a lifetime, a label, `Self`, `_`, a predicate of `#[cfg]`, a metavariable and a fragment of a macro and a
keyword among its tokens MUST be no names.

#### Scenario: Lifetimes, labels, Self, _, cfg and macro tokens
- **WHEN** Go to Declaration runs on a lifetime, a label, `Self`, `_`, a predicate of `#[cfg]`, a metavariable or a fragment of a macro, or a keyword among its tokens
- **THEN** there is no name there
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`what_is_no_name`)

### Requirement: Rust calls and paths with type arguments

In Rust a method called with type arguments MUST be a call, and a name with type arguments before `::` a type.

#### Scenario: Type arguments in a call and in a path
- **WHEN** a method is called with type arguments, or a name with type arguments stands before `::`
- **THEN** the method is a call and the name a type
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`the_kinds_of_paths_and_calls`)

### Requirement: A Rust binding is seen after its statement and in the block it guards

In Rust the binding of a `let` MUST be seen only after its statement, so `let total = total + 1` reads the `total`
before it; the binding of an `if let` or a `while let` only in the rest of its condition and the block it guards; the
binding of a `for` only in its body. An item, a parameter and the binding of an arm are seen in all of their scope,
and a binding shadows an item.

#### Scenario: A let that shadows, an if let, a while let, a for, a let else
- **WHEN** a function shadows its parameter by `let total = total + 1`, takes `found` apart by `if let Some(found) = found`, loops `while let Some(items) = next(&items)` and `for items in items.iter()`, then declares `let Some(found) = found else { … }`
- **THEN** each value and collection goes to the parameter, each block to its binding
- **AND** after the block of the `if let`, `found` is the parameter again
- **AND** the usages of the parameter `total` are its declaration and the value read from it
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_binding_is_seen_after_its_statement_and_in_the_block_it_guards`)

### Requirement: A Rust format string uses the names it takes from the code around it

A string among the tokens of a Rust macro may be a format string, so a name it would take from the code around the
macro MUST be a usage of that name: a word right after the `{` of a placeholder (`{total}`, `{total:>8}`) or before a
`$` (`{:width$}`). A doubled brace, the braces of a code point and a string outside a macro MUST name nothing, and a
name inside a string is no place to go from.

#### Scenario: Placeholders, a width, a doubled brace
- **WHEN** Find Usages runs on a parameter that `format!("{total} {{total}} {total:>8} {:width$} …", total)` reads, a string outside a macro spelling `{total}` too
- **THEN** its usages are its declaration, the two placeholders and the argument; `width` has its declaration and the width
- **AND** Go to Declaration finds no name inside the string
  [@test] crates/code-symbols/src/rust/navigate_test.rs (`a_format_string_uses_the_names_it_takes_from_the_code_around_it`)

### Requirement: A Bazel file has the scopes of the file, a def and a comprehension

A Bazel file has the scope of the file and, inside it, the body of a `def` or a `lambda` and a comprehension: a name
MUST go to the `def`, the assignment or the `load` of the file that binds it, and a name declared inside a `def` (a
parameter, a name the body assigns) or by a comprehension is a local, whose usages stay in that `def` or that
comprehension.

#### Scenario: A name of the file goes to what binds it
- **WHEN** Go to Declaration runs on a call of a `def` of the file, or a read of a name the file assigns, in a `BUILD` or a `.bzl` file
- **THEN** it goes to the `def` or the assignment that binds it
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_call_of_a_def_of_the_file_goes_to_the_def`, `a_read_of_an_assigned_name_goes_to_its_assignment`, `a_bzl_file_reads_the_same_names_as_a_build_file`)

#### Scenario: A qualified call is bound by no scope of the file
- **WHEN** a call is qualified by a receiver
- **THEN** it names its receiver, and no `def`, assignment or `load` of the file is its declaration
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_qualified_call_names_its_receiver_and_has_no_declaration_of_its_own`)

#### Scenario: A name declared inside a def is a local
- **WHEN** Find Usages runs on a parameter of a `def` or a name its body assigns
- **THEN** it is a local, whose usages stay in that `def`
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`the_usages_of_a_parameter_stay_inside_its_def`)

#### Scenario: The variable of a comprehension is a local
- **WHEN** a comprehension binds a name, inside a `def` or at the top level of the file, that the code around it spells too
- **THEN** the name is a local of the comprehension: past it the name is the one around again
- **AND** the usages of the name of the file hold none of the comprehension
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_comprehension_is_a_scope_and_a_default_is_read_where_its_function_is_defined`)

### Requirement: A Bazel default and the first collection of a comprehension are read outside their scope

The default of a parameter of a `def` or a `lambda` MUST be read where the function is defined, and the first
collection of a comprehension where the comprehension stands: neither sees the names their scope declares. A later
collection and the condition of a comprehension see its variables.

#### Scenario: A default and a first collection
- **WHEN** a `def` has the parameters `name, deps = name, key = lambda name = name: name` and the comprehension `[name for name in name.split() if name]`, beside a `name` of the file
- **THEN** both defaults go to the `name` of the file, the body of the `lambda` to its parameter
- **AND** the first collection goes to the parameter of the `def`, the element and the condition to the variable of the comprehension
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_comprehension_is_a_scope_and_a_default_is_read_where_its_function_is_defined`)

### Requirement: A name a Bazel load binds is declared by its string

A name a `load` binds is written as a string (`load(":a.bzl", "macro")`), so that string MUST be the declaration a use
goes to, at the name inside its quotes, and the use counts as an import.

#### Scenario: A call of a loaded macro goes to the line that loads it
- **WHEN** Go to Declaration runs on a call of a name a `load` binds
- **THEN** it goes to the string of the `load`, at the name inside its quotes
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_call_of_a_loaded_macro_goes_to_the_line_that_loads_it`)

#### Scenario: The usages of a loaded name
- **WHEN** Find Usages runs on a loaded name
- **THEN** its usages are its import and its calls
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`the_usages_of_a_loaded_name_are_its_import_and_its_calls`)

### Requirement: A label in a Bazel string is no name

A label in a string (`":x"`, `"//pkg:x"`) MUST be no name: a target is reached by the build graph, not by the tree.

#### Scenario: A label in a string
- **WHEN** Go to Declaration runs on a label in a string (`":x"`, `"//pkg:x"`)
- **THEN** there is no name there
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`a_label_in_a_string_is_no_name`)

### Requirement: A name its own file does not bind is looked for in the review, ranked

The declarations of a name that its own file does not declare by the scopes MUST be looked for in the files of the
review, on the side of the name, in a language that sees it, ranked by what the name says of its owner: a member reached through a type, `this` or `self`,
or a property of a declared type (`items.add` of `val items: Store`) prefers that type; then the same file, the files
that import its package, its directory.

#### Scenario: A member of the type a property is declared with comes first
- **WHEN** Go to Declaration runs on `items.add` of `val items: Store`, and the review declares `add` in `Store` and in another type
- **THEN** the `add` of `Store` comes first
  [@test] crates/file-diffs/src/declarations_test.rs (`prefers_the_member_of_the_type_a_property_is_declared_with`)

#### Scenario: The same file, the files that import its package, its directory
- **WHEN** the name says nothing of its owner and several files of the review declare it
- **THEN** the same file comes first, then the files that import its package, then its directory

### Requirement: The declarations of the review cost no git process

The texts of a lookup in the review MUST be the blobs the diffs read and the summaries those of the store: no git
process.

#### Scenario: A constant of a type in another file of the review
- **WHEN** Go to Declaration runs on a constant of a type that another file of the review declares
- **THEN** it goes there
- **AND** no git process runs
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_to_a_constant_of_a_type_in_another_file_of_the_review_without_git`)

### Requirement: Several declarations are a choice at the name

Several declarations MUST be a choice at the name, as IntelliJ's "Choose Declaration": the member's owner, the file and
the line of each, the first focused, ↑ and ↓ and Enter or a click to show one.

#### Scenario: An ambiguous name offers its declarations
- **WHEN** Go to Declaration finds several declarations of a name
- **THEN** a choice opens at the name with the member's owner, the file and the line of each, the first focused
- **AND** ↑ and ↓ and Enter or a click show the one chosen
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code`)

### Requirement: No declaration says so in a toast

A name with no declaration MUST say so in a toast ("No declaration of Any found").

#### Scenario: The review has no declaration of the name
- **WHEN** Go to Declaration finds no declaration of `Any`
- **THEN** a toast says "No declaration of Any found"
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should say when the review has no declaration of a name`)

### Requirement: Back and Forward return along the jumps

Back (⌘[, Ctrl+Alt+← off macOS) MUST return to the name a jump started from, and Forward (⌘], Ctrl+Alt+→) to the
declaration again.

#### Scenario: Back returns to the name, Forward to the declaration
- **GIVEN** a jump went from a name to its declaration
- **WHEN** the user presses Back (⌘[, Ctrl+Alt+← off macOS), then Forward (⌘], Ctrl+Alt+→)
- **THEN** Back returns to the name the jump started from
- **AND** Forward shows the declaration again
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)

### Requirement: A name the review does not declare is looked for in the index

A name that the files of the review do not declare MUST be looked for in the index of accio, at the commit of its side,
`Owner.name` first when the owner is known; only declarations of the language family and of a kind the name can mean,
ranked by the imports of the file, the owner, then the top directory. A declaration in a file of the review is shown
there.

#### Scenario: The index is asked at the commit of the side
- **WHEN** Go to Declaration runs on a name that no file of the review declares
- **THEN** the index of accio is asked at the commit of its side: the commit or the parent for a commit, the head or the base for a range, the working tree for uncommitted changes
- **AND** `Owner.name` is asked first when the owner is known
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

#### Scenario: Only the declarations the name can mean, ranked
- **WHEN** the index answers declarations of the name
- **THEN** only those of the language family and of a kind the name can mean are taken (a call: a function, a constructor or a type; a type: a type)
- **AND** they are ranked by the imports of the file, the owner, then the top directory

#### Scenario: A declaration the index finds in a file of the review
- **WHEN** the index answers a declaration that lies in a file of the review
- **THEN** it is shown in the review, not in a view of its file
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

### Requirement: The files outside the review are read in two git processes

One `cat-file --batch-check` MUST name the blobs of the other files and one `cat-file --batch` MUST read them.

#### Scenario: A declaration outside the review
- **WHEN** the index answers a declaration in a file the review lacks
- **THEN** one `cat-file --batch-check` names its blob at the revision and one `cat-file --batch` reads its text
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

### Requirement: The index is never built or waited for

Nothing MUST run when accio is missing, and without a usable index of the repository accio MUST answer at once
(`--no-build`): never an index build, a wait for one or a refresh.

#### Scenario: Accio is missing
- **WHEN** accio is not installed
- **THEN** nothing of accio runs

#### Scenario: Accio has no usable index of the repository
- **WHEN** accio cannot use an index of the repository
- **THEN** it answers at once (`--no-build`) that it has no index, with its reason
- **AND** no index is built, waited for or refreshed
  [@test] crates/file-diffs/src/accio_test.rs (`an_index_accio_cannot_use_answers_no_index_with_its_reason`)

### Requirement: The names of one Go to Declaration are asked in one accio process

The names of one Go to Declaration (`Owner.name`, then the bare name) MUST be asked in one accio process, which answers
an entry per name in the order asked; the first that declares the name is taken.

#### Scenario: The owner's name and the bare name in one process
- **WHEN** Go to Declaration asks the index for `Owner.name` and the bare name
- **THEN** one accio process answers an entry per name, in the order asked
- **AND** the first that declares the name is taken
  [@test] crates/file-diffs/src/accio_test.rs (`every_name_of_a_lookup_is_asked_in_one_process`)
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

### Requirement: Both sides of the review boundary judge a candidate by nine words

Both sides of the review boundary MUST judge a candidate by one list of nine words (`class`, `function`, `property`,
`constant`, `enum_entry`, `type_alias`, `module`, `macro`, `local`): the review folds the kind of its own summary to
them, and the index answers them beside its own kind (accio's `group`). A declaration of the review is named by that
word.

#### Scenario: A method of a Rust impl wherever it was found
- **WHEN** a method of a Rust `impl` is a candidate, in the review or in the index
- **THEN** it is a candidate for a call and none for a type position
  [@test] crates/file-diffs/src/declarations_test.rs (`one_vocabulary_filters_the_hits_of_the_index_as_it_filters_the_review`)

#### Scenario: A declaration of the review is named by its word
- **WHEN** the review declares a type alias and an enum entry
- **THEN** they are named `type_alias` and `enum_entry`, as the index names them
  [@test] crates/file-diffs/src/declarations_test.rs (`a_type_alias_and_an_enum_entry_are_named_alike_on_both_sides`)

### Requirement: An unrecognized accio declaration kind discards no name

An unrecognized accio declaration kind MUST NOT by itself discard a name that otherwise fits the language and query; a
hit without the shared word (an accio older than 0.16) is kept as a candidate, since only the index folds its kinds to
the nine words and this side keeps no copy of that fold.

#### Scenario: An unknown kind is kept
- **WHEN** accio answers a declaration of a kind this side does not know
- **THEN** the kind reads as unknown and the hit stays a candidate
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)
  [@test] crates/file-diffs/src/accio_test.rs (`unknown_accio_kinds_deserialize_to_unknown`)

#### Scenario: A hit without the shared word
- **WHEN** a hit comes without the shared word (an accio older than 0.16)
- **THEN** it is kept as a candidate wherever it is
  [@test] crates/file-diffs/src/declarations_test.rs (`one_vocabulary_filters_the_hits_of_the_index_as_it_filters_the_review`)
  [@test] crates/file-diffs/src/accio_test.rs (`a_hit_carries_the_word_a_review_shares_when_accio_answers_one`)

### Requirement: An unrecognized access kind is shown as read

An unrecognized access kind of accio MUST be shown as `read`, never as a raw string.

#### Scenario: An unknown access kind
- **WHEN** accio answers a usage with an access kind this side does not know
- **THEN** it is shown as `read`
  [@test] crates/file-diffs/src/usages_test.rs (`an_unknown_accio_access_kind_is_shown_as_read`)
  [@test] crates/file-diffs/src/accio_test.rs (`unknown_accio_kinds_deserialize_to_unknown`)

### Requirement: A declaration outside the review opens its whole file over it

A declaration outside the review MUST open its whole file over the review: its path, "at <commit>, outside the review"
("the working tree, outside the review" for uncommitted changes), coloured as the diffs, its name selected, and Go to
Declaration in it as in the review (a declaration of the review closes the file and shows it there).

#### Scenario: The file of a declaration the index knows
- **WHEN** Go to Declaration goes to a declaration outside the review
- **THEN** its whole file opens over the review with its path and "at <commit>, outside the review", coloured as the diffs, its name selected
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open a declaration outside the review that the index of the repository knows, go back to the review, and from that file to a declaration of the review`)

#### Scenario: Uncommitted changes
- **WHEN** the side is the working tree of uncommitted changes
- **THEN** the file says "the working tree, outside the review"

#### Scenario: From that file to a declaration of the review
- **WHEN** Go to Declaration in that file goes to a declaration of the review
- **THEN** the file closes and the declaration is shown in the review
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open a declaration outside the review that the index of the repository knows, go back to the review, and from that file to a declaration of the review`)

### Requirement: Back closes the file outside the review

Back (the key or the button "Back" of the file) MUST close the file and return to the name the jump started from; a
file of the tree, J and K, or N and P MUST show in the review and close the files over it.

#### Scenario: Back returns to the review
- **GIVEN** a file outside the review is open over it
- **WHEN** the user presses Back or the button "Back" of the file
- **THEN** the file closes and the review shows the name the jump started from
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open a declaration outside the review that the index of the repository knows, go back to the review, and from that file to a declaration of the review`)

#### Scenario: The tree and the keys of the review close the files
- **GIVEN** a file outside the review is open over it
- **WHEN** the user picks a file of the tree, or presses J and K, or N and P
- **THEN** the review shows it and the files over it close

### Requirement: ⌥F7 and the menu of the code list the usages at the name

⌥F7 (Alt+F7 off macOS) on the selection and the item "Find Usages" of the menu of the code MUST list the usages of the
name at the name: "N usages of X" (the declarations not counted), the usages in the review, then those outside it, each
with how it uses the name (declaration, import, call, write, read), its line with the name in bold, and its file and
line.

#### Scenario: The list of usages in the review and outside it
- **WHEN** the user presses ⌥F7 (Alt+F7 off macOS) on the selection, or chooses "Find Usages" in the menu of the code on a name
- **THEN** a list at the name says "N usages of X", the declarations not counted
- **AND** it lists the usages in the review, then those outside it, each with how it uses the name (declaration, import, call, write, read), its line with the name in bold, and its file and line
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should list the usages of a name in the review and outside it, from ⌥F7 and from the menu of the code, and show the one chosen`)

#### Scenario: An argument and an unrecognized access kind read
- **WHEN** a usage is an `argument`, or of an unrecognized access kind
- **THEN** the list shows it as `read`
  [@test] crates/file-diffs/src/usages_test.rs (`an_unknown_accio_access_kind_is_shown_as_read`)

### Requirement: A usage of the list is shown in the review or in the view of its file

↑ and ↓ and Enter or a click MUST show a usage of the list, in the review or in the view of its file, and Back MUST
return to the name; Escape MUST give the focus back to the view.

#### Scenario: The usage chosen is shown and Back returns
- **GIVEN** the list of usages is open
- **WHEN** the user picks one with ↑ and ↓ and Enter, or a click
- **THEN** it is shown in the review or in the view of its file
- **AND** Back returns to the name
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should list the usages of a name in the review and outside it, from ⌥F7 and from the menu of the code, and show the one chosen`)

#### Scenario: Escape gives the focus back
- **GIVEN** the list of usages is open
- **WHEN** the user presses Escape
- **THEN** the focus goes back to the view

### Requirement: Find Usages reports omitted candidates, no usage and no name

A name-only candidate omitted by a location-based query MUST be reported by count; a name with no usage and no omitted
candidates MUST say so in a toast ("No usages of version found"); the item MUST be disabled, with the reason, where no
name is.

#### Scenario: Omitted name-only candidates are counted
- **WHEN** a location-based query omits name-only candidates
- **THEN** the list reports their count

#### Scenario: A name with no usage but its declaration
- **WHEN** Find Usages finds no usage of `version` and omits no candidate
- **THEN** a toast says "No usages of version found"
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should say when a name has no usage but its declaration`)

#### Scenario: The item is disabled where no name is
- **WHEN** the menu of the code opens where no name is
- **THEN** "Find Usages" is disabled, with the reason
  [@test] src/lib/diff/code-menu.test.ts (`offers both items for a name, and disables them with the reason where no name is`)

### Requirement: The usages of a local are the names its scopes bind to it

The usages of a local (a parameter, a local of a function) MUST be the names its scopes bind to its declaration, in its
file. In Kotlin a local is any declaration inside code, also outside a function (an `init` block, a `catch` or a `when`
of a property's initializer), and the parameter of a function type (`(event: Event) -> Unit`) is one that no scope
sees.

#### Scenario: The usages of a parameter apart from the property it shadows
- **WHEN** Find Usages runs on a parameter or a local of a function
- **THEN** its usages are the names its scopes bind to its declaration, in its file only
  [@test] crates/code-symbols/src/navigate_test.rs (`finds_the_usages_of_a_kotlin_parameter_apart_from_the_property_it_shadows`)
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_usages_of_a_local_are_its_own_and_of_a_member_all_unbound`)
  [@test] crates/file-diffs/src/usages_test.rs (`finds_a_word_in_the_files_of_the_review_without_git_and_a_local_in_its_file_only`)

#### Scenario: A Kotlin local outside a function
- **WHEN** Find Usages runs on a declaration inside code outside a function (an `init` block, a `catch` or a `when` of a property's initializer)
- **THEN** it is a local
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_local_outside_a_function_is_a_local`)

#### Scenario: A local of a Java initializer
- **WHEN** Find Usages runs on a local declared in an initializer block of a Java class (`{ … }`, `static { … }`)
- **THEN** it is a local
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_local_of_an_initializer_is_a_local`)

#### Scenario: A parameter of a Kotlin function type
- **WHEN** Find Usages runs on the parameter of a function type (`(event: Event) -> Unit`)
- **THEN** it is a local that no scope sees
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_parameter_of_a_function_type_declares_nothing_the_code_sees`)

### Requirement: The usages of any other name are the names no local binds

The usages of any other name MUST be the names of that text that no local binds, in its file and in the same side of
the other files of the review in a language that sees it, from the texts the diffs read: no git process.

#### Scenario: The usages in another file without the locals that shadow the name
- **WHEN** Find Usages runs on a name that is no local
- **THEN** its usages are the names of that text that no local binds, in its file and in the same side of the other files of the review in a language that sees it
- **AND** no git process runs
  [@test] crates/code-symbols/src/navigate_test.rs (`finds_the_usages_of_a_rust_binding_and_of_an_imported_function`, `finds_the_usages_of_a_name_in_another_file_without_the_locals_that_shadow_it`)
  [@test] crates/code-symbols/src/csharp/navigate_test.rs (`the_usages_of_a_local_are_its_own_and_of_a_member_all_unbound`, `the_usages_in_another_file_are_the_names_no_local_binds`)
  [@test] crates/file-diffs/src/usages_test.rs (`finds_a_word_in_the_files_of_the_review_without_git_and_a_local_in_its_file_only`)

#### Scenario: The places of the usages of a Kotlin name
- **WHEN** Find Usages runs on a Kotlin name: a package of one segment, an assigned member, a call with type arguments, or a name in a CRLF text
- **THEN** each usage is at its place, with how it uses the name
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`the_name_of_a_package_of_one_segment_is_one_usage`, `the_qualifier_of_an_assigned_member_is_its_whole_receiver`, `a_call_with_type_arguments_is_a_call_and_an_increment_a_write`, `the_usages_of_a_name_are_at_their_places_in_one_pass_over_a_crlf_text`)

### Requirement: A write is an assignment, an increment, a C# out or ref argument or a destructuring target

A usage MUST be a write where its name ends the left side of an assignment, in every language but Bazel (where every
`=` declares its name), or is the operand of `++` or `--` (Kotlin, Java, C#, TypeScript and JavaScript; Rust has
none). In C# an `out` or `ref` argument MUST be a write, as an assignment and an increment are; only C# has one, and an
argument of any other language reads (a Rust `&mut x` too). In TypeScript and JavaScript the targets of a destructuring
assignment MUST be writes too.

#### Scenario: An assignment and an increment write
- **WHEN** a name ends the left side of an assignment or is the operand of `++` or `--`
- **THEN** its usage is a write
  [@test] crates/code-symbols/src/navigate_test.rs (`tells_a_java_write_an_increment_and_an_import`)
  [@test] crates/code-symbols/src/java/navigate_test.rs (`a_write_is_the_left_side_of_an_assignment_and_the_operand_of_an_increment`)
  [@test] crates/code-symbols/src/kotlin/navigate_test.rs (`a_call_with_type_arguments_is_a_call_and_an_increment_a_write`)

#### Scenario: A Bazel assignment declares
- **WHEN** a name of a Bazel file is assigned by `=`
- **THEN** its usage there is its declaration, no write
  [@test] crates/code-symbols/src/starlark/navigate_test.rs (`the_usages_of_a_parameter_stay_inside_its_def`)

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
  [@test] crates/code-symbols/src/js/navigate_test.rs (`an_assignment_a_destructuring_and_an_increment_write`)

### Requirement: The usages outside the review come from the index at the side

The usages outside the review MUST come from the index of accio at the commit of the side (100 at most), only in the
files the review lacks and in a language that sees the name; one `cat-file --batch-check` and one `cat-file --batch`
read their files, for their lines and the view of each. Nothing MUST run when accio is missing or has no index of the
repository.

#### Scenario: The usages the index knows at the revision of the side
- **WHEN** Find Usages runs and accio has an index of the repository
- **THEN** the usages outside the review come from it at the commit of the side, 100 at most, only in the files the review lacks and in a language that sees the name
- **AND** one `cat-file --batch-check` and one `cat-file --batch` read their files, for their lines and the view of each
  [@test] crates/file-diffs/src/usages_test.rs (`adds_the_usages_outside_the_review_that_the_index_knows_at_the_revision_of_the_side`)
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should list the usages of a name in the review and outside it, from ⌥F7 and from the menu of the code, and show the one chosen`)

#### Scenario: A cut answer
- **WHEN** the index has more usages than it answers
- **THEN** the list says "The index has more usages outside the review"

#### Scenario: No accio or no index
- **WHEN** accio is missing or has no index of the repository
- **THEN** nothing of it runs

### Requirement: With the declaration in the review, usages are asked at its place

When the review has the declaration, usages MUST be queried at its path, line and column with `--resolved`: only type-
or import-linked hits are listed, and the number of name-only candidates omitted is shown. Without a declaration in the
review, usages remain queried by name, with typed and import hits ahead of name-only hits.

#### Scenario: A name-only namesake is not listed
- **WHEN** the review has the declaration of the name
- **THEN** accio is asked at its path, line and column with `--resolved`
- **AND** only type- or import-linked hits are listed, and the number of name-only candidates omitted is shown
  [@test] crates/file-diffs/src/usages_test.rs (`does_not_list_a_name_only_namesake_when_the_review_has_the_declaration`)
  [@test] crates/file-diffs/src/accio_test.rs (`resolved_usages_are_asked_at_a_name_column_and_count_name_only_hits`)

#### Scenario: Without a declaration in the review
- **WHEN** the review has no declaration of the name
- **THEN** usages are queried by name, typed and import hits ahead of name-only hits

### Requirement: Columns are converted to the units of accio, never found as a word

Every column sent to accio or read from its hits MUST be converted between the review's UTF-16 units and the units of
that accio (`Columns`: bytes before 0.21, characters for `--at` and UTF-16 for a hit from 0.21, by its `--version`),
never looked for as a word on the line.

#### Scenario: The units follow the version of accio
- **WHEN** a column is sent to accio or read from its hits
- **THEN** it is converted by the units of its `--version`: bytes before 0.21, characters for `--at` and UTF-16 for a hit from 0.21
  [@test] crates/file-diffs/src/usages_test.rs (`the_column_units_follow_the_version_of_accio`, `the_accio_column_counts_bytes_after_a_utf16_surrogate_pair`)

#### Scenario: A usage outside is at the column the index answers
- **WHEN** a line writes the name in a string before its use
- **THEN** the usage is at the column the index answers in its units, not at a word found on the line
  [@test] crates/file-diffs/src/usages_test.rs (`an_outside_usage_is_at_the_column_the_index_answers_in_its_units`)

### Requirement: An index that fails takes nothing from the review

An index that fails MUST take nothing from what the review knows: the usages of the review stay, with "accio did not
answer: <why>" under them, and a name without a declaration in the review says why in its toast.

#### Scenario: Find Usages when the index fails
- **WHEN** the index fails during Find Usages
- **THEN** the usages of the review stay, with "accio did not answer: <why>" under them
  [@test] crates/file-diffs/src/usages_test.rs (`an_index_that_fails_takes_nothing_from_the_review_and_says_why`)
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should keep what the review knows and say why when the index of the repository fails`)

#### Scenario: Go to Declaration when the index fails
- **WHEN** the index fails for a name without a declaration in the review
- **THEN** its toast says why
  [@test] crates/file-diffs/src/usages_test.rs (`an_index_that_fails_takes_nothing_from_the_review_and_says_why`)
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should keep what the review knows and say why when the index of the repository fails`)

