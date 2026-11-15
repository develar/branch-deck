# Proposal

## Why

The review of 69cb668a8 found two faults of the status that took the place of the Safe Push button of a lost start, a
focusable `span`: it had no role, and its reason, which only its tooltip said, was no description of it (WCAG 4.1.2);
and as a start under the focus of the keyboard turned lost, the button left for the span and the focus fell to the page,
the keys of the row with it.

## What Changes

- The slot keeps one element in every state: the Safe Push button. Of a lost start it is disabled and not dimmed (a
  class of the chain of `Button`, `aria-disabled:not-aria-busy:opacity-100`, which `cn` merges with its dimming), with
  its ? icon and `data-status="unknown"`, and its reason is its description (`aria-describedby` to a `sr-only` text: the
  tooltips of Base UI give none).
- Tests: the slot of a lost start is a button named "Start Unknown" with its reason as description, and the focus stays
  on the slot as a start `brd` runs turns lost, in both engines (`foreign-operation.spec.ts` joins the WebKit runs for its
  `@webkit` test). On the status span the first fails in Chromium, the second in both engines.

## Capabilities

### Modified Capabilities

- `branch/patronus`: the slot of a lost start is its one button.

## Impact

- `src/components/branch/SafePushButton.tsx`, `tests/e2e/operations/foreign-operation.spec.ts`,
  `playwright.webkit.config.ts`.
