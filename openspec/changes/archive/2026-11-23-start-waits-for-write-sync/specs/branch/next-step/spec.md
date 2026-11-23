## ADDED Requirements

### Requirement: The readiness store: The session asks the readiness of the subject of a start that runs

The session MUST ask the readiness of the subject of each start that runs in it and whose decision waits for it (never
a Dry Run or a start without approvals), whatever its row shows: a plan decided again after the sync that rebuilt its
branch waits for the review of its new commits.

#### Scenario: The subject of a start that runs
- **WHEN** a Safe Push of a subject runs in the session, or a Dry Run of it
- **THEN** the session asks its readiness for the Safe Push, whatever its row shows, and none for the Dry Run
  [@test] crates/repository-model/src/readiness_facts_test.rs (`the_subject_of_a_running_start_is_asked_whatever_its_row_shows`)
