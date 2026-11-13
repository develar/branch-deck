## MODIFIED Requirements

### Requirement: Read ahead: A listing reads ahead the requests without their last reading

Each listing GitHub answered MUST read ahead the waiting requests the session has no last reading of at the head and
the base the listing names: never one being read; again one that could not be read and one whose renames wait for the
index, which only an index of the repository makes them do: a reading whose renames no index can answer is final. A repository that reads no review plans (`branchdeck.reviewExperimental`) reads none, and its rows show nothing.

#### Scenario: What a listing asks
- **WHEN** a listing names three requests, one being read, one read, one whose renames wait; then a new head of one
- **THEN** only the one that waits is asked, a failure is asked again, and the new head is asked and shows nothing of the old one
  [@test] crates/repository-model/src/review_readings_test.rs (`a_listing_asks_the_waiting_requests_without_their_last_reading`)
  [@test] crates/repository-model/src/session_test.rs (`a_listing_reads_ahead_its_waiting_requests_and_the_prunes_keep_their_refs`)

#### Scenario: A rename no index of the repository answers
- **WHEN** a request renames a private member of a repository without an index, and the next listing names it again
- **THEN** its reading is final, not proven, and the next listing reads only the request it adds
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say a private member renamed where the repository has no index is not proven, and not read it again on the next listing`)
