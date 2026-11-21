---
name: Review Units
description: Requirements for the units of a review (experimental), the changes a reviewer reads once instead of the files; the pair of the review, its blocks cut by the declaration that holds them into a change of a form, of a body, a new or removed declaration, a new file, a group of mechanical changes or of a repeated edit over its files, and a change of no declaration; every block in one unit, the order of the story, and a fingerprint of each change that a rebase or an amend keeps.
style: plain-1
targets:
  - crates/review-plan/src/units.rs
  - crates/review-plan/src/renamed_uses.rs
  - crates/review-plan/src/moved_across.rs
  - crates/review-plan/src/pair.rs
  - crates/review-plan/src/whitespace.rs
  - psi-syntax:crates/diff-marks/src/index.rs
  - psi-syntax:crates/line-diff/src/text_lines.rs
  - crates/trial-check/src/declaration_checks.rs
  - crates/file-diffs/src/service.rs
  - crates/bd-rewrite/src/review_plan_command.rs
  - src/presenters/review-units.ts
  - src/presenters/review-story.ts
  - src/presenters/diff-marks.ts
  - src/components/diff/diff-painter.ts
  - src/lib/diff/rows.ts
  - src/components/diff/DiffView.tsx
  - src/components/commit/CommitReview.tsx
  - src/hooks/useReviewPlan.ts
  - src/pages/ReviewPage.tsx
  - src/components/review/PullRequestReview.tsx
---

# Review Units

Status: Experimental
Date: 2026-10-30

## Purpose

A review plan knows what each hunk of a branch does: which declaration it changes, whether the form of that
declaration changed or its body alone, which hunks only move imports or whitespace, which edit repeats over many
files. A review that lists files throws this away: a file with one real change and one change of imports is read as a
whole, a change of a body of a public function is no line of the plan, and an edit that 30 files repeat is read 30
times.

A unit is one change a reviewer reads once: a change of the form of a declaration, a change of its body, a new or a
removed declaration, a new file, all the changes of one mechanical kind over every file that has them, one repeated
edit over its files, and a change that no declaration holds. The plan cuts the diff the review shows into units
(`ReviewPlan::units`), every block in one, and puts them in the order of the story: a declaration before its uses, a
test right after its subject, the mechanical changes last. Each unit has a fingerprint of its change that stays the
same when a rebase or an amend keeps the change: what a later review knows it has read, and what a note to the agent
is anchored to. The vision of the review, its progress and the rounds of polish:
[Change Review](../../../../docs/design/change-review.md).

## Goals

- Cut every block of the reviewed diff into exactly one unit, and put every changed file in one unit at least: the
  units hide nothing and show nothing twice.
- Tell a change of a form from a change of a body, for every declaration, public or not.
- Read one repeated edit and one kind of mechanical change once, over all of its files.
- Name a change by its content, not by its place: the same change on another base or after an amend has the same
  fingerprint, and the whitespace policy of a window changes neither the identity nor the fingerprint.
- Cost no git process of its own: the units read the blocks, the summaries and the hunk text the plan reads already.

## Non-goals

- A declaration moved whole to another file as one unit of the plan: its two places stay a unit `removed` and a unit
  `added` (or a file taken whole), each saying the other
  ([A declaration moved whole](#requirement-units-a-declaration-moved-whole-to-another-file-says-where)), and the
  window reads it once, at its new place
  ([Read at its new place](#requirement-in-the-review-window-a-declaration-moved-whole-is-read-at-its-new-place)); a
  move whose body changed and a move of lines that are no declaration say nothing.
- The usages of a change of a body: accio answers the callers of the API alone, until a query of many names costs one
  process (JetBrains/accio#74).
- A change of a form that the summary does not see (`Form`): a Kotlin return type that is inferred changes with the
  body, and the unit is `behaviour`.

## Requirements

### Requirement: Pair: The units cut the pair the review shows

The units MUST cut the pair the review shows: per file the reviewed commits change, the blocks of its blob before the
first commit that changes it against its blob after the last one, under `Default`, as the diffs of the range show them.

#### Scenario: A file two commits change
- **GIVEN** a file that two of the reviewed commits change
- **WHEN** the units are cut
- **THEN** the file is cut by the blocks of its blob before the first commit that changes it against its blob after
  the last one, under `Default`, as the diffs of the range show them
  [@test] crates/review-plan/src/units_test.rs (`the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice`)

### Requirement: Pair: The blocks come from the commit's hunks or one batch of blobs

A file one commit changes MUST take the hunks of that commit; a file several commits change MUST take its first blob
against its last, with one `cat-file --batch` for the blobs of all such files that the memory of the diffs and the
`file_diff` store lack.

#### Scenario: A file one commit changes
- **GIVEN** a file that one of the reviewed commits changes
- **WHEN** the units are cut
- **THEN** the file takes the hunks of that commit

#### Scenario: Files several commits change
- **GIVEN** files that several of the reviewed commits change, some of whose blobs the memory of the diffs and the
  `file_diff` store lack
- **WHEN** the units are cut
- **THEN** each such file takes its first blob against its last
- **AND** the blobs they lack are read by one `cat-file --batch` for all such files

### Requirement: Pair: A file the same before and after the commits has no unit

A file whose blob and path are the same before and after the commits is no file of the pair: it MUST have no unit.

#### Scenario: A file changed and changed back
- **GIVEN** a file whose blob and path are the same before and after the commits
- **WHEN** the units are cut
- **THEN** it is no file of the pair and has no unit

### Requirement: Units: Every block is in exactly one unit

Every block of every file of the pair MUST be in exactly one unit, and every file of the pair in one at least.

#### Scenario: The changes of a file and the files of the pair
- **GIVEN** a file with a change of a form, of a body, a new and a removed declaration and a new import, a file of no
  language and a new file
- **WHEN** the units are cut
- **THEN** every block of every file of the pair is in exactly one unit
- **AND** every file of the pair is in one unit at least
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

### Requirement: Units: A block goes to the first unit it fits

A block MUST go to the repeated edit of its file's group when the block is that edit; else to the imports when its
changed lines are each of an import statement or blank, one at least; else to the whitespace when it changes
whitespace alone, in a file of a grammar its lines equal by tokens; else to the declaration that holds most of its
changed lines; else to the change outside any declaration, one unit per run of such blocks.

#### Scenario: A block that is the repeated edit
- **WHEN** a block is the repeated edit of its file's group
- **THEN** it goes to the repeated edit of that group
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`)

#### Scenario: A block of imports alone
- **WHEN** a block is no repeated edit and its changed lines are each of an import statement or blank, one at least
  (`file_diffs::service::imports_only`, the test the diffs pale a block by)
- **THEN** it goes to the imports
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A block of whitespace alone
- **WHEN** a block is neither of those and changes whitespace alone ([Review Plan](../review-plan/spec.md#requirement-mechanical-changes-whitespace-is-told-by-the-tokens-of-a-grammar)):
  its lines are equal by tokens, as those of a call wrapped inside its brackets
- **THEN** it goes to the whitespace
- **AND** a block that changes the spaces inside a string does not
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`, `a_block_of_whitespace_is_told_by_its_tokens`)

#### Scenario: A block inside declarations
- **WHEN** a block is none of those and declarations hold its changed lines
- **THEN** it goes to the declaration that holds most of its changed lines, the innermost one around each line
- **AND** the lines are read on the new side, on the old side for a block that only removes
  [@test] crates/review-plan/src/units_test.rs (`a_block_belongs_to_the_declaration_of_most_of_its_lines`)

#### Scenario: Blocks outside any declaration
- **WHEN** blocks are none of those and no declaration holds their changed lines
- **THEN** they go to the change outside any declaration, one unit per run of such blocks

### Requirement: Units: The declarations of a file are its spanned declarations

The declarations of a file MUST be those of its summary that are no `Binding`, none the parser may have misread
(`in_error`) and none inside a function or a value (a local): `spanned_declarations`. Their lines and those of the
import statements, which a summary counts as git does, MUST be read as lines of the diffs, which the blocks name: a
lone `\r` ends a line there too.

#### Scenario: A binding, a misread declaration and a local
- **GIVEN** a summary with a `Binding`, a declaration the parser may have misread (`in_error`) and a declaration inside
  a function or a value (a local)
- **WHEN** the declarations of the file are read (`spanned_declarations`)
- **THEN** none of the three is a declaration of the file

#### Scenario: A block below a lone carriage return
- **GIVEN** a file with a lone `\r` above its declarations
- **WHEN** a block changes the code of a declaration below it, and another wraps a call of a declaration
- **THEN** the first goes to its declaration, a change of its behaviour, and the second to the whitespace
  [@test] crates/review-plan/src/units_test.rs (`a_block_below_a_lone_carriage_return_goes_to_the_declaration_of_its_lines`)

### Requirement: Units: A file without a full summary is one unit other

A side whose summary is missing, degraded or truncated MUST make the file one unit `other` (`unparsed`).

#### Scenario: A summary missing, degraded or truncated
- **WHEN** the summary of a side of a file is missing, degraded or truncated
- **THEN** the file is one unit `other` (`unparsed`)

### Requirement: Units: Declarations are paired by key, form, then order

A declaration MUST be paired across the two sides by its key and, among overloads of one key, by an equal form, then
in order.

#### Scenario: Two overloads of one key
- **GIVEN** two overloads of one name, of which only one has a changed body
- **WHEN** the declarations are paired
- **THEN** each overload is paired with the one of an equal form
- **AND** only the one whose body changed is a unit, named by its signature
  [@test] crates/review-plan/src/units_test.rs (`an_overload_is_a_unit_of_its_own`)

### Requirement: Units: A pair is shape, behaviour, moved or whitespace

A pair whose form differs MUST be `shape`, with the parts that changed. One of the same form that moved (every line
of it deleted in blocks of old lines alone and added in blocks of new lines alone, the same tokens) MUST go to the
unit `moved` where the index says its order is free and MUST be `behaviour` elsewhere, never whitespace. Else it is whitespace when its lines are equal by tokens, else `behaviour`. The lines of
a pair are those of the declaration and of every block of its unit.

#### Scenario: A change of a form and a change of a body
- **WHEN** the form of a paired declaration differs, and of another only its body changed
- **THEN** the first is `shape`, with the parts that changed
- **AND** the second is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A pair whose lines are equal by tokens
- **WHEN** the form of a paired declaration is the same and its lines are equal by tokens
- **THEN** it is whitespace, and its blocks go to the unit of whitespace

#### Scenario: A declaration at another place of its file
- **WHEN** a function stands at the end of its file now, between two other functions, or there with a call wrapped on the way, its code the same, and the index says its order is free
- **THEN** its two blocks go to the unit `moved`, whatever functions stand around it at the two places
- **AND** it is `behaviour` where the index says its order matters, and without an index
- **AND** moved with a changed line it is `behaviour`, and so is a function whose statement stands at another place inside it, whatever the index says
- **AND** a blank line added where the function stays is whitespace: no line of it moved
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_at_another_place_with_the_same_code_is_moved_where_the_index_says_its_order_is_free`)

#### Scenario: A space inside a string of a body
- **WHEN** the body of a paired declaration differs only by the spaces inside a string
- **THEN** it is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`a_block_of_whitespace_is_told_by_its_tokens`)

#### Scenario: A block that reaches past its declaration
- **WHEN** a block joins the two lines of a declaration and changes the comment after it, and the lines of the declaration alone are equal by tokens
- **THEN** the declaration is `behaviour`: the lines of its block are not
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_is_whitespace_only_with_every_line_of_its_blocks`)

### Requirement: Units: A pair that moved is folded only where its place has no meaning

The place of a pair that moved has no meaning where the index says so (`accio moves`, `reordered`, `order: "free"`)
and no other code moved with it: a declaration of free order MUST hold the name of the pair on both sides (its own,
or one around it that moved whole), and every line of code its blocks add MUST be a line of such a declaration. Every
other pair that moved MUST be `behaviour`, also before the index answered and where none answers: a fold is never
wrong.

#### Scenario: The declarations whose place has no meaning
- **WHEN** a function, a type or a type alias of Kotlin, Java, C# or Rust moves across declarations, or a hoisted function of TypeScript (a `function` declaration, a method, a signature) or a type alias of it, and the accio of this machine is asked
- **THEN** it goes to the unit `moved`: a function of Kotlin, Java or C# across a property or a field and a function, a function of Rust across a type, a constant and a function, a hoisted function of TypeScript across a constant, a class of Kotlin moved whole
- **AND** a value or a constant (its initializer runs in its turn), an enum entry, a macro, a module, a function a `const` of TypeScript holds, a class or an enum of TypeScript, and every declaration of Starlark are `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`a_moved_declaration_is_folded_only_where_the_index_says_its_place_has_no_meaning`)

#### Scenario: A method that moved with its decorator
- **WHEN** a method of a class of TypeScript or JavaScript moves with its decorator, on a line of its own or on the line of the method, or with a decorator of one of its parameters: the route of a controller with a parameter, above the routes with a fixed path
- **THEN** it is `behaviour`, a unit that is read: its decorator runs in another turn, and the routes are matched in the order of the methods
- **AND** a method without a decorator that moves across decorated ones goes to the unit `moved`: their order among themselves stays
- **AND** a method named by an expression (`[key()]() {}`), which runs in its turn as the class is made, is no declaration of the file: its lines are a change of the class, read
  [@test] crates/review-plan/src/units_test.rs (`a_moved_declaration_is_folded_only_where_the_index_says_its_place_has_no_meaning`)

#### Scenario: What a declaration crossed
- **WHEN** a function moves across code that is no declaration beside it (an initializer block, the end of an `impl` and the start of the next), a function of Rust across a macro, an overload of TypeScript across an overload of its name, or a function moves with the property beside it in one block
- **THEN** it is `behaviour`, a unit that is read
- **AND** so is a function the index says is free that moved with a property beside it in its blocks, which the index did not say is free
  [@test] crates/review-plan/src/units_test.rs (`a_moved_declaration_is_folded_only_where_the_index_says_its_place_has_no_meaning`)
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_at_another_place_with_the_same_code_is_moved_where_the_index_says_its_order_is_free`)

#### Scenario: The plan of git alone, then the plan the index answered
- **WHEN** a plan of a function that moved is read with an index that says its order is free, and of a change where nothing moved
- **THEN** the plan of git alone reads it (`behaviour`) and the plan the index answered folds it, its story told again with the fold; the index is asked of the moves once, beside its other queries, in the query the diffs of the same subject ask (`FileDiffs::moves`, kept by the two trees)
- **AND** a change where nothing moved asks no moves
  [@test] crates/review-plan/src/stages_test.rs (`a_moved_declaration_is_read_in_the_plan_of_git_and_folded_once_the_index_says_its_order_is_free`)

### Requirement: Units: A body changed in a rename the index has not proven says so

A unit `behaviour` whose every changed line is a run `Renamed` of a name that is not local, not proven by the index,
MUST say those renames (`renamed.names`) and what they wait on (`renamed.wait`: `checking` while the index is asked,
`notProven` else), read from the marks that say the file is not proven in the header
([Range Reading](../../review/range-reading/spec.md)), and again with the answer of the index. Any other unit MUST
have no `renamed`.

#### Scenario: The calls of a renamed private function without an index
- **GIVEN** a private function `sumOf` renamed to `totalOf` at its declaration and its three calls, and no index
- **WHEN** the plan is read
- **THEN** the units of the three calls are `behaviour` with `renamed` `sumOf` → `totalOf`, `notProven`
- **AND** a call whose function also changed in another line is `behaviour` without `renamed`, and so is a body whose
  runs are all quiet, of a local name, or of a rename the index says is more than a rename
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_body_changed_in_a_rename_no_index_proved_is_told_by_the_rename`)

#### Scenario: While the index is asked
- **GIVEN** the same rename and an index that does not prove it
- **WHEN** the plan is sent before the index answered, and then whole
- **THEN** each plan sent before says `checking`, and the plan whole `notProven`
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_rename_the_index_is_asked_of_is_checking_until_it_answers`)

#### Scenario: A rename the index proved
- **WHEN** the accio of this machine proves the rename
- **THEN** no unit has `renamed`, and the reading proves the file
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_rename_the_index_proved_leaves_no_word_on_the_body`)

### Requirement: Units: A declaration renamed where the index has not proven it is one unit

A declaration renamed by a rename its file declares, not proven by the index, MUST be one unit with the declaration of
its old name, named by the new one, with `renamed_from` (`old`, `wait`), when both are of one kind and key but the last
name, the old key gone, and the same code but for renamed names and whitespace. Its members pair under the new name.
Any other pair MUST stay as without a rename.

#### Scenario: A private function renamed
- **GIVEN** a private function `sumOf` renamed to `totalOf` at its declaration and its three calls, and no index
- **WHEN** the plan is read
- **THEN** `totalOf` is one unit `behaviour` with `renamed_from` `sumOf`, `notProven`, and no unit is `sumOf`
- **AND** the plan sent while the index is asked says `checking`, the same units
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_body_changed_in_a_rename_no_index_proved_is_told_by_the_rename`, `a_rename_the_index_is_asked_of_is_checking_until_it_answers`)

#### Scenario: A body that changed with the name
- **WHEN** the function renamed rounds otherwise too, or a class renamed has a member that changed
- **THEN** the new one is `added` and the old one `removed`
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_declaration_renamed_with_more_than_its_name_stays_removed_and_added`, `a_class_renamed_is_one_unit_with_its_members`)

#### Scenario: A class renamed
- **WHEN** a class is renamed with no other change, and another file uses it
- **THEN** the class is one unit with `renamed_from`, no member is a unit, and the use is told by its rename
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_class_renamed_is_one_unit_with_its_members`)

#### Scenario: A rename the index proved
- **WHEN** the accio of this machine proves the rename
- **THEN** `totalOf` is `added` and `sumOf` `removed`, as without the pairing, and the story is told again where the plan sent before paired them
  [@test] crates/review-plan/src/renamed_uses_test.rs (`a_rename_the_index_proved_leaves_no_word_on_the_body`)

### Requirement: Units: A declaration of one side alone is added or removed

A declaration of one side alone whose lines a block changes MUST be `added` or `removed`, also when another unit holds
that block: then it has no block. A declaration renamed where the index has not proven it is no declaration of one side
alone ([A declaration renamed](#requirement-units-a-declaration-renamed-where-the-index-has-not-proven-it-is-one-unit)).

#### Scenario: A new and a removed declaration
- **WHEN** a block changes the lines of a declaration of the new side alone, and of one of the old side alone
- **THEN** the first is `added` and the second `removed`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: Another unit holds the block
- **WHEN** another unit holds the block that changes the lines of a declaration of one side alone
- **THEN** the declaration is still `added` or `removed`, and it has no block

### Requirement: Units: A declaration moved whole to another file says where

A unit `removed` MUST say the file, the line and the unit of the place it went to (`moved_to`), and a unit `added`
those of the place it came from (`moved_from`), where exactly one declaration of one side alone before the commits and
one after them, in two files, have the same code, the same content of their tokens; the other place is a unit `added`
or `removed`, or an outermost declaration of a new or a deleted file, whose unit says nothing. Every other unit MUST
have neither.

#### Scenario: A function moved to a file that stays, into a new file and out of a deleted file
- **GIVEN** `formatTotal` and `parseTotal` in `Orders.kt`, and `legacyTotal` alone in `Legacy.kt`
- **WHEN** the commits move `formatTotal` to `Report.kt`, `parseTotal` into the new `Parsing.kt` and `legacyTotal`
  into `Orders.kt`, deleting `Legacy.kt`, each with the same code
- **THEN** the removed `formatTotal` says `moved_to` `Report.kt`, the line of its name there and the added unit there,
  the added one `moved_from` `Orders.kt`, the line of its name before the commits and the removed unit there
- **AND** the removed `parseTotal` says `moved_to` `Parsing.kt` with its unit `newFile`, and the added `legacyTotal`
  `moved_from` `Legacy.kt` with its unit `deletedFile`
- **AND** the units `newFile` of `Parsing.kt` and `deletedFile` of `Legacy.kt` say neither, and the units are those
  without a move
  [@test] crates/review-plan/src/moved_across_test.rs (`a_declaration_moved_whole_to_another_file_says_where_it_went_and_came_from`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)

#### Scenario: A function moved into a class of another file, indented again
- **WHEN** a top-level function moves into a class of another file that was there, indented again
- **THEN** the removed one says `moved_to` and the added one `moved_from`: the content of their tokens is the same
  [@test] crates/review-plan/src/moved_across_test.rs (`a_function_moved_into_a_class_of_another_file_indented_again_is_a_move`)

#### Scenario: What is no move
- **WHEN** a function moves to another file with a changed line, one is added to two files and removed from one, one
  is removed from two files and added to one, or one moves to another class of its own file
- **THEN** no unit says `moved_from` or `moved_to`: a word on two files must not guess, and the diff of one file shows
  a move inside it
  [@test] crates/review-plan/src/moved_across_test.rs (`a_move_with_a_changed_line_or_of_two_places_on_a_side_says_nothing`)

#### Scenario: The places a move pairs
- **WHEN** the two places of one code are a unit `removed` and a unit `added` of two files that stay, a unit `removed`
  and an outermost declaration of a new file, or an outermost declaration of a deleted file and a unit `added`
- **THEN** the unit of the declaration says the other place, and a file taken whole (`newFile`, `deletedFile`, in
  `deletedFiles` too) says nothing
- **AND** a new and a deleted file alone, a member of a new or a removed type (part of its unit), a declaration of a
  file a kind takes whole (`mechanical`, `other`) and a pair of one declaration (`shape`, `behaviour`, `moved`) are no
  place of a move

### Requirement: Units: The place a move left is nothing to read where it holds the move alone

The slice of the place that declarations moved whole to other files left MUST be `moved_out` when it holds nothing but
them: the blocks of a unit `removed` change the old side as removing those declarations would, blank lines aside; a
deleted file holds them, its imports, its package line and its comments alone. The other places show what it removes.
Every other slice MUST NOT be `moved_out`: its lines are read where they stand.

#### Scenario: The places the moves left, each holding them alone
- **GIVEN** `formatTotal`, `count`, `parseTotal` and `parseSum` in `Orders.kt`, and `Legacy.kt` of a comment, its
  package, an import and `legacyTotal`
- **WHEN** the commits move `formatTotal` to `Report.kt`, `parseTotal` and `parseSum`, one block, into the new
  `Parsing.kt`, and `legacyTotal` to `Report.kt`, deleting `Legacy.kt`, each with the same code
- **THEN** the slices of `formatTotal`, of `parseTotal`, whose unit holds the block of both, and of `Legacy.kt` are
  `moved_out`
  [@test] crates/review-plan/src/moved_across_test.rs (`the_place_a_move_left_is_nothing_to_read_where_it_holds_the_move_alone`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)

#### Scenario: A place that holds more than the move
- **WHEN** a block removes a constant that moved nowhere beside a function that moved, a deleted file holds a function
  that moved nowhere beside one that moved, or a block adds `legacyTotal` in the place of two functions that moved
- **THEN** no slice is `moved_out`, and the moves say where they went and came from all the same
  [@test] crates/review-plan/src/moved_across_test.rs (`the_place_a_move_left_is_nothing_to_read_where_it_holds_the_move_alone`, `a_declaration_moved_whole_to_another_file_says_where_it_went_and_came_from`)

#### Scenario: A function between two others without a blank line
- **WHEN** a function moves from between two others it touches, its closing brace the line before the next, or with the
  annotation the next one has too
- **THEN** its slice is `moved_out`, however the comparison takes the lines it shares with its neighbours: the old side
  without its blocks reads as the old side without the function
  [@test] crates/review-plan/src/moved_across_test.rs (`a_function_moved_from_between_two_others_without_a_blank_line_is_the_move_alone`)

### Requirement: Units: A declaration inside a new or removed one is part of its unit

A declaration of one side alone inside another of that side alone (a member of a new type, a method of a removed
class) MUST be part of the unit of the outermost one, and so MUST its blocks.

#### Scenario: A member of a new type
- **WHEN** the commits add a type with a member
- **THEN** the member and its blocks are part of the unit of the type, not a unit of its own
  [@test] crates/review-plan/src/units_test.rs (`a_member_of_a_new_type_is_part_of_the_type`)

### Requirement: Units: A mechanical file is one whole slice of the unit of its kind

A file whose change is mechanical (`files`: a build file, imports only, whitespace only, a pure rename, a generated
file) MUST be one slice of the unit of its kind, the file whole; a generated file the commits add or delete too.

#### Scenario: A mechanical file
- **WHEN** the change of a file is mechanical (`files`)
- **THEN** the file is one slice of the unit of its kind, the file whole
  [@test] crates/review-plan/src/units_test.rs (`the_generated_files_are_one_unit_after_the_other_mechanical_ones`)

#### Scenario: A generated file the commits add or delete
- **WHEN** the commits add or delete a generated file
- **THEN** it is one slice of the unit of the generated files, the file whole

### Requirement: Units: A new file and a deleted file are units of their own

A file the commits add MUST be one unit `newFile`, a file they delete one unit `deletedFile` in its place in the story.

#### Scenario: A new file
- **WHEN** the commits add a file
- **THEN** it is one unit `newFile`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A deleted file
- **WHEN** the commits delete a file
- **THEN** it is one unit `deletedFile` in its place in the story
  [@test] crates/review-plan/src/units_test.rs (`a_deleted_file_is_a_unit_and_the_deleted_files_of_a_directory_are_one`)
  [@test] src/presenters/review-units.test.ts (`shows a deleted file whole in its place, and the files deleted from one directory as a group, each collapsed`)

### Requirement: Units: The files deleted from one directory are one unit

3 or more files deleted from one directory MUST be one unit `deletedFiles` with the directory, a section per file,
each collapsed ("Deleted"), under "Deleted from old/a/ (3)".

#### Scenario: Three files deleted from one directory
- **WHEN** the commits delete 3 or more files from one directory, `old/a/`
- **THEN** they are one unit `deletedFiles` with the directory
- **AND** it shows a section per file, each collapsed ("Deleted"), under "Deleted from old/a/ (3)"
  [@test] crates/review-plan/src/units_test.rs (`a_deleted_file_is_a_unit_and_the_deleted_files_of_a_directory_are_one`)
  [@test] src/presenters/review-units.test.ts (`shows a deleted file whole in its place, and the files deleted from one directory as a group, each collapsed`)

### Requirement: Units: A file the review cannot cut is one unit other with the reason

A binary file, a file over 8 MiB, a change of its type, a change of the line ends alone where the indentation has a
meaning, and a file of no language the review parses MUST each be one unit `other` with the reason.

#### Scenario: A file of no language
- **WHEN** the commits change a file of no language the review parses
- **THEN** it is one unit `other` with the reason
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A binary file, a large file, a change of type or of the line ends
- **WHEN** the file is binary, over 8 MiB, changes its type, or changes only its line ends where the indentation has
  a meaning
- **THEN** it is one unit `other` with the reason

### Requirement: Units: A mechanical kind and a repeated edit are one unit each

One kind of mechanical change MUST be one unit over every file that has it, and one repeated edit one unit over its
files.

#### Scenario: A repeated edit and the whitespace over their files
- **GIVEN** the same edit of three files, and a file that only reindents one line
- **WHEN** the units are cut
- **THEN** the repeated edit is one unit over its three files
- **AND** the whitespace is one unit over the files that have it
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`)

#### Scenario: The generated files
- **WHEN** the commits change several generated files
- **THEN** they are one unit over all of them
  [@test] crates/review-plan/src/units_test.rs (`the_generated_files_are_one_unit_after_the_other_mechanical_ones`)

### Requirement: Order: The units follow the order of the files of the plan

The units MUST come in the order of the files of the plan (`order`: a declaration before its uses, a test right after
its subject), each unit at the first block it holds, a unit without a block right after the unit of the block that
changed its lines, the units without a block of one block in the order of their lines; a repeated edit at the place
of its first file.

#### Scenario: A declaration before its use
- **GIVEN** a file that declares a function and a file that uses it, both changed
- **WHEN** the units are put in order
- **THEN** the unit of the declaration comes before the unit of its use, each at the first block it holds
  [@test] crates/review-plan/src/units_test.rs (`the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice`)

#### Scenario: A unit without a block
- **WHEN** a unit has no block
- **THEN** it comes right after the unit of the block that changed its lines

#### Scenario: The units without a block of one block
- **WHEN** one block changes the lines of several declarations that hold none of it
- **THEN** they follow the unit of the block in the order the source declares them, not by their names, so the
  heading of the block names them so
  [@test] crates/review-plan/src/units_test.rs (`the_declarations_of_one_block_follow_in_the_order_of_their_lines`)

#### Scenario: A repeated edit
- **WHEN** a repeated edit spans several files
- **THEN** it comes at the place of its first file

### Requirement: Order: The mechanical units come last, by kind

The mechanical units MUST come last, by kind: build files, lists of modules, imports, whitespace, moved declarations,
renames, the generated files last of all (`mechanical:generated`), which a reviewer skips whole.

#### Scenario: The generated files after the other mechanical units
- **WHEN** the commits change generated files beside other files
- **THEN** the mechanical units come last, in the order build files, lists of modules, imports, whitespace, moved declarations, renames
- **AND** the generated files are last of all (`mechanical:generated`), which a reviewer skips whole
  [@test] crates/review-plan/src/units_test.rs (`the_generated_files_are_one_unit_after_the_other_mechanical_ones`)

### Requirement: API and Tests: A unit is api by the API of the plan

A unit of a declaration MUST be `api` exactly when it has its entry in the API of the plan, or would have it by a
change of its form, by the surface of its declaration on each side
([Review Plan](../review-plan/spec.md#api-surface)).

#### Scenario: An addition
- **WHEN** a unit is an addition
- **THEN** it is `api` when it is API after the commits
- **AND** an addition of unstable API is not
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)

#### Scenario: A removal, a change of a form or of a body
- **WHEN** a unit is a removal, a change of its form or of its body
- **THEN** it is `api` when it is API or unstable API on a side
- **AND** a private declaration is not
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)
  [@test] crates/review-plan/src/api_test.rs (`a_change_of_form_to_or_from_api_is_listed`)

#### Scenario: A promotion and the declarations that are no API
- **WHEN** a unit is a promotion, or of a test, a fixture of its class or module, an entry point or an override
- **THEN** a promotion is `api`
- **AND** a test, a fixture of its class or module, an entry point and an override are not
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)

### Requirement: API and Tests: A unit of no declaration is neither api nor test

A unit of no declaration MUST be neither `api` nor `test`.

#### Scenario: A unit of no declaration
- **WHEN** a unit is of no declaration
- **THEN** it is neither `api` nor `test`

### Requirement: API and Tests: A unit of test code is test

A unit of a declaration MUST be `test` when it is test code (a test, a type that declares one, a member of such a type
or module), after the commits, before them for a removal.

#### Scenario: A test and its fixture
- **WHEN** the commits change a test and a fixture of its class
- **THEN** both units are `test`
  [@test] crates/review-plan/src/units_test.rs (`a_unit_is_api_by_the_rule_of_the_api_of_the_plan`)

#### Scenario: A removed test
- **WHEN** the commits remove a test
- **THEN** its unit is `test` by what it is before the commits

### Requirement: Identity: The id names a unit within a plan

`id` MUST name a unit within a plan: of a declaration its file, key and signature.

#### Scenario: The id of a declaration
- **WHEN** a unit is of a declaration
- **THEN** its `id` is its file, key and signature
- **AND** the same change of that declaration on another base has the same `id`
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_names_the_change_wherever_it_sits`)

### Requirement: Identity: The fingerprint names the change across plans

`fingerprint` MUST name the change across plans: of a declaration a digest of its key, its signature and the content
of its tokens on both sides ([Equal by Tokens](../../commit/equal-by-tokens/spec.md)); of the blocks of a slice
their file and the content of their tokens; of a file taken whole its path and blobs; of a group the set of the
fingerprints of its slices. A side without tokens MUST be read by its text without whitespace.

#### Scenario: The same change on another base, and another change
- **GIVEN** a change of the body of a declaration
- **WHEN** the same change sits on another base, below a line added before the declaration
- **THEN** its unit has the same `fingerprint`
- **AND** a different change of that body has another one
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_names_the_change_wherever_it_sits`)

#### Scenario: A slice, a file taken whole and a group
- **WHEN** a unit is of the blocks of a slice, of a file taken whole, or a group
- **THEN** the fingerprint of the blocks of a slice is of their file and the content of their tokens
- **AND** that of a file taken whole of its path and blobs
- **AND** that of a group of the set of the fingerprints of its slices

#### Scenario: The same change with other whitespace
- **WHEN** the same change of a body is written with other whitespace: wrapped, indented again
- **THEN** its unit has the same `fingerprint`
- **AND** so has the same change of a function that is the last of its file: the content is the texts of the tokens and the tree between them
- **AND** a change that differs by the spaces inside a string, or by a line break that ends a statement, has another one
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_is_the_same_after_a_change_of_whitespace_alone`)

### Requirement: Identity: A digest is the first 16 bytes of a SHA-256

A digest MUST be the first 16 bytes of a SHA-256, so that it is the same on every machine and in every version.

#### Scenario: The digest of a fingerprint
- **WHEN** a fingerprint is computed
- **THEN** its digest is the first 16 bytes of a SHA-256, the same on every machine and in every version

### Requirement: Identity: The whitespace policy changes neither the id nor the fingerprint

The kind of a unit SHALL be allowed to change with the whitespace policy of a window; its `id` and its `fingerprint`
MUST NOT change with it.

#### Scenario: Another whitespace policy of a window
- **WHEN** a window reads another whitespace policy
- **THEN** the kind of a unit may change
- **AND** its `id` and its `fingerprint` stay the same

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 40`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices`, its `fingerprint` and, of a body changed in a rename the index has not proven, `renamed`,
of a declaration renamed so, `renamed_from`, of a declaration moved whole to another file, `moved_from` or `moved_to`;
of a slice that holds nothing but declarations moved whole to other files, `moved_out`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 40`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
  `line`, its `slices` and its `fingerprint`
- **AND** `kind` is `shape` with `changes`, `behaviour`, `added`, `removed`, `newFile`, `deletedFile`, `deletedFiles`
  with `directory`, `mechanical` with `mechanical`, `repeatedEdit` with `group`, or `other` with `reason`
- **AND** each of its `slices` has `path`, `old_path`, `whole`, the `blocks` by `index` with their `old` and `new` lines
  from 1, both included, `null` for a side with none, and a `fingerprint`
- **AND** a `behaviour` unit whose body changed in renamed names alone that the index has not proven has `renamed`, its
  `names` (`old`, `new`) and its `wait` (`checking`, `notProven`); no other unit has it
- **AND** a declaration renamed where the index has not proven it is one unit with `renamed_from`, its `old` name and its
  `wait`
- **AND** an `added` declaration moved whole from another file has `moved_from`, a `removed` one moved whole to another
  file `moved_to`, each the `path` of the other place, the `line` of its name there, from 1, and the `unit` that shows
  it; no other unit has them
- **AND** a slice that holds nothing but declarations moved whole to other files has `moved_out: true`; no other slice
  has it
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_renamed_of_a_body_changed_in_a_rename_alone_and_of_the_declaration`, `prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)

### Requirement: In the Review Window: The diffs are the story of the units

With the plan, the diffs of All Changes and of Since Your Review MUST be the story of the units, in their order
(`presentNarrative`), by the steps of its story, each step's parts together
([Review Tour](../review-tour/spec.md#in-the-review-window), a step of the concerns as
[Review Concerns](../review-concerns/spec.md#in-the-review-window) lays it out).

#### Scenario: The diffs of a review with the plan
- **GIVEN** a review with the plan
- **WHEN** All Changes or Since Your Review shows its diffs
- **THEN** they are the story of the units, in their order (`presentNarrative`), by the steps of its story, each
  step's parts together
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

### Requirement: In the Review Window: A run of units of one file is one section

In the story, a run of units of one file MUST be one section, each change under its heading (none over a block of a
file of no language the review parses, whose header names it), a unit with no block of its own beside the heading
before it; a group MUST be a section per file of it.

#### Scenario: A run of changes of one file
- **WHEN** several units of one file follow each other in the story
- **THEN** they are one section, each change under its heading ("open — parameters changed", "close — body changed",
  "fresh — new", "Changes outside a declaration")
- **AND** a unit with no block of its own stands beside the heading before it
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`, `words a change by its kind`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

#### Scenario: A block of a file of no language
- **WHEN** a block is of a file of no language the review parses
- **THEN** no heading stands over it, and the header of the file names it

#### Scenario: A group
- **WHEN** a group of the story spans several files
- **THEN** it is a section per file of it
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`)

### Requirement: In the Review Window: A body changed in a rename the index has not proven is worded as the rename

The heading of a unit with `renamed` MUST word the renames and their wait in place of "body changed" ("balance — only
renamed sumOf → totalOf, not proven"), of one with `renamed_from` its old name and the wait after what else of its form
changed ("totalOf — renamed from sumOf, not proven"), and a note of that rename in its row its sites alone; the badge
MUST say "Renamed, not proven" or "Renamed, checking". Any other `behaviour` keeps "body changed".

#### Scenario: The heading of a rename no index proved
- **GIVEN** a review with its plan of a commit that renames a private `sumOf` to `totalOf`, and no index
- **WHEN** the review shows the diffs
- **THEN** the heading of a call that changed in the name alone says "balance — only renamed sumOf → totalOf, not proven"
- **AND** the heading of a call whose line also changed in another way says "half — body changed"
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] src/presenters/review-units.test.ts (`words a body that changed in renamed names alone as the renames, not proven or checking, as Rust decided`)

#### Scenario: The heading of a declaration renamed
- **GIVEN** the same review
- **WHEN** it shows the declaration of `totalOf`
- **THEN** its heading says "totalOf — renamed from sumOf, not proven", and no heading names `sumOf`
- **AND** "… checking" while the index is asked, and "totalOf — parameters changed, renamed from sumOf, not proven" where its parameters changed too
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] src/presenters/review-units.test.ts (`words a declaration renamed where the index has not proven it as renamed from its old name, after what else of its form changed`)

#### Scenario: The row of a declaration renamed says the rename once
- **GIVEN** the same review, the block of the declaration of `totalOf` starting with the note of its rename
- **WHEN** the review shows it
- **THEN** its row says "totalOf — renamed from sumOf, not proven · 3 sites", the note its sites alone, and a screen reader hears the note in full
- **AND** in the pull request of the helper "totalOf — renamed from sumOf, not proven · 4 sites"; a note of another rename keeps its two spellings
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say of the calls and the declaration of a helper renamed where no index proves it that they are the rename, not proven, as the header says of its file`)
  [@test] src/presenters/diff-marks.test.ts (`says of a rename the heading of its row names already its sites alone, and names it in full to a screen reader`)

#### Scenario: A pull request that renames a private helper
- **GIVEN** the review of a pull request whose one commit renames the private `sumOf` of a ledger to `totalOf`, and no index
- **WHEN** its changes show
- **THEN** the header says "4 changes to read in 1 file · 1 file not proven", and the headings of its three calls "balance — only renamed sumOf → totalOf, not proven", "balanceOn — …" and "averageAmount — …"
- **AND** the heading of the helper says "totalOf — renamed from sumOf, not proven"; no heading says "body changed", "new" or "removed"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say of the calls and the declaration of a helper renamed where no index proves it that they are the rename, not proven, as the header says of its file`)

#### Scenario: The card of the name
- **WHEN** a mention of the story names such a unit
- **THEN** its card says "Renamed, not proven", "Renamed, checking" while the index is asked, the same of a declaration renamed, and "Body changed" of a unit without either
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

### Requirement: In the Review Window: A heading sets the name in the code font

A heading MUST set the name of its declaration in the code font and what changed in the font of the text, muted; a
heading of no declaration MUST be in the text colour.

#### Scenario: The heading of a declaration and of no declaration
- **WHEN** a section shows the heading of a change
- **THEN** the name of its declaration is in the code font and what changed in the font of the text, muted
- **AND** a heading of no declaration is in the text colour

### Requirement: In the Review Window: A mechanical group starts collapsed

A mechanical group MUST start collapsed on every file, its header saying the kind (of a generated file why it is
generated, [Review Plan](../review-plan/spec.md#in-the-review-window)).

#### Scenario: The group of the imports
- **WHEN** the story shows the group of the imports
- **THEN** it starts collapsed on every file, its header saying the kind
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

#### Scenario: The group of the generated files
- **WHEN** the story shows the group of the generated files
- **THEN** each file starts collapsed, its header saying why it is generated
  [@test] src/presenters/review-units.test.ts (`fold each with why it is generated: its name, its attributes, the line its tool wrote`)

### Requirement: In the Review Window: A repeated edit shows once on its first file

A repeated edit MUST show on its first file under "Same edit in N files", the other files collapsed, "Same edit as
Alpha.kt".

#### Scenario: The same edit in four files
- **GIVEN** the same edit in Alpha, Beta, Gamma and Delta
- **WHEN** the story shows the repeated edit
- **THEN** it shows on Alpha under "Same edit in N files"
- **AND** the other files are collapsed, "Same edit as Alpha.kt"
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)
  [@test] src/presenters/review-units.test.ts (`makes a run of changes of one file one section with a heading per change, and a group a section per file`)

### Requirement: In the Review Window: A block another section shows is one row

A block that another section shows MUST be one row in the section of its file, "Shown in Imports only": a group by its
name, on each of its files, never the count of its heading; no change for N and P. A block of a slice that holds the
move alone (`moved_out`) MUST be such a row that says what moved and where ("Moved formatTotal to Report.kt"), another
step by its number; in a fold the words of the row.

#### Scenario: A block of the imports in its file's section
- **WHEN** the section of a file holds a block that the group of the imports shows
- **THEN** the block is one row there, "Shown in Imports only", without the count of its heading "Imports only (1)"
- **AND** N and P pass it, no change of that section
  [@test] src/lib/diff/rows.test.ts (`makes the rows of a section of a review: a heading before each change it marks, one row for a block another section shows`)
  [@test] src/presenters/review-units.test.ts (`names a block a group shows by the name of the group on each of its files, never by the count of its heading`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

#### Scenario: A block of the place a declaration left
- **WHEN** the section of `Orders.kt` holds the block `parseTotal` and `parseSum` left for the new `Parsing.kt`, the one
  `formatTotal` left for `Report.kt` in another step, and the section of the deleted `Legacy.kt` the block `legacyTotal`
  left for `Orders.kt`
- **THEN** each is one row, "Moved parseTotal and parseSum to Parsing.kt", "Moved formatTotal to Report.kt in step 1",
  "Moved legacyTotal to Orders.kt", none of their lines shown
- **AND** N and P pass it, no change of that section; a fold that hides it says its words
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/review-units.test.ts (`makes the place a declaration left one row that opens where it went, where Rust says it holds the move alone, and keeps its lines under the heading where it holds more`, `names the step of where a declaration went in the row of the place it left, when it is another, and in a fold the row itself`)

### Requirement: In the Review Window: A click on the row opens the other section

A click on the row of a block another section shows, or on its place in a fold, MUST open that section, expanded, as
the section at the top: the tree and the footer follow it. The row of the place a declaration left opens the section of
where it went, of another step and another file too.

#### Scenario: The row of the imports and the row of a repeated edit
- **WHEN** the user clicks the row of a block another section shows
- **THEN** that section opens, expanded
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`, `should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

#### Scenario: The row of the place a declaration left
- **WHEN** the user clicks the row "Moved parseTotal and parseSum to Parsing.kt", "Moved formatTotal to Report.kt in
  step 1" or "Moved legacyTotal to Orders.kt"
- **THEN** the section of `Parsing.kt`, of `Report.kt` in step 1, of `Orders.kt` in step 2 opens, the tree and the
  footer following it
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)

#### Scenario: The place of a block in a fold
- **WHEN** the user clicks the place of a block in a fold ("1 change in step 1")
- **THEN** that section opens, expanded, and the footer names it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

### Requirement: In the Review Window: A declaration moved whole is read at its new place

The heading of a unit `added` with `moved_from` MUST say where it came from ("formatTotal — moved from Orders.kt"), of
a unit `removed` with `moved_to` where it went, the file by its name among the files of the review; a new file MUST
name above its lines what moved into it, a deleted file that is not the move alone where its declarations went. Its
lines are read where it stands now: nothing of them is folded or mechanical, and what is left to read counts as the
diffs count it.

#### Scenario: The headings of the moves of a branch
- **GIVEN** the review of the branch `moves` of the `review_moves` template, with the plan
- **WHEN** it shows its diffs
- **THEN** the heading of `formatTotal` in `Report.kt` says "formatTotal — moved from Orders.kt", and its lines show
- **AND** the new `Parsing.kt` says "parseTotal — moved from Orders.kt · parseSum — moved from Orders.kt", and
  `legacyTotal` in `Orders.kt` "legacyTotal — moved from Legacy.kt"
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/review-units.test.ts (`words a declaration moved whole between two files where it stands now and where it left, as Rust decided`, `names in the row of the place two declarations left both and where each went, and a file taken whole by the moves into it, out of it, or its row`)

#### Scenario: A place that holds more than the move
- **WHEN** the block of the place a declaration left holds more than the move, or a deleted file more than its moves
- **THEN** its lines show under the heading "formatTotal — moved to Report.kt", "legacyTotal — moved to Orders.kt"
  [@test] src/presenters/review-units.test.ts (`makes the place a declaration left one row that opens where it went, where Rust says it holds the move alone, and keeps its lines under the heading where it holds more`, `names in the row of the place two declarations left both and where each went, and a file taken whole by the moves into it, out of it, or its row`)

#### Scenario: One place of a move in the range
- **WHEN** the review shows a range that holds one place of a move alone: a commit shown alone that copied the function,
  or Since Your Review of the commit that dropped its old copy
- **THEN** the plan of that range says no move: the function is new or removed there, and read
  [@test] crates/review-plan/src/moved_across_test.rs (`a_range_that_holds_one_place_of_a_move_says_nothing`)

### Requirement: In the Review Window: The words of a move are said once

The note of the marks of a move at the start of the block of a heading of that move MUST name the line of its other end
alone ("legacyTotal — moved from Legacy.kt · line 4"), its button and its name to a screen reader in full; the card of
a name of a unit with `moved_from` or `moved_to` MUST say "Moved from Orders.kt" or "Moved to Report.kt", and a step
that does it to a unit another step shows "Moves formatTotal, shown in step 2".

#### Scenario: The note of the move in the row of its heading
- **GIVEN** the review of the branch `moves`
- **WHEN** it shows `legacyTotal` in `Orders.kt`, whose block starts with the note of its move from `Legacy.kt`
- **THEN** the row says "legacyTotal — moved from Legacy.kt · line 4", the line a button to the other end
- **AND** a note of a move in a row of its own, or of another file, says its words as before ("Moved from Orders.kt:7")
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/diff-marks.test.ts (`says of a move the heading of its row names already the line of the other end alone, and names it in full to a screen reader`)

#### Scenario: The card of a name and the facts of a step
- **WHEN** a mention of the story names a unit with `moved_from` or `moved_to`, and a step does it to a unit the stop of
  another step shows
- **THEN** the card says "Moved from Orders.kt" or "Moved to Report.kt", never "New" or "Removed", and the facts of the
  step "Moves format, shown in step 2"
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`, `words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

### Requirement: In the Review Window: A heading and a row of words are one line high

A heading and a row of words MUST have the height of a line.

#### Scenario: The height of a heading and of a row
- **WHEN** a section shows a heading or a row of words
- **THEN** it has the height of a line

### Requirement: In the Review Window: A file of the tree opens its first open section

A file of the tree MUST open its first open section (of its concern), of a file in parts its first open part, a
place of the card the section that shows its line.

#### Scenario: A file of the tree whose change is in two sections
- **GIVEN** a file whose own change is in one section and whose repeated edit is in the group of the edit
- **WHEN** the user clicks its row in the tree
- **THEN** its first open section (of its concern) opens
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

#### Scenario: A file in parts
- **WHEN** the user clicks the row of a file the story shows in three parts
- **THEN** its first part opens, "part 1 of 3" in the footer
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A place of the card
- **WHEN** the user opens a place of the card
- **THEN** the section that shows its line opens

### Requirement: In the Review Window: J and K step through the sections

J and K MUST step through the sections, the tree following the section at the top.

#### Scenario: K and J
- **WHEN** the user presses K, then J, in the diffs
- **THEN** each key steps to the section before or after
- **AND** the tree selects the file of the section at the top
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

### Requirement: In the Review Window: The uses left outside follow the change

After the section of a change of a form or a removal, each of its first 5 uses left outside (`outside_sites` of its API
entry, by its key and file) MUST be a section of its own, "Left outside": the file at the head of the review around the
line of the use (`get_outside_place`, the lines of context of a change before and after it).

#### Scenario: A changed API with uses left outside
- **GIVEN** a change of the form of `helper` that leaves a use in `src/Base.kt` as it was
- **WHEN** the story shows the section of the change
- **THEN** each of its first 5 uses left outside is a section of its own after it, "Left outside"
- **AND** each shows the file at the head of the review around the line of the use, under the heading "helper — left
  outside: src/Base.kt:1"
  [@test] src/presenters/review-units.test.ts (`follow the section of the change: each its file around the use, at most five`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show after a changed API the uses it left outside, each its file at the head around the use, none that the head lacks`)

### Requirement: In the Review Window: A use left outside is no change and no row of the tree

A section of a use left outside MUST be no change for N and P and no row of the tree, which keeps the file of the
review selected.

#### Scenario: The tree keeps the file of the review
- **WHEN** the story shows a section of a use left outside
- **THEN** N and P pass it
- **AND** the tree has no row of it and keeps the file of the review selected
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show after a changed API the uses it left outside, each its file at the head around the use, none that the head lacks`)

### Requirement: In the Review Window: A use in a file the head lacks shows nothing

A use left outside in a file the head lacks MUST show nothing.

#### Scenario: Uses in files the head lacks
- **WHEN** the index names uses in files the head lacks
- **THEN** they show nothing
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show after a changed API the uses it left outside, each its file at the head around the use, none that the head lacks`)

### Requirement: In the Review Window: A commit shown alone has its own plan

The story MUST show in every review window with the plan: a commit of the branch review shown alone (Enter or Show
Commit Alone on its row in the tree, ⌥↑/⌥↓) MUST have its own plan (`ReviewSubject::Commit`, the commit against its
first parent) for its card and its diffs, beside the plan of the part, which stays the card under Since Your Review.

#### Scenario: A commit shown alone from its row in the tree
- **WHEN** the user shows a commit of the branch review alone (Enter or Show Commit Alone on its row in the tree,
  ⌥↑/⌥↓)
- **THEN** its card and its diffs are of its own plan, the commit against its first parent (`ReviewSubject::Commit`)
- **AND** the plan of the part stays beside it, the card under Since Your Review
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show the plan of a commit shown alone from its row in the tree: its overview and the story of its diffs @webkit`)
  [@test] crates/review-plan/src/units_test.rs (`a_commit_is_read_against_its_first_parent`)

### Requirement: In the Review Window: A commit window and a pull request read their plan

The review window of a commit and the review of a pull request MUST read the plan of what they show, and show none for
a repository that turns it off (`disabled`).

#### Scenario: The review window of a commit
- **WHEN** the review window of a commit opens
- **THEN** its overview and its diffs are of the plan of that commit
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show the plan of a commit in the review window of a commit: its overview and the story of its diffs`)

#### Scenario: The review of a pull request
- **WHEN** the review of a pull request opens in a repository that turns the plan on
- **THEN** it shows the plan of the pull request above its changes
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`)

#### Scenario: A repository that turns the plan off
- **WHEN** the repository turns the plan off (`disabled`)
- **THEN** the review window of a commit and the review of a pull request show no plan

### Requirement: In the Review Window: A file the units do not cut shows whole

A file whose blocks are not those of the units (the window reads another whitespace policy than `Default`) and a file
no unit names MUST show whole, in the place of its first unit, else after the story.

#### Scenario: Another whitespace policy
- **WHEN** the window reads another whitespace policy than `Default`, and the blocks of a file are not those of its
  units
- **THEN** the file shows whole, in the place of its first unit
  [@test] src/presenters/review-units.test.ts (`shows a file whole where its blocks are not those of the units, and a file no unit names after the story`)

#### Scenario: A file no unit names
- **WHEN** no unit names a file of the review
- **THEN** it shows whole after the story
  [@test] src/presenters/review-units.test.ts (`shows a file whole where its blocks are not those of the units, and a file no unit names after the story`)

## User Experience

`brd review-plan` prints, after `repeated_edits`:

```json
"units": [
  {"id": "declaration:src/a/Api.kt:a.open:path:String,mode:Int", "kind": "shape", "changes": ["parameters"],
   "name": "open", "key": "a.open", "signature": "path:String,mode:Int", "api": true, "test": false, "line": 3,
   "slices": [{"path": "src/a/Api.kt", "old_path": null, "whole": false,
     "blocks": [{"index": 0, "old": [3, 3], "new": [3, 3]}], "fingerprint": "5f0c…"}], "fingerprint": "5f0c…"},
  {"id": "new:src/a/Session.kt", "kind": "newFile", "name": "Session.kt", "api": false, "test": false,
   "slices": [{"path":
   "src/a/Session.kt", "old_path": null, "whole": true, "blocks": [], "fingerprint": "a1d9…"}], "fingerprint": "a1d9…"},
  {"id": "mechanical:importsOnly", "kind": "mechanical", "mechanical": "importsOnly", "api": false, "test": false,
   "slices": […]}
]
```

The section of a run: `diff-unit-heading` before a change (the name in the font of the code, bold, the words in Inter,
muted, a top border), and a block of
another section one `diff-unit-elsewhere` row in the tint of a fold. The first file of a group has the group on the
divider above its card, in `text-xs text-muted-foreground` (`review-section-heading`), never in its header: "Same edit
in 4 files", "Imports only (2)", "Left outside".

## Open Questions / Risks

- A block that changes two declarations goes to the one of most of its lines; the other has its own unit only when
  it is new or removed.
- The pair of a file several commits change is diffed under `Default` here; a window under another whitespace policy
  shows other blocks, which the window must map to the units by their lines.
- A container whose name changed gives its members new keys: they are removed and added.
- A Kotlin return type that is inferred is no part of the form: a change of it is `behaviour`.
- An outline module of Rust (`mod x;`) is a declaration of one line: removed from one crate and added to another it is
  a move by its tokens, true where the file it names moved with it (each of the 6 measured), a guess where two modules
  of one name are unrelated and no other place of that line is in the change.

## Measurements

`brd review-plan` (release build) on `~/projects/idea-1`, branch `air-local-launch`, 2026-10-30, the summaries in the
object cache (a next round of a branch: its blobs parsed before), accio answering every query:

| Range | Files | Units | `behaviour` | Other kinds | API entries | Time |
|---|---|---|---|---|---|---|
| `HEAD~1..HEAD` | 31 | 43 | 17 (39%) | 8 `newFile`, 8 `added`, 8 `other`, 2 `mechanical` | 33 | 0.19 s |
| `HEAD~3..HEAD` | 38 | 57 | 24 (42%) | 9 `newFile`, 10 `added`, 2 `removed`, 1 `shape`, 9 `other`, 2 `mechanical` | 36 | 0.22 s |
| `HEAD~5..HEAD` | 40 | 67 | 29 (43%) | 9 `newFile`, 16 `added`, 1 `removed`, 1 `shape`, 9 `other`, 2 `mechanical` | 36 | 0.11 s |

About 40% of the units are a change of a body alone, which the plan before the units did not show at all.

The moves between files (`moved_from`, `moved_to`), 2026-11-21: `brd review-plan` (release build of the change) on
shared clones without an index, at a load of 57–360. The branch `air-local-launch` is gone; its build of 2026-10-30
(`6e2c9551a209`, 31, 38 and 40 files as in the table above) is still in the reflog of idea-1. Its units are 40, 54 and
64 now, the rules since and no index; the same ids, kinds and fingerprints as `brd` 1.18.0, before this change, there
and on the commits of this repository below. The lines are those of a removed declaration from the line of its name to
its end: what a reviewer stops reading twice once the two places are one unit.

| Ranges | Units | `removed` | with `moved_to` | Their lines | `added` with `moved_from` |
|---|---|---|---|---|---|
| `air-local-launch` `HEAD~1`, `HEAD~3`, `HEAD~5` | 40, 54, 64 | 0, 2, 1 | 0 | 0 | 0 |
| the 94 ranges of idea-1 of the review-concerns lab | 6121 | 539 | 5, 4 of them into a new file | 42 | 1 |
| 10 commits of Branch Deck that move code between files and crates (`ded365452`, `7083b1ace`, `03bfb36f6`, `2734ea9e3`, `56f811c38`, `4fb2c5c4f`, `5b5acecbf`, and none in `7f17c7560`, `fd6b8e6cc`, `cc539f195`, whose moved code changed) | 598 | 154 | 80 | 700 | 15 |

Every pair is a move: 11 read by hand (a private Kotlin function and two TypeScript functions extracted into new
files, a TypeScript constant with its doc, the Rust modules `performance`, `dependencies` and `diff_scan` whose files
moved with them, a Rust function with its doc and attribute, a type, a test with raw strings), the other 74 by their
text without whitespace. Few moves are exact in feature work (5 of 539 removals); a refactoring that moves code is
half of its removals (80 of 154), the rest moved with a changed line or none.
