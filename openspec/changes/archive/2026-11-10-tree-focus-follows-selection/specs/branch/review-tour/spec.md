## MODIFIED Requirements

### Requirement: In the Review Window: The bar of the step at the top is one line

Above the diffs the bar of the step of the section at the top (`review-step-bar`) MUST be one line, "Step 3 / 4 · The
view paints with the first frame", the whole title in its hover and no text of the body, "Overview · 4 steps" while
the overview is at the top. It MUST follow the diffs as they scroll and as J, K, a row of the tree and a step of the
overview move them, and a click shows the opening of the step (of the first one from the overview).

#### Scenario: The bar as the diffs scroll
- **WHEN** the diffs scroll, or J, K, a row of the tree or a step of the overview move them
- **THEN** the bar follows, naming the step of the section at the top, "Overview · 4 steps" while the overview is at the top
- **AND** a click on the bar shows the opening of the step, of the first one from the overview
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

#### Scenario: The bar at the end of a document
- **WHEN** J shows a section that the end of the document keeps from the top of the view, then the reader scrolls to
  the end
- **THEN** the bar, the footer and the tree name that section while the reader does not scroll, as the selection of a
  scroll the review made does
- **AND** once the reader scrolls to the end they name the last section, in view entire
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should name one section in the bar, the footer and the tree at the end of the document: the one J showed until the reader scrolls, then the last one`)

### Requirement: In the Review Window: J and K walk the document

J and K MUST walk the document: from the overview J shows the opening of the first step, from an opening its card,
from a card the next card or the opening of the next step; K the same way back, from the opening of the first step to
the overview. The first J of a review that opens on its overview, nothing scrolled nor chosen yet, MUST show the
opening of the first step too.

#### Scenario: J and K through the openings and the cards
- **WHEN** the user presses J from the overview and on
- **THEN** J shows the opening of the first step, then its card, then the next card or the opening of the next step
- **AND** K goes the same way back, from the opening of the first step to the overview
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

#### Scenario: The first J of a review just opened
- **WHEN** a review opens on its overview and the user presses J before anything scrolled the diffs or chose a section
- **THEN** J shows the opening of the first step, its row selected
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show the opening of the first step on the first J of a review just opened on its overview`)
