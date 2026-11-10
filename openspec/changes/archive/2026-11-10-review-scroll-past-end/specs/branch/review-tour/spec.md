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
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should go through the steps from the overview, by J and K and by the title of an opening, the bar and the tree following`)

#### Scenario: The bar at the end of a document
- **WHEN** J goes through the sections of a document shorter than the view below its first part, to the last
- **THEN** each comes to the top, the diffs scrolling past their end, and the bar, the footer and the tree name it
- **AND** a wheel to the end shows the last card at the top, the space past the end empty below it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should scroll past the end of a short document, so J brings each section to the top, the bar, the footer and the tree naming it, and a wheel to the end shows the space below the last card`)
