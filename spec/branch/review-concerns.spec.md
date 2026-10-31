---
name: Review Concerns
description: Requirements for the concerns of a review (experimental), the tasks a range does, each a set of parts of its units joined by evidence the plan read already (a changed declaration and the changes that name it, the files of a group, a rare identifier the changed lines of several files share), the rest attached to the concern of its file or its subject, a generated file to the concern of the source its header names, what no evidence joined last; in the order of the story, with an id a growing concern keeps and a name by its evidence.
style: plain-1
targets:
  - ../../crates/review-plan/src/concerns.rs
  - ../../crates/review-plan/src/units.rs
  - ../../crates/bd-rewrite/src/review_plan_command.rs
---

# Review Concerns

Status: Experimental
Date: 2026-10-31

## Summary

The units cut a review into the changes a reviewer reads once, but a branch does several tasks, and the files of one
task lie all over the tree: a server dropped from six configs in five directories, a changed function and the files
that call it. A commit is no task either: an agent's round mixes tasks, and five commits change one guide.

A concern is one task of the range: a set of parts of its units, joined only by evidence the plan read already, no
git process and no accio, so the plan before the usages and the plan with them group alike. The window lists the
concerns, each with its files, and the story of the diffs follows them. The vision, the research behind the edges and
the measurements: [Change Review](../../docs/design/change-review.md).

## Goals

- Join only on evidence, with near no false join: a wrong join hides one task inside another.
- Join across languages and formats: a config, a guide and a test of one task.
- Cost no git process and a few milliseconds per range.
- Keep the id of a concern while it grows, so that a later review knows it.

## Non-goals

- A name by a language model: a concern is named by its evidence.
- Two tasks inside one block of a file of no language, or inside one declaration both changed: such a block is one
  node, and joins both.
- Moves between files: the units see a removal and an addition.

## Requirements

### Nodes
- A node must be a unit; one per block for a unit `other` of a file of no language the summaries parse; one per file
  for a repeated edit and for `deletedFiles`. A mechanical unit must give no node.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)

### Edges
- A node that changes the form of a declaration, adds or removes one (`shape`, `added`, `removed`; a change of a body
  alone forces no use to change) and a node of another unit whose changed lines name it as a whole word must join, when the second is in the same file or in a file that links to the first
  by that name (`links`). A name of fewer than 3 characters joins nothing, and a declaration that more than 6 nodes
  name joins none of them: a hub is no evidence.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_changed_declaration_and_the_changes_of_its_uses_are_one_concern`, `a_declaration_named_by_many_changes_joins_none`)
- The nodes of a group must join.
- A rare identifier must join nodes of two files: a run of letters, digits, `_` and `-` that starts with a letter, and
  its parts between `_` and `-`, lowercase, of 4 or more characters, no keyword, common word or common key of a config,
  no part of a changed path (`-` and `_` alike); a token a node both removes and adds is no change of it; all removed,
  or all added, in 2 to 10 nodes of two or more files, and in a range of 16 nodes or more in a quarter of them at most.
  Two nodes join when their rare tokens are alike: the cosine of their weights (`ln((n + 1) / df)` over the nodes) at
  0.35 or more, or, with 4 or more shared tokens, 80% of the weight of the smaller in the larger. The pairs most alike
  join first.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)
- A token must be a name of code, never a word of prose: of a Markdown or text file only the code it quotes (a
  backtick span, a fence, an indented line); of a source file no comment line, and only a name a changed file declares
  on either side or a word of a string literal, never a name of a library (`list`, `as_bytes`); of a config every
  line.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_word_of_prose_or_of_a_comment_joins_nothing`, `a_name_of_a_library_joins_nothing_a_word_of_a_string_does`)
- A pair of tokens must join two concerns of more than one node each only when two rare tokens have nodes in both: one
  shared word bridges no two concerns.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`one_shared_identifier_bridges_no_two_concerns`)

### The Rest
- The rest is off unless `--concern-edges` names it: measured, it adds recall and costs more purity than it is worth
  ([Measurements](#measurements)). With it on, a node joined to nothing must join the concern of its file with the most changed lines, else a test the concern of
  its subject (a file of the stem it names); a generated file must follow the concern of the changed file its header
  names by its whole path, of the most changed lines, at the end of its parts; a glob (`edit docs/*`) names none, and
  such a file stays in the Mechanical group.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_test_follows_its_subject_and_a_change_alone_the_concern_of_its_file`)
- A generated file must follow the concern of the changed file its header names by its whole path, also with the rest
  off.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_generated_file_follows_the_concern_of_the_source_its_header_names`)
- The nodes no edge joined must be the last concern, `rest`.

### Order, Identity, Name
- The concerns must come in the order of their first unit in the story, `rest` last; the parts of a concern in the
  order of the units, one per unit and file, with the blocks it holds of a file of no language (`null` for all).
- `id` must be the fingerprint of the first part (its unit's fingerprint, its file, its block), so that a concern that
  grows keeps it; `fingerprint` the digest of the set of the fingerprints of its parts.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_concern_keeps_its_id_as_it_grows`)
- The name must be the evidence of the concern: of a concern a token joined, the rare token with the most of its nodes
  in it (more than half), a simple word before a compound one, the one its changed lines name first; a removed and an
  added token of the same nodes `replacement`; else its first changed declaration; else the directory its files
  share.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_generated_file_follows_the_concern_of_the_source_its_header_names`, `the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)

### In `bd`
- `bd review-plan` must print `concerns` (`"schema": 7`): per concern its `id`, `name` (`source` `token` with
  `token`, `replacement` with `removed` and `added`, `declaration` with `name`, `directory` with `directory`, `rest`),
  its `parts` (`unit`, `path`, `blocks`), its `paths`, its `evidence` (`kind`: `uses`, `group`, `token`, `rest`, its
  `detail` and `joins`, the most joins first) and its `fingerprint`. The hidden `--concern-edges uses,tokens,rest`
  (or `none`) picks the edges for a measurement of each; without it `uses,tokens`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
- The concerns must run no git process.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

## User Experience

```json
"concerns": [
  {"id": "3f1c…", "name": {"source": "token", "token": "proxy"},
   "parts": [{"unit": "other:.codex/config.toml", "path": ".codex/config.toml", "blocks": null},
             {"unit": "deleted:dotnet/.codex/config.toml", "path": "dotnet/.codex/config.toml", "blocks": null}],
   "paths": [".codex/config.toml", "dotnet/.codex/config.toml"],
   "evidence": [{"kind": "token", "detail": "ij-mcp-proxy", "joins": 5}], "fingerprint": "9a02…"},
  {"id": "…", "name": {"source": "rest"}, "parts": […]}
]
```

The window does not show the concerns yet ([Change Review → Next](../../docs/design/change-review.md#next)).

## Open Questions / Risks

- A block of a file of no language that two tasks rewrote is one node and joins both; a guide that every commit of a
  range edits stays in `rest` or joins them.
- A declaration both tasks change is one unit and joins them: the plan cannot tell whose change it is.
- The stop words are a short list: a common word of prose that is no word of the list joins two blocks of prose when
  their cosine is high enough.
- The name of a token concern is a word, not a sentence: `proxy` for a server dropped from six configs.

## Measurements

The lab of 2026-10-31 (`~/projects/review-concerns-lab`, its `results/scores.md`): four corpora of tangled ranges
with a label per hunk, `bd review-plan` per range, each hunk mapped to the concern part whose blocks hold it; a part
of `rest` and a mechanical slice count as a group of its own. Purity is the share of groups of more than one hunk that
hold one task (Wilson 95% lower bound in parentheses); recall the share of the pairs of a task that share a group.
Merge precision alone says little: one group over a whole range scores 91–98% on it.

| Corpus | Cases, hunks | One group per file: recall / purity | `uses,tokens` (default): recall / purity | With `rest`: recall / purity |
|---|---|---|---|---|
| this repository, 2–3 atomic commits tangled | 200, 6047 | 72.9 / 91.4 (89.6) | 67.8 / 93.9 (92.2) | 74.7 / 90.8 (88.7) |
| tangled-ccs-commits (HF), 2–5 concerns | 245, 7750 | 80.8 / 100 (99.7) | 75.0 / 98.8 (98.0) | 78.3 / 98.7 (97.8) |
| jsoup, gson, commons-lang tangled | 200, 3511 | 81.4 / 89.9 (87.1) | 46.4 / 95.4 (93.3) | 57.0 / 88.4 (85.2) |
| idea-1 branches, a label per line by the commit | 94, 9451 | 77.8 / 84.7 (82.6) | 67.0 / 86.2 (84.2) | 79.3 / 81.0 (78.4) |

The default is purer than a group per file on three corpora and joins changes across files and formats a group per
file never does; its recall is lower, the rest buys it back at the cost of purity, so it stays off. The rounds that
led here, each measured on the same corpora:

- The first token edge joined singletons by one shared word and chained a branch into one concern; a big block of
  prose shared words by chance. Fixed by the cosine of the rare tokens, the unchanged words of a block left out, and
  no token bridging two concerns.
- The `uses` edge from a change of a body alone joined callers that had nothing to do with it: purity of `uses` alone
  on idea-1 87.2 → 88.1, on Java 94.4 → 96.1 once only a changed form, a new or a removed declaration joins its uses.
- Words of prose and comments (`suite`, `same`, `compiles`) and names of libraries (`list`, `as_bytes`,
  `IllegalStateException`) joined tasks: tokens come from code only, of a source file only the names the change
  declares. Wrong token joins on idea-1 64 → 61, on the synthetic corpus 10 → 5; what remains are names the project
  declares that two tasks share (`turnEnded`).
- A crash of the symbol dependencies on `use m::m;` (5c20eed6) had scored 28 cases as singletons.

A range takes 0.13–0.19 s median for the whole plan (concerns 12.7 ms for 168 units). The false merges left are of
three kinds: a concern (146 on idea-1), a mechanical slice that holds lines of two tasks (27), and a unit, one
declaration, that two tasks changed (43).
