---
name: Review Units
description: Requirements for the units of a review (experimental), the changes a reviewer reads once instead of the files; the pair of the review, its blocks cut by the declaration that holds them into a change of a form, of a body, a new or removed declaration, a new file, a group of mechanical changes or of a repeated edit over its files, and a change of no declaration; every block in one unit, the order of the story, and a fingerprint of each change that a rebase or an amend keeps.
style: plain-1
targets:
  - ../../crates/review-plan/src/units.rs
  - ../../crates/review-plan/src/pair.rs
  - ../../crates/trial-check/src/declaration_checks.rs
  - ../../crates/file-diffs/src/service.rs
  - ../../crates/bd-rewrite/src/review_plan_command.rs
---

# Review Units

Status: Experimental
Date: 2026-10-30

## Summary

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
is anchored to.

## Goals

- Cut every block of the reviewed diff into exactly one unit, and put every changed file in one unit at least: the
  units hide nothing and show nothing twice.
- Tell a change of a form from a change of a body, for every declaration, public or not.
- Read one repeated edit and one kind of mechanical change once, over all of its files.
- Name a change by its content, not by its place: the same change on another base or after an amend has the same
  fingerprint, and the whitespace policy of a window changes neither the identity nor the fingerprint.
- Cost no git process of its own: the units read the blocks, the summaries and the hunk text the plan reads already.

## Non-goals

- A move of a block between files as one unit: the units see a removal and an addition.
- The usages of a change of a body: accio answers the callers of the API alone, until a query of many names costs one
  process (JetBrains/accio#74).
- A change of a form that the summary does not see (`Form`): a Kotlin return type that is inferred changes with the
  body, and the unit is `behaviour`.

## Requirements

### Pair
- The units must cut the pair the review shows: per file the reviewed commits change, the blocks of its blob before
  the first commit that changes it against its blob after the last one, under `Default`, as the diffs of the range
  show them. A file one commit changes must take the hunks of that commit; a file several commits change its first
  blob against its last, one `cat-file --batch` for the blobs of all such files that the memory of the diffs and the
  `file_diff` store lack.
  [@test] ../../crates/review-plan/src/units_test.rs (`the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice`)
- A file whose blob and path are the same before and after the commits is no file of the pair: it has no unit.

### Units
- Every block of every file of the pair must be in exactly one unit, and every file of the pair in one at least.
  [@test] ../../crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)
- A block must go to the repeated edit of its file's group when the block is that edit; else to the imports when its
  changed lines are each of an import statement or blank (`file_diffs::service::imports_only`, the test the diffs pale
  a block by), one at least; else to the whitespace when its text without whitespace is the same on both sides, in a
  file where the indentation has no meaning; else to the declaration that holds most of its changed lines, the
  innermost one around each line, on the new side, on the old side for a block that only removes; else to the change
  outside any declaration, one unit per run of such blocks.
  [@test] ../../crates/review-plan/src/units_test.rs (`a_block_belongs_to_the_declaration_of_most_of_its_lines`)
- The declarations of a file are those of its summary that are no `Binding`, none the parser may have misread
  (`in_error`) and none inside a function or a value (a local): `spanned_declarations`. A side whose summary is
  missing, degraded or truncated makes the file one unit `other` (`unparsed`).
- A declaration must be paired across the two sides by its key and, among overloads of one key, by an equal form,
  then in order. A pair whose form differs is `shape`, with the parts that changed; one whose form is the same and
  whose text without whitespace is not is `behaviour`; one whose text is the same too is whitespace, and its blocks go
  to the unit of whitespace. A declaration of one side alone whose lines a block changes is `added` or `removed`, also
  when another unit holds that block: then it has no block. A declaration of one side alone inside another of that
  side alone (a member of a new type, a method of a removed class) is part of the unit of the outermost one, and so
  are its blocks.
  [@test] ../../crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`, `an_overload_is_a_unit_of_its_own`)
  [@test] ../../crates/review-plan/src/units_test.rs (`a_member_of_a_new_type_is_part_of_the_type`)
- A file whose change is mechanical (`files`: a build file, imports only, whitespace only, a pure rename, a deletion)
  must be one slice of the unit of its kind, the file whole. A file the commits add is one unit `newFile`. A binary
  file, a file over 8 MiB, a change of its type, a change of the line ends alone where the indentation has a meaning,
  and a file of no language the review parses are each one unit `other` with the reason.
- One kind of mechanical change must be one unit over every file that has it, and one repeated edit one unit over its
  files.
  [@test] ../../crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`)

### Order
- The units must come in the order of the files of the plan (`order`: a declaration before its uses, a test right
  after its subject), each unit at the first block it holds, a unit without a block right after the unit of the block
  that changed its lines; a repeated edit at the place of its first file; the mechanical units last, by kind: build
  files, imports, whitespace, renames, deletions.
  [@test] ../../crates/review-plan/src/units_test.rs (`the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice`)

### Identity
- `id` must name a unit within a plan: of a declaration its file, key and signature.
- `fingerprint` must name the change across plans: of a declaration a digest of its key, its signature and its text
  without whitespace on both sides; of the blocks of a slice their file and their text without whitespace; of a file
  taken whole its path and blobs; of a group the set of the fingerprints of its slices. A digest is the first 16 bytes
  of a SHA-256, so that it is the same on every machine and in every version.
  [@test] ../../crates/review-plan/src/units_test.rs (`the_fingerprint_names_the_change_wherever_it_sits`)
- The kind of a unit may change with the whitespace policy of a window, its `id` and its `fingerprint` must not.

### In `bd`
- `bd review-plan` must print `units` (`"schema": 4`): per unit its `id`, `kind` (`shape` with `changes`,
  `behaviour`, `added`, `removed`, `newFile`, `mechanical` with `mechanical`, `repeatedEdit` with `group`, `other` with
  `reason`), `name`, `key`, `signature`, `public`, `line`, its `slices` (`path`, `old_path`, `whole`, the `blocks` by
  `index` with their `old` and `new` lines from 1, both included, `null` for a side with none, and a `fingerprint`)
  and its `fingerprint`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

## User Experience

`bd review-plan` prints, after `repeated_edits`:

```json
"units": [
  {"id": "declaration:src/a/Api.kt:a.open:path:String,mode:Int", "kind": "shape", "changes": ["parameters"],
   "name": "open", "key": "a.open", "signature": "path:String,mode:Int", "public": true, "line": 3,
   "slices": [{"path": "src/a/Api.kt", "old_path": null, "whole": false,
     "blocks": [{"index": 0, "old": [3, 3], "new": [3, 3]}], "fingerprint": "5f0c…"}], "fingerprint": "5f0c…"},
  {"id": "new:src/a/Session.kt", "kind": "newFile", "name": "Session.kt", "public": false, "slices": [{"path":
   "src/a/Session.kt", "old_path": null, "whole": true, "blocks": [], "fingerprint": "a1d9…"}], "fingerprint": "a1d9…"},
  {"id": "mechanical:importsOnly", "kind": "mechanical", "mechanical": "importsOnly", "public": false, "slices": […]}
]
```

## Open Questions / Risks

- A block that changes two declarations goes to the one of most of its lines; the other has its own unit only when
  it is new or removed.
- The pair of a file several commits change is diffed under `Default` here; a window under another whitespace policy
  shows other blocks, which the window must map to the units by their lines.
- A container whose name changed gives its members new keys: they are removed and added.
- A Kotlin return type that is inferred is no part of the form: a change of it is `behaviour`.

## Measurements

<!-- The corpus runs of `bd review-plan` with their units go here: the count of units, the share of `behaviour`. -->
