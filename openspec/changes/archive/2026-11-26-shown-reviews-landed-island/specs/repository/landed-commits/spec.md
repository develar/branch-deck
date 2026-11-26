## ADDED Requirements

### Requirement: After the update: A branch that left the table gets its review after landing from its row

A row of the island of a branch that left the table MUST offer Request Review After Landing… (`landed:<name>`), its
review after landing opened now on the commit that landed it ([Shown Reviews](../../branch/shown-reviews/spec.md)),
refused without Patronus or GitHub, with its review after landing there already, and while nothing tells that commit
(`show_nothing_landed`); Activity offers none.

#### Scenario: From the row of feature-auth
- **WHEN** a robot of the user landed feature-auth, the update took it out of the table, and the user presses Request
  Review After Landing… of its row and picks alice
- **THEN** the review opens ready on what the robot landed and asks alice
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should open the review after landing of a branch that left the table from its row of the Landed island, asking the reviewers picked there`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_branch_that_landed_and_left_the_table_offers_its_review_after_landing_refused_without_what_it_opens_on`)

