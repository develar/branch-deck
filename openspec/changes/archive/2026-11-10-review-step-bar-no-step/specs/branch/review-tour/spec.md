## MODIFIED Requirements

### Requirement: In the Review Window: The Steps list each step by its number and full title

The Steps MUST list each step by its number and its full title (the title Rust gives the commit that starts it; a step
of the concerns its concern), wrapping, never cut, which shows its opening, then the lead of its body; "Other changes"
remains as a rest item without a number and MUST be excluded from the count beside Steps and the step count in the bar,
the bar and the hover of the map of the files naming it without a number too.

#### Scenario: The steps of the overview
- **WHEN** the overview lists the steps
- **THEN** each shows its number and its full title, wrapping, never cut, then the lead of its body
- **AND** a click on it shows its opening
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`, `should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it`)
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

#### Scenario: Other changes
- **WHEN** the plan has a rest of other changes
- **THEN** "Other changes" is listed without a number
- **AND** it is excluded from the count beside Steps and from the step count in the bar
- **AND** the bar and the hover of the map name it without a number: "Other changes", never "Step 2 · Other changes"
  [@test] src/presenters/review-story.test.ts (`does not give Other changes a number in the overview`)
  [@test] src/presenters/review-steps.test.ts (`words Other changes in the hover of the map of the files by its name alone, as it has no number of a step`)

### Requirement: In the Review Window: A fold names the changes another section shows

A fold MUST count only the unchanged lines it hides and name after them each place that shows a block it hides, with
the count of those blocks ("⋯ 9 unchanged lines · 1 change in part 2", "in Other changes" of another concern, a group
by its name alone, "in Imports only", never the count of its heading), a click there showing that section; it MUST
name the lines of a block the whitespace policy ignores ("· 1 line hidden by Whitespace").

#### Scenario: A fold over the change of another part
- **WHEN** a fold of a section hides a block another section shows: an import the group of the imports shows
- **THEN** it counts the unchanged lines alone, 66 where the old side holds 66 and the new one 67
- **AND** it says "1 change in part 2", and a click there shows that section, expanded
  [@test] src/lib/diff/rows.test.ts (`names in a fold of a section the blocks another section shows that it hides, and counts only its unchanged lines`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A fold over the change of a group
- **WHEN** a fold of a section hides a block a group shows: the import of the group headed "Imports only (1)", a line
  of a repeated edit on a file after the first of the group
- **THEN** it names the group by its name alone, "1 change in Imports only", "1 change in Same edit in 2 files"
  [@test] src/presenters/review-units.test.ts (`names a block a group shows by the name of the group on each of its files, never by the count of its heading`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A fold over a line the policy ignores
- **WHEN** a fold hides blocks the whitespace policy ignores: a deleted blank line, a line indented again
- **THEN** it says "24 unchanged lines · 2 lines hidden by Whitespace"
  [@test] src/lib/diff/rows.test.ts (`counts the unchanged lines a fold hides, apart from the lines of the blocks the policy ignores there`)

## ADDED Requirements

### Requirement: In the Review Window: The bar names what is no step alone

What the story tells as no step MUST be named alone in the bar above the diffs, with no place among the steps: "Other
changes" at its opening and its cards, a group the story shows after its steps by its name without the count of its
heading ("Imports only"), the footer naming no step there; a use left outside is of the step of the section before
it. A click shows the opening of Other changes, the first card of such a group.

#### Scenario: The bar at what is no step
- **WHEN** Other changes, or a group after the steps, is at the top of the diffs of a review of one step
- **THEN** the bar names it alone, "Other changes", "Imports only", never "Step 2 / 1", and the footer names no step
- **AND** a click on the bar at Other changes shows its opening
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should name in the bar above the diffs a step by its place, Other changes and the group of the imports after the story by their name alone, a click showing the opening of Other changes`)
  [@test] src/presenters/review-units.test.ts (`names in the bar above the diffs a step by its place among the told steps and its title, Other changes and a group after the story by their name alone`, `names in the bar a use left outside by the step of the section before it, a group after the story from any of its files, a file of no unit by Other changes, and has no bar in a story that tells no step`)
