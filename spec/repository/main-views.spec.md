---
name: Main Views
description: Requirements for the three views of the main window, Branches, Reviews (the pull requests the user takes part in) and Landing (the Safe Push statistics): switched only by the user, the branches given back as they were left, a hidden view reading nothing, a calm view that tells a failed read as its state.
style: plain-1
targets:
  - ../../src/stores/main-view.ts
  - ../../src/pages/IndexPage.tsx
  - ../../src/components/repository/MainViewSwitcher.tsx
  - ../../src/components/ui/toggle-group.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/LandingPopover.tsx
  - ../../src/components/landing/LandingView.tsx
  - ../../src/components/reviews/ReviewsView.tsx
  - ../../src/components/landing/charts/WeekChart.tsx
  - ../../src/components/landing/charts/MergeTimeChart.tsx
  - ../../src/components/landing/charts/TimeChart.tsx
  - ../../src/components/landing/charts/ChartFigure.tsx
  - ../../src/components/landing/charts/stackedBar.tsx
  - ../../src/components/landing/charts/KindChart.tsx
  - ../../src/components/landing/charts/HourChart.tsx
  - ../../src/components/landing/charts/Sparkline.tsx
  - ../../src/components/landing/charts/TableView.tsx
  - ../../src/components/landing/charts/ChartKey.tsx
  - ../../src/components/landing/charts/RankedList.tsx
  - ../../src/components/ui/chart.tsx
  - ../../src/index.css
  - ../../src/hooks/useRobotStats.ts
  - ../../src/stores/robot-stats.ts
  - ../../src/stores/settings/appSettings.ts
  - ../../src/stores/commit-move.ts
  - ../../src/lib/robot-stats.ts
  - ../../src/lib/duration.ts
  - ../../src/lib/main-menu.ts
  - ../../src/lib/app-commands.ts
  - ../../src-tauri/src/menu.rs
---

# Main Views

Status: Implemented
Date: 2026-10-22

## Summary

The main window has three views of the repository: Branches, the branch tables where the work is cut into pieces;
Reviews, every open pull request the user takes part in on GitHub ([Reviews](reviews.spec.md)); and Landing, how those
pieces went into master ([Safe Push Statistics](safe-push-statistics.spec.md)). The switcher in the title bar names
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

### Switching

- The switcher after the repository in the title bar (Branches · Reviews · Landing, one option set: the stock
  `ToggleGroup`) shows the view, the shown one pressed; a click, ⌘1, ⌘2 and ⌘3 of the native View menu, ☰ View →
  Branches / Reviews / Landing with Ctrl+1, Ctrl+2 and Ctrl+3 of a custom title bar, and the palette's Views switch it; the arrow keys move the focus between the
  views, and Enter or Space shows the focused one. "Show Statistics" of the landing overview closes
  it and shows Landing.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from Ctrl+3 and from View → Landing of ☰, and come back by Ctrl+1`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should open Reviews by its keys of a custom title bar: Ctrl+2`)
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)
  [@test] ../../src/lib/main-menu.test.ts (`maps the accelerators of the native menu to their commands`)
- Only the user changes the view: a sync keeps Landing; a reveal of a branch (a toast, the Activity window, a review
  window, F2, Go to <branch>) shows the branches first, committed at once, so the row it focuses takes the focus.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)
- The views stay mounted once shown (React's `Activity`); the branches keep their scroll position and their focused row: a switch
  that leaves the focus inside the view that hides moves it to the heading of the other, a click on the switcher keeps
  it there. Every start shows the branches.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

### Landing

- The Period of the header (Last 14 days, Last 30 days, Last 10 weeks, Last 6 months; 30 days by default) is the period
  every island counts, kept in the app settings for every window and the next start; the header says the target and
  the zone of the hours (`Into master · Europe/Berlin`). The failure causes are of the last 14 days in every period.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should count the chosen period: its robots, its weeks, and keep the choice`)
  [@test] ../../src/lib/robot-stats.test.ts (`names every period by its days, the shortest first`)
- Landing shows the statistics of the selected repository in islands, each one question, each as tall as its content:
  into master on the full width (what landed and what it cost: merged, robots per merge, the time to merge, the first
  try and the time lost to failures against everyone else, then where the time went as one bar, then what a merge
  landed), the robots per week beside the time to merge by week against everyone else's, and why robots failed in the
  last 14 days beside when robots run fastest by the start hour. A figure says how it compares to everyone else (an
  arrow and words, success where it is better, destructive where it is worse) and its trend over the weeks that ended
  (the time lost too), the trend from its smallest value to its largest, not from zero; every chart has a table twin
  with every value, and the keyboard walks its marks with their tooltips.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)
- The week of now is "This week": muted in the chart of the weeks and "so far" in its tooltip and table, so its robots
  of a few days never read as a drop; the trends of counts end with the week before. The share of failed robots stands
  on the cap of each week while the weeks are 8 or fewer.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)
- The failure causes are three ranked lists of one form (the name, a thin bar, the value): "What failed" by kind, its
  bar the share of the failed robots; "Failed the most" and "Passed on a retry", their bars scaled to the first one. A
  check opens the newest robot it failed or passed on a retry in, in Patronus.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)
- The start hours say when to start: "Robots that start 09:00–10:00 merge fastest: 1h, against 3h at 14:00", the
  longest run of the fastest hours against the slowest hour, none while no two hours differ; the chart illustrates the
  sentence: that run in full colour and named in the key, the two medians on their bars, an hour label every third
  hour; the chart runs from the first hour with a value to the last.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should say when robots merge fastest: the fastest hours against the slowest`)
- The statistics are read while Landing shows: when it shows and when the window gets the focus; a hidden Landing
  reads nothing; a change of the period reads its numbers. Refresh asks again whatever the age of the lists, and
  tells its failure (`notifyError`).
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)
- A read nobody asked for tells its failure as the state of the view: not on Space, the Safe Push CLI not logged in
  (Copy Command; no Period then), Patronus failing, no Safe Push in the period; kept numbers of a failing Patronus say
  they are of an earlier read.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should say why there are no statistics`, `should say there was no Safe Push in the period`)
- A new answer replaces the numbers whole; while the first read runs, the numbers of the lists show and the island of
  the failure causes is busy until the checks are read. After a change of the period the numbers of the one before
  stay dimmed until those of the chosen one come; the numbers say their period (`data-period`).

## User Experience

- The switcher is the stock `ToggleGroup` (`outline`, `sm`, `spacing={0}`) on the canvas, the shown view in its
  pressed state, no colour of its own; the names give way to the icons below `@5xl` (the three of them would crowd the row of a narrow window), the name then the accessible name
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

- [Safe Push Statistics](safe-push-statistics.spec.md), [Title Bar](title-bar.spec.md),
  [Landing Overview](landing-overview.spec.md), [Command Palette](command-palette.spec.md).
