## MODIFIED Requirements

### Requirement: serve: The supervisor gives the slot back on every way out

On every way out, after `exit.json`, the supervisor MUST give the slot back (`give_back_run`), which also deletes the
run ref. No signal cuts the give-back. Each line of the supervisor after `exit.json` (the give-back, a quarantine, the
heavy turn, their failures, a panic) MUST go to the log of the agent with the id of the run too: a client forgets the
run once it ended, and `supervisor.log` goes with its directory.

#### Scenario: The end of a run
- **WHEN** a run ends in any way
- **THEN** after `exit.json` the supervisor gives the slot back and deletes the run ref, and no signal cuts it
  [@test] crates/host-agent/tests/serve.rs (`a_run_waits_for_its_pack_checks_out_by_the_diff_and_gives_its_slot_back`)

#### Scenario: The give-back of a forgotten run
- **WHEN** a client forgets a run as soon as it ended, as `brd-host run` does
- **THEN** the log of the agent has the line of its give-back, `brd-host supervise <run>: gave back the slot <n>, …`
  [@test] crates/host-agent/tests/serve.rs (`the_give_back_of_a_forgotten_run_writes_its_lines_to_the_log_of_the_agent`)

#### Scenario: A panic after the end of a run
- **WHEN** the supervisor panics after `exit.json`, such as in the git of its give-back
- **THEN** the log of the agent has one line `brd-host supervise <run>: panicked: panicked at <place>: <message>`, and
  `supervisor.log` has the panic as before
  [@test] crates/host-agent/src/log_test.rs (`a_panic_after_the_end_of_a_run_reaches_the_log_of_the_agent_with_the_id_of_the_run`)

#### Scenario: A pack after the end of its run
- **WHEN** a pack comes after its run ended
- **THEN** it lands, and it marks no run
  [@test] crates/host-agent/tests/serve/pack.rs (`a_pack_after_the_end_of_its_run_lands_and_marks_no_run`)
