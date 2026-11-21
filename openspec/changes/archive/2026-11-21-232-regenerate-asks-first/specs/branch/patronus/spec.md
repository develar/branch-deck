## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: A start confirms only for what a person decides

A start that can run MUST either start or confirm first, and MUST confirm only for what a person decides
(`Confirmation`). S, "Safe Push With Profile…" and the start without review MUST always confirm.

#### Scenario: A start that lands more or replaces a robot
- **WHEN** a start lands more than its row (a stacked branch), runs another profile than the default checks (the
  branch's own, the profile of its last robot, or one chosen), or cancels the Safe Push running on an older commit
- **THEN** it confirms first
  [@test] src/presenters/land.test.ts (`opens the popover for a key and for each confirmation of the decision: a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open`)

#### Scenario: Approvals nobody can judge before the start
- **WHEN** Patronus can't judge the approvals before the start (`unknown`, GitHub doesn't list the pull requests) or
  couldn't answer the review (`unavailable`, `ReviewUnknown`)
- **THEN** it confirms first
  [@test] crates/domain-policy/src/land_test.rs (`missing_approvals_refuse_unless_skipped_and_approvals_judged_at_the_start_confirm`)

#### Scenario: A push that dismisses approvals, unread rules, a start without review
- **WHEN** the push dismisses approvals Patronus doesn't count, the rules of the repository can't be read for a push
  into an approved pull request, or it is a start without review
- **THEN** it confirms first
  [@test] src/presenters/land.test.ts (`lets a start without review wait for its profile, and confirm the one that skips the review`)

#### Scenario: A copy with a generated file merged by words
- **WHEN** the copy a start lands holds a generated file the words merged in a conflict of its sync (of a branch: its
  own or of a branch it is built on; of a whole stack: its stack branch), on any route, a Dry Run too
- **THEN** it confirms first (`Confirmation::Regenerate`), every time until the copy no longer holds it, and the
  popover says the file, why it is generated and the commit the copy lacks; Start Anyway starts it
- **AND** `brd` accepts it only by `--allow-generated-merged-by-words`
  [@test] crates/domain-policy/src/land_test.rs (`a_start_whose_copy_holds_a_generated_file_the_words_merged_confirms_first`)

#### Scenario: Nothing to confirm
- **WHEN** a start can run and nothing needs a person's decision
- **THEN** it starts at once, unless it was asked by S, "Safe Push With Profile…" or as the start without review
  [@test] src/presenters/land.test.ts (`starts at once when Rust decided there is nothing to say, and waits for its decision before it`)
