## MODIFIED Requirements

### Requirement: View: A patch into another epoch moves the mirror to the session of now

A patch of the session of a mirror into another epoch (the session opened anew: a slot of a lease pool leased again)
applies to no view of the epoch before. The bridge MUST then read the view of the session anew (`get_snapshot` of the
window of the mirror) and take it in one change that replaces every part, the patches that came meanwhile after it; a
session that answers no view MUST end the mirror with `sessionClosed`. The bridge MUST tell each news of a view it
reads anew, or takes from a resync, and of the patches it caught up after it, that it has not told
(`ide_host::mirror::untold_news`): the news past the last one it told of the numbering of the view
(`RepositoryView::news_line`), which a session opened anew in place numbers on, so no news of a patch that came while it
read is lost and none it told before is told again; the news of a view of another numbering (its first view, the
session of another prefix) were told before the mirror came, and it tells none of them.

#### Scenario: A slot leased again
- **GIVEN** the main view shows a slot of a lease pool
- **WHEN** the slot goes back and an agent leases it again, and the stamp of that lease opens the session anew
- **THEN** one change of every part moves the mirror to the view of the new lease, none of the rows of the lease before,
  the main view shows the new lease with no row of the lease before selected, and the mirror holds the view of the
  session, without an end of the mirror
  [@test] crates/ide-host/src/server_test.rs (`a_patch_into_another_epoch_reads_the_view_anew_and_tells_the_news_of_the_patches_that_came_meanwhile`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_slot_leased_again_moves_the_mirror_to_the_view_of_the_new_lease`)
  [@test] tests/e2e/air/air-page.spec.ts (`a slot of a lease pool the main view shows that an agent leased again moves the mirror to the view of the new lease: none of its rows is selected as the row of the same name of the lease before was, and the view subscribed once`)

#### Scenario: A robot fails while the mirror reads the view anew
- **GIVEN** the main view is hidden, and the mirror reads the view of its session anew (a patch into another epoch, or
  after a patch it lacks)
- **WHEN** a patch that comes meanwhile brings the failure of a robot
- **THEN** the failure is a notification of the IDE once the mirror caught the patch up
  [@test] crates/ide-host/src/server_test.rs (`a_patch_into_another_epoch_reads_the_view_anew_and_tells_the_news_of_the_patches_that_came_meanwhile`)
  [@test] crates/repository-model/src/session_test.rs (`a_session_opened_anew_numbers_its_news_on_under_the_line_of_the_open_before`)
  [@test] crates/repository-model/src/facts_test.rs (`the_news_of_a_store_numbered_on_go_on_after_the_last_id_of_the_open_before`)

#### Scenario: A news told before the view read anew
- **WHEN** the view the mirror reads anew holds a news it told before, beside one it did not
- **THEN** only the one it did not tell is told, in the same epoch and in the next open of the session; of a view of
  another numbering none
  [@test] crates/ide-host/src/server_test.rs (`a_view_read_anew_tells_no_news_the_mirror_told_before_it`, `a_view_read_anew_of_another_numbering_tells_none_of_its_news`)
