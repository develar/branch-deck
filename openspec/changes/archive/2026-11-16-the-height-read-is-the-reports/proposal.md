# The height the diffs read as they mount is the one their observer reports

## Why

The review of e4acc6530 (bd-flakes-review): the diffs read their size as they mount by the computed style
(`laidOutBoxes`), which at a zoom of the app says a size before the floor of the report of their `ResizeObserver`
(WebKit 680.894897 against 680.890625 at 110%, Chromium 680.895): the first report changed the width and the height of
the view, and laid its document out again.

## What Changes

- `observedContentBox` of src/lib/laid-out.ts: the content box by the arithmetic of the report, the border box over
  `currentCSSZoom`, less borders, padding and bars, floored to 1/64 px; equal to the first report at 90%, 100%, 110% and
  125% in WebKit and Chromium. The diffs read it as they mount.
- Capability `commit/commit-review`: a scenario of the height read as the view mounts.
