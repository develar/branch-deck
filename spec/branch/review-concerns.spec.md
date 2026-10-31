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
- A node that changes a declaration (`shape`, `behaviour`, `added`, `removed`) and a node of another unit whose changed
  lines name it as a whole word must join, when the second is in the same file or in a file that links to the first
  by that name (`links`). A name of fewer than 3 characters joins nothing, and a declaration that more than 6 nodes
  name joins none of them: a hub is no evidence.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_changed_declaration_and_the_changes_of_its_uses_are_one_concern`, `a_declaration_named_by_many_changes_joins_none`)
- The nodes of a group must join.
- A rare identifier must join nodes of two files: a run of letters, digits, `_` and `-` that starts with a letter, and
  its parts between `_` and `-`, lowercase, of 4 or more characters, no keyword, common word or common key of a config,
  no part of a changed path; a token a node both removes and adds is no change of it; all removed, or all added, in 2
  to 10 nodes of two or more files. Two nodes join when their rare tokens are alike: the cosine of their weights
  (`ln((n + 1) / df)` over the nodes) at 0.35 or more, or, with 4 or more shared tokens, 80% of the weight of the smaller
  in the larger. The pairs most alike join first.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)
- A pair of tokens must join two concerns of more than one node each only when two rare tokens have nodes in both: one
  shared word bridges no two concerns.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`one_shared_identifier_bridges_no_two_concerns`)

### The Rest
- A node joined to nothing must join the concern of its file with the most changed lines, else a test the concern of
  its subject (a file of the stem it names); a generated file must follow the concern of the changed file its header
  names by its whole path, of the most changed lines, at the end of its parts; a glob (`edit docs/*`) names none, and
  such a file stays in the Mechanical group.
  [@test] ../../crates/review-plan/src/concerns_test.rs (`a_test_follows_its_subject_and_a_change_alone_the_concern_of_its_file`, `a_generated_file_follows_the_concern_of_the_source_its_header_names`)
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
  (or `none`) picks the edges for a measurement of each.
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

`bd review-plan` (release build) on the local virtual branches of `~/projects/idea-1` of 2 to 15 commits,
2026-10-31. A weak label: the issue key of the subjects of the commits that change a file, a file of one key only; a
pair of labelled files in one concern is right when their keys are equal. Pairs of files, not of parts, so a file of
both tasks counts for neither.

| Edges | Pairs right | Pairs wrong | Weak precision |
|---|---|---|---|
| `uses` | 426 | 6 | 0.986 |
| `tokens` | 357 | 1 | 0.997 |
| `uses,tokens,rest` | 966 | 45 | 0.955 |

The one wrong pair of `tokens` is two commits of one task under one prefix (`setup build:`), right in fact; the wrong
pairs of the full set are two files of `IJAI-4197` that join the 30 files of `IJAI-3261` through declarations both
tasks change. The concerns of a range of 168 units took 12.7 ms (59 files, 203 nodes, 353 rare tokens).
`accio-search-policy` (16 files): six MCP configs one concern (`proxy`), the deleted one among them; three concerns of
the permissions of grep and find with their settings and the generated `default.rules`; the guide in `rest`.
