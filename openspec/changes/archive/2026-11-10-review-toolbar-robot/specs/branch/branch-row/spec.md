## MODIFIED Requirements

### Requirement: Keyboard: O opens the pull request of the focused row

O MUST open the pull request of the focused row, as "Open Pull Request #7" of its menu; nothing for a branch without one.

#### Scenario: O on a row with a pull request
- **WHEN** the user presses O on the focused row of a branch with a pull request
- **THEN** its pull request opens
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)

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
- **AND** the review ("Review Changes", "Open Pull Request #7" or "Create Pull Request…", "Request Review…", "Retarget #7 to
  master", "Link GitHub Stack" of a layer whose stack of GitHub is out of line:
  [Stacks of GitHub](../stacked-branches/spec.md#stacks-of-github))
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
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
  [@test] src/presenters/robot-actions.test.ts (`cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)
