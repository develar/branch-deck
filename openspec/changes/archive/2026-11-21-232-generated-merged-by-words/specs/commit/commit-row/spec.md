## ADDED Requirements

### Requirement: Conflict view: Resolved by Words marks a generated file

A generated file among the files the sync resolved by words (`WordResolutionSummary.generated`, also of a copy reused
by its mapping note) MUST be marked in "Resolved by Words" with why it is generated: the words merged its text, not its
generator, so the copy may not build or the file be stale; the Status cell of the branch says what to do
([Branch Row](../../branch/branch-row/spec.md#status-cell)).

#### Scenario: A lock file resolved by words
- **WHEN** the user opens "Resolved by Words" of a commit whose copy merged `Cargo.lock` by words
- **THEN** the merge of `Cargo.lock` is marked as a generated file ("Lock file"), merged by words, not by its generator
- **AND** a file a person writes in the same view has no mark
