# Proposal

## Why

The tool window of the IDE read "Branch Deck | Branch Deck 37% | Activity": the main tab named Branches still holds the
switch Branches · Reviews · Landing of the desktop app, so the name of the tab repeats one option of the switch.

## What Changes

- The app file has the views `reviews` and `landing`, tabs of the tool window in the order Branches, Reviews,
  Landing, Activity; each is a page of its own (`/reviews`, `/landing`), with the repository of the main view.
- No page in the IDE shows the switch. A switch to the view of another tab asks `brd mcp` for that tab
  (`plugin:air|show_view`), with an event for its page: the reveal of a branch, the focus of the field of a link.
- The session gets the union of the demands of the views of a mirror, so the tab of Reviews and the main view do not
  take back the demand of each other.

Capabilities: `repository/main-views`, `repository/ide-embedding`.
