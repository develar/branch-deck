# Proposal

## Why

"auth has no open pull request: Create Pull Request… asks for its review", the reason of a disabled "Request Review…"
(`noOpenPullRequest`), reads as if Create asked for the review of a pull request that exists.

## What Changes

- The reason says what Create does: "auth has no open pull request: Create Pull Request… asks for a review as it opens
  one", and of a whole stack "Whole air stack has no open pull request: Create Pull Request of the Stack… asks for a
  review as it opens one".

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/next-step`: Facts without an answer: the words of Request Review… on a mirror without an open pull request.

## Impact

- `src/presenters/actions.ts`, its tests and `review.test.ts`, `docs/contributing/ux-writing.md`,
  `tests/e2e/next-step/facts-without-answer.spec.ts`.
