## ADDED Requirements

### Requirement: Card: A review after landing carries the badge Shown

A request whose pull request is a review after landing (`ReviewRequest.shown`, Rust's by its base, of any prefix) MUST
carry the badge "Shown" after its title, in Your Turn, in Reviews and in the header of its review window, its hover
"In master already: your review gates no Safe Push, and the approval of every reviewer it asked closes it"; its meta
names no base, which is the fork point of what landed.

#### Scenario: A review after landing of carol
- **WHEN** carol's review after landing of login waits for the user's review
- **THEN** its row and its review window carry "Shown", its hover says what it is
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should tell a reviewer a review after landing by its badge, offer no Safe Push of it, and take the approval`)
