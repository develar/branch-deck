## ADDED Requirements

### Requirement: run: An exclusive run is a measurement that holds a quiet host

`--exclusive` MUST make a run heavy and exclusive: a measurement such as an A/B or a benchmark, never an ordinary
check. Its `Start` MUST carry `heavy` and `exclusive`, and its line MUST say what it waits for and what it holds. An
agent without the feature `exclusive` would run it beside the others, so the client MUST refuse before the start.

#### Scenario: An exclusive run
- **WHEN** a run starts with `--exclusive`
- **THEN** its `Start` has `"heavy": true` and `"exclusive": true`, and after the line of the run comes `brd-host: an
  exclusive run by --exclusive: on <alias> it waits for a quiet host without another heavy run, and while it runs no
  heavy run is admitted and each new run that is not heavy waits for its end`
  [@test] crates/host-client/src/run_test.rs (`an_exclusive_run_is_a_heavy_one_and_an_agent_before_it_refuses_before_the_start`)
  [@test] crates/bd-host/src/cli_test.rs (`the_command_line_of_bt_gives_niceness_0_and_no_outputs`)

#### Scenario: An agent without the feature exclusive
- **WHEN** an exclusive run starts on an agent whose hello has no feature `exclusive`
- **THEN** the client refuses before `Start` with `bad_config` (exit 75): `the agent on <alias> predates exclusive runs,
  so it would run this measurement beside the others: `brd-host upgrade <alias>` sends this build`
  [@test] crates/host-client/src/run_test.rs (`an_exclusive_run_is_a_heavy_one_and_an_agent_before_it_refuses_before_the_start`)

### Requirement: serve: An exclusive run holds a quiet host alone

An exclusive run MUST be admitted to the heavy turn only without a holder and on a quiet host. While it holds, no heavy
run MUST be admitted, and each new run that is not heavy MUST wait for its end before its lend, in the step
`exclusive`, up to 1 h. The runs that already run go on.

#### Scenario: The admission of an exclusive run
- **WHEN** an exclusive run comes to the head of the queue
- **THEN** it waits until no heavy run holds cores and the host is quiet, and then no heavy run is admitted beside it,
  one of a measure that would fit neither
- **AND** the line of its wait says `all its <cores> cores of a quiet host, as an exclusive run` and the busy cores of
  the host outside the heavy runs, so the run sees what keeps it out
  [@test] crates/lease-pool/src/host_turn_test.rs (`an_exclusive_run_waits_for_a_quiet_host_and_holds_it_alone`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`)

#### Scenario: A run that is not heavy and an exclusive run
- **WHEN** a run that is not heavy starts while an exclusive run holds the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for a measurement: run <id> of
  <repository> holds the host for <time> (<command>); a measurement holds the host; this run starts when it ends`
- **AND** it takes its slot once the exclusive run gave the turn back, with the step `exclusive` first of the host;
  after 1 h it refuses with `slot_timeout`
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_run_that_is_not_heavy_waits_for_an_exclusive_run_and_starts_after_it`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_that_is_not_heavy_waits_for_the_end_of_an_exclusive_run`)

#### Scenario: brd-host runs and an exclusive run
- **WHEN** `brd-host runs` shows the heavy turn while an exclusive run holds it or waits
- **THEN** a holder is `run <id> of <repository> holds the whole host for a measurement for <time> (exclusive):
  <command>`, and a waiting one `waits <time> for a quiet host (exclusive)`
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_that_is_not_heavy_waits_for_the_end_of_an_exclusive_run`)

## MODIFIED Requirements

### Requirement: run: The command line of run

`brd-host run` MUST take the alias of a host, the options of its synopsis and, after `--`, the command with its
arguments. `--fetch` MUST be repeatable, and `--nice` MUST take 0 to 19.

#### Scenario: The synopsis of run
- **WHEN** a caller runs `brd-host run`
- **THEN** it takes this command line

```sh
brd-host run <alias> [--head] [--apply] [--fetch <dir>]... [--no-outputs] [--heavy] [--exclusive] [--nice <0-19>]
            [--repo <path>] [--affinity <key>] [--report-fd <n>] -- <command> <args>
```

### Requirement: The report of a run: The phases are the steps of both ends in their order

The steps MUST be `pack` and `upload` of the client; `heavy`, `exclusive`, `git`, `slot`, `pack_wait`, `checkout`,
`prepare`, `command`, `measure` and `stage` of the host; `fetch` and `total` of the client. A run that ends early has only
the steps up to its last one. Only a heavy run has `heavy`, only a run that is not heavy and waited for an exclusive run
has `exclusive`, and only a run that waited for an upgrade of git has `git`.

#### Scenario: The steps of a run
- **WHEN** a run goes through
- **THEN** its phases are `pack`, `upload`, `slot`, `pack_wait`, `checkout`, `prepare`, `command`, `measure`, `stage`,
  `fetch` and `total`, with `heavy` first of the host for a heavy run, and `git` only when it waited for an upgrade of
  git
- **AND** `measure` is the measure of the cores outside the command: the cores of the host and the first look at the
  Bazel servers of the slot before it, the last sample and the record of the measure after it, so a volume that hangs
  those looks shows in a step and not in the gap before `total`
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)

#### Scenario: A run that ends early
- **WHEN** a run ends before its last step
- **THEN** its phases have only the steps up to its last one
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)
