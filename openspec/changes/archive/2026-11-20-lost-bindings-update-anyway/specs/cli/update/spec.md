## ADDED Requirements

### Requirement: Update: `--dry-run` names the lost bindings of the replay

`brd update --dry-run` MUST read the lost bindings of its replay as the update does: a clean replay that loses one is
`state: "conflict"` (exit 1) whose `conflict.lost_bindings` names each (`file`, `name`, `binding`, `binding_line`,
`use_line`, `commit`), nothing written.

#### Scenario: The dry run of the incident
- **WHEN** `brd update --dry-run` checks the replay of a commit that drops a property and an import the upstream uses again
- **THEN** it exits with 1, `conflict.lost_bindings` names `backend` and `rethrow`, and nothing is written
  [@test] crates/bd-update/src/resolve_test.rs (`a_dry_run_names_the_lost_bindings_of_the_replay`)

### Requirement: Finish: `replayed` counts no `fixup!` commit

`replayed` of a finish MUST count the local commits on the baseline, never the `fixup!` commits of its lost-bindings
rounds.

#### Scenario: A finish after a lost-bindings round
- **WHEN** a session finishes after a lost-bindings round wrote its fixup
- **THEN** `replayed` is the count of the local commits replayed, 1 for one commit
  [@test] crates/bd-update/src/resolve_test.rs (`a_clean_replay_that_loses_bindings_opens_a_lost_bindings_round_whose_fixup_finishes_the_update`)
