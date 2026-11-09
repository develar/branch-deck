# Specs

## Summary

A spec is the contract of a feature: what it does, as a user or an agent sees it, each rule with the tests that prove
it. The specs are of [OpenSpec](https://github.com/Fission-AI/OpenSpec) 1.16 (`@fission-ai/openspec`, a devDependency):
one capability per file, `openspec/specs/<area>/<name>/spec.md`, its id `<area>/<name>` (`branch/unapply`).
`pnpm lint:specs` checks their links and runs `openspec validate --strict`; `pnpm exec openspec show branch/unapply`
and `pnpm exec openspec list --specs` read them.

## The file

```markdown
---
name: Unapply and Reapply
description: Requirements for unapplying a virtual branch, …
style: plain-1
targets:
  - crates/sync-core/src/unapply_branch.rs
  - src/hooks/useUnapplyBranch.ts
---

# Unapply and Reapply

Status: Implemented
Date: 2026-10-21

## Purpose

Some work must leave the local branch for a while: …

## Goals

- …

## Non-goals

- …

## Requirements

### Requirement: Result: The files the commits changed follow them

The files the commits changed MUST follow them out of the current branch, so that `git status` shows nothing of them.

#### Scenario: Added files are gone, changed files are as the remaining commits leave them
- **WHEN** the unapply runs
- **THEN** a file the commits added is gone
- **AND** a file they changed is as the remaining commits leave it
  [@test] crates/git-ops/src/unapply_test.rs (`the_files_follow_the_unapply_and_the_virtual_branch_moves_in_the_same_transaction`)

## User Experience

- …

## Open Questions / Risks

## References

- [Git Operations](../../../../docs/design/git-operations.md) ("Drop and unapply")
```

- **Front matter**: `name`, `description` (one sentence of what the spec requires; the brief of a review shows it for
  a new spec), `style: plain-1`, and `targets:`: the files where the feature lives (its rule, intent, `brd` command and
  presenter), each once, by its path from the root of the repository. A rule of `crates/domain-policy/src/` names its
  spec in its `//!` header, and the spec lists it; every presenter of `src/presenters/` is the target of a spec.
- **Title**, then `Status:` (Implemented, Experimental, Draft) and `Date:`; OpenSpec reads neither.
- **Purpose**: what the feature is for, in a paragraph (50 characters at least). Goals and Non-goals follow it.
- **Requirements**: only `### Requirement:` and `#### Scenario:` headings. Prose of a group that is no rule goes into
  a `## Concepts` section before Requirements.
- After Requirements: User Experience (the UX tokens: the words, the keys, the toasts), Testing / Local Run,
  Measurements, Open Questions / Risks, References.

## A requirement

- **Its name** is `<Group>: <behaviour>`: the group of the rules it belongs to (`Result`, `Refusals`, `` `brd` ``),
  then the behaviour in sentence case, about 70 characters at most, without a period. OpenSpec has no group headings,
  so the group lives in the name: the requirements of a group stand together, in the order of the old groups; a link
  to `#refusals` names the group (the first requirement `### Requirement: Refusals: …`). Names are unique in their
  spec, and so are their anchors.
- **Its text** (everything between its heading and its first scenario) states one rule with SHALL or MUST, in one to
  three sentences, 500 characters at most (`openspec validate --strict` fails longer ones). No test link and no
  table in it; examples and cases go into its scenarios. A requirement that holds several rules is several
  requirements.
- **Its scenarios**, one at least: `#### Scenario: <case>`, its steps `- **GIVEN** …` (optional), `- **WHEN** …`,
  `- **THEN** …`, `- **AND** …`, its test links indented two spaces under the last step. A scenario no test
  proves yet has no test link.
- **A table** stays a table, in the body of a scenario: `openspec/specs/branch/next-step/spec.md` "The table", its rows
  `  | <row> | … |`, which the cases of `crates/domain-policy/cases/` name by `#the-table:<row>`; the refusal codes of
  `cli/host` at column 0, which a test reads from the first column.
- **A fenced block** may stand in the text of a requirement, its fences at column 0; `openspec validate` counts no
  line of it (the golden reports of `cli/host`, which its tests read by the fence `json golden=<name>`).

## Test links

A test link is a line of its own: the marker, the path of a test file from the root of the repository, then in
parentheses the names of tests in it, each in backticks (a Rust function, the title of a TypeScript test as its source
writes it). Every requirement names the tests that prove it, exactly; a renamed or moved test updates its links in
the same commit. The review of a branch shows them as the proof of the step that changes the rule.

## Links

A markdown link between specs is relative (`../parked-branches/spec.md`, `../../review/review-window/spec.md#kept-branches`),
one into `docs/` relative too (`../../../../docs/design/git-operations.md`). Its anchor is the one GitHub gives the
heading: `### Requirement: Undo: The undo token names full ids` is `#requirement-undo-the-undo-token-names-full-ids`;
the group alone, `#undo`, names its first requirement. `pnpm lint:specs` checks both.

## While the specs move

The specs are moving from their old format (`### <Group>` headings of `- ` bullets, each with its test links) to
the one above, one spec per commit. A spec without `### Requirement:` is still in the old format: edit it in that
format until its conversion lands; `openspec validate` skips it until then, the links and targets are checked as of
every spec.
