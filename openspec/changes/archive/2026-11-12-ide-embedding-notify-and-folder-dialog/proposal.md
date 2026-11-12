# Proposal

## Why

Two open items of Branch Deck in the IDE. A Safe Push that failed while the IDE showed another tab was no notification
with an action: the page's notification of the OS has none, and only the page told it, as its frame lost the focus. And
the folder button beside the selector of the repositories (Open Repository…) showed in the IDE and did nothing: `brd mcp`
has no folder dialog, the roots of the project name the repositories.

## What Changes

- `brd mcp` tells the failure of a robot, a news that a patch of a mirror brings, as `x-air-app/notify` while the main
  view is not shown and "Safe Push failures" is on, with the action Show in Branch Deck: the new app-only tool
  `reveal_branch`, which asks `openView` of `main` and tells it `reveal-branch`; the main view reveals the branch as Go
  to of the palette does. The page names the news of its own notification (`extra`), which brd leaves out once it told
  that news and else gives its kind and action.
- `DialogHost::has_folder_dialog`, false in `brd mcp`, reaches the page as `titleBar.folderDialog`: the page offers no
  Open Repository… beside the selector or in the palette and no Add Companion… where it is false.
