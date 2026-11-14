## MODIFIED Requirements

### Requirement: The sidebar and the diffs: A map of the files stands in place of the scrollbar

The diffs MUST have a map of the files in place of their scrollbar: a column at their right, beside the diffs and not
above them, the whole review scaled to its height (never shorter than the view, so a short review keeps its files where
its cards are). The map MUST observe the sizes of its track and of the view once, as it mounts: a new layout of the
diffs observes nothing anew.

#### Scenario: The scale of the map
- **WHEN** the review shows its diffs
- **THEN** a column at their right, beside them, maps the whole review scaled to its height
- **AND** a review shorter than the view is scaled as the view, so its files stay where its cards are
  [@test] src/lib/diff/file-map.test.ts (`scales the document, never shorter than the view, to the height of the map, a tick at the start of each section`)

#### Scenario: The diffs lay out again
- **WHEN** the review opens, and its diffs lay out again (the heights they measure, the size of the view, a file expanded)
- **THEN** the map observes its track once, and the view lays out no frame again for an observation made anew (a ResizeObserver loop)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should observe the size of the map of the files once as the review opens, however often its diffs lay out`)
