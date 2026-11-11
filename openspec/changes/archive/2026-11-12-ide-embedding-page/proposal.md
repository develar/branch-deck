## Why

The spec `repository/ide-embedding` names the page of the IDE, its shim and its theme, and no code had them yet. The
page of the desktop app must run in a frame of the IDE unchanged: the commands, the channels, the patches of the session
and the events reach `brd mcp` over the frame link of `x-air-app`.

## What Changes

- `src/platform/tauri-page.ts`: the internals of Tauri a page runs on without the shell, shared by the page of the IDE
  and the page mock of the E2E tests (bundled by `tests/e2e/setup/tauri-mock-script.ts`).
- `src/platform/air/`: the frame link, the shim (`bd_invoke`, the call events, the mirror, the events, the plugins as
  the IDE offers them), the mirror of the view turned back into the patches of the session, the theme of the host
  context over the semantic tokens, and `bd_bootstrap`.
- `src/air-main.ts` and `vite build --mode air` into `dist-air/`, multi-file.
- A fake IDE for the E2E tests (`tests/e2e/air-host/`), with the `AppBridge` of MCP Apps.

## Capabilities

### Modified Capabilities

- `repository/ide-embedding`: the Page requirements say how the shim maps each call and what the mirror is to the page,
  with their tests.

## Impact

- `src/platform/`, `src/air-main.ts`, `vite.config.ts`, `tests/e2e/setup/`, `tests/e2e/air-host/`,
  `docs/design/ide-embedding.md`, `docs/design/e2e-testing.md`.
