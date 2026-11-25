## MODIFIED Requirements

### Requirement: Overview: Hovering or a click opens the overview

Hovering the button (after a short delay) or a click MUST open the overview; it SHALL be loaded while the delay runs.
The pointer that leaves the button MUST close an overview its hover opened, also after the button changed what it shows
under the still pointer.

#### Scenario: The pointer rests on the button
- **WHEN** the pointer hovers the button for a short delay
- **THEN** the overview opens, loaded while the delay ran
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: A click on the button
- **WHEN** the user clicks the button
- **THEN** the overview opens

#### Scenario: The button changes under the pointer
- **GIVEN** the pointer rests on the button and the overview its hover opened shows
- **WHEN** a robot fails, so the count and the bar of the button change under the pointer, and the pointer then leaves
- **THEN** the overview closes, in WebKit as in Chromium
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should close as the pointer leaves the button, after the work the button shows changed under the still pointer`)
