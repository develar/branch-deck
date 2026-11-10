## MODIFIED Requirements

### Requirement: Row: The lanes are opaque 1px strokes stretched over the row

The lanes (`StackLanes.tsx`) MUST be 1px strokes in `--bd-lane-line`, the muted foreground at 40% over the card: opaque,
so the curve of a fork doesn't darken the line it leaves, and stronger than a separator. One SVG per cell stretches over
the row (`preserveAspectRatio="none"`, `vector-effect: non-scaling-stroke`); the lines into a node stop at its edge, or
at the outer edge of the ring of its robot, in px from its centre (`bd-node-cut`).

#### Scenario: The fork of a side child
- **WHEN** a side child forks off its parent's lane
- **THEN** the fork is a quadratic curve from the parent's lane at the top of the row into the left edge of the node, and the lines end at the edge of the node

#### Scenario: A running robot around the node
- **WHEN** the robot of the branch runs
- **THEN** the line up, the line down and the fork curve stop at the outer edge of its ring, whatever the height of the
  row, and the short line of a collapsed root starts below the ring

### Requirement: Row: The landing span and the path to the root are 1.5px strokes

The segments of a landing span MUST be a 1.5px stroke in `--bd-robot-line` over the lines (`landing-lanes`), the colour
of the ring and of the robot's progress line: `--color-primary`, `--color-destructive` while the robot will fail. The
path to the root MUST be a 1.5px stroke in `--bd-lane-path`, the foreground at 70% over the card, under the landing span
(`stack-path-lanes`).

#### Scenario: The path of a hovered branch
- **WHEN** the path to the root of a branch is highlighted
- **THEN** it fades in over 120 ms (`bd-path-in`,
  `@starting-style`; none under reduced motion) and is gone at once
- **AND** a node the hovered push sends gets a 1.5px `outline-current` 2px off it

#### Scenario: The span of a robot that will fail
- **WHEN** a required check of the robot that lands the span failed
- **THEN** its segments are `destructive` (`data-failing`), as its ring
  [@test] src/lib/stack-lanes.test.ts (`marks the span of a robot that will fail, the lanes passing the rows between too, and leaves the others`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line and the dots of a robot that will fail in the colour of its ring`)
