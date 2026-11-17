## ADDED Requirements

### Requirement: The surfaces: A wait is a state, which the status says

A step that is a state the subject waits in (`NextStep.wait`, `StepKind::is_wait`: rows 3 of a pull request, 5′, 6,
7 of a whole stack's pull request, 8, 10, 8′, 10′, merge′, merge″) MUST be said by the status of the subject, never by
its slot of actions, whose `data-step` still names it ("Status says, Actions do",
[UX Writing](../../../../docs/contributing/ux-writing.md#actions)): a click on it would only open the pull request.

#### Scenario: A branch whose pull request awaits a review
- **WHEN** the step of a branch is a wait, and no fact of its own says more (`RowFact::Wait`; a failed robot, parked, a block keep the cell)
- **THEN** its Status cell says it in sentence case as every status ("Awaiting review", "Changes requested" in `destructive`, "Draft", "Into release-252", "In #12", "Merges when checks pass"), its icon where a status has its dot (`StepWaitStatus`), its slot shows no step
- **AND** a click on the status opens the pull request, and so does O on the focused row
- **AND** the pull request chip keeps its own state: the row doesn't say "awaiting" twice; the preview says the wait as its status, the landing overview in the reason under the name
  [@test] crates/domain-policy/src/row_status_test.rs (`a_wait_is_the_state_of_the_cell_where_nothing_of_its_own_says_more`)
  [@test] src/presenters/next-step.test.ts (`a wait is a state: the words its status says, in sentence case, the hover of its step, and the pull request it opens; a step of an action has none`)
  [@test] src/presenters/row-status.test.ts (`says the state the next step waits in, worded by the step, and the preview its words: no action of the slot says it`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

#### Scenario: The hover of a wait
- **WHEN** the pointer rests on a wait in the Status cell
- **THEN** the hover gives the whole text, a line per missing group ("Awaiting Ilya Korennoy · IJPL Code Platform", "1 more approval · IJPL Code Platform" for a group that has an approval and needs more), then "Open Pull Request #1078" with O
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner a stacked branch's own pull request asks, not for its root`)

#### Scenario: The wait of a whole stack
- **WHEN** the step of the whole stack of an island is a wait
- **THEN** the header says it beside the pull request of the stack, its slot shows no step, and a click opens the pull request
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does, the move made from the click`)

## MODIFIED Requirements

### Requirement: The table: The next step is the first row that matches

The next step of a branch MUST be the first row of the table that matches. "Safe Push elsewhere" is the reason every
other surface MUST disable "Safe Push" with (`safePushBlocker`, the `safe-push` action Rust offers).

#### Scenario: The first row that matches decides the step
- **WHEN** the next step of a branch is decided
- **THEN** it is the first of these rows that matches, its slot as the row says
- **AND** every other surface offers "Safe Push" as "Safe Push elsewhere" says, disabled with its reason

  | # | State | Slot | Safe Push elsewhere |
  |---|---|---|---|
  | 0 | A fact the rows below decide on has no answer yet (`PendingFact`; [below](#facts-without-an-answer)): Patronus hasn't answered for the repository or its view of the robots it listed first isn't in, GitHub hasn't listed the pull requests, the check of what the branch needs to land is still running, Patronus hasn't told why the robot of the branch failed, the first answer of the readiness; never while a reason of row 1 or of the branch holds the start back, which it says instead. Or a read failed (`UnavailableFact`): the Safe Push CLI is not logged in, Patronus fails, Patronus didn't give why the robot failed | muted "Checking…" with its spinner, disabled, the fact on hover ("Waits for GitHub to list the pull requests"); after a failed read what helps: "Copy Login Command", "Refresh Patronus", or the restart disabled with the failure | disabled with the same words ("Waits for the answer of Patronus", "Waits for Patronus to check the approvals", "Run ./safePush.cmd once to log in to Patronus"); a Dry Run only while Patronus hasn't answered or can't be asked |
  | 1 | Syncing, failed to sync, no commits, blocked, held by a Safe Push, parked or built on a parked branch, landed, a robot runs, merged (by the robot, by the merged Safe Push of a branch built on it or of the whole stack, or the pull request of these commits; all of it, no commit added since) | nothing; the Status cell says it ("Landed with oauth", "Parked with auth"), a running robot first with what else holds the branch beside it ([Branch Row](../branch-row/spec.md#status-cell)), as `brd status` does (`next_step.status`) | disabled: "Wait for the sync to finish", "Parked: Unpark it to push", "Built on parked auth: Unpark auth to push", "Holds parked auth: land the stack in pieces" (the whole stack), "Landed in origin/master", "Landed in master with oauth", "Merged by Patronus", "Pull request #7 is merged", … (a running robot of an older commit: enabled, the start replaces it) |
  | 1′ | The build of what its Safe Push lands needs a commit of a branch outside it (`landingNeeds` of the repository view, [below](#what-the-safe-push-needs)), or the user declared that another branch lands first ([Declared Needs](../declared-needs/spec.md); before the waits of row 0) | "Needs stills-codec" (a commit not in a branch by its hash), "Needs docs", the reason on hover; a click runs the fix, "Stack lane-run on stills-codec", "Unstack ui from auth" | enabled: its popover says why before the start, with the fix and Start Anyway |
  | 1″ | Someone else pushed commits to its remote branch that it lacks (`othersBehind` of the remote status, [Outgoing](../../repository/outgoing/spec.md#remote-commits-of-others)): a push, also the one of a start or a restart, drops them | "Pick Remote Commits", the reason on hover ("The remote has 1 commit of someone else that auth doesn't: a push drops it"); a click opens the question of the push button, which lists the commits with the pick | enabled: its start asks before the push that drops them |
  | 2 | A robot failed or was canceled, not for approvals, and no running Safe Push of a branch it is built on (of the whole stack: of a branch of it) lands a part of it; while one does, the failure of the earlier piece waits for it: the rows below decide, the Safe Push disabled with "A Safe Push of auth is running: wait until it merges" | "Restart Safe Push" (Safe Push… where Patronus can't restart it) | enabled |
  | 3 | A robot failed for approvals, started from the open pull request into the target, readiness `approved` | "Restart Safe Push" | enabled |
  | 4 | removed: a stacked branch takes the rows of its own pull request into master, see below | — | — |
  | 5 | An open pull request into a virtual branch (`{prefix}/virtual/…`, `base` `virtual` of the open pull request the session reads, `domain_policy::pull_request::base_of`: of its parent, of another branch of the table, or of one that landed, was archived or deleted): every pull request of a virtual branch goes into the target | "Retarget #7 to master" ("Retarget #7" in the slot) | enabled: the start retargets #7 into master first, then starts from it, as `brd safe-push` (`PlannedEffect::Retarget` of `domain_policy::land::decide`) |
  | 5 | Merge route: an open pull request into the branch of the layer below it that GitHub lists in no stack (`layerNotLinked` of the merge, [Stacks of GitHub](../stacked-branches/spec.md#stacks-of-github)): its merge would land nothing | "Link GitHub Stack" ("Link Stack" in the slot): Link GitHub Stack (`Intent::LinkStack`) | its merge waits for the link |
  | 5′ | An open pull request into another base, which may be meant (a release branch; `base` `other`) | a wait: no step; the Status cell says "Into release-252", the reason on hover, and opens it; "Retarget #7 to master" stays in "⋯" | disabled: "Approvals in #7 don't count: its base is release-252"; a start that counts no approval (without review, a profile that skips the review) starts, as `brd safe-push` (D3b) |
  | 6 | An open draft | a wait: no step; the Status cell says "Draft" and opens it | disabled: "#7 is a draft: Patronus merges it once it is ready for review" |
  | 7 | `missing`, no open pull request (none, or a closed one) | "Create Pull Request…" (the code owners preselected; on the remote or not, its dialog pushes the branch; disabled only while the GitHub repository is unknown); a branch of a whole stack whose pull request is open: a wait, no step; the Status cell says "In #12" and opens it (below) | disabled: "Code owners must approve in a pull request: IJPL Code Platform" |
  | 8 | `missing`, a member of a missing group requested changes (and wasn't asked again since) | a wait: no step; the Status cell says "Changes requested" in `destructive` and opens the pull request | disabled: "Ilya Korennoy requested changes on #1078" |
  | 9 | `missing`, a missing group asks fewer people than the approvals it needs (nobody, or one of a group that needs two) | "Request Review…", its popover ([Pull Requests](../pull-requests/spec.md#request-review)) | disabled: "Waits for the approval of Air Platform: nobody of it is requested on #1078" |
  | 10 | `missing`, someone of every missing group is asked | a wait: no step; the Status cell says "Awaiting review" (the people in its hover; "Awaiting 2 reviews" for several) and opens the pull request | disabled: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078" |
  | 8′ | `unknown`, `unavailable`, an open pull request into the target where a reviewer requested changes and wasn't asked again since (`changesRequestedBy`, bots left out) | a wait, as 8 | enabled: Patronus can't judge the approvals, the start checks them |
  | 10′ | `unknown`, `unavailable`, an open pull request into the target that waits for a requested reviewer (`readiness::waiting_reviewers`: asked and not approving since, bots left out) | a wait: the Status cell says "Awaiting review" ("Awaiting 2 reviews"), its hover "Waits for ikorennoy on #1078 · Patronus can't tell yet whether the approvals there count: the start checks them", and opens the pull request | enabled, as 8′ |
  | 11 | `approved`, `notRequired`; `unknown`, `unavailable` where GitHub tells no wait | "Safe Push" | enabled |

  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`, `the_cases_are_well_formed_and_name_every_row_of_the_next_step_table`)
  [@test] crates/domain-policy/src/next_step_test.rs (`every_row_with_every_fact_without_a_fresh_answer_never_answers_an_action_the_fresh_one_would_replace`)

#### Scenario: Row 0 waits for a fact without an answer, or says a read that failed
- **GIVEN** a fact the rows below decide on has no answer yet, or its read failed
- **WHEN** no reason of row 1 or of the branch holds the start back
- **THEN** the slot is a muted "Checking…", or what helps after a failed read
  [@test] crates/domain-policy/src/next_step_test.rs (`row_0_waits_for_a_fact_no_answer_told_yet_and_a_reason_of_the_subject_outranks_it`, `row_0_a_read_that_failed_decides_no_start_and_offers_what_helps`)
  [@test] src/presenters/next-step.test.ts (`row 0: words the fact a step waits for and a read that failed, with what helps: a login to copy, a refresh, a restart disabled`)

#### Scenario: Row 1 offers nothing and the Status cell says why
- **WHEN** the branch syncs, failed to sync, has no commits, is blocked, held, parked, landed, merged or its robot runs
- **THEN** the slot offers nothing and the Status cell says why
- **AND** both starts are disabled with the reason, but for a running robot of an older commit: its start is enabled and replaces it
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
  [@test] src/presenters/next-step.test.ts (`row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer nothing after a merge, and say so in the menu`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should offer no Safe Push of a branch whose pull request merged`)

#### Scenario: Row 1′ names the branch the build needs
- **WHEN** the build of what its Safe Push lands needs a branch outside it, or the user declared one that lands first
- **THEN** the slot names that branch and a click runs the fix
- **AND** the Safe Push stays enabled, its popover saying why before the start
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_needs_a_branch_outside_what_the_safe_push_lands_and_keeps_the_start`)
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the build needs and runs the fix, or opens the Safe Push`)

#### Scenario: Row 1″ takes in the commits of others on the remote
- **WHEN** someone else pushed commits to its remote branch that it lacks
- **THEN** the slot is "Pick Remote Commits", the reason on hover
- **AND** a start asks before the push that drops them
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_double_prime_takes_in_the_commits_of_others_on_the_remote_before_a_start`)
  [@test] src/presenters/next-step.test.ts (`row 1″: takes in the commits someone else pushed to the remote branch, which a push would drop`)
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should make the pick the next step of a branch on Space, before its Safe Push, and run it from the slot`)

#### Scenario: Rows 2 and 3 restart a failed robot
- **WHEN** a robot failed or was canceled, not for approvals, or failed for approvals with the readiness `approved`
- **THEN** the slot is "Restart Safe Push" (Safe Push… where Patronus can't restart it)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)
  [@test] src/presenters/next-step.test.ts (`rows 2 and 3: restarts the robot, or the pull request that collects the approvals, or a Safe Push where it can't restart`)

#### Scenario: Row 5 retargets a pull request into a virtual branch, row 5′ opens one into another base
- **WHEN** the open pull request goes into a virtual branch
- **THEN** the step is "Retarget #7 to master" and the start retargets it first
- **AND** an open pull request into another base that may be meant is a quiet "Open Pull Request", its retarget in "⋯"
  [@test] crates/domain-policy/src/next_step_test.rs (`row_5_retargets_a_pull_request_into_a_virtual_branch_and_5_prime_opens_one_into_another_base`)
  [@test] src/presenters/next-step.test.ts (`row 5: retargets a pull request into a virtual branch, short in its column; 5′ opens one into another base, the retarget in the menu`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should retarget a pull request into the branch of a parent that landed, whose approvals don't count`, `should retarget a pull request into the virtual branch of a branch in the table, as a parent's, whose approvals don't count`, `should retarget the pull request of a stacked branch into its parent's branch from its slot, whose approvals don't count`, `should open a pull request into another base that may be meant, and keep its retarget in the menu`)

#### Scenario: Row 6 opens a draft
- **WHEN** the open pull request is a draft
- **THEN** the step is a wait: the Status cell says "Draft" and opens it, the slot shows no step
- **AND** its Safe Push is disabled with why, a Dry Run runs
  [@test] crates/domain-policy/src/next_step_test.rs (`row_6_a_draft_is_a_step_that_opens_it_the_safe_push_waits_a_dry_run_runs`)
  [@test] src/presenters/next-step.test.ts (`row 6: a draft is a quiet step that opens it; its Safe Push says why, a Dry Run runs`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`)

#### Scenario: Row 7 creates the pull request
- **WHEN** the approvals are `missing` and no pull request is open
- **THEN** the step is "Create Pull Request…" with the code owners preselected, its dialog pushing the branch
- **AND** a branch of a whole stack whose pull request is open has a quiet "In #12"
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] src/presenters/next-step.test.ts (`row 7: Create Pull Request… naming the code owners, or In #12 of the whole stack, quietly`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer Create Pull Request… with the code owner when approvals count only in one`, `should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`)

#### Scenario: Rows 8 to 10 tell the missing approvals
- **WHEN** the approvals are `missing` in an open pull request
- **THEN** the step is "Changes Requested", Request Review… or the wait naming the people asked
- **AND** the Safe Push is disabled with the reason, also on S
  [@test] src/presenters/next-step.test.ts (`rows 8 to 10: changes requested, Request Review…, the wait naming the people asked`)
  [@test] src/lib/pull-request.test.ts (`names who requested changes and wasn't asked again since, bots left out`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

#### Scenario: Rows 8′ and 10′ tell the wait GitHub tells while Patronus can't judge
- **WHEN** the readiness is `unknown` or `unavailable` and a reviewer requested changes or is waited for in the open pull request
- **THEN** the step is a quiet "Changes Requested" or "Awaiting ikorennoy"
- **AND** the Safe Push stays enabled, the start checks the approvals
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_8_prime_and_10_prime_tell_the_wait_github_tells_while_patronus_cannot_judge`)
  [@test] src/presenters/next-step.test.ts (`rows 8′ and 10′: the wait GitHub tells while Patronus can't judge, the Safe Push enabled`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the reviewer GitHub asks while Patronus can't judge the approvals, keep Safe Push, and tell the approval`)

#### Scenario: Row 11 is the Safe Push
- **WHEN** the readiness is `approved` or `notRequired`, or `unknown` or `unavailable` where GitHub tells no wait
- **THEN** the step is "Safe Push", enabled
  [@test] crates/domain-policy/src/next_step_test.rs (`row_11_safe_push_and_the_start_without_review_while_github_is_signed_out`)
  [@test] src/presenters/next-step.test.ts (`row 11: Safe Push by the branches it lands, and what Patronus checks while GitHub lists no pull request`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer Safe Push once the code owner approved, name whose approval counts, and start it from the pull request with a click`, `should keep Safe Push while Patronus doesn't tell whose approvals count, and say so before it starts`)

### Requirement: The table: An operation on the row disables the action of the slot

An operation on the row ([Running Operations](../../../../docs/design/operations.md)), as the view of the session has
it in every window (`RepositoryView.operations`: a rewrite of the branch or of its commits, the retarget of its pull
request; `useRowOperation`), MUST disable the action of the slot with its busy state ("Renaming auth to login…"), never
a wait: it only opens the pull request.

#### Scenario: A rename runs on the row
- **WHEN** an operation runs on the row, in any window of the repository
- **THEN** the action of the slot is disabled with its busy state ("Renaming auth to login…")
- **AND** a wait stays, as it only opens the pull request
  [@test] src/presenters/next-step.test.ts (`lays what runs on the row over the step: its own action runs, the others wait, never a quiet step nor another field of the pull request`)
  [@test] src/hooks/useBranchContextActions.test.ts (`says why a branch can't start a Safe Push in the words of its next step, and what runs on the row over it`)

### Requirement: Facts without an answer: A stale readiness keeps its step, its action waits

A stale answer of the readiness (the branch moved on: a new tip, a new review, Check Approvals Again) MUST keep its
step while the new one loads, so the row doesn't flicker, but its action MUST wait for the new answer, which may decide
another step.

#### Scenario: A new tip while the step is Safe Push
- **WHEN** the readiness of the branch is stale
- **THEN** Safe Push, Restart, Create Pull Request… and Request Review… are disabled with "Waits for Patronus to check the approvals again", S says it
- **AND** a wait only opens the pull request and stays
  [@test] crates/domain-policy/src/next_step_test.rs (`a_stale_readiness_keeps_its_step_and_its_action_waits_for_the_new_answer`)
  [@test] src/presenters/next-step.test.ts (`keeps a stale answer, its action waiting for the new one, and a start reads only a fresh one`)

### Requirement: A merge of the pull request: The route of a merge takes its own table

A repository that lands by a merge of the pull request on GitHub (`LandingRoute::PullRequest`,
[Landing Routes](../landing-routes/spec.md#merge)) MUST take the table of its route while no robot of Patronus lands
it (Patronus answers that the repository is not its) and a merge there lands into its target; the gate refuses
neither a mirror nor a repository off GitHub: there the rows of the Patronus table stay, the slot hidden.

#### Scenario: The rows of the route
- **WHEN** the step of a branch of a repository that lands by a merge is decided
- **THEN** rows 1, 0, 1′ and 1″ come first as a push's (row 0 also while the listing of the pull requests has no answer, and while GitHub has not computed whether it can merge the pull request, `mergeState`: `UNKNOWN` is asked again, never "no")
- **AND** then the first of these rows that matches by what GitHub tells of the open pull request of the subject into the target (`land_direct::merge_of`)

  | # | State | Slot | Start elsewhere |
  |---|---|---|---|
  | pr | No open pull request into the target | "Create Pull Request…", its node the user's step | Merge disabled: "auth has no open pull request: create one, GitHub merges it" |
  | merge | GitHub can merge it (`CLEAN`, `HAS_HOOKS`, `UNSTABLE` with a confirmation); its required checks still run (`BLOCKED` by them) where the repository allows auto-merge; it shows another commit than the copy, which a push restarts the checks of | "Merge #7", or "Auto-Merge #7" (the push first when it shows another commit), its node ready | disabled with the refusal of the gate ("You have no write access to acme/monorepo"), else enabled: its popover names the method and what it asks |
  | merge′ | Its auto-merge is enabled | a wait: the Status cell says "Merges when checks pass", Cancel Auto-Merge in "⋯"; its node landing | — |
  | merge″ | What holds the merge back: a draft, changes requested, approvals the base requires, required checks that fail, or still run without auto-merge, a head behind its base (`BEHIND`), a conflict (`DIRTY`), a pull request that shows another commit without auto-merge, what GitHub doesn't name | a wait: the Status cell says its reason ("Conflict", "Checks running"); the node a fix (a conflict, changes, failing checks), the user's step (a head behind, a branch to push, a draft) or a wait (approvals, running checks) | Merge disabled with the reason |

  [@test] crates/domain-policy/src/next_step_test.rs (`a_merge_of_the_pull_request_is_the_step_of_its_route_by_what_github_tells`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`a_merge_of_the_pull_request_is_ready_landing_a_fix_or_a_wait_by_what_holds_it_back`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_merge_of_the_pull_request_offers_its_merge_and_cancel_auto_merge_while_github_merges_it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A pull request into a virtual branch or another base
- **WHEN** the open pull request goes into a virtual branch or another base
- **THEN** it takes rows 5 and 5′ of the table above, the merge disabled with the base

## REMOVED Requirements

### Requirement: The surfaces: A quiet step is the state as its label

**Reason**: a wait is a state, which the status says, not a quiet step of the slot of actions

**Migration**: `The surfaces: A wait is a state, which the status says`
