## MODIFIED Requirements

### Requirement: Picture: The waves are columns and each unit a card

The Plan of Landing MUST show the waves as columns from left to right, each unit a card: its node (with the ring of a
running robot), its name, its size, the time of its robot, the required checks it starts ("7 checks", named in the
hover); a chain of needs on one row where it can (`layoutPlan`), each card at the top of its row; columns 10 to 18rem
wide, past the room of the island scrolling across with a fade at the side that has more; the parked branches apart. A
stale plan MUST say so and offer nothing. Every unit, wave and edge is Rust's.

#### Scenario: A robot that lands, a branch next, two that wait
- **GIVEN** `stills-codec` landing, `lane-run` (with `ui-perf-nightly`) using its `Stills`, and `lane-encode` using
  `Bundle` of `lane-bundle`, which nothing lands
- **WHEN** the user opens the Plan
- **THEN** the waves are "Landing now: stills-codec", "Next: lane-bundle", "After stills-codec: lane-run", "After
  lane-bundle: lane-encode", `stills-codec` with the time of its robot, `lane-run` "2 branches · 2 commits"

#### Scenario: The narrowest islands
- **GIVEN** the four waves wider than the island
- **WHEN** the Plan shows
- **THEN** a fade on the right says it scrolls, and the focus moved to a card past it scrolls the card into view
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should show the waves of the plan: the running robot now, what is ready next, what waits after what, each edge with its evidence on hover, and Land Together framed with its price, undone in one step`)
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should show the waves of the plan: the running robot now, what is ready next, what waits after what, each edge with its evidence on hover, and Land Together framed with its price, undone in one step`)
  [@test] src/lib/landing-plan-layout.test.ts (`lays the waves of idea-1 out in columns, each chain on one row past the column between and a pair to land together on one row with its frame below`, `frames the second unit that waits for one needed unit in its own column, its price row kept free`)
  [@test] src/presenters/landing-plan.test.ts (`names the waves, the size and the checks of a unit`)

### Requirement: Picture: An edge is a curve with its evidence on hover

Each edge between two units of the plan MUST be a curve between the headers of their cards, its evidence in a chip
halfway ("Effect, Input +3"), the sentence of the edge and its files in the hover, which fades every card, curve and
frame off the edge; an edge no curve draws (an unassigned commit, a cycle, a companion) a line on the card of its unit;
the actions of an edge that is no offer below it, from the one builder.

#### Scenario: The evidence of a wait
- **WHEN** the user hovers the chip of the edge `lane-run → stills-codec`
- **THEN** it says "Stills", its hover "Uses Stills of stills-codec in BuildLog.kt: stills-codec is landing in its Safe
  Push, …" and `lane/src/BuildLog.kt`, the cards of `lane-run` and `stills-codec` as they are and the others faded
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should show the waves of the plan: the running robot now, what is ready next, what waits after what, each edge with its evidence on hover, and Land Together framed with its price, undone in one step`)
  [@test] src/presenters/landing-plan.test.ts (`says an edge by its evidence in a few words and its state`)

### Requirement: Picture: An offer is a frame around the pair with its price and its buttons

Land Together MUST show in the plan as a dashed frame around the cards of the pair, on one row, its price below as one
sentence of its risks ("1 robot instead of 2 · checks unknown · a failure holds both", each whole in its hover), then
the actions of its edge (Land Together, "Land lane-bundle First"); its press MUST restack on fresh needs and keep the
Plan, and one undo MUST put the two units back. Of a stack that forks the price MUST say it lands as the whole stack, and
the toast of the restack MUST offer "Safe Push Whole Stack" beside Undo, as Rust offers it. The plan offers Land
Together nowhere without its price.

#### Scenario: Land Together from the plan, and Undo
- **WHEN** the user presses Land Together in the frame of `lane-encode` and `lane-bundle`
- **THEN** `lane-bundle` is one unit of 2 branches, the frame is gone, and Undo puts the two units and the offer back
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should show the waves of the plan: the running robot now, what is ready next, what waits after what, each edge with its evidence on hover, and Land Together framed with its price, undone in one step`)
  [@test] src/presenters/landing-plan.test.ts (`words the price of Land Together as its risks, one by one`)

#### Scenario: A stack that forks
- **GIVEN** `lane-run` forks into `lane-docs` and `ui-perf-nightly`
- **WHEN** the user presses "Land Together · Safe Push Whole Stack" in its frame with `stills-codec`, whose price says "1
  robot instead of 2, the whole stack"
- **THEN** `stills-codec` is one unit of 4 branches whose next step is the Safe Push of its whole stack, "Safe Push ·
  4", which its stack branch holds while it is not built (`StackNotBuilt`)
- **AND** the toast "Stacked lane-run on stills-codec" offers "Safe Push Whole Stack" beside Undo, whose press opens
  the start of the whole stack at the row of `stills-codec`
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should land a stack that forks together with the branch it needs, the Safe Push of the whole stack its next step and the action of its toast`)
  [@test] src/presenters/landing-plan.test.ts (`words the price of Land Together as its risks, one by one`)

### Requirement: Popover: Land Together says its price

Where the Safe Push popover offers Land Together, it MUST say its price under the actions in the words of the plan, one
muted sentence that wraps between words.

#### Scenario: The price in the popover
- **WHEN** the popover of `lane-run` offers Land Together with `stills-codec`
- **THEN** it says "Land Together: 1 robot instead of 2 · checks unknown · a failure holds both"
  [@test] tests/e2e/next-step/land-together.spec.ts (`should land a branch with the branch it needs in one robot from the Safe Push popover, one unit in the Landing overview, and undo it`)
