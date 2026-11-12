## ADDED Requirements

### Requirement: Host slots: An idle host slot is cleaned once it waited after its last run

The keeper MUST apply the clean rules of the slot's own `.branchdeck.toml` to an idle host slot that waited
`branchdeck.hostSlotIdleClean` (6 hours without it; `0` never) since its last run gave it back, when no clean came after
that run; a slot never lent, one that is not idle and one with a process of a run in it stay as they are.

#### Scenario: A slot idle after its run
- **WHEN** a host slot waited idle the time of `branchdeck.hostSlotIdleClean` since its last run, or 6 hours without it
- **THEN** its clean rules run once, not sooner, and never with `0`
  [@test] crates/bd-lease/src/host_slots_test.rs (`an_idle_host_slot_is_cleaned_once_it_waited_after_its_last_run`)
