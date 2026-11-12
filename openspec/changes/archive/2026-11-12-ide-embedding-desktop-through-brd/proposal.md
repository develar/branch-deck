# Proposal

## Why

The copy of the contract of Air (2026-11-12) moved the badge, the progress and the notifications of the page to brd
alone, made the actions of the app file tools that only the app sees, and turned the close of a window of the page into
`x-air-app/closeView`; the page and `brd mcp` still called the host tools `air_set_badge`, `air_set_progress` and
`air_notify`, listed two tools, ran `sync` only for the origin `action`, and had no command palette and no close. A cancel,
the end of a session and two review tabs had no test, and nothing checked the copy against its schemas.

## What Changes

- The shim sends the badge, the progress, a notification (the browser's `Notification` too) and the close of its
  window as `bd_invoke` of their plugin calls of Tauri; it calls no host tool but `air_copy`.
- `tools/list` of `brd mcp` lists `sync` and `open_command_palette`, each only for the app; `sync` runs for the origins
  `action` and `click`, `open_command_palette` sends the main view the event of the palette; `plugin:window|close` is
  `x-air-app/closeView` of the instance of the call.
- The fake IDE of the E2E tests closes the frame of a `closeView` and runs an action of the title of the tool window.
- Tests: a cancelled read, the end of a session, two review tabs, Escape in a review tab, the actions of the title, and
  the check of the copy of the contract (`src/platform/air/contract.test.ts`, Ajv and `yaml` as dev dependencies).
