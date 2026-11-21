## MODIFIED Requirements

### Requirement: The report of a run: A machine caller names its run options in full

A machine caller such as bt MUST name its run options in full, because the defaults of `run` are the ones of a
person: `brd-host run <alias> --nice 0 --no-outputs --fetch out/bt-runs --report-fd 3 -- <command…>`, with `--repo <path>` when it names a repository.

#### Scenario: The command line of bt
- **WHEN** bt runs a command on a host
- **THEN** it passes `--nice 0 --no-outputs --fetch out/bt-runs --report-fd 3`, and `--repo <path>` when it names a
  repository
- **AND** without `--head` the run ships the working tree, HEAD plus the local changes as one commit
- **AND** niceness 0 keeps the CPU for the run
- **AND** without `--no-outputs`, an output of `[host]` would replace its local copy
  [@test] crates/bd-host/src/cli_test.rs (`the_command_line_of_bt_gives_niceness_0_and_no_outputs`)
