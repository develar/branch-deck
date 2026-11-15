## MODIFIED Requirements

### Requirement: disk: disk asks the agent what the pools of its host hold

`brd-host disk <alias>` MUST ask the agent what the lease pools of its host hold on disk (`Request::Disk`, feature
`disk`, every agent of this build has it): the agent answers the report of `brd lease disk --all` for its home
(`pool_disk`, by the first repository of its list that opens), and heartbeats while it walks.

#### Scenario: The disk of a host
- **WHEN** `brd-host disk <alias>` runs against an agent of this build
- **THEN** the agent answers the report of `brd lease disk --all` for its home, and heartbeats while it walks
  [@test] crates/host-link/src/proto_test.rs (`a_request_and_a_reply_have_a_tagged_json_shape`, `an_agent_on_macos_has_the_upgrade_and_every_agent_the_disk_the_trim_and_the_heavy_turn`)
  [@test] crates/host-client/src/run_test.rs (`runs_cancel_slots_disk_and_trim_answer_the_replies_of_the_agent`)
