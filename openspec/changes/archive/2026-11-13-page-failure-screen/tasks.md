# Tasks

- [x] 1.1 `PageFailure` and the `onUncaughtError` of the root in `src/main.tsx`; the failure of a load shows it too.
- [x] 1.2 `src/lib/page-errors.ts`: the uncaught errors and rejections of the window reach the log of the app.
- [x] 1.3 The E2E test of the first frame: a render that throws shows the failure, logs it, and Close Window and Reload
  act; the mock of `plugin:log` records the errors, `setupRepo` takes `waitForPage`.
