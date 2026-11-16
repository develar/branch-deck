## MODIFIED Requirements

### Requirement: The sidebar and the diffs: The diffs scroll past their end

The diffs of a review window MUST scroll past their end, as IntelliJ's editor does, by the empty space the view needs
for its last stop (the card of the last file, an opening below the last card) to reach the top, sized from the height
of the view as it mounts and laid out again on a resize, never on a scroll. A choice, J, K, N and P bring their section
to the top. A diff in another scroller (the preview of a commit, the conflict viewer) and a file outside the review end
at their end.

#### Scenario: A short document
- **WHEN** the last card is lower than the view, and the diffs are scrolled to their end
- **THEN** the last card is at the top, the space below it empty, and it is the one the tree, the footer and the map name
- **AND** where the last card is as high as the view there is no space past the end
  [@test] src/lib/diff/layout.test.ts (`scrolls a review past its end by what its view needs for the card of the last file to reach the top, none where the last card fills the view`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should scroll past the end of a short document, so J brings each section to the top, the bar, the footer and the tree naming it, and a wheel to the end shows the space below the last card`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should scroll the diffs with a map of the files in place of their scrollbar: a click shows the file under it, the hover names it, the thumb drags and follows`)

#### Scenario: A key before the first report of the height
- **WHEN** N is pressed in a review window before the observer of the size of its diffs reported their height, which a
  page under load gets after the key
- **THEN** N stops where it stops after the report, the space past the end laid out from the height the view read as it
  mounted, and the report moves nothing
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N pressed before the diffs reported their height stops where it stops after, the space past their end there from the start`)

#### Scenario: The height read as the view mounts is the one its observer reports
- **WHEN** the diffs mount at a zoom of the app (110%, where the computed style says a size before the floor of the report)
- **THEN** they read the content box as their `ResizeObserver` reports it, by its arithmetic (`observedContentBox`): its first report changes no size of theirs, and a later resize does
  [@test] src/lib/laid-out.test.ts (`reads the box by the arithmetic of the report at a zoom, where the computed style says the size before its floor`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the diffs read as they mount the size their observer reports, at a zoom of 110% too: its first report changes no size of theirs, a later one does`)

#### Scenario: A view that ends at its end
- **WHEN** a view without the space past the end is scrolled to its end, the header of its last file below the top
- **THEN** the last file, in view entire, is the one it shows
  [@test] src/lib/diff/layout.test.ts (`shows the last file at the end of the document, whose header can never reach the top, else the file at the top`)
