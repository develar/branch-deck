# Proposal

## Why

Since 0e309ea3 a loop that leaves an element of the observers of the page pending is an error, and master failed
`review-groups.spec.ts` 4 times in 16: "A ResizeObserver loop of an observer of the page: left div pending". Its trace:
in the delivery of a report, a block above the code of the review grew, and the view, its file map and the parent the
painter observes shrank by 28 px. That change was in none of their subtrees and of none of their ancestors, so the
delivery kept their observations, which the update then skipped. The layout is the page's: any change can resize any
element.

## What Changes

- While a callback of an observer runs (of the page or of a library) and the microtasks right after it, the page is
  watched for any change; on one, every element of ours no deeper than the elements of the latest report leaves its
  observers. A change after the update lets nothing go: the next update delivers it in its own frame. Outside those
  callbacks the page is not watched at all.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: which elements a change of the rendering update lets go.

## Impact

- `src/lib/observe-size.ts`, `docs/contributing/frontend.md`, `docs/design/ui-theme.md`, `docs/design/diff-view.md`.
