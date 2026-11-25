# Writing E2E Tests

**Status:** Active  
**Updated:** 2026-11-15  
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
  it: Patronus answers after the sync and moves the header row (`waitForHeaderFacts`); the text font reflows the page
  once it loaded, which the setup and the open of a sub-window wait for (`fontsLoaded`: the steps of the welcome card
  moved 20 px under a box measured before it). A width or a flag that the
  `ResizeObserver` of the branch columns writes (`--bd-track-*`, `data-wide`), and every box it moves, is read after
  `waitForBranchLayout(page, syncs)` (the end of the sync after the `finishedSyncCount` read before it started, the
  facts of the steps, the reports of that frame), never at the first
  row `syncAndWaitForBranches` returns at, and after a resize once `resizeReportsDelivered`. A chain of commands gets a
  wait per step, each of which takes seconds under load: a rewrite and its sync `runAndWaitForSync`, Update Project
  `runAndWaitForUpdate`, a start `runAndWaitForAnswer(page, "execute", …)` (the end of its stream), the facts of a next step
  `waitForStepFacts`, an operation the session admitted behind a held sync, a lock or an update `intentAnswer(page,
  kind)` asked before the release (its whole chain, the sync, the push, the start or the creation, runs after it, and
  the `execute` of the update it waited behind answers meanwhile: a look at the chip of the start failed in 9 of 200
  runs of `patronus-actions.spec.ts` beside busy loops at load 170 to 270), the step an update stands at while a hold
  keeps it `pressUpdateUntilStep` (its fetch and check before the wait for the lock outlast a look under load), any
  other step `nextAnswer(page, command)` before the assertion that reads its answer (a start
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
- **A list the page shows is polled**, never read once: `await expect.poll(() => branchTree(page)).toEqual([…])`, not
  `expect(await branchTree(page))`. The page shows a list (the rows, the units of the Landing overview, the items of a
  menu) once the facts it is of came, seconds after the last state the test waited for under load: the Landing overview
  of `land-together.spec.ts`, read as it opened, was empty in 20 of 20 runs beside 14 busy loops. `pnpm lint:arch`
  (`e2e-list-read-once`) refuses an `expect(await …)` of an `evaluateAll`, `allTextContents` or `allInnerTexts`, or of
  a function of the spec or of `tests/e2e/helpers` that reads one.
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
- **The rows of a diff** are drawn by the painter outside React, without a role or a name: find them by the helpers of
  `tests/e2e/helpers/diff-helpers.ts`, which read the test ids and the data attributes the painter writes, never by a
  `dv-` class. A row is `diffRows(scope, kind)` (`diff-row` with `data-kind`: `context`, `added`, `deleted`,
  `changed`, `inline`, `once`, `merge`, `fold`, `note`), the cell of a side `diffSides(scope, side)` (`diff-side` with
  `data-side`, `data-cell`, `data-tint`), the code of a line `DIFF_CODE` (`diff-code`), its number
  `diffLineNumbers` (`diff-line-number`); a part of a line by what it is: `changedParts` (`data-change`),
  of them `editedParts` the ranges read as a change and `renamedNames` the names a change renamed
  (`data-change="3"`, `RENAMED_NAME`, with what the name was as its `title`; the part the name starts with,
  `RENAMED_NAME_START`, `data-name-start`; the old name an Inline row strikes through before it is a deleted part
  with `data-renamed`),
  `deletedParts` of an Inline row (`data-inline-deleted`), `findMarks(scope, "current")` (`data-hit`); and
  `visualLines` of Soft Wraps, `horizontalBars` (`expectBarReachesEndOf(row, bar)`: the bar of a column scrolls as
  far as a row wider than the column), `foldButtons`, the underline of a link (`diff-link`). The marks of a
  diff: `remarks(scope, kind)` are the words of the notes of its runs (`diff-remark`, a button with `aria-expanded`
  where Inline shows the run once; `renamed` the one note of a rename, "Renamed total → sum · 3 sites", the renames
  it counts beside the ones it words in `diff-remark-more`, whose `title` lists them), `quietSides(scope, side)` the cells of the lines played down
  (`data-tint="quiet"`), `remarkHits(scope)` the hits of the find a note names (`diff-remark-hits`); of a move,
  `moveNotes(scope, end)` are the rows of the notes at its ends (`data-move`: `source`, `destination`; `data-landed`
  on the one a jump landed on: `expectLandedOn(row)`, `expectLandedAloneOn(scope, row)`), `moveTargets(scope)` the buttons of the places they name (`diff-move-target`; the
  one the key of the moves takes has `aria-keyshortcuts`: `keyedMoveTargets(scope)`), `movePlaces(scope)` the places
  that are plain words because no view shows their file (`diff-move-place`),
  `movedSides(scope, end, side)` the cells of its lines (`data-cell="moved"`), `acrossSides(scope, end, side)` those
  of a move between two files (`data-across`); Back and Forward of the diffs are `diffHistoryKeys(browserName)`. Of
  what the files of a commit say of each other: the file a note names as the one that declares a name is
  `diff-rename-declared` (a button where a view shows that file), what the index proved `diff-rename-proof`
  (`data-proof`). The index of the repository answers from the fixture of the test server:
  `answerStoreRename(repo.path)` of `marks-helpers.ts` writes the answer for the rename of the `commit_marks`
  template before a view asks (`{ outside: true }`: a use the commit left alone). Marks that come after the rows of
  a file: `lateSubjectMarks(page, path)` makes the diff of that file say nothing of its subject, and
  `holdCommand(page, "get_index_marks")` holds the answer that brings its marks, until the test read the rows
  before it. A quiet run of a renamed name is the local of the `commit_marks` template (`total` → `sum`); a run of a
  renamed name that is a label is a change the test builds, with a line of the function of the name that really
  changed (`commitTally` of `renamed-names.spec.ts`). The painter adds a row to the page as a
  scroll brings it near, so the order of the page is the order of the paints: read the code of rows in the order of
  their files by `codeOf(rows)` of `marks-helpers.ts` (`data-file`, `data-row`), never by the order of a locator, and
  find one row among several by its text (`rows.filter({ hasText })`), never by `nth`. A row is found by what it
  holds with a locator of the page (`filter({ has: remarks(page, "renamed") })`): one scoped to a window matches
  nothing inside a row, and a count of zero then passes. The scopes of a file come after its diff and paint its
  rows anew, which drops a selection: a test that selects in them waits for `expectScopesShown(code, path)`. The
  painter also replaces the element of a row as its tokens arrive: what a test reads of a row and its neighbours it
  reads in one step of the page (`evaluateAll`, as `codeOf`), never with `evaluate` on a row found a step before,
  whose element may have left the page. For the same reason a scroll asked of a row (`scrollIntoView` in the page)
  may be asked of an element that just left it and move nothing: ask until the row stands where it should
  (`toPass`), measure a row by a box that is there (`boundingBox` is null for a moment), and before a test reads
  what a view keeps across a new layout, let the scroll event reach the views (`resizeReportsDelivered(page)`: a
  rendering update). The painter draws
  only the rows around the view: `scrollDiffsTo(scope, target)` scrolls a view in the scroller of its parent (the
  preview of a commit) until a row far below is in view, and never past it once it is painted: a scroll past a row
  that a late paint brought leaves it and its neighbours at the edge of the rows painted, so a read of the row above
  it finds none (`kindsAboveSites` of `renamed-names.spec.ts`, under load). A part is an
  element per token of the syntax: read a text over several by `joinedText`. Code that runs in the page takes the
  same selectors (`diffRowSelector`, `diffSideSelector`). The files of a review move down once its overview (the
  heading of a commit shown alone) came and was measured, after the files show: a test that reads the place of a row
  or of a word right after the window opens waits for `expectOverviewLaidOut(code)` first, or it points at where the
  row was (16 of 40 runs of such a test alone on the Studio).
- **Through the user's path**: open, select and press like the user (hover, focus, right-click, keys); call a command
  directly only to set up a state the user path cannot reach in a test. An error path fails the command with
  `routeCommandError`, not by breaking the repository.
- **Errors are declared**: a test fails on a line the app logs at the error level (`logError`, `notifyError`, an
  uncaught error or a ResizeObserver loop of a page) and on a `console.error` of a page that logs nothing with it,
  unless it declares it, `expectErrorLog(page, /^Cannot …: …$/)` (`<title>: <error>`), and on a declaration that
  matched nothing. The page tells an error the log of the app (`logError`), never its console alone, which the user's
  `BranchDeck.log` lacks. A helper that makes the failure declares it with its call: `routeCommandError` the line that
  ends in its message (`unlogged` says why the page logs none, a precondition that is no failure); a refusal that is
  only the means of a test may come, `allowErrorLog(page, /…/, because)`, a reason required (`refuseIslandStarts`: no
  robot holds the next start). There is no list of errors allowed everywhere. A test that ends while the operation it
  started still runs misses its error, or logs one after it: wait for the effect, the toast of the refusal
  (`tools-settings.spec.ts`, "Sign In Again"). The repository a test switches to is a real one (`otherRepository`); a
  path git cannot read only where the test checks that state, its line declared ([Logging](../design/e2e-testing.md)).
  A ResizeObserver loop of a library's observers is a warning of the page, which fails no test; 3 of them within 5 s,
  a loop that left an element of an observer of ours pending, and a cycle of ours are error lines
  (docs/contributing/frontend.md, "A flushSync in the delivery of a ResizeObserver"). The page a
  test moves to another window's page (`openSubWindowPage`) tells it leaves, and only a request of it that fails then
  (WebKit's "Load failed" and "Fetch API cannot load … due to access control checks") counts no error. Base-test reads
  an uncaught error of WebKit by its whole line (`toldText`): Playwright splits an error line of WebKit's console at its
  first colon, so the "access control checks" of a leaving page was the message "/localhost:…" alone and counted
  (quiet-changes.spec.ts and moved-lines.spec.ts in loaded runs, 2026-11-15). It tells a request left behind by the
  first line of the error (`isLeftBehind` of tests/e2e/setup/network-failure.ts): the log of the page puts the stack of
  the error on the lines after it, and an unhandled "Load failed" with its stack counted (moved-lines.spec.ts in WebKit,
  2026-11-16). A network
  failure that counts fails the test with the network of its pages (`networkDiagnostics` of base-test, also attached
  as "network failures"): each failure and each failed request with its method and URL, when in the test, its page,
  whether that page was closed and had left then, when each page closed, and how long the first view of each session
  took from its subscription.
  A ResizeObserver loop is found by `E2E_RO_TRACE=1` (`tests/e2e/setup/resize-observers.ts`: the deliveries, the end of
  each callback, the texts that changed in observed elements, what was left pending; an element our observers let go
  shows there too, unobserved), under load with `E2E_CPU_THROTTLE=6` (Chromium; alone it runs the page's own weighing
  of a loop) or many WebKit workers (`--repeat-each 30 --workers 8`).
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
- **The words of a tooltip are on the page twice**: its trigger's description is a hidden span with the same string
  (`data-slot="tooltip-description"`, `src/components/ui/tooltip.tsx`), and `getByText` finds hidden elements too, so a
  page-wide `getByText` of text a tooltip also says resolves to two elements and fails in strict mode. Scope it to the
  container that shows the text (`getByTestId(…).getByText(…)`), or assert the description itself with
  `toHaveAccessibleDescription`.
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
- **A frame of `requestAnimationFrame` is a timer of the test clock**, which `context.clock.install` fakes: it comes every
  16 ms of the clock, not with a rendering update, and a loaded WebKit rendered no update in eight of them
  (`resize-loops.spec.ts`, 4 of 72 runs under `E2E_RO_TRACE`). A test waits for a rendering update by the first report
  of a `ResizeObserver` made for it, and makes its next change in the task after it: a change or a new observation in
  the delivery of its observer, no deeper than its report, is a loop of the test's own. In the page that is
  `window.__renderingUpdate()` (`tests/e2e/setup/rendering-updates.ts`, an init script of every context), which resolves
  after the next update with its time on `document.timeline`; from the test `resizeReportsDelivered(page)`.
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
  `expectPushedToast`; two moves queued behind the lock, each with its sync, 3 to 8 s under load: both answers,
  `releaseTheTwoMoves` of move-commits.spec.ts), or once the state its sync shows is there (`releaseTheSyncOfTheUnstack`), and the facts of the
  steps are waited for as long as the test lets them take (`waitForStepFacts`). The landing needs of tips a sync moved
  say no wait on a slot (each step keeps the needs before while they are computed): `waitForStepFacts` waits for them
  too (`waitForFreshNeeds`, `data-landing-needs` of Sync), so no test reads a step of stale needs; a test of the stale
  needs holds them (`holdRemoteAnswers({ hold: ["landingNeeds"] })`) and asserts `data-landing-needs="stale"`.
- **A menu opened right after the first sync shows the items it opened with**: the facts that sync asked for (the
  listing of GitHub, the permission of Patronus, the route) come after it, and an open menu doesn't move
  (`frozenMenuGroups` of src/lib/menu.ts): an item in its place changes there ("Merge" to "Merge #7", Request Review…
  enabled), one that took the place of another shows there ("Open Pull Request #7" where "Create Pull Request…" was),
  but an item a fact brings (Link GitHub Stack, a robot's Restart Safe Push, Fix Conflict…) shows only at the next
  open, and one that leaves stays disabled. A test that clicks such an item opens the menu once no step says
  "Checking…" (`waitForStepFacts(page.getByTestId("next-step-slot"))`), or once the row shows the fact the item comes
  with (the chip of its pull request), never with an answer held: a menu opened before the listing of
  github-stacks.spec.ts waited 30 s for "Link GitHub Stack". A submenu opens by `openSubmenu` of
  selection-helpers.ts, which asserts the open and says when its item moved from under the pointer (a scroll of the
  menu); `hover()` of an item with a submenu is refused (`refuseSubmenuHovers` of fixtures/base-test.ts), and
  `hoverSubmenuTrigger` is the plain move for a test that holds the delay.
- **A stalled action says the page or the machine**: the summary of a run (`tests/e2e/setup/timing-reporter.ts`)
  words each stalled action in one line, then what the line rests on, so a stall needs no reconstruction afterwards:
  ```
  22.8 s Press "h" in commit-files/quiet-changes.spec.ts › … H shows the old lines of the notes …
      → the page: it did not answer before the end of its test, its main thread stuck in a task
        worker 3, 16:18:05–16:18:28, load 85–92, swap-ins +0, no long task of its pages, a page did not answer, beside it 3.9 s Click (worker 2)
  ```
  The page is a page that did not answer an evaluate within 2 s, or a long task of its own over half of the stall; the
  machine is two or more actions of other workers 5× the median and slower in the same time, or a swap of 50 pages a
  second and more between the samples of the counters around the stall, which come every 5 s (`SWAP_INS_PER_SECOND`: on
  2026-11-15 neither this Mac at a load of 6 to 14 nor the Mac Studio at 26 to 165 swapped in a page in 5 minutes, this
  Mac 0.08 a second over 3 days, so a loaded Mac's trickle is no swap that stalls it); "the machine (the page was not
  asked)", or "neither (…)", when no answer and no long task of the page said otherwise (a passing test asks nothing,
  WebKit tells no long task); neither is an action that waited for something of its own (an overlay over its target, a
  page that did not take it). The pieces: the reporter samples the load and the free memory every second, the swap every
  fifth and once more as the run ends, so a stall at its end has a sample of the swap after it (`machine-sampler.ts`: no
  process but `vm_stat` on macOS, `/proc/vmstat` on Linux), the page fixture tells the long tasks of every page from 500
  ms as they end, in a line of the console, so a page keeps no buffer (`watchLongTasks` of
  `tests/e2e/helpers/page-health.ts`), and asks the pages of a failed test whether they answer: 4 s before its timeout,
  on the clock of the runner from the start of the page fixture, while they are open, since Playwright closes the pages
  of a test that times out before its fixtures end, else at its end; a page that closes in the probe (the timeout came
  in it) is unknown, never stuck; a passing test asks nothing. The pieces attach as `page health` and `timing.json`
  keeps them with the samples. A press into a page stuck in its key handler ends as the stall of quiet-changes did,
  "Test timeout … keyboard.press: Target page, context or browser has been closed". Kept by `stall-context.test.ts` (the
  verdicts, with `pnpm test:unit`) and `harness/page-health.spec.ts` (the probe of a stuck page). Before it, the press
  of 22.8 s on the Mac Studio (2026-11-15) could be told from neither: the timeline had to be rebuilt from the list
  reporter, and every test of its 50 s took two to three times its WebKit time.
- **A drop that never reached the page**: under load Chromium, as Playwright drives it, ends some drags on a target
  that took its `dragenter` and its `dragover` (both cancelled with the same `dropEffect`, `takeDrag` of
  `src/hooks/useCommitDrag.ts`) with a `dragleave` and a `dragend` and no `drop`: the release came 6 ms after the
  `dragenter` of the Stack on chip, before the browser had the page's answer, and the sync the drop starts never came
  (`stacked-branches.spec.ts`, 1 to 4 of 30 runs beside busy loops, about one in eight at load 200). A slow answer of
  the page alone (a 300 ms busy `dragover`) loses nothing. `dropOn` (`tests/e2e/helpers/selection-helpers.ts`) counts
  the `drop` and `dragend` events of the page, and when a target that took the drag (`data-drop-target="over"`) got no
  `drop` it makes the gesture again, at most twice: the drag of the row of `startDrag`, the row of the target (the chip
  shows only under it), the target. With the first drop of every gesture lost on purpose (a probe that stops it and
  sends the `dragleave` and the `dragend` Chromium sends), 10 of 10 lost it without the repeat and 39 of 39 dropped
  with it, 5 of them on the third gesture.
- **Throwaway Playwright specs** (screenshots, debugging) go into `shots.local/` of the worktree (`*.local` is
  ignored), with a config there that spreads `../playwright.config` and sets `testDir: "."`, `globalSetup:
  "../tests/e2e/setup/global-setup.ts"` and `webServer[].cwd: ".."`; run `npx playwright test --config
  shots.local/shots.config.ts`. Delete them before a commit: `pnpm lint:frontend` lints them too. The README images
  are no throwaway: `pnpm screenshots:readme` writes them.
