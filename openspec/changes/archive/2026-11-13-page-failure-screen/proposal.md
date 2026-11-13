# Proposal

## Why

A user started the release app and saw a white window with nothing in it. When a render throws an error that nothing
catches, React 19 unmounts the whole page. The backend then shows the window by the deadline of the stage that the
page reached, over a blank canvas. The error goes only to the console of the webview, and the release app has no
inspector for it, so the log of the app does not say why. An error of a module as it loads, of a handler, or of a
promise that nobody awaits does not reach the log either.

## What Changes

- The root of the page catches an error of a render that nothing caught (`onUncaughtError` of `createRoot`). It logs
  the error with its stack and its component stack, and it renders `PageFailure` instead of the page: "Cannot show the
  window", the message of the error, Reload and Close Window. Then it says the shell and the content of the window, so
  the window shows the failure at once. A page that did not load shows the same screen, titled "Cannot open the page of
  the window".
- `src/lib/page-errors.ts`, the first import of the entry, sends the `error` and `unhandledrejection` events of the
  window to the log of the app.
- E2E: the mock of `plugin:log` records the lines of the error level (`__e2eErrorLogs`). `setupRepo` takes
  `waitForPage: false` for a page that never mounts.
- Capability: `repository/title-bar`.
