---
name: Main Views
description: Requirements for the two views of the main window, Branches and Landing (the Safe Push statistics): switched only by the user, the branches given back as they were left, a hidden view reading nothing, a calm view that tells a failed read as its state.
style: plain-1
targets:
  - ../../src/stores/main-view.ts
  - ../../src/pages/IndexPage.tsx
  - ../../src/components/repository/MainViewSwitcher.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/LandingPopover.tsx
  - ../../src/components/landing/LandingView.tsx
  - ../../src/components/landing/charts/WeekChart.tsx
  - ../../src/components/landing/charts/TimeChart.tsx
  - ../../src/components/landing/charts/KindChart.tsx
  - ../../src/components/landing/charts/HourChart.tsx
  - ../../src/components/landing/charts/Sparkline.tsx
  - ../../src/components/landing/charts/TableView.tsx
  - ../../src/components/ui/chart.tsx
  - ../../src/index.css
  - ../../src/hooks/useRobotStats.ts
  - ../../src/stores/robot-stats.ts
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

The main window has two views of the repository: Branches, the branch tables where the work is cut into pieces, and
Landing, how those pieces went into master ([Safe Push Statistics](safe-push-statistics.spec.md)). The switcher in the
title bar names where the user is; ⌘1 and ⌘2 switch, as the View menu, ☰ and the palette do. What runs now stays in
the landing overview beside the table: its steps run at their rows. Landing is for a deliberate look back, so it takes
nothing from the work: the view changes only when the user changes it, and the branches come back as they were left.

## Goals

- Never change the view for the user: a robot, a sync, an error or a start leave it as it is.
- Give the branches back as they were: the rows, their forms, the scroll position and the focus.
- Read the statistics only while they show; tell a read nobody asked for as a state of the view, not a toast.

## Non-goals

- No live view of the running robots in Landing: the landing overview shows them at their rows.
- No view state across launches: every start shows the branches.

## Requirements

### Switching

- The switcher after the repository in the title bar (Branches · Landing, a tab list) shows the view; a click, ⌘1 and
  ⌘2 of the native View menu, ☰ View → Branches / Landing with Ctrl+1 and Ctrl+2 of a custom title bar, and the
  palette's Views switch it; the arrow keys move between the tabs. "Show Statistics" of the landing overview closes
  it and shows Landing.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from Ctrl+2 and from View → Landing of ☰, and come back by Ctrl+1`)
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)
  [@test] ../../src/lib/main-menu.test.ts (`maps the accelerators of the native menu to their commands`)
- Only the user changes the view: a sync keeps Landing; a reveal of a branch (a toast, the Activity window, a review
  window, F2, Go to <branch>) shows the branches first, committed at once, so the row it focuses takes the focus.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)
- Both views stay mounted (React's `Activity`); the branches keep their scroll position and their focused row: a switch
  that leaves the focus inside the view that hides moves it to the heading of the other, a click on the switcher keeps
  it there. Every start shows the branches.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

### Landing

- Landing shows the statistics of the selected repository in four islands: into master (merged, robots per merge, the
  time to merge and the first try against everyone else, the merges per week, what a merge landed), where the time
  went, why robots failed in the last 14 days, and when robots run fastest by the start hour. A figure says how it
  compares to everyone else (an arrow and words, success where it is better, destructive where it is worse) and its
  trend over the weeks; every chart has a table twin with every value, and the keyboard walks its marks with their
  tooltips.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)
- The statistics are read while Landing shows: when it shows and when the window gets the focus; a hidden Landing
  reads nothing. Refresh asks again whatever the age of the lists, and tells its failure (`notifyError`).
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)
- A read nobody asked for tells its failure as the state of the view: not on Space, the Safe Push CLI not logged in
  (Copy Command), Patronus failing, no Safe Push in the 70 days; kept numbers of a failing Patronus say they are of an
  earlier read.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should say why there are no statistics`, `should say there was no Safe Push`)
- A new answer replaces the numbers whole; while the first read runs, the numbers of the lists show and the island of
  the failure causes is busy until the checks are read.

## User Experience

- The switcher is two `xs` ghost buttons on the canvas, the shown view soft primary (no surface on the canvas); the
  names give way to the icons below `@4xl`.
- The charts are shadcn Charts (Recharts, `src/components/ui/chart.tsx`), in a chunk of their own that loads the first
  time Landing shows. Their series take the chart palette `--chart-1…4` of `src/index.css` (blue, orange, aqua,
  yellow, the categorical slots of the dataviz reference palette in an order that passes its colour-blind checks on
  the light and the dark card), each entity one colour in every chart (Failed is orange in the weeks, the time and the
  kinds); bars of at most 24px with a 2px card gap and a rounded data end, hairline solid grid, the legend for two
  series or more, the values in the text tokens; the fastest hours in full colour, the others muted (emphasis, not a
  second hue). A read in progress dims the numbers it replaces, never a skeleton.

## Testing / Local Run

- `pnpm e2e tests/e2e/landing/statistics.spec.ts`; the fixture `setPatronusHistory` lists more robots of the user and
  of others.

## References

- [Safe Push Statistics](safe-push-statistics.spec.md), [Title Bar](title-bar.spec.md),
  [Landing Overview](landing-overview.spec.md), [Command Palette](command-palette.spec.md).
