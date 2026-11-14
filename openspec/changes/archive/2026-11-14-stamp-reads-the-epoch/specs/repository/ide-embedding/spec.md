## ADDED Requirements

### Requirement: View: A patch into another epoch moves the mirror to the session of now

A patch of the session of a mirror into another epoch (the session opened anew: a slot of a lease pool leased again)
applies to no view of the epoch before. `brd mcp` MUST then move the mirror to the session of the repository as at a
change of the prefix, in one change that replaces every part, between the messages of air-ide; a session that answers
no view MUST end the mirror with `sessionClosed`.

#### Scenario: A slot leased again
- **GIVEN** the main view shows a slot of a lease pool
- **WHEN** the slot goes back and an agent leases it again, and the stamp of that lease opens the session anew
- **THEN** one change of every part moves the mirror to the view of the new lease, none of the rows of the lease before,
  the main view shows the new lease with no row of the lease before selected, and the mirror holds the view of the
  session, without an end of the mirror
  [@test] crates/mcp-host/src/server_test.rs (`a_patch_into_another_epoch_asks_the_server_to_read_the_view_anew`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_slot_leased_again_moves_the_mirror_to_the_view_of_the_new_lease`)
  [@test] tests/e2e/air/air-page.spec.ts (`a slot of a lease pool the main view shows that an agent leased again moves the mirror to the view of the new lease: none of its rows is selected as the row of the same name of the lease before was, and the view subscribed once`)
