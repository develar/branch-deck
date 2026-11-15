# Proposal

## Why

On the first sync of a repository the toolbar got Update and "Last synced just now" at once. The toolbar stands at its
end, and the time of the last sync stands right of Activity, before Sync: it came with its room and moved Activity and
the landing indicator 171 px to the left from under a still pointer (1239 → 1068 at 1400 px), whose tooltip closed.
Update comes on the left of the group and moved nothing.

## What Changes

- The time of the last sync keeps the room of its widest text from the selection of a repository on, empty before the
  first sync, as the landing indicator keeps its place: the time that comes fills it.
- An E2E test in both engines holds the pointer on Activity through the first sync, started from the menu, and finds
  Activity and the landing indicator in their boxes, the tooltip open; on the header before, Activity moved from
  x 1119 to 948 at 1280 px.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the first sync moves no control of the toolbar.

## Impact

- `src/components/repository/ConfigurationHeader.tsx`, `tests/e2e/repository/header.spec.ts`,
  `playwright.webkit.config.ts`, two HTML snapshots of a repository not synced yet, `docs/contributing/frontend.md`.
