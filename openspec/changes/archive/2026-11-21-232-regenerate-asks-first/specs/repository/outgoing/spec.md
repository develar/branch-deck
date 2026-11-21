## ADDED Requirements

### Requirement: Push: A push of a copy with a generated file merged by words asks first

A push that sends a copy holding a generated file the words merged in a conflict of its sync MUST ask first, every
time until the copy no longer holds it, also when forced: its plan asks it (`Confirmation::Regenerate`, one per copy),
so the click opens its description with the files and Push Anyway, Force Push beside a drop, which pushes with it
accepted. A plan run without it MUST push nothing.

#### Scenario: The push of such a copy
- **WHEN** the user clicks the push of a branch whose copy holds a lock file the words merged
- **THEN** its description opens with the file, why it is generated and the commit the copy lacks, and Push Anyway
- **AND** Push Anyway pushes it; a push run without the confirmation pushes nothing
- **AND** a forced push asks the same, and the push of a branch whose copy is clean asks nothing
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_copy_with_a_generated_file_merged_by_words_asks_first_and_runs_once_accepted`)
  [@test] src/presenters/push-button.test.ts (`asks first for a copy with a generated file merged by words`)
  [@test] tests/e2e/outgoing/regenerate.spec.ts (`should mark a branch whose copy merged a generated file by words and ask before its push`)

#### Scenario: A stack and the stack branch
- **WHEN** a push of a stack sends copies that hold such files, or the stack branch going along holds one, or the
  stack branch of a whole stack is pushed alone
- **THEN** each such copy asks, the stack branch by its whole stack, also when forced

## MODIFIED Requirements

### Requirement: Push: A safe push pushes on the click

A safe push (Push, the first one too, the Force Push of a rebased, amended or restacked branch) MUST push on the
click, unless the copy it sends holds a generated file the words merged, which
[asks first](#requirement-push-a-push-of-a-copy-with-a-generated-file-merged-by-words-asks-first).

#### Scenario: A force push of a rebased branch
- **WHEN** the user clicks the force push of a rebased branch, which drops nothing
- **THEN** it pushes at once
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should push a rebased branch at once: its force push drops nothing`)

#### Scenario: A force push of an amended or restacked branch
- **WHEN** the user clicks the force push of an amended or restacked branch
- **THEN** it pushes on the click, in the push colour
  [@test] src/presenters/push-button.test.ts (`says what a force push of an amended or restacked branch replaces, and pushes it on the click in the push colour`)
