## MODIFIED Requirements

### Requirement: `brd push`: A push the remote refuses with every lease holding is `push_refused`

A push the remote refuses while every lease holds (a hook, a protected branch, a pack too large) MUST be
`push_refused` with what git said of each ref and what the remote said why (its `remote:` lines, git's `error:` lines,
not the progress of any stage, each line once), nothing pushed; refs of the same report named together.

#### Scenario: A hook refuses the push
- **WHEN** the remote refuses the push while every lease holds
- **THEN** it is `push_refused` with what git said of each ref and the `remote:` and `error:` lines, not the progress, and nothing is pushed
  [@test] crates/bd-remote/src/push_test.rs (`a_push_the_remote_refuses_with_its_leases_holding_is_push_refused`)

#### Scenario: A mirror refuses an atomic push
- **WHEN** a mirror refuses the push of a stack, every ref with the same words, and forwards the progress and the reason of the remote behind it, the reason twice
- **THEN** the refs are named together with those words, the reason once with one `remote:`, without the progress
  [@test] crates/sync-core/src/push_test.rs (`a_stack_the_remote_refuses_names_its_branches_together_and_says_the_reason_once`)

