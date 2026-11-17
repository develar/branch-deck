## MODIFIED Requirements

### Requirement: Status: The items are capped tracks after a gutter

The items MUST be the tracks `state` (capped at `max-w-40`, its text truncates), `pr` and `landed` (capped at `max-w-48`),
each after an 8px gutter while its column has content; `landed` is a `flex items-center gap-2` of the Landed chip, then the
`text-xs text-muted-foreground` "lands with oauth" or "on master".

#### Scenario: A running robot in the state track
- **WHEN** a running robot is in the `state` track
- **THEN** it is at least `13ch` wide ("59m · ~1h 59m"), so its ticking time moves no column

## ADDED Requirements

### Requirement: Status: A running robot reads whole and no item paints over the next track

A running robot is a live value, read whole: the cap of the `state` track MUST leave it (`data-live-width`), and its
track widens for it. What an item paints MUST lie inside its track: a content wider than the cap truncates, never paints
over the next track.

#### Scenario: A robot past its usual time beside a pull request
- **WHEN** a robot runs past its usual time ("1h 57m · usually ~1h 39m", wider than the cap) and its branch has a pull request
- **THEN** its time reads whole, the `state` track widens for it, and the pull request chip stands after it, in every row
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`widens the Status column for a robot past its usual time, which paints over no chip beside it`)
