## Why

On a branch whose Safe Push runs, the graph drew the robot in pieces. The grey line of the branch row ran from the
node through the lower arc of the ring (it started at the node's 4 units, which the row stretches, inside the ring's
7.75px), then turned into the 1.5px primary line of the rail at the bottom of the row: the landing span gave the row
of the robot's own branch no line down. The dots of the commits the robot took stayed grey, and a robot that will
fail kept a primary line under its red ring.

## What Changes

- The lines of a branch row stop at the edge of its node, or of the ring of its robot, measured in px from its centre
  whatever the height of the row: a mask of the SVG (`bd-node-cut`), so the line up, the line down and the fork curve
  stop alike, and the short line of a collapsed root starts below the ring.
- A running robot that covers the first expanded commit of a branch draws its line from the node down to the rail
  (`useRobotLineIntoRail`): its own branch, a branch a robot below lands, and a branch of the whole stack.
- The dots of the commits the robot took are in the colour of its line (`data-robot-dot="covered"`).
- The line of a robot, its span in the graph and its dots are one CSS variable, `--bd-robot-line`: primary, and
  destructive while the robot will fail (`bd-robot-failing`), as its ring.
- The last commit row of a branch without children draws the robot's line in place of the plain end of its lane.

## Capabilities

### Modified Capabilities

- `branch/patronus`: Commits a running robot covers: The line of the robot runs over the commits it covers.
- `branch/branch-row`: Row: The lanes are opaque 1px strokes stretched over the row; Row: The landing span and the
  path to the root are 1.5px strokes.
