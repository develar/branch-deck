## ADDED Requirements

### Requirement: serve: The relief prunes ahead of the minimum

Above `branchdeck.leaseMinFree` of the host but below one and a half of it, the relief MUST prune ahead of the minimum
([brd lease](../lease/spec.md), "Host slots: The keeper prunes ahead of the minimum"), once per volume, the leased
slots largest first by `disk/`. A prune ahead that freed nothing MUST make the next one wait 10 minutes; a volume
below the minimum is relieved at once.

#### Scenario: A volume between the minimum and one and a half of it
- **WHEN** the relief finds a volume that has the minimum but less than one and a half of it
- **THEN** it prunes the leased slots by the cargo rules of their leases and cleans the free slots but the one of each
  pool's next get, up to one and a half of the minimum
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_prunes_ahead_of_the_minimum_and_keeps_the_free_slot_of_the_next_get`)

#### Scenario: A prune ahead that freed nothing
- **WHEN** a prune ahead freed nothing, as each free slot is clean and each leased slot was pruned within its age
- **THEN** the next prune ahead waits 10 minutes, and one that freed bytes lets the next run at once
  [@test] crates/host-agent/src/keeper_test.rs (`a_prune_ahead_that_freed_nothing_makes_the_next_one_wait`)
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_prunes_ahead_of_the_minimum_and_keeps_the_free_slot_of_the_next_get`)

### Requirement: serve: A prune ahead that ends below the minimum starts no wait

A prune ahead that ends below the minimum MUST NOT make the next relief wait, MUST NOT name a wait in its line, and MUST
relieve no host slot: the next round runs the whole relief.

#### Scenario: A prune ahead that ends below the minimum
- **WHEN** builds write faster than a prune ahead deletes, and the volume ends below the minimum
- **THEN** the relief relieves no host slot in that round, starts no wait, and its line names no wait
- **AND** the next round measures the volume below the minimum and runs the whole relief, the idle host slots among it
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_prune_ahead_that_ends_below_the_minimum_leaves_the_host_slots_to_the_next_round`)
  [@test] crates/host-agent/src/keeper_test.rs (`a_prune_ahead_that_ends_below_the_minimum_leaves_the_relief_to_the_next_round`)

## MODIFIED Requirements

### Requirement: setup: The agent of a local host keeps this machine as a host

The agent of a local host MUST be the agent of every host: its keeper relieves every pool of this machine below
`branchdeck.leaseMinFree` and prunes ahead of it below one and a half of it, runs `brew upgrade git` below the floor of
the git of a host, and `serve` holds the power assertions while a run goes on. No other machine reaches it.

#### Scenario: The keeper of a local host
- **WHEN** the agent of a local host runs
- **THEN** its keeper fills the host slots of the repositories of its runs, relieves every pool of this machine below
  `branchdeck.leaseMinFree`, prunes ahead of it below one and a half of it, and upgrades the git of Homebrew below the
  floor, as on any host
- **AND** it listens on the loopback address only, so a connection from another machine finds no agent

### Requirement: serve: The relief runs alone and logs what it deleted

The relief MUST run on a thread of its own, one at a time, and write one line of the log per repository whose relief
took a step, with what each step deleted (the trash that earlier deletes left, each leased slot it pruned with its
holder and what it freed, each leased slot where cargo builds, each host slot with what it freed) and the free space after. The line of a prune ahead MUST
name its threshold, and a prune ahead that freed nothing MUST write no line.

#### Scenario: A relief that took a step
- **WHEN** the relief of a repository took a step
- **THEN** the log gets one line for that repository with what each step deleted, each host slot with what it freed,
  and the free space after
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_keeps_the_minimum_free_space_once_per_volume_and_waits_after_a_relief_that_left_it_low`)

#### Scenario: The line of a prune ahead
- **WHEN** a prune ahead of the minimum pruned leased slots, passed over one where cargo builds, and cleaned a free slot
- **THEN** the line reads `the volume of the pools of <repository> had <size> free, less than <size>, one and a half of
  `branchdeck.leaseMinFree` <size>, below which the keeper prunes ahead of it: the prune of the leased slots by the cargo
  rules of their leases freed <slot> (<holder>) <size>, left the leased slot(s) <slot> (<holder>) as cargo builds there,
  the shrink freed <size> of <n> free slot(s); now <size> free`, without a leased slot that freed nothing
- **AND** a prune ahead that deleted the trash that earlier deletes left names it first: `deleted the trash that
  earlier deletes left, <size>`
  [@test] crates/host-agent/src/keeper_test.rs (`the_line_of_a_prune_ahead_names_the_threshold_and_each_leased_slot_with_its_holder`)
  [@test] crates/host-agent/src/keeper_test.rs (`a_prune_ahead_that_ends_below_the_minimum_leaves_the_relief_to_the_next_round`)
