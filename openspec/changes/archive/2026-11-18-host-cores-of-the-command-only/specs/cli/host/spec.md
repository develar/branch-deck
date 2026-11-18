## MODIFIED Requirements

### Requirement: serve: A run measures the cores of its command

The supervisor of every run MUST sample the CPU of its command group and of the Bazel servers of its slot every 10 s,
and at the end measure the mean (the larger of the sampled CPU and of the reaped children over the wall time) and the
peak. A command of 10 s or more MUST add its measure to the last 5 of its repository and command; a command is
expected to use their 80th percentile.

#### Scenario: The samples of a run
- **WHEN** the command of a run runs
- **THEN** each sample rewrites `cores.json` of the run and notes the live cores in the heavy turn: of a holder as its
  `live`, of a run that is not heavy in `light`
- **AND** at the end its log says `the command used <mean> cores on average and <peak> at its peak, of the <cores> of
  the host`
- **AND** a process that started before the command, such as a Bazel server of the slot that a later sample finds,
  counts from that sample on, and a sample counts at most the cores of the machine
  [@test] crates/host-agent/src/cores_test.rs (`the_samples_count_the_cpu_of_the_command_group_and_the_end_its_measure`, `a_process_older_than_the_command_counts_only_from_its_first_sample`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_cpu_time_of_a_process_grows_while_it_computes`)
  [@test] crates/sync-utils/src/cores_test.rs (`cores_read_and_write_with_one_decimal_and_are_the_mean_of_a_cpu_time`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

#### Scenario: The measure follows the command
- **WHEN** a run of a command ends, a cancelled one too
- **THEN** its measure joins the last 5 of the command, and the next run expects the second largest of their means
  (the largest of up to 4), so a command that got heavier is expected heavier at its next run, and one that got lighter
  frees its cores within a few runs
- **AND** a command of less than 10 s adds no measure, and a command without a measure for 30 days is forgotten
  [@test] crates/lease-pool/src/host_cores_test.rs (`the_expectation_of_a_command_follows_its_last_runs`, `a_command_without_a_measure_for_30_days_is_forgotten`)
