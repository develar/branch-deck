# Proposal

## Why

The main view of the IDE took its repository from `recentProjects`, the recent projects of the desktop app, so it
showed what the desktop app showed rather than a root of the project, and a pick in its selector would have switched
the desktop app. `bd_bootstrap` named the first root, which the page did not read.

## What Changes

- The views of `brd mcp` see the Git roots of the project as their recent projects, the root the main view shows
  first (`crates/mcp-host/src/projects.rs`): in the snapshot of `bd_bootstrap`, in each change of `recentProjects` and in
  the answers that carry changes. The page is unchanged: it selects the first, and its selector lists the roots.
- A write of `recentProjects` by the page is a pick: brd shows its first root, remembers it by the set of the roots in
  `ideRootPicks` of `repository-state.json` (`app_settings::ide_roots`), and writes what the page changed of each root
  into the recent projects in their place, whose order stays the desktop app's.
- A change of the roots after a page started reaches the views as a change of `recentProjects`.
