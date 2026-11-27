## MODIFIED Requirements

### Requirement: Rounds: The round of a candidate prints `landed_as` with the interdiff

The round of a candidate, and the round of each commit it landed with (its fixups; by changed lines the other commits
of its set), MUST print `session.commit.landed_as`: the baseline commit, its subject, and the interdiff of the two from
their changed lines without context, the candidate and the commits it landed with summed: `local_only` (what a drop
loses) and `upstream_only`, per path at most 20 lines, at most 20 paths.

#### Scenario: The round of a twin
- **WHEN** the round of a candidate opens
- **THEN** `session.commit.landed_as` has the baseline commit, its subject, `local_only` (absent when a drop loses
  nothing) and `upstream_only`
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`, `a_twin_with_every_line_of_the_commit_drops_without_a_question`)

#### Scenario: The interdiff of a candidate with fixups
- **WHEN** the candidate has fixups
- **THEN** the interdiff sums the candidate and its fixups, and names a binary file
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_interdiff_of_a_twin_sums_the_fixups_and_names_a_binary_file`)

#### Scenario: The round of a squashed pull request
- **WHEN** the round of a commit opens whose pull request GitHub squashed under another identity
- **THEN** `session.commit.landed_as` names the squash
  [@test] crates/bd-update/src/resolve_test.rs (`a_commit_squashed_upstream_with_its_fixup_drops_as_landed_in_its_round_while_another_commit_conflicts`)

### Requirement: Rounds: `--landed` refuses what it cannot drop

`--landed` MUST refuse, changing nothing, a round without a candidate (`not_landed_round`, exit 2), a commit with
`local_only`, or of a candidate by changed lines whose change the upstream does not have as it is, without `--user`, the
user's yes (`landed_differs`, exit 2, `ask_user`), and a commit that still changes files with `onto` for every
conflicting file (`landed_not_empty`, exit 2, with `paths`).

#### Scenario: A round without a twin
- **WHEN** `--landed` names the commit of a round without a twin
- **THEN** it is `not_landed_round` (exit 2), and nothing changes
  [@test] crates/bd-update/src/resolve_test.rs (`landed_refuses_a_round_without_a_twin_and_a_commit_that_keeps_a_change`)

#### Scenario: A drop that would lose lines, without the user's yes
- **WHEN** `--landed` drops a commit with `local_only` without `--user`
- **THEN** it is `landed_differs` (exit 2, `ask_user`), and nothing changes
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`, `landed_refuses_a_round_without_a_twin_and_a_commit_that_keeps_a_change`)

#### Scenario: A commit that keeps a change after `onto`
- **WHEN** `onto` for every conflicting file leaves the commit with a change
- **THEN** it is `landed_not_empty` (exit 2, with `paths`), even with `--user`
  [@test] crates/bd-update/src/resolve_test.rs (`landed_refuses_a_round_without_a_twin_and_a_commit_that_keeps_a_change`)

#### Scenario: A squash rewritten since, without the user's yes
- **WHEN** `--landed` drops a commit of a candidate by changed lines whose squash the upstream rewrote since, without `--user`
- **THEN** it is `landed_differs` (exit 2, `ask_user`), and nothing changes
  [@test] crates/bd-update/src/resolve_test.rs (`landed_of_a_candidate_by_changed_lines_whose_squash_was_rewritten_asks_the_user`)
