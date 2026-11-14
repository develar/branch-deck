## MODIFIED Requirements

### Requirement: Every platform: A window whose page fails says so

When a render throws an error that nothing in the page catches, or the page does not load, the window MUST show the
failure at once, not a blank canvas: a title, the message of the error, Reload and Close Window. The log of the app
MUST hold every error of the page, caught or not; a single ResizeObserver loop as a warning, a repeated one as an error.

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
- **THEN** the log of the app has a warning of it; more than two within a second are an error, a cycle
- **AND** no observer of the page makes one: an element a change resized after its report leaves its observer at once and is observed anew in the next task, and an element released so in every frame of a second is logged as an error
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app in the review of a pull request as its overview, its commits and their cards come and go @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the menu of a row and its submenu open and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the toasts of two checks for updates stack and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should attribute a loop to the observer of the app or of a library whose element it left pending`)

