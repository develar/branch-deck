## MODIFIED Requirements

### Requirement: `brd robot-stats`: It starts only the git that finds the repository

`brd robot-stats` MUST start only the git that finds the Space repository and the target of the baseline, the branch it
tracks on origin; the GitHub repository of the branches only for a baseline of another remote.

#### Scenario: The processes of a run
- **WHEN** `brd robot-stats` runs
- **THEN** the only git it starts finds the Space repository and the branch the baseline tracks on origin
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_robot_stats`)
