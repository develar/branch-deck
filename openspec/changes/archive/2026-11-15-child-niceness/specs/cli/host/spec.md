## ADDED Requirements

### Requirement: The environment of a run and `[host]`: A command never runs above the agent

A prepare step and the command MUST run at the niceness of the spec, or at the niceness of the agent when that is
higher: a process without privilege may not lower its niceness, so a niceness MUST never fail the start of a run. The
line before the command MUST name the niceness the command runs at, the agent's for a spec without one.

#### Scenario: A niceness below the one of the agent
- **GIVEN** an agent at a niceness above the one of the spec of a run
- **WHEN** the run starts its prepare steps and its command
- **THEN** they start, at the niceness of the agent, and the line before the command names it
  [@test] crates/host-agent/tests/serve/system.rs (`a_run_asked_below_the_niceness_of_the_agent_runs_at_the_agents_and_says_so`)
  [@test] crates/sync-utils/src/child_process_test.rs (`a_child_never_runs_above_its_parent_and_starts_whatever_priority_it_asks_for`)

#### Scenario: A run without a niceness
- **WHEN** the spec of a run names no niceness
- **THEN** its prepare steps and its command run at the niceness of the agent, and the line before the command names it
  [@test] crates/host-agent/tests/serve/system.rs (`a_run_asked_below_the_niceness_of_the_agent_runs_at_the_agents_and_says_so`)
