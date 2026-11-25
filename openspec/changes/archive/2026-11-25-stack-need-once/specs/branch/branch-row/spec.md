## MODIFIED Requirements

### Requirement: Status cell: The state the next step waits in is a status, never an action

Where no fact of the branch's own says more, the first column MUST say the state its next step waits in
(`RowFact::Wait`, [Next Step](../next-step/spec.md)), and the slot MUST show no step of it: the Status column says
where the work stands and whose move it is, the Actions column only what the user can do.

#### Scenario: A branch awaiting a review
- **WHEN** the next step of a branch waits for the reviewers of its pull request, and nothing of its own says more
- **THEN** the first column says "Awaiting review" ("Changes requested" in `destructive`, "Draft", "Into release-252", "In #12", "Merges when checks pass", "Needs stills-codec with lane-run" of a branch of a whole stack its root's row hosts), muted, its icon where the dot is, truncated to the track; its hover the whole state
- **AND** the slot of the Actions shows no step, and a click on the status opens the pull request
  [@test] crates/domain-policy/src/row_status_test.rs (`a_wait_is_the_state_of_the_cell_where_nothing_of_its_own_says_more`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

### Requirement: Status: The items are capped tracks after a gutter

The items MUST be the tracks `state` (capped at `max-w-40`, its text truncates; the state a step waits in, which stands
for the empty step, at `max-w-64`, `data-step-wait`), `pr` and `landed` (capped at `max-w-48`),
each after an 8px gutter while its column has content; `landed` is a `flex items-center gap-2` of the Landed chip, then the
`text-xs text-muted-foreground` "lands with oauth" or "on master".

#### Scenario: A running robot in the state track
- **WHEN** a running robot is in the `state` track
- **THEN** it is at least `13ch` wide ("59m · ~1h 59m"), so its ticking time moves no column

#### Scenario: The state a step waits in
- **WHEN** a row waits ("Lands after stills-codec") and its step's slot is empty
- **THEN** the state reads whole at 1280px and 960px, up to 16rem, never "Lands after stills-co…"
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say on the root's row that the whole stack lands after the running Safe Push of what it needs, with no start in its slot and Start Anyway in the popover's ⋯`)
