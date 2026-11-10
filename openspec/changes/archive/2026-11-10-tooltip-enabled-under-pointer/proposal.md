# Proposal

## Why

A tooltip that is disabled while it has nothing to say (the description of Approve in a review window until the
code-owner review of the pull request came, the reason of a button of the actions) opened nothing when it got its words
with the pointer resting on its trigger: Base UI listens on the trigger of a disabled root only once it is enabled,
after the pointer came, and the tooltip opened only at the next move. `review-requests.spec.ts` hovered Approve as the
changes showed, and 1 run in 40 under load hovered before the review came.

## What Changes

- The tooltip of `src/components/ui/tooltip.tsx` replays the hover of its trigger when it is enabled
  (`hoverAgain` of `src/lib/pointer-moves.ts`): each shown element of its control that the pointer moved over and has not
  left gets the enter and a move where the pointer is, after the commit that attaches the listeners of Base UI, and the
  tooltip opens after its delay. A trigger the pointer did not move over gets nothing, as content under a still pointer
  opens nothing. The replay of a lazy root shares the events (`enterAndMove`).
- An E2E test holds the code-owner review, rests the pointer on Approve, lets the delay run out with nothing to say, and
  sees the description open after the review came.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/branch-row`: Hover and click: a tooltip enabled under the pointer that moved over its trigger opens after its
  delay.

## Impact

- `src/components/ui/tooltip.tsx`, `src/lib/pointer-moves.ts`, `src/hooks/useLazyRoot.ts`;
  `docs/contributing/frontend.md` (Pitfalls).
