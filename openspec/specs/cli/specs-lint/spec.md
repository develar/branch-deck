---
name: The check of the specs
description: Requirements for `pnpm lint:specs`, the check that the test links, the targets and the links into a spec of the specs and the docs name tests, files and headings that exist, in this repository or in a crate of the git dependency psi-syntax at the revision Cargo.lock pins, that the requirements of a group stand together, openspec validates them strictly and AGENTS.md keeps within its budget.
style: plain-1
targets:
  - crates/spec-links/src/lib.rs
  - crates/spec-links/src/main.rs
  - package.json
  - .github/workflows/test.yml
---

# The check of the specs

Status: Implemented
Date: 2026-11-13

## Purpose

A spec is the contract of a feature, and its test links are the proof of each rule. `pnpm lint:specs` (the crate
`spec-links`) fails a renamed, moved or deleted test or file at once, not in a reader of the spec. It checks every link
of the specs and the design docs to the code, the order of the requirements, and `openspec validate --strict`. A test
of a crate that moved to psi-syntax keeps its link: the link names the file of that crate at the revision that
Branch Deck builds.

## Goals

- Every link of a spec to the code is checked on every run, a link into psi-syntax as a local one.
- The check reads only what cargo has fetched: no network, no change of Cargo.lock.

## Non-goals

- A path into a git dependency other than psi-syntax, or into a crate of psi-syntax that the workspace does not depend
  on: a link names the code Branch Deck builds.
- The links of the archived changes, which are history.

## Requirements

### Requirement: Test links: A test link names a test file and the tests it declares

A test link MUST be a line of its own: the marker, a path that names a test file, then in parentheses names in
backticks, each a test of that file: a Rust test by its function, a TypeScript test by its title as the source writes
it. Under `openspec/` its path MUST be from the root of the repository.

#### Scenario: A link names a test that is gone, a file that is gone, or stands inside a line
- **WHEN** a spec links a title its test file does not declare, a file that does not exist, or a link inside a line of
  prose
- **THEN** each is a problem at its line
  [@test] crates/spec-links/src/lib_test.rs (`a_test_link_names_a_file_and_the_tests_it_declares_on_a_line_of_its_own`)

#### Scenario: A path from the file under openspec
- **WHEN** a spec of `openspec/` links a test by a path that starts with `../`
- **THEN** the link is a problem, as `openspec archive` copies a delta two directories up
  [@test] crates/spec-links/src/lib_test.rs (`under_openspec_a_test_path_is_from_the_root_a_path_from_the_file_is_a_problem`)

### Requirement: Targets: Every spec lists its targets once, each a file from the root

Every spec MUST list its `targets:` in its front matter, each once, each a path from the root of the repository of a
file that exists. Every problem MUST name its file by its path from the root with `/`, on Windows too.

#### Scenario: A target listed twice, gone, from the spec or a directory
- **WHEN** a spec lists a target twice, a file that does not exist, a path from the spec, or a directory, or it has no
  front matter or no targets
- **THEN** each is a problem of the spec
  [@test] crates/spec-links/src/lib_test.rs (`every_spec_lists_its_targets_once_each_a_path_from_the_root_of_a_file_that_exists`)

#### Scenario: The problems of a walk on every platform
- **WHEN** the checks walk the directories of the repository
- **THEN** each problem names its file with `/`
  [@test] crates/spec-links/src/lib_test.rs (`every_check_answers_the_paths_with_a_slash`)

### Requirement: Targets: A rule and a presenter are targets of their specs

Every rule of `crates/domain-policy/src/` MUST name its spec in its `//!` header, and every spec it names MUST list it
as a target; every presenter of `src/presenters/` MUST be a target of a spec.

#### Scenario: A rule that names no spec or a spec that lists it not
- **WHEN** a rule names no spec in its header, names one that does not exist, or one that lists it not
- **THEN** each is a problem
  [@test] crates/spec-links/src/lib_test.rs (`a_rule_of_domain_policy_names_its_specs_in_its_header_and_each_of_them_lists_it`)

#### Scenario: A presenter that no spec lists
- **WHEN** no spec lists a presenter
- **THEN** it is a problem, and its test file is none
  [@test] crates/spec-links/src/lib_test.rs (`every_presenter_of_the_page_is_a_target_of_a_spec_its_tests_aside`)

### Requirement: Git dependencies: A path into psi-syntax names a file of its crate at the pinned revision

A test link and a target MAY name a file of a crate of the git dependency psi-syntax as
`psi-syntax:crates/<crate>/<path>`. The check MUST read it in the checkout cargo fetched of that crate at the revision
Cargo.lock pins, by `cargo metadata --offline --locked`, and check its tests as those of a local link.

#### Scenario: A link and a target into psi-syntax
- **GIVEN** the workspace depends on the crates `diff-marks` of psi-syntax at one revision and `line-diff` at two
- **WHEN** a spec links a test of `psi-syntax:crates/diff-marks/…` and lists its files as targets
- **THEN** a test it declares passes and one it does not is a problem, as a file that is gone
- **AND** a crate the workspace does not depend on, a crate at two revisions and a path that is no file of a crate are
  problems of the link
  [@test] crates/spec-links/src/lib_test.rs (`a_test_link_and_a_target_into_psi_syntax_name_a_file_of_its_crate_at_the_revision_cargo_lock_pins`)

#### Scenario: The checkout cargo fetched
- **GIVEN** a workspace whose Cargo.lock pins spec-syntax at the revision of this repository, which cargo fetched
- **WHEN** the check asks its cargo for `psi-syntax:crates/spec-syntax/src/lib.rs`
- **THEN** it answers the file of the checkout named by that revision
  [@test] crates/spec-links/src/lib_test.rs (`cargo_metadata_answers_the_checkout_cargo_fetched_of_psi_syntax_at_the_revision_cargo_lock_pins`)

### Requirement: Git dependencies: Sources not fetched are a problem, and cargo runs once at most

The check MUST ask cargo only when a spec or a doc names a path into psi-syntax, and once for every such path. When
cargo has not fetched the sources, each such path MUST be a problem that names cargo's reason and `cargo fetch`; the
check never fetches and never writes Cargo.lock.

#### Scenario: No link into psi-syntax, then two without sources
- **WHEN** the specs name no path into psi-syntax
- **THEN** no cargo runs
- **WHEN** a link and a target name one and cargo answers an error
- **THEN** each is a problem that says the sources are not fetched, and cargo ran once
  [@test] crates/spec-links/src/lib_test.rs (`the_sources_of_psi_syntax_are_asked_once_at_the_first_link_into_them_and_a_link_without_them_is_a_problem`)

#### Scenario: A revision cargo has not fetched
- **GIVEN** a workspace whose Cargo.lock pins a revision of psi-syntax that cargo has never fetched
- **WHEN** the check resolves a path into it
- **THEN** the problem says the sources are not fetched, with cargo's error of the offline mode
  [@test] crates/spec-links/src/lib_test.rs (`cargo_metadata_of_a_revision_of_psi_syntax_cargo_has_not_fetched_fails_offline_with_its_reason`)

### Requirement: Spec links: A link into a spec names one of its headings or a group

A markdown link into a spec MUST name a spec that exists, and its anchor MUST be the anchor GitHub gives one of its
headings, or name a group of its requirements; the archived changes are not checked.

#### Scenario: The anchors GitHub gives the headings
- **WHEN** a spec has a repeated heading and a heading inside fenced code
- **THEN** the repeated one is numbered and the fenced one is none
  [@test] crates/spec-links/src/lib_test.rs (`the_anchors_of_the_headings_are_those_github_gives_them_a_repeated_one_numbered_none_of_fenced_code`)

#### Scenario: A link to a spec that is gone or a heading it lacks
- **WHEN** a doc links a spec that does not exist, or an anchor of no heading and no group
- **THEN** each is a problem at its line
  [@test] crates/spec-links/src/lib_test.rs (`a_link_into_a_spec_names_one_of_its_headings_or_a_group_of_its_requirements`)

### Requirement: Spec links: The requirements of a group stand together

In every spec the requirements of a group, `### Requirement: <Group>: <behaviour>`, MUST stand together; a name without
`: ` is of the untitled group.

#### Scenario: A group in two runs
- **WHEN** the requirements of a group stand in two places
- **THEN** the first requirement of its second run is a problem, and a heading inside fenced code is none
  [@test] crates/spec-links/src/lib_test.rs (`the_requirements_of_a_group_stand_together_in_a_spec_the_untitled_group_too_a_heading_of_fenced_code_aside`)

### Requirement: Spec links: OpenSpec validates every spec and change strictly

The check MUST run `openspec validate --strict` over the specs, the changes and the archived changes, each that exists,
and report each issue it answers.

#### Scenario: A weak spec and an invalid change
- **WHEN** a spec has a requirement without a scenario, or a change is invalid without an issue
- **THEN** each issue, or the invalid change, is a problem
  [@test] crates/spec-links/src/lib_test.rs (`openspec_validate_runs_over_every_spec_and_the_changes_nothing_without_openspec`)

### Requirement: Spec links: No spec of the old layout

A tracked file MUST NOT lie under `spec/`, and a file MUST NOT name a spec by its old path `spec/<area>/<name>.spec.md`,
CHANGELOG.md and the archived changes aside.

#### Scenario: A spec under spec/ and a file that names one
- **WHEN** a file lies under `spec/`, or a file names a spec by its old path
- **THEN** each is a problem, and a path of another repository is none
  [@test] crates/spec-links/src/lib_test.rs (`no_spec_lies_under_spec_and_no_file_names_one_by_its_old_path_a_fixture_of_another_repository_and_the_history_aside`)

#### Scenario: The tracked files
- **WHEN** the check lists the files of the repository
- **THEN** it reads those git tracks, an untracked one aside
  [@test] crates/spec-links/src/lib_test.rs (`the_tracked_files_are_those_git_tracks_in_every_directory_an_untracked_one_aside`)

### Requirement: Spec links: AGENTS.md keeps within its budget

AGENTS.md MUST be 32 KiB at most, as every session of every agent reads it whole.

#### Scenario: AGENTS.md over its budget
- **WHEN** AGENTS.md is longer than 32 KiB
- **THEN** it is a problem that names its size and its budget

## User Experience

- `pnpm lint:specs` prints each problem on a line, `<file>:<line>: <problem>`, and exits 1 with their count.
- A path into psi-syntax: `psi-syntax:crates/<crate>/<path>`, the path of a test link
  (`psi-syntax:crates/diff-marks/src/marks_test.rs`) or of an item of `targets:`.
- Sources not fetched: "the sources of psi-syntax are not fetched (`cargo fetch` fetches them): cargo metadata
  --offline: <cargo's error>".

## Testing / Local Run

- `cargo nextest run -p spec-links`, then `pnpm lint:specs`.

## Measurements

- `cargo metadata --offline --locked` of this workspace: 1.3 s on the Mac of the author (2026-11-13), paid only by a run
  whose specs name a path into psi-syntax.

## References

- [Specs](../../../../docs/contributing/specs.md) ("Test links", "Changes")
- [Checks](../../../../docs/contributing/checks.md)
