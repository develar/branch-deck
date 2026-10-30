# Writing E2E Tests

**Status:** Active  
**Updated:** 2026-10-30  
**Tags:** [e2e, playwright, tests]

## Summary

How to write a Playwright test of the page; the infrastructure (mock IPC, templates, sub-windows, snapshots) is
[E2E Testing Architecture](../design/e2e-testing.md). [AGENTS.md](../../AGENTS.md) ("E2E Tests") keeps the
rules in one line each and links here.

## Rules

- **Invoke every surface**: an action reachable by a key on the row, a menu item, a footer button, Enter in a tree, a
  toast action gets one step per surface that presses or clicks it and asserts the effect. A check of a menu item's
  label and key hint (`toHaveText("Review ChangesD")`) is not a test of the action: the commit "Review Changes" item
  shipped dead behind one. Labels and hints belong to the HTML snapshots.
- **Assert the effect**: what the user sees (text, focus, `aria-*`, a closed dialog) or what reached the backend (the
  command's result on the next screen, `getOpenedSubWindows`, `getEmittedEvents`, the counts of a fixture route), not
  the internal state of a store.
- **No vacuous negatives**: "nothing happened" (`toHaveCount(0)`, `toBeHidden()`) passes when the action never ran or
  the element was never there. Anchor it: assert the thing existed first, or assert on the recorded list
  (`getOpenedSubWindows` is `[]` after the press), or follow it with a positive step the same press would have blocked.
- **Wait for the state, never for time**: wait for the element or the event a state comes with (`runAndWaitForSync`,
  `data-sync-count`, the patch of a version a command answered by `data-view-version`, `waitForViewVersion`, an
  `aria-busy` that clears); an app timer runs with `page.clock.fastForward`, a timer of the session (a poll, a
  retry, the sweep) with `repo.advanceBackendClock(ms)`, a debounced
  settings change with `waitForStorePersistence` (the key as the writer answered it, as a reload reads it). No
  `waitForTimeout`: under load
  (several checkouts running e2e and cargo) any fixed wait is too short; Oxlint refuses it in `tests/`, and a
  `setTimeout` in a spec, a fixture or a helper. A box or a point measured for a press waits for the facts that move
  it: Patronus answers after the sync and moves the header row (`waitForHeaderFacts`). A width or a flag that the
  `ResizeObserver` of the branch columns writes (`--bd-track-*`, `data-wide`), and every box it moves, is read after
  `waitForBranchLayout` (the end of the sync, the facts of the steps, the reports of that frame), never at the first
  row `syncAndWaitForBranches` returns at, and after a resize once `resizeReportsDelivered`. A chain of commands gets a
  wait per step, each of which takes seconds under load: a rewrite and its sync `runAndWaitForSync`, Update Project
  `runAndWaitForUpdate`, a start `runAndWaitForAnswer(page, "execute", …)` (the end of its stream), the facts of a next step
  `waitForStepFacts`, any other step `nextAnswer(page, command)` before the assertion that reads its answer. The press
  goes into the wait (`runAndWaitForAnswer(page, command, () => button.click())`) or comes after `nextAnswer`: a wait
  that starts after the press misses a quick answer and holds the test until its timeout.
- **Close a read toast** before acting where it lies (`closeToast` of `tests/e2e/helpers/toast-helpers.ts`): the toasts
  lie above menus and dialogs at the bottom right, and a click or hover under one waits until it times out (5 s, 10 s
  with an action); six tests spent 7-9 s each this way, which the timing summary names as stalled actions.
- **A few starts per test**: the 30 s timeout of a test holds its steps under load, where a click costs 0.5 s and a
  start of a robot or a push 1-5 s; a test of every surface of one start gives each a few starts, and the next surfaces
  are tests of their own. A test that takes more than 20 s on the Studio beside 40 `yes` workers is split; the step
  profile of the run says where its time went (`node scripts/e2e-steps.mjs <part of its title> [--kinds]`,
  [A test of many starts](../design/e2e-testing.md#performance-considerations)).
- **Locators**: role and accessible name first (`getByRole("menuitem", { name: "Review Changes" })`), `getByTestId` for
  what has no role or name; never CSS classes or DOM shape, which the theme and the React Compiler change.
- **Through the user's path**: open, select and press like the user (hover, focus, right-click, keys); call a command
  directly only to set up a state the user path cannot reach in a test. An error path fails the command with
  `routeCommandError`, not by breaking the repository.
- **Remote answers come from the fakes**: Patronus, GitHub and YouTrack are fakes of each test repository behind the
  app's real services; a test sets their fixture (`setPatronus`, `setPullRequests`, `setYouTrack`, …) and asserts what
  the fake got (`getPatronusLog`, `getGitHubLog`, `getYouTrack`). The page before an answer holds it at the fake
  (`holdRemoteAnswers`, waited for with `getArrivedAnswers`), so the request has reached the backend. A page answer of a write (`routeCommand`, a
  fulfilling `page.route`) fails `command_registry_test.rs`, and each read's page answers are pinned and only go down
  ([Remote services](../design/e2e-testing.md#remote-services)).
- **Linked**: every requirement of a spec names its test (a test link with the exact test title, the link a line of its
  own); a renamed, split or deleted test updates its links in the same commit, which `pnpm lint:specs` checks.
- **Snapshots**: `captureHtmlSnapshot` for structure; `pnpm e2e-up` rewrites them, and the diff is reviewed like code: a
  changed snapshot can hide a regression.

## Pitfalls

- **One `pnpm e2e` per checkout**, even on two ports: a global setup that finds the frontend changed runs `pnpm build`,
  which empties `dist/` under the other run (timeouts in page setup, not test failures).
- **`CI=1` does not speed up E2E**: it means 1 worker, 2 retries and no reuse of a running server
  (`playwright.config.ts`). A test that fails only under parallel load is a race: fix the wait, don't serialize.
- **A timeout is read in the trace of its action** before it is called load: the step profile says where the time of
  the passing runs went, and the action that never ended in a failed one says why (`--trace retain-on-failure`; the
  tracing slows the frames and raises the rate of a race, 20 of 40 runs where 1 of 30 failed without it). "… intercepts
  pointer events" for the whole timeout is a popup the page left open, a bug of the page: stack-island.spec.ts waited
  30 s on Sync under the backdrop of a right-click menu whose focus the closing menu of a "⋯" had taken
  ([A stall under load](../design/e2e-testing.md#performance-considerations)).
- **Typing into a Base UI dialog in E2E**: wait for `toBeFocused()` first; `press`/`type` on an unfocused input puts the
  caret at 0 and the text lands at the start. A Base UI menu takes its focus a frame after it shows (`enqueueFocus`), and
  a closing one gives it back in a microtask after its popup goes: a key for an open menu waits for the focus in it
  (`branch-row-layout.spec.ts`, the right-click within the exit of a "⋯" menu), never only for the popup's visibility.
- **Throwaway Playwright specs** (screenshots, debugging) go into `shots.local/` of the worktree (`*.local` is
  ignored), with a config there that spreads `../playwright.config` and sets `testDir: "."`, `globalSetup:
  "../tests/e2e/setup/global-setup.ts"` and `webServer[].cwd: ".."`; run `npx playwright test --config
  shots.local/shots.config.ts`. Delete them before a commit: `pnpm lint:frontend` lints them too. The README images
  are no throwaway: `pnpm screenshots:readme` writes them.
