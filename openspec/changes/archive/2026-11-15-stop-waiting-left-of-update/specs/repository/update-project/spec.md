## MODIFIED Requirements

### Requirement: Header: Stop Waiting ends the wait with nothing changed but the fetch

Stop Waiting MUST come on the left of the Update button, where the button stays under the pointer (the header stands at
its end), and MUST end the wait with nothing changed but the fetch ("Canceled the update before it started"):
`cancel_operation` of its operation, whose update is the failed effect of its outcome, `canceled`.

#### Scenario: Stop Waiting comes beside a hovered button
- **GIVEN** the pointer rests on the Update button of a running update, its steps open
- **WHEN** the update starts to wait for the repository lock
- **THEN** Stop Waiting comes on the left of the button, which stays where it was, its steps open
  [@test] tests/e2e/update-project/update-project.spec.ts (`should show Stop Waiting where the Update button stays under the pointer, its steps open, and stop the wait with it`)

#### Scenario: The user stops the wait
- **GIVEN** the update waits for the repository lock
- **WHEN** the user clicks Stop Waiting beside the Update button
- **THEN** `cancel_operation` of its operation ends the wait, its update the failed effect of its outcome, `canceled`
- **AND** nothing changed but the fetch, "Canceled the update before it started"
  [@test] src/stores/update-project.test.ts (`names whom it waits for while another holder has the repository lock, and Stop Waiting cancels its operation`)
  [@test] crates/operations/src/update_test.rs (`stop_waiting_ends_the_wait_of_an_update_for_the_lock_with_nothing_moved`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should say whom an update waits for after its check, whose Stop Waiting changes nothing but the fetch`, `should show Stop Waiting where the Update button stays under the pointer, its steps open, and stop the wait with it`)
  [@test] src/lib/update-state.test.ts (`names whom an update waits for while another holder has the repository lock, with Stop Waiting, until its next step`)
