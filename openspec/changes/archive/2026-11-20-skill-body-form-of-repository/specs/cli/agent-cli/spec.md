## MODIFIED Requirements

### Requirement: `brd install`: The skill says how to write the commit body a review reads

Below the subject the skill MUST say, in six lines, the commit body a review reads: the body has the form of the
repository; code names in backticks and `Look closely:` fit every form; with no form of its own, the lead, a paragraph
per decision, the labels of the verification, the trailers last
([The Commit Body a Review Reads](../../../../docs/guide/commit-bodies.md)).

#### Scenario: The commit body
- **WHEN** an agent reads the skill below the commit subject
- **THEN** six lines say how to write the commit body a review reads, and the form of the repository comes before the
  labels of the verification
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_says_how_to_write_the_commit_body_a_review_reads_under_the_subject`)
