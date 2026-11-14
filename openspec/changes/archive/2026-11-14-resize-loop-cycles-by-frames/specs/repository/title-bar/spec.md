## MODIFIED Requirements

### Requirement: Every platform: A window whose page fails says so

When a render throws an error that nothing in the page catches, or the page does not load, the window MUST show the
failure at once, not a blank canvas: a title, the message of the error, Reload and Close Window. The log of the app
MUST hold every error of the page, caught or not; a ResizeObserver loop of the observers of a library as a warning, one
that repeats and one of the page's own observers as an error.

#### Scenario: A render throws
- **WHEN** a render of the page throws an error that nothing catches
- **THEN** the window shows "Cannot show the window" with the message of the error, at once, not by a deadline
- **AND** the log of the app has the error with its stack and the components it was thrown in
- **AND** where the page draws the title bar, the top row of the failure drags the window
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on the failure of a render that threw, log it, and reload or close the window`)

#### Scenario: The page does not load
- **WHEN** the window has a path that no page has, or the chunk of its page does not load
- **THEN** the window shows the same failure, titled "Cannot open the page of the window"
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the failure of a page that did not load`)

#### Scenario: Reload and Close Window
- **WHEN** the user presses Close Window on the failure
- **THEN** the window closes
- **WHEN** the user presses Reload
- **THEN** the page loads again, and shows itself when the failure does not come again
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on the failure of a render that threw, log it, and reload or close the window`)

#### Scenario: An error outside a render
- **WHEN** a module of the page throws as it loads, a handler throws, or a promise that nobody awaits fails
- **THEN** the log of the app has the error
  [@test] tests/e2e/repository/first-frame.spec.ts (`should log an error of a script and a rejection that nobody awaits`)

#### Scenario: An error the page catches
- **WHEN** the page catches an error of a command or of its session (a failed subscription, sync or read)
- **THEN** the log of the app has it, not only the console of the page
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says that a repository without commits has nothing to sync yet`)

#### Scenario: A ResizeObserver loop
- **WHEN** the engine reports a ResizeObserver loop, whose notifications come in the next frame
- **THEN** the log of the app has a warning of a loop of the observers of a library; three within five seconds are an error, a loop that repeats however slowly
- **AND** a loop that left an element of an observer of the page pending is an error at once
  [@test] src/lib/page-errors.test.ts (`logs a loop of the observers of a library as a warning, and three within five seconds as an error, however slowly they repeat`, `logs loops five seconds apart as warnings`, `logs a loop that left an element of the observers of the page pending as an error at once`)
  [@test] src/lib/observe-size.test.ts (`names at a loop the elements of ours resized since their report, or observed with a size and never reported`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should attribute a loop to the observer of the app or of a library whose element it left pending`)

#### Scenario: No observer of the page makes a loop
- **WHEN** a change in the rendering update of a report of an observer of the page resizes an element it observes, no deeper than the elements of the latest report
- **THEN** the element leaves its observers at once and is observed anew at the next frame or the next task, whichever comes first
- **AND** an element deeper than them stays observed, delivered at its depth in that frame, and one let go comes back at once when a report of shallower elements shows it deeper
- **AND** an element a task after the update resized has its size in the frame that follows
  [@test] src/lib/observe-size.test.ts (`lets an element a change of the delivery touches go at once, and observes it anew as the delivery ends`, `observes an element a task after the update let go anew in the next frame, when that frame comes before the next task`, `leaves an element deeper than the elements of the latest report observed: the update delivers it at its depth`, `observes an element its report let go anew as a report of shallower elements shows it deeper than they are`, `leaves an element it no longer observes alone`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app in the review of a pull request as its overview, its commits and their cards come and go @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the menu of a row and its submenu open and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the toasts of two checks for updates stack and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should paint no frame of a review with its overview over its first file as it opens, on a slow CPU`)

#### Scenario: A cycle of an observer of the page
- **WHEN** an element is let go so in ten frames in a row, resized there by a change of its own
- **THEN** the log of the app has an error, once while the cycle goes on, however slowly the window renders its frames
- **AND** an element resized between frames (a drag), or by a change after the update, is no cycle
  [@test] src/lib/observe-size.test.ts (`logs a cycle once when its own callback resizes an element in its delivery in ten frames in a row`, `counts an element its observers let go in one delivery once, and a frame without it ends the count`, `logs no cycle of an element resized between frames, nor of one let go by a change after the update`)
