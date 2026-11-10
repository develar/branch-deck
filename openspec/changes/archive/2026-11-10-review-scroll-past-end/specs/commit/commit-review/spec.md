## ADDED Requirements

### Requirement: The sidebar and the diffs: The diffs scroll past their end

The diffs of a review window MUST scroll past their end, as IntelliJ's editor does, by the empty space the view needs
for its last stop (the card of the last file, an opening below the last card) to reach the top, sized from the height
of the view and laid out again on a resize, never on a scroll. A choice, J, K, N and P bring their section to the top.
A diff in another scroller (the preview of a commit, the conflict viewer) and a file outside the review end at their
end.

#### Scenario: A short document
- **WHEN** the last card is lower than the view, and the diffs are scrolled to their end
- **THEN** the last card is at the top, the space below it empty, and it is the one the tree, the footer and the map name
- **AND** where the last card is as high as the view there is no space past the end
  [@test] src/lib/diff/layout.test.ts (`scrolls a review past its end by what its view needs for the card of the last file to reach the top, none where the last card fills the view`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should scroll past the end of a short document, so J brings each section to the top, the bar, the footer and the tree naming it, and a wheel to the end shows the space below the last card`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should scroll the diffs with a map of the files in place of their scrollbar: a click shows the file under it, the hover names it, the thumb drags and follows`)

#### Scenario: A view that ends at its end
- **WHEN** a view without the space past the end is scrolled to its end, the header of its last file below the top
- **THEN** the last file, in view entire, is the one it shows
  [@test] src/lib/diff/layout.test.ts (`shows the last file at the end of the document, whose header can never reach the top, else the file at the top`)

## MODIFIED Requirements

### Requirement: The sidebar and the diffs: A file chosen in the tree scrolls its header to the top

A file chosen in the tree MUST scroll its header to the top, the last file too: the diffs scroll past their end.

#### Scenario: A row of the tree
- **WHEN** the user chooses a file in the tree
- **THEN** the diffs scroll its header to the top
- **AND** the header of the last file reaches the top too, and it is shown and selected
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: The file whose header is at the top is the selected one

As the diffs scroll, the file whose header is at the top MUST become the selected file, its directories opened;
scrolled to their end, past it, the last file is at the top and is the selected one, the position and the map following
it.

#### Scenario: The diffs scroll
- **WHEN** the diffs scroll
- **THEN** the file whose header is at the top becomes the selected file, its directories opened
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should scroll the diffs with a map of the files in place of their scrollbar: a click shows the file under it, the hover names it, the thumb drags and follows`)

#### Scenario: The diffs scrolled to their end
- **WHEN** the diffs are scrolled to their end
- **THEN** the last file is at the top and is the selected one, the position and the map following it
  [@test] src/lib/diff/layout.test.ts (`scrolls a review past its end by what its view needs for the card of the last file to reach the top, none where the last card fills the view`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should scroll the diffs with a map of the files in place of their scrollbar: a click shows the file under it, the hover names it, the thumb drags and follows`)

### Requirement: The sidebar and the diffs: A scroll the review made selects through its key or row

A scroll the review made (a choice, J, K, N, P) MUST change the selection only through the key or the row that made it,
its section brought to the top.

#### Scenario: A choice of the last file
- **WHEN** a choice, J, K, N or P scrolls the diffs, also to the last file
- **THEN** the selection is the one of the key or the row that made the scroll, its section at the top
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
