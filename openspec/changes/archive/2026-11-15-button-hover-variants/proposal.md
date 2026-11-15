# Proposal

## Why

00ae41495 kept a disabled button that keeps the focus from its hover and its press by a filter of the class string in
`Button`, and dimmed it by an `opacity-50` appended there. The review asked for the rule in the CSS instead: a variant of
the disabled state (`aria-disabled:hover:`) cannot give the button back its background at rest, which only its caller
knows (`bg-primary/10` of `ghostButtonColorClass`), so the place for it is the `hover` and `active` variants themselves.
And the Safe Push slot of a start whose answer was lost, a disabled button, came out dimmed: it read as an action that is
off, beside the real actions of the lost start.

## What Changes

- `src/index.css` redefines Tailwind's `hover` (in `@media (hover: hover)`) and `active` with
  `:where(:not([aria-disabled="true"]))`: no control that is `aria-disabled="true"` takes a hover or a press, its own nor
  its caller's, and `:where` leaves the specificity of every utility as it was. `group-hover:` and `group-active:` are
  made of them, so the children of such a control take none either. The selector names no button: the triggers of a
  tooltip or a popover give a `Button` their own `data-slot`.
- `Button` dims by `aria-disabled:not-aria-busy:opacity-50` in its variants; the filter and the appended `opacity-50` go.
- "Start Unknown" of a lost start is a status of its slot with its ? icon, no button and not dimmed, its reason on the
  hover and on the focus of the keyboard.
- `holderName` says the holder shown or spoken, one function.

## Capabilities

### Modified Capabilities

- `branch/patronus`: the slot of a lost start is a status.
- `branch/move-commits`: a busy button keeps the background at rest its caller gives.

## Impact

- `src/index.css`, `src/components/ui/button.tsx`, `src/components/branch/SafePushButton.tsx`,
  `src/presenters/lock-wait.ts`, `tests/e2e/repository/repository-lock.spec.ts`,
  `tests/e2e/operations/foreign-operation.spec.ts`, the HTML snapshots (the classes of `Button`),
  `docs/contributing/frontend.md`.
