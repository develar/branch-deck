## ADDED Requirements

### Requirement: `brd push`: A copy with a generated file the words merged is `generated_merged_by_words`

A push that sends a copy holding a generated file the words merged in a conflict of the sync (`regenerate` of
`brd sync`) MUST be refused with `generated_merged_by_words` (exit 4) before anything is pushed, `--force` too, unless
`--allow-generated-merged-by-words`, given only once the user agreed; `--dry-run` MUST print the push without it.

#### Scenario: A lock file merged by words
- **GIVEN** the copy of `b` merged `Cargo.lock` by words, and lacks a commit of `a`
- **WHEN** `brd push b` runs, or `brd push b --force`
- **THEN** it is refused with `generated_merged_by_words` (exit 4), nothing pushed
- **AND** the message names `Cargo.lock`, why it is generated ("a lock file"), the copy that merged it, the commit of
  `a` it lacks, the fix and `--allow-generated-merged-by-words`; its next step is `brd restack b a`, which needs approval
- **AND** `brd push b --allow-generated-merged-by-words` pushes it
  [@test] crates/bd-cli/src/sync_test.rs (`brd_sync_names_the_generated_files_a_branch_must_regenerate_and_brd_push_refuses_them_until_allowed`)

#### Scenario: A stack
- **WHEN** a push of a stack sends such a copy, or the stack branch going along holds one
- **THEN** it is refused the same way, naming the first such copy

### Requirement: `brd safe-push`: A copy with a generated file the words merged is `generated_merged_by_words`

A start whose copy holds a generated file the words merged in a conflict of the sync (of a branch, also of a branch it
is built on; of a whole stack, its stack branch), a Patronus Dry Run too, MUST be refused with
`generated_merged_by_words` (exit 4), nothing pushed or started, with the message of `brd push`; `--dry-run` MUST list
it in `refusals`, and `--allow-generated-merged-by-words` MUST start anyway.

#### Scenario: The start of such a copy
- **WHEN** `brd safe-push` starts a branch whose copy, or that of a branch it is built on, holds such a file
- **THEN** it is refused with `generated_merged_by_words` (exit 4), the message naming the file, why it is generated,
  the copy that merged it, the commit it lacks and the fix
- **AND** `--allow-generated-merged-by-words` starts it
  [@test] crates/domain-policy/src/land_test.rs (`a_start_whose_copy_holds_a_generated_file_the_words_merged_confirms_first`)

### Requirement: `brd land`: A copy with a generated file the words merged is `generated_merged_by_words`

`brd land` MUST refuse a copy that holds a generated file the words merged with `generated_merged_by_words` (exit 4)
on every route, nothing pushed, with the message of `brd push`, unless `--allow-generated-merged-by-words`, given only
once the user agreed.

#### Scenario: A land of such a copy
- **WHEN** `brd land` lands a branch, or the whole stack, whose copy holds such a file
- **THEN** it is refused with `generated_merged_by_words` (exit 4), on the robot, the push and the merge routes
- **AND** `--allow-generated-merged-by-words` lands it
  [@test] crates/domain-policy/src/land_test.rs (`a_start_whose_copy_holds_a_generated_file_the_words_merged_confirms_first`)

## MODIFIED Requirements

### Requirement: Flag names: A flag that accepts a refusal is `--allow-<what refused>`

A flag that accepts a refusal, given only once the user agreed, MUST be `--allow-<what refused>`, named after the
error code it lifts where one code is lifted.

#### Scenario: The flags that accept a refusal
- **WHEN** a command offers to go past a refusal on the user's word
- **THEN** its flag is one of `--allow-unchecked` (`unchecked_tree`), `--allow-checks-not-passed`
  (`checks_not_passed`), `--allow-dangling`, `--allow-missing`, `--allow-unknown-rules`, `--allow-unknown-review`,
  `--allow-live-session`, `--allow-generated-merged-by-words` (`generated_merged_by_words`)

### Requirement: Output contract: A refusal names the condition of every flag past it

A refusal MUST name in its `message` the condition of every way past it that it offers, next to it: a flag that
starts anyway (`--allow-dangling`, `--allow-missing`: only when the user says it is a false alarm;
`--allow-unknown-rules`, `--without-approvals`, `--allow-generated-merged-by-words`: only when the user says so), and
one that drops, cancels or replaces (`--force`, `--cancel-running`, `--restart-running`, `--confirm-preserved`: once
the user approved).

#### Scenario: A dangling reference
- **WHEN** a dangling reference refuses the Safe Push
- **THEN** the message describes the references and offers `--allow-dangling` only when the user says it is a false
  alarm
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_dangling_reference_refuses_the_safe_push_unless_allowed`)
  [@test] crates/bd-core/src/check_summary_test.rs (`a_dangling_reference_message_describes_two_whole_and_counts_from_three_on`)

#### Scenario: Rules GitHub does not answer
- **WHEN** the rules of GitHub have no answer
- **THEN** the message offers `--allow-unknown-rules` only when the user says so
  [@test] crates/bd-remote/src/safe_push_test.rs (`rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)

#### Scenario: A generated file the words merged
- **WHEN** a copy that holds a generated file the words merged refuses a push or a start
- **THEN** the message says to ask the user, and to pass `--allow-generated-merged-by-words` only once they agree
  [@test] crates/bd-cli/src/sync_test.rs (`brd_sync_names_the_generated_files_a_branch_must_regenerate_and_brd_push_refuses_them_until_allowed`)
