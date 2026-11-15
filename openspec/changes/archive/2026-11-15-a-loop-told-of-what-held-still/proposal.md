# Proposal

## Why

The review of 4c60618c reproduced the race its attribution of a loop left open: at a zoom of the app other than 100%
the arithmetic proposes elements laid out as reported, and one that a task resized (rows that shrink, a frame callback,
a transition) or a remount took out of the page between the loop and the next update was laid out by the engine then
otherwise than reported, a false error of the page ("left div pending") in both engines.

## What Changes

- A candidate keeps its layout at the loop; one laid out otherwise in the update of the probe's report (`sameBoxOf`), or
  out of the page then, is not named. A pending element that changed again so is no error: rarer and safer than a
  false one.
- The docs say what the root the probe observes does to that update: the next iteration from the root's depth, a loop of
  a library in every frame one in every other while candidates wait.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: an element that changed or left the page after a loop is not named.

## Impact

- `src/lib/observe-size.ts`, `docs/contributing/frontend.md`, `tests/e2e/resize-loops/resize-loops.spec.ts`.
