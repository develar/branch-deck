# Proposal

## Why

The page asks the index of the renames of one subject at a time (`src/stores/file-diffs.ts`, `askIndex`). A question
whose subject no view shows gives way, and ends later, when the backend has killed its query. Its end was told from
the end of the running question by the key of its subject. A subject shown again before its first question ended is
asked again under the same key, so the end of the killed question marked the subject as answered with names left
asked and freed the turn while the second question still ran: the next subject shown started a question that killed
it, and the names of the subject stayed without a proof, each site a stop, until a view showed the subject anew.

## What Changes

- Each question has a number (`question`); the end of a question that is not the last one started says nothing of
  what runs and marks no subject.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: the end of a question that gave way does not end the question that took its place.

## Impact

- `src/stores/file-diffs.ts`, `src/stores/file-diffs.test.ts`.
