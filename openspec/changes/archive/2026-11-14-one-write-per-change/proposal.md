# Proposal

## Why

The E2E test "should save valid typing when the window closes, and nothing else" failed under load with two writes of
one change of the GitHub repository: two writes of the git config and two `repository_config_changed`. The blur of the
field wrote the change; its answer set the new value and cleared the typing, which the page renders only in a task of
its own; a close of the window between that answer and that render ran the save of the render before, which still
held the typing, and wrote it again. A close during the write wrote nothing but did not wait for it either.

## What Changes

- `FieldWrites` (`src/lib/field-writes.ts`), the writes of one field: a save while a write runs waits for it, and a
  save of the value a write took writes nothing until the field dropped its typing. `useConfigDraft` saves through it.
- Capability `repository/settings`: "Scope: A window closed with valid typing saves it first" says that one change is
  one write, with the scenario of a close as the write of a blur answers.
