# Proposal

## Why

Two gaps of the tabs of the tool window (`ide-view-tabs`). An event for a tab that the IDE builds for the open was
lost, as its page did not listen yet, so "Add Pull Request by Link…" did not focus the field of a Reviews tab never
shown. A branch that the tab of Activity revealed stayed behind the tab of Activity.

## What Changes

- The host delivers an event to a view only after its page sent `ui/notifications/initialized` (Air, idea-1), so brd
  tells an event at once, and the page sends `initialized` only once its first render listens.
- The focus of the window of the main view (`plugin:window|set_focus`) shows the tool window with the tab of the main
  view (`openView {view: "main"}`), so a reveal that Activity or a review asks shows the tab of the branches.

Capabilities: `repository/ide-embedding`, `repository/main-views`.
