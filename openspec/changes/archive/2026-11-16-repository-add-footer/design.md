# Design

## Context

The picker uses Base UI Select, whose repository options live in a listbox. The existing browseRepository action already handles validation, selection and recent repositories.

## Goals / Non-Goals

Make opening another repository discoverable inside the picker without changing its existing browse behavior or repository grouping.

## Decisions

Add an optional SelectContent footer outside Select.List. Only the list scrolls when the footer exists. Use a real ghost button in the xs size with a plus icon; close the picker before invoking browseRepository. Hide the footer on hosts without a folder dialog. Preserve the empty heading, explanatory text and accessible description, updating the instruction to name the footer action.

The persistent presentation decisions are recorded in docs/design/ui-theme.md.

## Risks / Trade-offs

The separate button must remain reachable by Tab in empty and populated popups. Verify Enter, Space, Escape and focus return in Chromium and WebKit. Preserve the existing layout for selects without a footer.
