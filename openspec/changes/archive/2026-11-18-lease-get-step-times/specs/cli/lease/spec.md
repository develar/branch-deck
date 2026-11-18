## MODIFIED Requirements

### Requirement: Get: A get prints each step on stderr as it starts

A get MUST print each step on stderr as it starts (`brd lease get: [<time since the get began>] <step>`): the reserve of
a free slot, under the clones lock the reserve of a slot freed meanwhile or a new one, the wait for the clones lock, the
stop of the Bazel servers, the `status` of a slot, the wait for a slot whose return or clean runs, the delete of the
trash, the clone of a template, the refresh of its index, the checkout, the download of the files of Git LFS and the
seed; the JSON goes to stdout at the end and lists the same steps with when each started (`steps`, `at_ms`).

#### Scenario: The steps of a get
- **WHEN** a get runs
- **THEN** its steps arrive on stderr in order and the JSON on stdout at the end
- **AND** the core tells the same steps to its caller (`LeaseGetParams::progress`)
- **AND** the answer lists them in that order, each with when it started
  [@test] crates/bd-lease/src/clones_lock_test.rs (`the_steps_of_a_get_arrive_in_order`)

#### Scenario: A step names when it started
- **WHEN** a step starts 3 min 12 s after the get began
- **THEN** its line is `brd lease get: [3 min 12 s] <step>`, so a slow step shows in the gap to the next line
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_step_on_stderr_names_when_it_started`)
