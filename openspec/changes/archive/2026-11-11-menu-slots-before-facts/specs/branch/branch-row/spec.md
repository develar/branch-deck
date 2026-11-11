## MODIFIED Requirements

### Requirement: Branch menu: The groups come in the order of how often they are wanted

The groups MUST come in the order of how often their actions are wanted, Safe Push first.

#### Scenario: The order of the groups
- **WHEN** the menu of a branch opens
- **THEN** it leads with "Fix Conflict…" for a branch with a conflict (its next step, above the rest)
- **AND** the landing: where Patronus answers "Safe Push" (`S`; "Safe
  Push · 3" for a stacked branch), then the actions of the branch's robot as every surface of the robot lists them
  (`getRobotActionItems`, [Patronus](../patronus/spec.md#restart-and-cancel): "Restart Safe Push" (it pushes first while the
  remote lacks work), "Cancel Safe Push", "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request
  other than the branch's own), then "Patronus ›" with the rarer starts: "Safe Push With Profile…" (always the popover,
  its profile to choose; disabled for a start that comments on a pull request, which runs the default checks), "Dry
  Run" and "Check Approvals Again"
- **AND** then the push of the branch, named as its push button, "Push oauth Only" after it
  while it pushes the stack ([Stacked Branches](../stacked-branches/spec.md#pull-requests)), on a branch with branches
  stacked on it "Push with Children (4 branches)" beside it (disabled with the reason while it pushes nothing beyond
  the branch: [Stacked Branches](../stacked-branches/spec.md#pull-requests)), and "Mark IJAI-1 Fixed" for a YouTrack issue
- **AND** the review ("Review Changes", "Create Pull Request…" or "Open Pull Request #7" in its place, both beside a
  closed pull request, "Request Review…", "Retarget #7 to master", "Link GitHub Stack" of a layer whose stack of GitHub
  is out of line: [Stacks of GitHub](../stacked-branches/spec.md#stacks-of-github))
- **AND** the shape ("Rename…" or "Name This Branch…", "Move to…", "Stack On… ›", "Unstack from …", "Ungroup")
- **AND** on every branch of a stack the stack ("Collapse Stack" `⌥←` or "Expand Stack" `⌥→`, "Whole Stack ›", the actions of the
  whole stack of its root: [Whole Stack](../whole-stack/spec.md))
- **AND** the commits ("Amend Changes…", "Add Issue Reference…", "Squash N Fixups…")
- **AND** the order of landing ("Add Need…", "Remove Need: docs": [Declared Needs](../declared-needs/spec.md))
- **AND** what sets the branch aside ("Park Branch…": [Parked Branches](../parked-branches/spec.md), "Unapply…")
- **AND** last "Copy ›" ("Copy Full Branch Name", "Copy Branch Name", "Copy Pull Request Link" for a branch
  with a pull request, "Copy Retarget Command" for an open one into a virtual branch)
  [@test] src/hooks/useBranchContextActions.test.ts (`leads with Safe Push, keeps every keyed item on top and puts the rarer starts and the copies into submenus`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge and Create Pull Request… in their places while GitHub lists, the open pull request in the place of Create @webkit`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
  [@test] src/presenters/robot-actions.test.ts (`cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)

### Requirement: Branch menu: Off Space the landing starts with the push

Off Space the landing MUST start with the push; for an `origin` on Space its starts are there from the first view,
disabled with "Waits for the answer of Patronus" until Patronus answers, and for an `origin` elsewhere none ever is
([Next Step](../next-step/spec.md#facts-without-an-answer)).

#### Scenario: A repository off Space
- **WHEN** Patronus answered that the repository is not on Space
- **THEN** the landing group starts with the push, and Rust offers no start of Patronus
  [@test] src/hooks/useBranchContextActions.test.ts (`starts with the push off Space, where Rust offers no start of Patronus`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_ready_branch_offers_every_action_enabled_and_off_space_no_start_of_patronus`)

#### Scenario: Before Patronus answers
- **WHEN** Patronus has not answered yet, and `origin` is on Space
- **THEN** its starts stay, disabled with "Waits for the answer of Patronus", and Create Pull Request… waits until GitHub lists the pull requests, as the review window does
- **AND** for an `origin` elsewhere the landing starts with the push from the first view
  [@test] src/hooks/useBranchContextActions.test.ts (`offers Safe Push disabled until Patronus answers, and Create Pull Request… until GitHub lists the pull requests, as the review window does`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should offer no start and no pull request on a local remote from the first view`)
