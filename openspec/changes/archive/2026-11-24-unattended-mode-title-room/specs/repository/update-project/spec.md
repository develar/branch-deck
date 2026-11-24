## MODIFIED Requirements

### Requirement: Header: The header stays one line down to the minimum window width

The header MUST stay one line down to the minimum window width, 960 px (`min_inner_size`), its controls apart:
the repository selector gives way first; the branch count shows while the filter bar is hidden, from
448 px (`@md`) of the room of the repository; the time of the last sync from 768 px (`@3xl`) of the header (992 px
under ☰ in Unattended Mode); Update, the landing indicator, Activity ([Activity](../activity/spec.md), with an empty
log too), Sync and its settings never shrink.

#### Scenario: The minimum window width
- **GIVEN** the window is 960 px wide (`min_inner_size`)
- **WHEN** the header is shown
- **THEN** it stays one line, its controls never overlapping, and the repository selector gives way first
- **AND** Update, the landing indicator, Activity (there with an empty log too), Sync and its settings never shrink
  [@test] tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)

#### Scenario: The branch count
- **GIVEN** the filter bar is hidden: below 8 branches, as from 8 branches on its "All 35" counts them
- **WHEN** the room of the repository (the selector, the views and the count) is 448 px (`@md`) or more
- **THEN** the header shows the branch count

#### Scenario: The time of the last sync
- **WHEN** the header is 768 px (`@3xl`) wide or more, 992 px (`@min-[62rem]`) between ☰ and the window buttons of a custom title bar while Unattended Mode is on
- **THEN** it shows the time of the last sync
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the toolbar between the main menu and the window buttons on one line at every width with Unattended Mode on, its word where the header has room`)
