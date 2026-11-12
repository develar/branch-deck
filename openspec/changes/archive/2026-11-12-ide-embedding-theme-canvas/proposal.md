# Proposal

## Why

The page in the IDE took the second background of the host for its canvas and the first for its cards. The second is
the raised surface of the IDE, lighter than the first, so in the dark theme the canvas was lighter than the cards on it
and the page did not blend with its tool window.

## What Changes

- `src/platform/air/theme.ts`: the canvas is the first background of the host, the background of the place; the cards,
  the popups and the fields the second. The canvas is darker than the cards in both themes.
- The E2E test of the theme checks the canvas and the cards of both themes.
