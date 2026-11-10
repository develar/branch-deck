# Tasks

- [x] 1.1 The patch of `@base-ui/react@1.9.0`: `useHoverFloatingInteraction` takes the shared state of the hover
  without its dispose, which the trigger keeps.
- [x] 1.2 The E2E tests of a tooltip, a hover card and a submenu whose trigger the pointer came back to while the closed
  popup still exited, failing without the patch in Chromium and WebKit.
- [x] 1.3 The pitfall in `docs/contributing/frontend.md`.
