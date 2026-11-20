## ADDED Requirements

### Requirement: Verification and choice: A step of the fix is named once

A step MUST be named once in the fix: a candidate of a later conflict that carries what the change does already (a
restack of the chain it builds on, a commit moved to the same branch) adds no step for it.

#### Scenario: A chain of branches each needing the one before
- **GIVEN** `b`, `c` and `d` each change the line the branch before wrote, each built on the baseline
- **WHEN** the fix is found
- **THEN** its steps are three restacks, `b` on `a`, `c` on `b`, `d` on `c`, each once
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_chain_of_branches_each_needing_the_one_before_is_restacked_once_each`)
