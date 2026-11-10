## Why

The line of a running robot in a branch row looked thinner and lighter than the same line on the rail right below it,
and its width jumped at the border of the row; the plain lanes did the same at 1x. The graph drew its lines as SVG
strokes in an SVG stretched over the row, the rail as boxes. A box snaps to whole device pixels; a stroke centred on a
pixel edge is antialiased over two half-covered columns. At 1x the 1.5px line of the robot was two 75% columns over
the two solid columns of the rail, at 2x four columns of 50/100/100/50% over three solid ones, and the 1px lane two
50% columns over one solid column.

## What Changes

- The straight lines of a branch row and of the rows below it are boxes at the x of the rail's lines (`bd-lane-x`),
  so a lane is the same device pixels above and below the border of a row, at any device pixel ratio.
- The widths are two variables the rail and the graph share: `--bd-lane-line-width` (1px) and `--bd-lane-span-width`,
  the path to the root and the robot's line and span, 1.5px at a device pixel ratio of 2 and above and 2px below, where
  a 1.5px box snaps to 2 anyway.
- The fork curve stays an SVG stroke, its SVG placed where the box of its parent's lane starts: an SVG snaps to the
  device pixels as that box, so the curve leaves the line above it in the same pixels.

## Capabilities

### Modified Capabilities

- `branch/branch-row`: Row: The lanes are opaque 1px strokes stretched over the row; Row: The landing span and the
  path to the root are 1.5px strokes.
- `branch/patronus`: Commits a running robot covers: The line of the robot runs over the commits it covers.
