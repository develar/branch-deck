## MODIFIED Requirements

### Requirement: The table: A fact without a fresh answer never answers a step a later one turns

A fact without a fresh answer MUST NOT answer a step a later answer would turn into another: one that loads or nobody
asks waits (row 0), a stale one keeps the step of the answer before (a stale readiness holds the action, the needs of
tips that moved keep the very step), a read that failed is an answer that says so (what helps for Patronus, never
missing approvals for the pull requests or the readiness, the gate of the start for the landing needs).

#### Scenario: Every row with every fact without a fresh answer
- **WHEN** a fact of any row loads, is asked by nobody, is stale or failed
- **THEN** the step never is an action the fresh answer would replace
- **AND** of the landing needs of tips that moved it is the very step of the answer before
  [@test] crates/domain-policy/src/next_step_test.rs (`every_row_with_every_fact_without_a_fresh_answer_never_answers_an_action_the_fresh_one_would_replace`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`waits_for_the_first_answers_as_the_step_does_and_says_a_read_that_failed`)

### Requirement: What the Safe Push needs: The needs are a fact with its inputs

The landing needs MUST be a fact with its inputs (the baseline, the tips, the commits the view names landed): unknown
until computed, never "nothing", one computation at a time, not computed again while the inputs stay. They come in a
patch of the view (`landingNeeds` `loading` until then, `stale` while those of tips that moved are computed), a step
waiting for those no answer stands for (row 0); a failed computation is `unavailable` for its inputs, and no step
waits for it.

#### Scenario: One computation at a time
- **WHEN** the inputs change while a computation runs
- **THEN** the newest wanted inputs are computed next, and the same inputs never again
  [@test] crates/repository-model/src/facts_test.rs (`the_landing_needs_are_computed_one_at_a_time_the_newest_wanted_next`)

#### Scenario: A failed computation
- **WHEN** the computation of the needs fails
- **THEN** they are `unavailable` for its inputs, and no step waits for them

### Requirement: Facts without an answer: The Safe Push waits for the landing needs

The landing needs MUST be such a fact too (`Needs::Computing` of `domain_policy::next_step`: no answer of the session's
check after the sync stands for the branch, its `landingNeeds` `loading` or `stale`; "Checking what JIRA-456 needs to
land…"), so the Safe Push waits for them as for the other facts instead of offering a start whose gate may refuse it
after the press.

#### Scenario: The needs are still computed
- **WHEN** the check of what the branch needs to land hasn't answered, and no answer before stands for the branch
- **THEN** the step, S and the popover wait for the answer: "Checking what JIRA-456 needs to land…"
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should check what a branch needs to land before it offers its Safe Push, S and the popover waiting for the answer`)
  [@test] crates/domain-policy/src/next_step_test.rs (`the_needs_of_tips_that_moved_keep_the_step_of_the_answer_before_and_a_branch_it_does_not_answer_waits`)

### Requirement: Facts without an answer: A start reads only fresh answers

A start MUST read only fresh answers: its decision waits for each value of the cached frame as for a fact that loads
(`domain_policy::land::standing`) and for the needs of tips that moved (`LandFact::LandingNeeds`), as the engine does.
Every press (the slot, S, the popover, "⋯", Dry Run, Start Anyway) waits for them as for any fact and starts nothing
once another repository opens; every remote action of the frame waits for the first sync
([Branch Row](../branch-row/spec.md#actions-cell)).

#### Scenario: A start over the cached frame
- **WHEN** a start is decided over a value of the cached frame
- **THEN** it waits for the fresh answer
  [@test] crates/domain-policy/src/land_test.rs (`every_fact_without_an_answer_waits_or_confirms_and_never_refuses`)

#### Scenario: A start while the needs of tips that moved are computed
- **WHEN** a start is decided, pressed or run while the needs of tips a sync moved are computed
- **THEN** its decision waits for them, and the press and the engine wait for them, then decide on them
- **AND** S opens the popover, whose start waits with "Checking what JIRA-456 needs to land…"; a click of the slot says "Checking…"
  [@test] crates/repository-model/src/derive_test.rs (`the_view_decides_the_default_starts_of_each_branch_and_a_read_answers_another_intent`)
  [@test] crates/repository-model/src/session_test.rs (`a_sync_that_moved_a_tip_keeps_the_steps_of_the_needs_before_and_its_start_waits_for_the_new_ones`)
  [@test] crates/operations/src/land_test.rs (`a_start_planned_while_the_needs_of_new_tips_are_computed_waits_for_them_and_is_decided_on_them`)
  [@test] src/presenters/land.test.ts (`waits for the facts Rust waits for, each saying what it waits for, and refuses with why a read failed`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should wait in the popover of S for the fresh needs of a new tip, and start on them`, `should wait in a click of the start of the slot for the fresh needs of a new tip, its button checking, and start on them`)

#### Scenario: A declared need while the needs of tips that moved are computed
- **WHEN** a branch declared a need, and the needs of its new tip are computed
- **THEN** its start is refused by the declared need at once and still waits for the gate beside it: Start Anyway of the declared need starts no robot before the gate answered
  [@test] crates/domain-policy/src/land_test.rs (`a_declared_need_refuses_at_once_and_the_start_waits_for_the_gate_beside_it`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_decides_the_default_starts_of_each_branch_and_a_read_answers_another_intent`)

#### Scenario: Another repository opens while a press waits
- **WHEN** another repository opens while a click of the slot or a Start Anyway of a toast waits for the fresh needs of a new tip
- **THEN** it starts nothing, in neither repository, and says why at once: "Cannot start the Safe Push of stills-codec", "… is no longer the open repository"
- **AND** a press whose repository shows again before its plan comes ends too, and says that another repository opened while it waited
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should start nothing when another repository opens while a click of the slot waits for the fresh needs of a new tip, and say why`, `should start nothing when another repository opens while a Start Anyway of a toast waits for the fresh needs of a new tip, and say why`, `should start nothing when another repository opens and the first one shows again while a click of the slot waits for its plan, and say why`)

## ADDED Requirements

### Requirement: Facts without an answer: The needs of tips that moved stand until the new ones come

While the needs of tips a sync moved on the same baseline are computed (`landingNeeds` `stale`), the answer before MUST
stand for each branch it still answers (`facts::NeedsBasis`: of that table, built on the same branches, its refusals
naming only branches the baseline lacks): its step, node and counts stay, no step is patched. A moved baseline, a
landed commit, a new or restacked branch and a need of a branch gone wait as for the first answer; no word says it
(`data-landing-needs`).

#### Scenario: One more commit on one branch
- **WHEN** a sync finds one more commit on stills-codec, and the session computes the needs of the new tips
- **THEN** lane-run still says "Needs stills-codec", stills-codec "Safe Push", no step waits and none is patched
- **AND** Sync says `data-landing-needs="stale"`, the landing indicator keeps its count, `aria-busy`, until the needs come
  [@test] crates/repository-model/src/facts_test.rs (`the_needs_of_tips_that_moved_stand_for_the_branches_they_still_answer_until_the_new_ones_come`)
  [@test] crates/repository-model/src/session_test.rs (`a_sync_that_moved_a_tip_keeps_the_steps_of_the_needs_before_and_its_start_waits_for_the_new_ones`)
  [@test] crates/domain-policy/src/next_step_test.rs (`the_needs_of_tips_that_moved_keep_the_step_of_the_answer_before_and_a_branch_it_does_not_answer_waits`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should keep what a branch needs while the needs of a new tip are computed, its fix waiting for them and running once on the fresh ones`)

#### Scenario: What keeps no answer before
- **WHEN** the baseline moved, a merged Safe Push landed a commit, or a branch is new, built on others now, or needs a branch the sync removed or the baseline has whole
- **THEN** its step waits for the new needs as for the first answer
  [@test] crates/repository-model/src/facts_test.rs (`the_needs_of_tips_that_moved_stand_for_the_branches_they_still_answer_until_the_new_ones_come`)

### Requirement: What the Safe Push needs: The fix runs on fresh needs

A press of the fix of row 1′ (the slot, "⋯", the toast of a refused start) MUST run on fresh needs: its rewrite names
the refusal it fixes (`RewriteIntent::fixes`) and waits for them holding nothing, a stop ending the wait; then, behind
the rewrites and the sync before it, it runs while they still ask for that fix, else is refused (`plan_changed`) and
the Safe Push popover opens where the start opens. While it runs or waits, the fix of the slot and of "⋯" are disabled.

#### Scenario: The fix pressed while the needs are computed
- **WHEN** the user presses "Needs stills-codec" of lane-run while the needs of a new tip are computed
- **THEN** the restack goes to the engine with the refusal it fixes and waits there, the row busy, nothing rewritten
- **AND** once the fresh needs still name stills-codec, the restack runs once
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should keep what a branch needs while the needs of a new tip are computed, its fix waiting for them and running once on the fresh ones`)
  [@test] src/presenters/next-step.test.ts (`row 1′: while the fix runs on the row, or waits in the session for the fresh needs, its item of the slot and of ⋯ are disabled`)
  [@test] crates/repository-model/src/facts_test.rs (`a_fix_of_row_1_prime_stands_while_the_fresh_needs_refuse_with_it_and_waits_while_they_are_computed`)
  [@test] crates/operations/src/rewrite_test.rs (`a_fix_waits_for_the_fresh_needs_holding_nothing_and_runs_while_they_still_ask_for_it`)

#### Scenario: The fresh needs no longer ask for the fix
- **WHEN** the fresh needs of lane-run no longer refuse with what the pressed fix fixes (the commit it needs moved to another branch, a fix before ran)
- **THEN** the engine refuses the fix, nothing is rewritten, and the Safe Push popover opens with the needs as they are
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should open the Safe Push popover instead of the fix pressed while the needs of a new tip are computed, once the fresh needs no longer ask for it`)
  [@test] crates/repository-model/src/facts_test.rs (`a_fix_of_row_1_prime_stands_while_the_fresh_needs_refuse_with_it_and_waits_while_they_are_computed`)
  [@test] crates/operations/src/rewrite_test.rs (`a_fix_the_fresh_needs_no_longer_ask_for_is_plan_changed_and_a_stop_cancels_its_wait`)

#### Scenario: The popover of a fix of the whole stack
- **WHEN** the fix of what the whole stack of a root needs opens the Safe Push popover instead of running
- **THEN** it opens where the Safe Push of the whole stack opens: at the header of its island, at the root's row of a small stack, as the Safe Push of the last branch of a straight chain
  [@test] src/stores/safe-push.test.ts (`puts the whole stack that forks at the header of its island, of a small stack at the root's row`, `starts the whole stack of a straight chain as the Safe Push of its last branch, at the header of its island`, `puts a branch at its row, or where its press asked it`)

#### Scenario: A second press right after the first
- **WHEN** the fix is pressed again, from its row or another, right after the first ran
- **THEN** the second waits behind the first and the sync it asked, and rewrites nothing more
  [@test] crates/operations/src/rewrite_test.rs (`a_second_press_of_the_fix_right_after_the_first_runs_no_second_restack`)

#### Scenario: A stop of the wait
- **WHEN** the user stops the fix while it waits for the fresh needs
- **THEN** it answers `canceled`, nothing written
  [@test] crates/operations/src/rewrite_test.rs (`a_fix_the_fresh_needs_no_longer_ask_for_is_plan_changed_and_a_stop_cancels_its_wait`)
