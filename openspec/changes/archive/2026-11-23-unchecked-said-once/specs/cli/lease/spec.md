## MODIFIED Requirements

### Requirement: Pick: Allow unchecked lands the tree without a check

`--allow-unchecked <reason>` MUST land the tree without a check: the reason is required (a blank one is
`invalid_arguments`), and only the pick that landed commits repeats it (`unchecked`) and names it once on stderr, not a
conflict, a dry run or a pick with nothing to land. `--unchecked`, its name before, MUST be the same flag. Without the
setting the pick reads no record and prints no `check`.

#### Scenario: A pick with a reason
- **WHEN** `brd lease pick --allow-unchecked <reason>` runs
- **THEN** the tree lands without a check, the output repeats the reason in `unchecked` and stderr names it
- **AND** a dry run, a pick with nothing to land and a conflict leave `unchecked` out
  [@test] crates/bd-lease/src/lease_pick_test.rs (`allow_unchecked_lands_without_a_check_and_says_why`)
