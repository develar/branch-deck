---
name: Main Views
description: Requirements for the three views of the main window, Branches, Reviews (the pull requests the user takes part in) and Landing (the Safe Push statistics): switched only by the user, the branches given back as they were left, a hidden view reading nothing, a calm view that tells a failed read as its state.
style: plain-1
targets:
  - src/stores/main-view.ts
  - src/pages/IndexPage.tsx
  - src/hooks/useCloseWhenViewHides.ts
  - src/hooks/useLazyRoot.ts
  - src/components/ui/tooltip.tsx
  - src/components/ui/popover.tsx
  - src/components/ui/dropdown-menu.tsx
  - src/components/ui/context-menu.tsx
  - src/components/ui/select.tsx
  - src/components/repository/MainViewSwitcher.tsx
  - src/components/ui/toggle-group.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/repository/LandingPopover.tsx
  - src/components/landing/LandingView.tsx
  - src/components/reviews/ReviewsView.tsx
  - src/components/landing/charts/WeekChart.tsx
  - src/components/landing/charts/MergeTimeChart.tsx
  - src/components/landing/charts/TimeChart.tsx
  - src/components/landing/charts/ChartFigure.tsx
  - src/components/landing/charts/stackedBar.tsx
  - src/components/landing/charts/KindChart.tsx
  - src/components/landing/charts/HourChart.tsx
  - src/components/landing/charts/Sparkline.tsx
  - src/components/landing/charts/TableView.tsx
  - src/components/landing/charts/ChartKey.tsx
  - src/components/landing/charts/RankedList.tsx
  - src/components/ui/chart.tsx
  - src/index.css
  - src/hooks/useRobotStats.ts
  - src/stores/robot-stats.ts
  - src/stores/settings/appSettings.ts
  - src/stores/commit-move.ts
  - src/lib/robot-stats.ts
  - src/lib/duration.ts
  - src/lib/main-menu.ts
  - src/lib/app-commands.ts
  - src-tauri/src/menu.rs
---

# Main Views

Status: Implemented
Date: 2026-10-22

## Purpose

The main window has three views of the repository: Branches, the branch tables where the work is cut into pieces;
Reviews, every open pull request the user takes part in on GitHub ([Reviews](../reviews/spec.md)); and Landing, how those
pieces went into master ([Safe Push Statistics](../safe-push-statistics/spec.md)). The switcher in the title bar names
where the user is; ⌘1, ⌘2 and ⌘3 switch, as the View menu, ☰ and the palette do. What runs now stays in
the landing overview beside the table: its steps run at their rows. Landing is for a deliberate look back, so it takes
nothing from the work: the view changes only when the user changes it, and the branches come back as they were left.

## Goals

- Never change the view for the user: a robot, a sync, an error or a start leave it as it is.
- Give the branches back as they were: the rows, their forms, the scroll position and the focus.
- Read the statistics and the rest of Reviews only while they show; tell a read nobody asked for as a state of the
  view, not a toast.

## Non-goals

- No live view of the running robots in Landing: the landing overview shows them at their rows.
- No view state across launches: every start shows the branches.

## Requirements

### Requirement: Switching: The switcher in the title bar shows the view

The switcher after the repository in the title bar (Branches · Reviews · Landing, one option set: the stock
`ToggleGroup`) SHALL show the view, the shown one pressed.

#### Scenario: The shown view is pressed
- **WHEN** a view shows
- **THEN** its option of the switcher after the repository in the title bar is pressed
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

### Requirement: Switching: A click, the keys, the menus and the palette switch the view

A click, ⌘1, ⌘2 and ⌘3 of the native View menu, ☰ View → Branches / Reviews / Landing with Ctrl+1, Ctrl+2 and Ctrl+3
of a custom title bar, and the palette's Views SHALL switch the view.

#### Scenario: The native View menu
- **WHEN** the user presses ⌘1, ⌘2 or ⌘3 of the native View menu
- **THEN** the view switches to Branches, Reviews or Landing
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)
  [@test] src/lib/main-menu.test.ts (`maps the accelerators of the native menu to their commands`)

#### Scenario: A custom title bar
- **GIVEN** a custom title bar
- **WHEN** the user presses Ctrl+1, Ctrl+2 or Ctrl+3, or chooses ☰ View → Branches / Reviews / Landing
- **THEN** the view switches
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from Ctrl+3 and from View → Landing of ☰, and come back by Ctrl+1`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should open Reviews by its keys of a custom title bar: Ctrl+2`)

#### Scenario: A click and the palette
- **WHEN** the user clicks a view of the switcher, or chooses one of the palette's Views
- **THEN** the view switches
  [@test] tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)

### Requirement: Switching: The arrow keys move the focus, Enter or Space shows the view

In the switcher the arrow keys SHALL move the focus between the views, and Enter or Space SHALL show the focused one.

#### Scenario: The switcher by its keys
- **GIVEN** the focus in the switcher
- **WHEN** the user presses an arrow key, then Enter or Space
- **THEN** the arrow moves the focus to the next view, and Enter or Space shows the focused one
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

### Requirement: Switching: Show Statistics of the landing overview shows Landing

"Show Statistics" of the landing overview SHALL close it and show Landing.

#### Scenario: Show Statistics
- **WHEN** the user presses "Show Statistics" in the landing overview
- **THEN** the landing overview closes and Landing shows
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

### Requirement: Switching: Only the user changes the view

Only the user MUST change the view: a sync keeps Landing; a reveal of a branch (a toast, the Activity window, a review
window, F2, Go to <branch>) shows the branches first, committed at once, so the row it focuses takes the focus.

#### Scenario: A sync keeps Landing
- **GIVEN** Landing shows
- **WHEN** a sync runs
- **THEN** Landing keeps showing
  [@test] tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)

#### Scenario: A reveal of a branch shows the branches
- **GIVEN** Landing shows
- **WHEN** a branch is revealed (a toast, the Activity window, a review window, F2, Go to <branch>)
- **THEN** the branches show first, committed at once
- **AND** the row the reveal focuses takes the focus
  [@test] tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)

### Requirement: Switching: The views stay mounted and the branches keep their place

The views MUST stay mounted once shown (React's `Activity`); the branches keep their scroll position and their focused
row.

#### Scenario: Back to the branches
- **GIVEN** a focused row of the branches, open or closed
- **WHEN** the user switches to another view and back
- **THEN** the branches keep their scroll position, and the row is focused, open or closed as before
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

### Requirement: Switching: A switch moves the focus to the heading of the other view

A switch that leaves the focus inside the view that hides MUST move it to the heading of the other; a click on the
switcher keeps it there.

#### Scenario: The focus was in the view that hides
- **GIVEN** the focus inside the branches
- **WHEN** the view switches to Landing
- **THEN** the focus moves to the heading of Landing
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

#### Scenario: A click on the switcher
- **WHEN** the user clicks a view of the switcher
- **THEN** the focus stays on the switcher

### Requirement: Switching: Every start shows the branches

Every start MUST show the branches.

#### Scenario: A start of the window
- **WHEN** the main window starts
- **THEN** it shows the branches

### Requirement: Switching: A focus the switch hid never stays on a hidden control

A focus the switch hid, in the view or in one of its popups, MUST move to the heading of the view that shows, or
nowhere until the heading of a view loaded for its first show takes it: WebKit drops the focus of a hidden element
only at a later layout, and until then the keys of the window went to it (P to the row of a hidden push button, which
pushed).

#### Scenario: The focus on a push button of the branches
- **GIVEN** the focus on the push button of a row
- **WHEN** the view switches to a Landing loaded for its first show, and the user presses P
- **THEN** the focus moves to the heading of Landing and nothing is pushed
  [@test] tests/e2e/landing/statistics.spec.ts (`should close the popups of the branches as the view hides, open none by itself as it shows, and leave the focus on no hidden control`)

### Requirement: Switching: A view that hides closes its popups

A view that hides MUST close its popups: a hidden view keeps its state but runs no effect, so a tooltip, a hover card,
a popover, a menu or a select open at the switch would come back open with the pointer elsewhere, its trigger never
told the pointer left, until a click elsewhere; the roots of `src/components/ui` close as their view hides
(`useCloseWhenViewHides`).

#### Scenario: A tooltip, a hover card and a menu open at the switch
- **GIVEN** a tooltip, a hover card or a menu of the branches open
- **WHEN** the view switches away and back with the pointer elsewhere
- **THEN** the popup is closed
  [@test] tests/e2e/landing/statistics.spec.ts (`should close the popups of the branches as the view hides, open none by itself as it shows, and leave the focus on no hidden control`)

#### Scenario: The issue popover of a hidden link
- **GIVEN** the issue popover of a link open
- **WHEN** the switch hides the link
- **THEN** the popover closes as the link hides
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should close as the view of its link hides`)

### Requirement: Switching: A view that shows again opens no popup by itself

A view that shows again MUST open no popup: the first hover or focus of a lazy trigger replays once (`useLazyRoot`), not
as its view shows again: else the trigger of every popup hovered before opened it by itself, and a push button focused
before took the focus from where the user had put it.

#### Scenario: Triggers hovered and focused before
- **GIVEN** a trigger hovered and a push button focused before a switch, and the focus put on the switcher
- **WHEN** the view switches away and back
- **THEN** no popup opens by itself, and the push button takes no focus back
  [@test] tests/e2e/landing/statistics.spec.ts (`should close the popups of the branches as the view hides, open none by itself as it shows, and leave the focus on no hidden control`)

### Requirement: Switching: The dialogs and the inline forms stay as they were

The dialogs and the inline forms MUST stay as they were across a switch.

#### Scenario: A dialog open at the switch
- **GIVEN** a dialog or an inline form open in a view
- **WHEN** the view switches away and back
- **THEN** the dialog or the form is as it was

### Requirement: Landing: The Period of the header is the period every island counts

The Period of the header (Last 14 days, Last 30 days, Last 10 weeks, Last 6 months; 30 days by default) MUST be the
period every island counts, kept in the app settings for every window and the next start. The failure causes are of
the last 14 days in every period.

#### Scenario: The chosen period is counted and kept
- **GIVEN** the Period of 30 days, the default
- **WHEN** the user chooses another Period
- **THEN** every island counts that period
- **AND** the choice is kept in the app settings for every window and the next start
  [@test] tests/e2e/landing/statistics.spec.ts (`should count the chosen period: its robots, its weeks, and keep the choice`)

#### Scenario: The periods
- **WHEN** the Period opens
- **THEN** it offers Last 14 days, Last 30 days, Last 10 weeks, Last 6 months
  [@test] src/lib/robot-stats.test.ts (`names every period by its days, the shortest first`)

#### Scenario: The failure causes in a longer period
- **WHEN** the Period is longer than 14 days
- **THEN** the failure causes are still of the last 14 days

### Requirement: Landing: The header says the target and the zone of the hours

The header MUST say the target and the zone of the hours (`Into master · Europe/Berlin`).

#### Scenario: The header of Landing
- **WHEN** Landing shows
- **THEN** its header says the target and the zone of the hours, `Into master · Europe/Berlin`
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: The statistics show in islands, each one question

Landing MUST show the statistics of the selected repository in islands, each one question, each as tall as its
content.

#### Scenario: The islands of Landing
- **WHEN** Landing shows
- **THEN** into master is on the full width: what landed and what it cost (merged, robots per merge, the time to merge,
  the first try and the time lost to failures against everyone else), then where the time went as one bar, then what a
  merge landed
- **AND** the robots per week stand beside the time to merge by week against everyone else's
- **AND** why robots failed in the last 14 days stands beside when robots run fastest by the start hour
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: A figure says how it compares to everyone else and its trend

A figure MUST say how it compares to everyone else (an arrow and words, success where it is better, destructive where
it is worse) and its trend over the weeks that ended (the time lost too), the trend from its smallest value to its
largest, not from zero.

#### Scenario: A figure worse than everyone else's
- **WHEN** a figure of the user is worse than everyone else's
- **THEN** it says so with an arrow and words, in destructive; better, in success
- **AND** its trend runs over the weeks that ended, from its smallest value to its largest, not from zero
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: Every chart has a table twin and the keyboard walks its marks

Every chart MUST have a table twin with every value, and the keyboard SHALL walk its marks with their tooltips.

#### Scenario: The table twin and the keys of a chart
- **WHEN** a chart shows
- **THEN** its table twin holds every value
- **AND** the arrow keys walk its marks, each with its tooltip
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: The week of now is This week, so far

The week of now MUST be "This week": muted in the chart of the weeks and "so far" in its tooltip and table, so its
robots of a few days never read as a drop; the trends of counts end with the week before.

#### Scenario: The current week
- **WHEN** the weeks show the week of now
- **THEN** it is "This week", muted in the chart of the weeks, "so far" in its tooltip and table
- **AND** the trends of counts end with the week before
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: The share of failed robots stands on the cap of each week

The share of failed robots MUST stand on the cap of each week while the weeks are 8 or fewer.

#### Scenario: Eight weeks or fewer
- **WHEN** the chart of the weeks shows 8 weeks or fewer
- **THEN** the share of failed robots stands on the cap of each week
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: The failure causes are three ranked lists of one form

The failure causes MUST be three ranked lists of one form (the name, a thin bar, the value): "What failed" by kind, its
bar the share of the failed robots; "Failed the most" and "Passed on a retry", their bars scaled to the first one.

#### Scenario: The three lists
- **WHEN** the failure causes show
- **THEN** "What failed" lists the kinds, each bar the share of the failed robots
- **AND** "Failed the most" and "Passed on a retry" list the checks, their bars scaled to the first one
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: A check opens its newest robot in Patronus

A check MUST open the newest robot it failed or passed on a retry in, in Patronus.

#### Scenario: A click on a check
- **WHEN** the user clicks a check of the failure causes
- **THEN** the newest robot it failed or passed on a retry in opens in Patronus
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: The start hours say when to start

The start hours MUST say when to start: "Robots that start 09:00–10:00 merge fastest: 1h, against 3h at 14:00", the
longest run of the fastest hours against the slowest hour, none while no two hours differ.

#### Scenario: The fastest hours against the slowest
- **WHEN** the hours differ
- **THEN** the sentence names the longest run of the fastest hours against the slowest hour: "Robots that start
  09:00–10:00 merge fastest: 1h, against 3h at 14:00"
  [@test] tests/e2e/landing/statistics.spec.ts (`should say when robots merge fastest: the fastest hours against the slowest`)

#### Scenario: No two hours differ
- **WHEN** no two hours differ
- **THEN** no sentence shows
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: The chart of the hours illustrates the sentence

The chart MUST illustrate the sentence: that run in full colour and named in the key, the two medians on their bars, an
hour label every third hour; the chart runs from the first hour with a value to the last.

#### Scenario: The chart of the start hours
- **WHEN** the sentence names a run of the fastest hours
- **THEN** the chart shows that run in full colour, named in the key, and the two medians on their bars
- **AND** it labels every third hour
  [@test] tests/e2e/landing/statistics.spec.ts (`should say when robots merge fastest: the fastest hours against the slowest`)

#### Scenario: The range of the hours
- **WHEN** the chart of the hours shows
- **THEN** it runs from the first hour with a value to the last
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

### Requirement: Landing: The statistics are read only while Landing shows

The statistics MUST be read while Landing shows: when it shows and when the window gets the focus; a hidden Landing
reads nothing; a change of the period reads its numbers.

#### Scenario: A focus of the window while the branches show
- **GIVEN** Landing is hidden
- **WHEN** the window gets the focus
- **THEN** no statistics are read
  [@test] tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)

#### Scenario: Landing shows and the window gets the focus
- **WHEN** Landing shows, and again when the window gets the focus while it shows
- **THEN** the statistics are read
  [@test] tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)

#### Scenario: A change of the period
- **WHEN** the user chooses another Period
- **THEN** the numbers of that period are read
  [@test] tests/e2e/landing/statistics.spec.ts (`should count the chosen period: its robots, its weeks, and keep the choice`)

### Requirement: Landing: Refresh asks again and tells its failure

Refresh MUST ask again whatever the age of the lists, and tell its failure (`notifyError`).

#### Scenario: Refresh
- **WHEN** the user presses Refresh
- **THEN** the statistics are asked again, whatever the age of the lists
  [@test] tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)

#### Scenario: A Refresh that fails
- **WHEN** the read of a Refresh fails
- **THEN** its failure is told (`notifyError`)

### Requirement: Landing: A read nobody asked for tells its failure as the state of the view

A read nobody asked for MUST tell its failure as the state of the view: not on Space, the Safe Push CLI not logged in
(Copy Command; no Period then), Patronus failing, no Safe Push in the period; kept numbers of a failing Patronus say
they are of an earlier read.

#### Scenario: Not logged in and not on Space
- **WHEN** the Safe Push CLI is not logged in, or the repository is not on Space
- **THEN** the view says so as its state, with Copy Command and no Period for the CLI not logged in
  [@test] tests/e2e/landing/statistics.spec.ts (`should say why there are no statistics`)

#### Scenario: No Safe Push in the period
- **WHEN** there was no Safe Push in the period
- **THEN** the view says so as its state
  [@test] tests/e2e/landing/statistics.spec.ts (`should say there was no Safe Push in the period`)

#### Scenario: Patronus failing
- **WHEN** Patronus fails
- **THEN** the view tells the failure as its state
- **AND** kept numbers say they are of an earlier read

### Requirement: Landing: A new answer replaces the numbers whole

A new answer MUST replace the numbers whole; while the first read runs, the numbers of the lists show and the island
of the failure causes is busy until the checks are read.

#### Scenario: The first read
- **WHEN** the first read runs
- **THEN** the numbers of the lists show, and the island of the failure causes is busy until the checks are read
- **AND** the new answer replaces the numbers whole

### Requirement: Landing: The numbers of the period before stay dimmed until the new ones come

After a change of the period the numbers of the one before MUST stay dimmed until those of the chosen one come; the
numbers say their period (`data-period`).

#### Scenario: The numbers of the period before
- **WHEN** the user chooses another Period
- **THEN** the numbers of the one before stay dimmed until those of the chosen one come

#### Scenario: The numbers say their period
- **WHEN** the numbers of a period show
- **THEN** they say their period (`data-period`)
  [@test] tests/e2e/landing/statistics.spec.ts (`should count the chosen period: its robots, its weeks, and keep the choice`)

## User Experience

- The switcher is the stock `ToggleGroup` (`outline`, `sm`, `spacing={0}`) on the canvas, the shown view in its
  pressed state, no colour of its own; the names give way to the icons where the room of the repository in the title
  row is below `@md` (the three of them would crowd the repository selector; [Title Bar](../title-bar/spec.md)), the name then the accessible name
  and the tooltip with ⌘1, ⌘2 and ⌘3.
- The charts are shadcn Charts (Recharts, `src/components/ui/chart.tsx`), in a chunk of their own that loads the first
  time Landing shows. Their series take the chart palette `--chart-1…4` of `src/index.css` (blue, orange, a neutral,
  yellow: slots 1, 2 and 4 of the dataviz reference palette and a neutral for Canceled, which ended without an
  outcome and in a hue would read as success; in this order the adjacent marks pass the colour-blind and
  normal-vision checks on the light and the dark card), each entity one colour in every chart: Merged blue in the
  weeks and the time; Failed orange in the weeks, the time, the kinds and the bars of the checks; the user in the ink
  of the text (the dots of the hours, the line of the time to merge), told apart by the shape and the key, never in
  the hue of another entity; everyone else as the context of the user in `--chart-context`, a lighter step of the same
  blue (the line of the time to merge by week, the hours that are not the fastest), the fastest hours in the full
  blue (emphasis, not a second hue). Bars of at most 24px with a 2px card gap and a rounded data end, 2px lines, a
  hairline solid grid, the axes of time in whole quarters, halves or hours and of counts in steps of 1, 2 or 5, one
  key (`ChartKey`) left-aligned under every chart of two series or more, its marks as the chart's (a rect, a line, a
  dot), the values in the text tokens; a direct label only where it is the point (the failed share of a week, the two
  medians of the hours sentence, the name at the end of a line). A read in progress dims the numbers it replaces,
  never a skeleton. The Period is a small Select after the heading, in the one row above the islands that it scopes.

## Testing / Local Run

- `pnpm e2e tests/e2e/landing/statistics.spec.ts`; the fixture `setPatronusHistory` lists more robots of the user and
  of others.

## References

- [Safe Push Statistics](../safe-push-statistics/spec.md), [Title Bar](../title-bar/spec.md),
  [Landing Overview](../landing-overview/spec.md), [Command Palette](../command-palette/spec.md).
