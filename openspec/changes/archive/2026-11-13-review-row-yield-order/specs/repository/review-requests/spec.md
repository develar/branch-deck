## MODIFIED Requirements

### Requirement: Card: A narrow row gives way in one order

A row too narrow for all it says MUST give way in one order: the meta first, to nothing, then the title, then the
labels, each cut with an ellipsis, never without one; what it leaves to read and its age never shrink.

#### Scenario: A long title and two labels at 1000 pt
- **WHEN** a request with a long title and two labels waits, and the window is 1000 pt wide
- **THEN** its meta is gone, its title is cut with an ellipsis, its labels, what it leaves to read and its age show whole
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should give way in a narrow row with the meta first, then the title, its labels, its reading and its age whole`)

#### Scenario: Narrow and wider in Reviews
- **WHEN** a row of Reviews with a long title is narrow, then wider
- **THEN** narrow, its meta is gone before its title is cut; wider, its title shows whole and its meta is cut
  [@test] tests/e2e/reviews/reviews.spec.ts (`should mark the user's moves alone, keep the other rows behind Show N More, and give the title the room before the meta`)
