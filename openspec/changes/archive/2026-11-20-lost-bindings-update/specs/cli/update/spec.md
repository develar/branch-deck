## ADDED Requirements

### Requirement: Resolve session: A clean replay that loses bindings starts at a lost-bindings round

When the clean replay of `brd update` loses bindings (Update Project, "Check: A clean replay that loses a binding is a
conflict"), it MUST NOT apply: it MUST start a resolve session at the lost-bindings round of the first local commit with
one, every commit replayed onto the tip the check replayed, with no replay of its own. Only the session ref and the
copies are written; the output is `state: "conflict"`, exit 1.

#### Scenario: The incident without a conflict of git
- **WHEN** `brd update` replays a commit that drops a private property and an import with their last use onto an upstream
  commit that uses both again in another hunk
- **THEN** it exits with 1 and prints a round whose `session.round_kind` is `lost_bindings`, `replayed` equal to `total`
- **AND** HEAD, the index and the working tree stay
  [@test] crates/bd-update/src/resolve_test.rs (`a_clean_replay_that_loses_bindings_opens_a_lost_bindings_round_whose_fixup_finishes_the_update`)
  [@test] crates/update-core/src/process_budget_test.rs (`a_bd_update_whose_clean_replay_loses_bindings_opens_its_round`)

### Requirement: Resolve session: Before its finish a session checks the lost bindings of its replay

A session whose every commit is replayed MUST read the lost bindings of its copies against the local commits before it
finishes, but the names a round kept; a finding MUST open the lost-bindings round of the first local commit with one
instead of the finish. A round of git of the same commit comes first. A finding is gone once the tip binds the name
again: the `fixup!` commit of its round does.

#### Scenario: The incident as it happened
- **WHEN** the replay conflicts on a block of the commit and its other hunks drop the import and the property cleanly,
  and the agent resolves the block
- **THEN** `--continue` of that round opens a lost-bindings round of the same commit, exit 1
- **AND** after its fixup the session finishes
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_round_whose_clean_hunks_drop_the_bindings_is_followed_by_a_lost_bindings_round`)
  [@test] crates/update-core/src/process_budget_test.rs (`the_fixup_of_a_lost_bindings_round_finishes_the_update`)

### Requirement: Rounds: A lost-bindings round prints its lost bindings and the files at the tip

A lost-bindings round MUST print `session.round_kind: "lost_bindings"` and `session.lost_bindings` (each `file`,
`name`, `binding` import or declaration, `binding_line` before the copy, `use_line` at the tip, `commit`). Each file of
the round is `kind: "lost_binding"`: `edit` is the file at the session's tip without markers, `before` the file before
the copy, `commit` the copy's; its `hint` names the `fixup!` commit `--continue` writes and `brd fold`.

#### Scenario: The round of the incident
- **WHEN** the round opens
- **THEN** `session.lost_bindings` names `backend` (declaration) and `rethrow` (import) with their lines and the commit
- **AND** the `edit` copy is the file at the tip and the `before` copy the upstream's file with what the commit removes
  [@test] crates/bd-update/src/resolve_test.rs (`a_clean_replay_that_loses_bindings_opens_a_lost_bindings_round_whose_fixup_finishes_the_update`)

### Requirement: `--continue`: The edits of a lost-bindings round become one `fixup!` commit

`--continue` of a lost-bindings round MUST take an edit anywhere in its files (no `resolution_outside_conflict`; a
take, a delete, `--rename-to`, `--landed` and `--drop-empty` are `invalid_arguments`) and write the files that changed
as one new commit `fixup! <subject of the round's commit>` on the tip, by the session's committer; the replayed commits
stay, and a round that changed no file writes none. The finish MUST name `brd fold` in `next` after a fixup.

#### Scenario: An edit that binds both names again
- **WHEN** the agent puts the import and the property back in the `edit` copy and runs `--continue`
- **THEN** the update applies, HEAD is `fixup! (launch) Drop the old path` on the replayed commit, author and committer alike
- **AND** `resolutions` lists the round with its `lost_bindings`, `verify.paths` the file, `next` `brd fold`
  [@test] crates/bd-update/src/resolve_test.rs (`a_clean_replay_that_loses_bindings_opens_a_lost_bindings_round_whose_fixup_finishes_the_update`)

### Requirement: `--continue`: An edit that leaves a lost name unbound is `lost_binding`

`--continue` MUST summarize each edit of a lost-bindings round in process and refuse a name of the round that the edit
still leaves unbound while the file uses it, `lost_binding` (exit 4) with the files in `paths`, the message naming each
name and its use; nothing changes, the round stays open and the copies keep the edit.

#### Scenario: An edit that binds nothing
- **WHEN** the agent edits the copy without keeping a binding and runs `--continue`
- **THEN** it is refused with `lost_binding`, the file in `paths`, and HEAD stays
  [@test] crates/bd-update/src/resolve_test.rs (`a_clean_replay_that_loses_bindings_opens_a_lost_bindings_round_whose_fixup_finishes_the_update`)
  [@test] crates/trial-check/src/lost_bindings_test.rs (`an_edit_that_keeps_the_binding_loses_nothing`)

### Requirement: `--continue`: `--keep-name` keeps a name unbound on the user's word

`--keep-name <name>` (repeatable) MUST accept a lost name of the round unbound: the user says it resolves another way.
A name that is no lost binding of the round is `invalid_arguments`. The round records the kept names (its resolution's
`kept_names`, `verify.kept_names` with the file and the line), and no later finish of the session names them again.

#### Scenario: Every name kept, no copy edited
- **WHEN** `--continue --keep-name backend --keep-name rethrow` runs with the copy as it was
- **THEN** the update applies with no `fixup!` commit and no `next`, and `verify.kept_names` names both
  [@test] crates/bd-update/src/resolve_test.rs (`keep_name_keeps_a_name_unbound_on_the_users_word`)
  [@test] crates/trial-check/src/lost_bindings_test.rs (`a_replay_brings_the_lost_bindings_of_its_copy_in_two_processes`)

#### Scenario: A name the round lacks, a name left
- **WHEN** `--keep-name` names no lost binding of the round, or keeps one name of two
- **THEN** it is `invalid_arguments`, or `lost_binding` naming the other
  [@test] crates/bd-update/src/resolve_test.rs (`keep_name_keeps_a_name_unbound_on_the_users_word`)

## MODIFIED Requirements

### Requirement: Error codes: Exit 4 is a state the agent cannot retry out of

The codes `session_stale`, `unsupported_conflict`, `uncommitted_overlap`, `finish_pending`, `lease_follows_home` and
`lost_binding` MUST exit with 4.

#### Scenario: The row of exit 4
- **WHEN** `brd update` fails with a code of this row
- **THEN** it exits with 4

  | Exit | Codes |
  |---|---|
  | 4 | `session_stale`, `unsupported_conflict`, `uncommitted_overlap`, `finish_pending`, `lease_follows_home`, `lost_binding` |

#### Scenario: Each code of the row
- **WHEN** the exit code and the retry of each error code are checked
- **THEN** each code of this row exits with 4 and is not retryable
  [@test] crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)
