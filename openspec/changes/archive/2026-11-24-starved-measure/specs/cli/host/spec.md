## ADDED Requirements

### Requirement: serve: A starved run adds no measure of its command

Beside each sample of a run, the supervisor MUST note the busy cores of the whole host by the sample of `serve`, less
the live cores of the run, weighted by the time of the sample. At the end a run whose mean of them and its own mean
exceed the cores of the host (`branchdeck.hostCores`) MUST add no measure and no wall of its command and MUST say why
in one line of its log; a run without a sample of the host adds its measure as before.

#### Scenario: A run beside a busy host
- **WHEN** the rest of the host used more cores on average beside a run than the cores of the host leave for its mean
- **THEN** its log says `this run adds no measure of its command: the rest of the host used <outside> cores on average
  beside its <mean>, past the <cores> of the host, so its time tells the load and not the command`
- **AND** `cores.json` and `walls.json` of the heavy turn get nothing of it, so the admissions and the estimates read
  the measures before it
- **AND** a run of the same command beside a host with room, or without a sample of the host, adds its measure
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_run_starved_by_the_rest_of_the_host_adds_no_measure_of_its_command`)
  [@test] crates/lease-pool/src/host_cores_test.rs (`a_run_beside_more_load_than_the_host_leaves_room_for_is_starved`)

#### Scenario: The rest of the host beside a run
- **WHEN** the supervisor notes the busy cores of the host beside the samples of a run
- **THEN** each sample counts the busy cores of the host less the run's own over its time, and a run that used more
  than the host counted adds none
- **AND** one note counts per sample, and a sample without a fresh sample of the host counts nothing
  [@test] crates/host-agent/src/cores_test.rs (`the_rest_of_the_host_beside_a_run_is_its_busy_cores_less_the_run_by_the_time_of_each_sample`)
