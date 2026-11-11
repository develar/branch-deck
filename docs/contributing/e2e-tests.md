# Writing E2E Tests

**Status:** Active  
**Updated:** 2026-11-10  
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
  retry, the sweep) with `moveClockUntil(repo, ms, done)` (`repo.advanceBackendClock(ms)` until the state), a debounced
  settings change with `waitForStorePersistence` (the key as the writer answered it, as a reload reads it). No
  `waitForTimeout`: under load
  (several checkouts running e2e and cargo) any fixed wait is too short; Oxlint refuses it in `tests/`, and a
  `setTimeout` in a spec, a fixture or a helper. A box or a point measured for a press waits for the facts that move
  it: Patronus answers after the sync and moves the header row (`waitForHeaderFacts`). A width or a flag that the
  `ResizeObserver` of the branch columns writes (`--bd-track-*`, `data-wide`), and every box it moves, is read after
  `waitForBranchLayout(page, syncs)` (the end of the sync after the `finishedSyncCount` read before it started, the
  facts of the steps, the reports of that frame), never at the first
  row `syncAndWaitForBranches` returns at, and after a resize once `resizeReportsDelivered`. A chain of commands gets a
  wait per step, each of which takes seconds under load: a rewrite and its sync `runAndWaitForSync`, Update Project
  `runAndWaitForUpdate`, a start `runAndWaitForAnswer(page, "execute", …)` (the end of its stream), the facts of a next step
  `waitForStepFacts`, any other step `nextAnswer(page, command)` before the assertion that reads its answer (a start
  that a review window asks for runs in the main window, `askReviewRun`: its answer is the main page's; a review
  window opens on the answers of its plan and its diffs, `openPlanWindow`). The press
  goes into the wait (`runAndWaitForAnswer(page, command, () => button.click())`) or comes after `nextAnswer`: a wait
  that starts after the press misses a quick answer and holds the test until its timeout. What an operation wrote (the
  remote's branches, a start, a restart, a cancel or a GitHub write at a fake) is read once after its answer, never
  polled: `expectPushedToRemote` and `expectPatronusStarts` read once, `confirmForcePush` and `pushBranchFromRow` wait
  for the answer of their push, and `pnpm lint:arch` (`e2e-remote-poll`) refuses an `expect.poll` of the remote or of
  such a write; a write of an operation that waits for the test (a start from a pull request reads the reply of the bot
  on the clock of the session) is an entry of its `ALLOWLIST` with its count. A 5 s poll after the press failed where
  the push, the update and the sync of one landing took longer under load (`merge-pull-request.spec.ts`,
  `push-to-master.spec.ts`).
- **Close a read toast** before acting where it lies (`closeToast` of `tests/e2e/helpers/toast-helpers.ts`): the toasts
  lie above menus and dialogs at the bottom right, and a click or hover under one waits until it times out (5 s, 10 s
  with an action); six tests spent 7-9 s each this way, which the timing summary names as stalled actions.
- **A few starts per test**: the 30 s timeout of a test holds its steps under load, where a click costs 0.5 s and a
  start of a robot or a push 1-5 s; a test of every surface of one start gives each a few starts, and the next surfaces
  are tests of their own. A test that takes more than 20 s on the Studio beside 40 `yes` workers is split; the step
  profile of the run says where its time went (`node scripts/e2e-steps.mjs <part of its title> [--kinds]`,
  [A test of many starts](../design/e2e-testing.md#performance-considerations)). What is not the subject costs no
  start: a commit the window needs goes in before the sync that opens it, a second recent project before the first
  sync (`addRecentProject` reloads the page), and the boxes of one measurement are asked at once (`Promise.all`), each
  a quarter of a second under load; a sweep of widths is two tests of half the widths (`title-bar.spec.ts`).
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

- **A popup closes by its state**: `expectClosed(popup)` of `tests/e2e/helpers/wait-helpers.ts`, never a wait for it to
  leave the page. Base UI marks a closing popup `data-closed` (a toast `data-ending-style`) at once, keeps it mounted
  while its exit animation runs, then unmounts it and gives the focus back; the exit ends only as the window paints, and
  a loaded machine paints no frame of a window for seconds. `expectClosed` waits for that state, ends the exit as the
  frames would (`finish()` of its animations) and asserts nothing of the popup shown, so a focus assertion after it
  holds too. The open tooltip is `openTooltip(page, options)`: a closing one keeps its role, and a plain
  `getByRole("tooltip")` finds it beside the new one. The `expect` of `fixtures/base-test.ts` refuses a removal
  assertion (`toHaveCount(0)`, `toBeHidden()`, `not.toBeVisible()`, `not.toBeAttached()`) that starts on an element of
  a popup whose exit runs, and names the exit in the error of one that timed out on it. The plain removal stays where
  the removal is the effect (a row a sync deletes, an item gone from an open menu) and in a test of a popup reopened
  during its exit, which ends the exit itself.
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
- **A wait for an exit hides under a quick machine**: an exit of 100 ms ends before an assertion's first retry, so a test
  that waits for one passes here and fails on a loaded machine. `E2E_EXIT_MS=8000 pnpm e2e <spec>` lengthens every exit
  (`[data-closed]`, `[data-ending-style]`) past an assertion's time: such a test fails at once, with the guard of
  `expect` or a timeout of the removal, the focus or a strict locator (on 2026-11-10, 211 of 984 tests before the fix).
  Run it on a new test of a popup.
- **Typing into a Base UI dialog in E2E**: wait for `toBeFocused()` first; `press`/`type` on an unfocused input puts the
  caret at 0 and the text lands at the start. A Base UI menu takes its focus a frame after it shows (`enqueueFocus`), and
  a closing one gives it back in a microtask after its popup goes: a key for an open menu waits for the focus in it
  (`branch-row-layout.spec.ts`, the right-click within the exit of a "⋯" menu), never only for the popup's visibility.
  A step that must land within an exit holds it: a style that lengthens the exit, then `finish()` of the popup's
  `getAnimations()` once the step is done; never an assertion that a quick step happened to land in it.
- **A command the page answers from a cache is asked once**: the texts of the blobs (`get_blob_texts`) are kept by
  blob id (`src/stores/blob-texts.ts`), and a read-ahead asks for them before the file is shown, so a hold installed
  after the window opened may never see the call a test waits for (1 run in 12 of `review-plan.spec.ts`). Hold it on
  the context before the window opens (`holdCommandCalls(page, command, { scope: "context" })`) and wait for a held
  call that asks for what the test holds unpainted, never for a call that may already have gone out.
- **Answers come in another order than their work** on a loaded machine: a command writes, the write tells the windows
  to read again, and that read can answer before the command does. Activity showed a rewrite twice, each with its Undo
  (2 of 370 runs of `move-commits.spec.ts`): the read brought the entry, then the record added it again. A test of
  such an order holds only the answer (`holdNextAnswer(page, command)` of wait-helpers.ts: the command runs at once, its
  answer waits for the release), never the call (`holdCommand`, which holds the work too).
- **A move of the session clock after a call the fake counted** comes before the answer ended in the session: a fake
  counts a call as it answers, the session applies the answer seconds later under load and counts the age of the list
  from that end, past the move, or takes the focus of the move into the list in flight. One move and one focus then ask
  nothing (`review-requests.spec.ts`, 2 of 40 under load): move, and focus, until the state shows (`moveClockUntil` of
  sync-helpers.ts, with `focus` and `asked`; `focusPastReviewRequestsAge` for the review requests). A loop that moves the
  clock is no `expect.poll` of 5 s either: the answers it waits for take seconds each under load. Likewise a toast or a
  row after an operation is checked once it answered (`runAndWaitForAnswer(page, "execute", …)` before
  `expectPushedToast`), or once the state its sync shows is there (`releaseTheSyncOfTheUnstack`), and the facts of the
  steps are waited for as long as the test lets them take (`waitForStepFacts`).
- **A menu opened right after the first sync changes as it shows**: the facts that sync asked for (the permission of
  Patronus, the route) come after it, in a patch of every next step, and the branch menu loses its Safe Push item at
  the top as Patronus answers. The items below move up a row under the pointer, the item that was under it hears the
  pointer leave, and Base UI cancels the open of its submenu or tooltip, as it should: the item left the pointer
  (stacked-branches.spec.ts, 3 runs in 150 under load). A test opens such a menu once no step says "Checking…"
  (`waitForStepFacts(page.getByTestId("next-step-slot"))`), never with an answer held. A submenu opens by `openSubmenu`
  of selection-helpers.ts, which asserts the open and says when the menu moved its item from under the pointer;
  `hover()` of an item with a submenu is refused (`refuseSubmenuHovers` of fixtures/base-test.ts), and
  `hoverSubmenuTrigger` is the plain move for a test that holds the delay.
- **Throwaway Playwright specs** (screenshots, debugging) go into `shots.local/` of the worktree (`*.local` is
  ignored), with a config there that spreads `../playwright.config` and sets `testDir: "."`, `globalSetup:
  "../tests/e2e/setup/global-setup.ts"` and `webServer[].cwd: ".."`; run `npx playwright test --config
  shots.local/shots.config.ts`. Delete them before a commit: `pnpm lint:frontend` lints them too. The README images
  are no throwaway: `pnpm screenshots:readme` writes them.
