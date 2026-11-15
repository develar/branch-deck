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
- **AND** a loop that left an element of an observer of the page pending is an error at once: one laid out at another size than reported in the box it observes (the content box unless the border box), a scroll bar that takes room in the border box and not in the content box, as the engine tells it
- **AND** the engine's own boxes tell the elements a loop left pending, from the update after it: at a zoom of the app other than 100% or a fractional device scale WebKit lays out borders, paddings and bars on a grid the computed style does not say, and an element laid out as reported is none; a loop is weighed at the time it came
- **AND** an element that changed or left the page between the loop and that update (a render of a task, a remount) is not named: a pending one that changed again is no error, rather than an error of one that was not pending
  [@test] src/lib/observe-size.test.ts (`tells nothing at a loop of an element a task resized before the probe reported, pending or not`, `tells nothing at a loop of an element that left the page before the probe reported, as in a remount`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should take a loop for a library's when a task resizes an element of the app after it, at a zoom of 110% @webkit`, `should take a loop for a library's when an element of the app leaves the page after it until the next update, at a zoom of 110% @webkit`)
  [@test] src/lib/observe-size.test.ts (`names no element at a loop that the engine lays out as reported, where the arithmetic lays it out otherwise, as at a zoom of the app`, `names no element at a loop whose border box, which its observation observes, the engine lays out as reported`, `names no element at a loop whose content box is empty, its border box laid out by the engine as reported`, `names an element of ours at a loop that has no box any more, which the probe does not report`, `tells the loops in their order: a loop of a library alone after one that waits for the probe waits as well`)
  [@test] src/lib/page-errors.test.ts (`weighs a loop by the time it came, however late the observers of the page tell its elements`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should take a loop for a library's when the engine lays an element of the app out as reported, at a device scale of 125% @webkit`, `should take a loop for a library's when the engine lays an element of the app out as reported, at a device scale of 150% @webkit`, `should take a loop for a library's when the engine lays an element of the app out as reported, at a zoom of 110% @webkit`)
- **AND** each kind of line comes once in ten seconds at most, with the count of the loops it left out
- **AND** the page takes the event of the engine as its own: no console reports the loop as an uncaught error
  [@test] src/lib/page-errors.test.ts (`logs a loop of the observers of a library as a warning, and three within five seconds as an error, however slowly they repeat`, `logs loops five seconds apart as warnings, one in ten seconds at most with the count of those it left out`, `logs a loop that left an element of the observers of the page pending as an error at once, then once in ten seconds with the count`)
  [@test] src/lib/observe-size.test.ts (`names at a loop the elements of ours resized since their report, or observed with a size and never reported`, `names at a loop an element whose content box changed inside the same border box, as the engine compares it`, `compares the box an observation observes alone, as the engine does`)
  [@test] src/lib/laid-out.test.ts (`reads the content box of a scroller without its scroll bar, as its observer reports it, in either box sizing`, `reads no scroll bar of an overlay bar or of none`, `reads no scroll bar of the pixel the rounding of the client box leaves`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should read the boxes of a scroller whose scroll bar takes room as its observer reports them @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should attribute a loop to the observer of the app or of a library whose element it left pending`)

#### Scenario: No observer of the page makes a loop
- **WHEN** the page changes in a delivery of an observer (of the page or of a library), from its report to the next frame or the next task, whichever comes first
- **THEN** a change in the callback or in the chain of 256 microtasks after it (past the flushSync with which floating-ui places a popup, 42 to 87 microtasks after the callback of its observer) lets go every element of the observers of the page no deeper than the elements of the latest report, as it may have resized it (the layout is the page's: a block above a view that grows shrinks the view, a sibling that grows shrinks it in a flex row, a view hidden around a table empties its cells); a change past the chain (a render in a task, a microtask of the update later still) lets go the elements it is in; each is observed anew as the delivery ends
- **AND** an element deeper than them stays observed, delivered at its depth in that frame, and one let go comes back at once when a report of shallower elements shows it deeper
- **AND** a change after the delivery lets nothing go: the next update delivers it in its own frame
- **AND** an element first observed in the delivery, no deeper than the elements of the latest report (a block a flushSync in a callback mounted, observed by its ref; the cells of a table mounted again; an element in place an effect observes), is observed only as it ends; one its observer observes again there leaves the engine's observer until then
- **AND** a report of an observer of a library opens the delivery at its depth as one of ours does; the observer of the library is the engine's class, prototype and methods, with the same entries in the same frames
  [@test] src/lib/observe-size.test.ts (`lets an element go at once on a change of the delivery, and observes it anew as the delivery ends`, `observes an element the delivery let go anew at the next frame, when that frame comes before the next task`, `leaves an element observed that a change after the delivery resizes: the next update delivers it in its own frame`, `leaves an element deeper than the elements of the latest report observed: the update delivers it at its depth`, `observes an element its report let go anew as a report of shallower elements shows it deeper than they are`, `leaves an element it no longer observes alone`)
  [@test] src/lib/observe-size.test.ts (`lets the view go on a change a commit of React makes in a microtask right after the callback`, `lets an element its observer observes again in the delivery leave the engine's observer until the delivery ends`, `keeps an element observed first in the delivery waiting when a report shallower than the delivery, but not than it, follows`)
  [@test] src/lib/observe-size.test.ts (`lets the view go that a sibling grown three promises after the callback shrinks, though the change is not in it`, `lets the view go that a sibling grown a hundred and twenty promises after the callback of a library shrinks, as floating-ui places a popup`, `lets the view go that a sibling grown 255 promises after the callback shrinks, in the last microtask of the chain`, `leaves the view observed that a sibling grown 256 promises after the callback shrinks, the first past the chain`, `makes the chain as long again from a callback that begins in the chain of another`)
  [@test] src/lib/observe-size.test.ts (`lets go in a task before the delivery ends only the elements a change is in, and observes an element first observed there as it ends`, `observes a row a callback inserted and first observed only as the delivery ends`, `observes a row a commit of React inserted and first observed in a microtask after the callback only as the delivery ends`, `observes an element first observed microtasks after the callback, with no change, only as the delivery ends`)
  [@test] src/lib/observe-size.test.ts (`lets every element of ours go that a change anywhere in the delivery can resize, and leaves one deeper than the report observed`, `keeps an element let go above a report of deeper elements let go when a report shallower than they are, but not than it, follows`)
  [@test] src/lib/observe-size.test.ts (`observes an element first observed in the delivery no deeper than its report only as it ends, with no change to tell it`, `observes the cells of a table mounted again in the delivery only as it ends`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should read a large step by its groups: their files flat by their directory in the module, their tests last, a new module marked, the wiring folded at the end`)
  [@test] src/lib/observe-size.test.ts (`lets an element of ours go that the callback of a library resizes in its delivery, in a frame without a report of ours`, `takes the depth of a report of a library deeper than ours as the depth of the delivery`, `gives a library the engine's observer: its class, prototype and methods, and its callback the same entries and observer`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should give the observer of a library the engine's class and the same entries in the same frames @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app in the review of a pull request as its overview, its commits and their cards come and go @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the menu of a row and its submenu open and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as a sibling grown 3 promises after the callback of a library shrinks the room of the repository path @webkit`, `should make no loop of the app as a sibling grown 120 promises after the callback of a library shrinks the room of the repository path @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the toasts of two checks for updates stack and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should paint no frame of a review with its overview over its first file as it opens, on a slow CPU`)

#### Scenario: A cycle of an observer of the page
- **WHEN** an element is let go so in ten of twenty frames (in every frame, in every other) after its report there, resized by a change of a callback of the page (its own observer's or another's)
- **THEN** the log of the app has an error, once while the cycle goes on, however slowly the window renders its frames
- **AND** an element let go three frames apart, one resized between frames (a drag), one let go before its own report in the delivery, or one let go by a change after the update, is no cycle
- **AND** the release that would make a cycle is measured against the layout of the element as reported, one arithmetic for both, so a drag is no cycle at any zoom or scale; a bar estimated a pixel apart is the same bar, one that came or went a change in the box observed
  [@test] src/lib/observe-size.test.ts (`logs no cycle of an element a drag resizes between frames where the arithmetic lays it out otherwise than the engine`)
  [@test] src/lib/laid-out.test.ts (`takes a bar the rounding of whole pixels estimates a pixel apart for the same bar`, `tells a bar that came or went, and a box of another size, in the box observed`)
  [@test] src/lib/observe-size.test.ts (`logs a cycle once when its own callback resizes an element in its delivery in every frame`, `logs a cycle that resizes an element in the delivery of every other frame`, `logs no cycle of an element let go three frames apart, nor of one whose releases a pause of twenty frames parts`, `counts an element its observers let go in one delivery once`, `logs no cycle of an element resized between frames, nor of one let go by a change after the update`, `logs no cycle of an element another callback lets go before its own report in every frame, as a drag resized it`)
