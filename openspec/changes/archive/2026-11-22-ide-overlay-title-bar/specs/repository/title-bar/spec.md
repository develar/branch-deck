## ADDED Requirements

### Requirement: macOS: In a window of the IDE the row is in its title bar, and the IDE moves the window

Where `bd_bootstrap` answers `{"kind":"overlay","doubleClick":"host","height":…}`, the row MUST start after the
traffic lights of the IDE at the height of the IDE. The page MUST leave the drag and the double click to the IDE
(`installTitleBarMouse` installs no double click), and the row MUST start at the edge while the IDE says full screen.

#### Scenario: The Settings window in the IDE
- **WHEN** the Settings window opens in the IDE on macOS
- **THEN** its row is 40px high and starts 88px from the left, its empty parts a drag region
- **AND** a press and a double click there go to the IDE, which moves or zooms the window
- **AND** in full screen the row starts at the edge
  [@test] tests/e2e/air/air-page.spec.ts (`the row of the Settings window is in the title bar of the IDE: it starts after the traffic lights, a press on its empty parts moves the window and a double click is the double click of the system, full screen moves it to the edge, and the row of the tool window is no drag region`)
