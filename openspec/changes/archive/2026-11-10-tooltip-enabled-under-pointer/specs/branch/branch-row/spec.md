## ADDED Requirements

### Requirement: Hover and click: A tooltip enabled under the pointer opens after its delay

A tooltip disabled while it has nothing to say MUST open after the delay of its hover when it gets its words with the
pointer that moved over its trigger resting on it, as for a pointer that came then; a trigger the pointer did not move
over, which came under a still pointer, stays closed until the next move.

#### Scenario: The description of Approve that came late
- **WHEN** the pointer rests on Approve in a review window while the code-owner review of the pull request has not come, so Approve has no description, and the delay of its hover ran out
- **AND** the review comes, the pointer still
- **THEN** the description of Approve opens after the delay of its hover
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open the description of Approve that came while the pointer rested on it, after the delay of its hover`)
