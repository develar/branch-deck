# Proposal

## Why

The tooltips of Base UI give their trigger no `aria-describedby`, and a closed tooltip's content is not in the page: a
screen reader heard none of the reasons of a disabled control ("Waits for the answer of Patronus" and the others), which
are the one explanation of its state. The app has 99 tooltips (`TooltipContent`) in 67 files, and wrappers that render
one per instance (`MenuItemButton`, `ReasonTooltip`).

## What Changes

- In `src/components/ui/tooltip.tsx`: the words of a tooltip are a string, its content or, beside content that is not
  one (keys, a relative time, a list of steps), a `description` of its own that the compiler asks for
  (`{children: string} | {children: ReactNode; description: string | false}`). The root finds them in its
  `TooltipContent` and renders them alone, no copy of the content (no second mount, no `id` or `data-testid` twice), in
  a hidden span at the end of the page, and the trigger is described by it (`aria-describedby`, after a description of
  its own). Not beside the trigger: there its text joined the text of the trigger's parent, and a lazy root took the
  element after the trigger for its trigger as it mounted. A lazy root not mounted yet describes nothing and adds no
  node: the large table keeps its 442 triggers at 11 descriptions (its roots that are not lazy), not one per trigger.
  `aria-description` was not taken: WebKit knows the attribute, but what VoiceOver of a WKWebView reads of it could not
  be checked, while `aria-describedby` is its help text.
- A trigger that wraps the control the focus reaches (the span of `MenuItemButton`, which takes the hover of a disabled
  button) gives the control the description too (`TooltipDescribedChild`). `<Tooltip disabled>` describes nothing, and
  `description={false}` marks a tooltip that only says the name of its trigger (four icon buttons, the old word of a
  substitution that the trigger says already); one that names its trigger and a key says the key (`Shortcut: D`).
- The slot of a lost start takes its description from its tooltip, its own `sr-only` one goes; its root mounts with it
  (`lazy={lost === null}`, a root whose `lazy` turns false mounts: `useLazyRoot`).

## Capabilities

### Modified Capabilities

- `branch/branch-row`: the words of a tooltip are the description of its trigger.

## Impact

- `src/components/ui/tooltip.tsx`, `src/hooks/useLazyRoot.ts`, the callers whose content is not a string (21 files:
  keys beside a text, a relative time, the steps of an update), four tooltips that only name their icon button, the
  HTML snapshots (`aria-describedby` of the triggers), `docs/contributing/frontend.md`.
