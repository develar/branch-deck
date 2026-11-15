## ADDED Requirements

### Requirement: Every platform: The first sync moves no control of the toolbar

The toolbar of the main window MUST keep the room of the time of the last sync from the selection of a repository on,
empty before its first sync, and Update MUST come on the left of the controls it would move: the toolbar stands at its
end, so what the first sync brings moves neither Activity nor the landing indicator from under a still pointer.

#### Scenario: The pointer on Activity during the first sync
- **GIVEN** a repository selected and never synced, the pointer on Activity, its tooltip open
- **WHEN** the first sync brings Update and "Last synced just now"
- **THEN** Activity and the landing indicator keep their boxes, and the tooltip of Activity stays open, in both engines
  [@test] tests/e2e/repository/header.spec.ts (`should keep Activity under the pointer, its tooltip open, as the first sync brings Update and the time of the last sync`)
