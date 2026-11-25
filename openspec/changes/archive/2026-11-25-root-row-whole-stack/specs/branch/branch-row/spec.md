## MODIFIED Requirements

### Requirement: Actions cell: On Space the Safe Push slot shows the next step

On Space (Patronus answered), the Safe Push slot MUST show the next step of the branch, the first row of the table of
[Next Step](../next-step/spec.md) that matches. The robot itself is in the Status cell. Nothing in the slot changes on
the row hover.

#### Scenario: The steps of the slot
- **WHEN** Patronus answered and the slot shows the next step of a branch
- **THEN** it is "Safe Push" for a branch with a push button and nothing in its way (a click starts it at once when it has nothing to say, else opens its popover at the slot,
  [Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run))
- **AND** the step after a robot ("Restart Safe Push")
- **AND** no step of a wait ("Awaiting review", "Changes requested", "Draft"): the Status cell says it
- **AND** the action that helps ("Create Pull Request…", "Request Review…", "Retarget #7")
- **AND** a stacked branch's own step ("Safe Push · 3"), its popover at its own row
- **AND** on the root of a stack that forks, whose row hosts its whole stack, the step of the whole stack ("Safe Push · 4",
  its popover at the root's row; none in an island, whose header shows it), the root's own start in its "⋯" ("Safe Push
  Root Only"): [Whole Stack](../whole-stack/spec.md#requirement-the-safe-push-of-the-whole-stack-in-the-app-the-row-of-its-root-hosts-it)
- **AND** nothing while a robot runs, after a merge, for a landed or held branch ([Patronus](../patronus/spec.md#robot-of-a-row))
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`, `should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should start the whole stack from the slot and S of the root of a small stack that forks, as the landing overview says it, and the root alone from Safe Push Root Only`)

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
  Run" and "Check Approvals Again"; on the root of a stack that forks whose row hosts its whole stack, "Safe Push Whole
  Stack" (`S`) as its slot, then "Safe Push Root Only", and "Dry Run Root Only" in "Patronus ›"
  ([Whole Stack](../whole-stack/spec.md#requirement-the-safe-push-of-the-whole-stack-in-the-app-the-row-of-its-root-hosts-it))
- **AND** then the push of the branch, named as its push button, "Push oauth Only" after it
  while it pushes the stack ([Stacked Branches](../stacked-branches/spec.md#pull-requests)), on a branch with branches
  stacked on it "Push with Children (4 branches)" beside it (disabled with the reason while it pushes nothing beyond
  the branch: [Stacked Branches](../stacked-branches/spec.md#pull-requests)), and "Mark IJAI-1 Fixed" for a YouTrack issue
- **AND** the review ("Review Changes", "Create Pull Request…" or "Open Pull Request #7" in its place, both beside a
  closed pull request, "Request Review…" on GitHub (disabled without an open pull request), "Retarget #7 to master",
  "Link GitHub Stack" of a layer whose stack of GitHub is out of line:
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
  [@test] src/hooks/useBranchContextActions.test.ts (`leads with Safe Push Whole Stack (S), as its slot, once, and offers the root's own start as Safe Push Root Only and Dry Run Root Only, without a key`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge, Create Pull Request… and Request Review… in their places while GitHub lists, the open pull request in the place of Create @webkit`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
  [@test] src/presenters/robot-actions.test.ts (`cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)

### Requirement: Keyboard: S opens the Safe Push popover of the focused row

S MUST open the Safe Push popover at the slot of the focused row when its "Safe Push" is enabled (Patronus answered, the
row has a push, its start is not refused (`landing.start`), no inline action running): S never starts by itself
(`requestSafePush` with `via: "key"`). Otherwise S says why in an `info` toast, the reason the button's hover says. On
the row of the root of a stack that forks, which hosts its whole stack, S is the start of the whole stack, as its slot
(`slotSubjectOf`): its popover at the row, at the header of an island.

#### Scenario: S and Enter
- **WHEN** the user presses S on the focused row whose "Safe Push" is enabled, then Enter
- **THEN** the popover opens at the slot and focuses its start button once it can start, so Enter starts the robot
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`)

#### Scenario: S where the start is not possible
- **WHEN** the user presses S on a row whose "Safe Push" is disabled
- **THEN** an `info` toast says the reason the hover of the button says

#### Scenario: S on the root that hosts its whole stack
- **WHEN** the user presses S on the row of the root of a stack that forks
- **THEN** the popover of the whole stack opens at the row, at the header of its island where it has one
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should start the whole stack from the slot and S of the root of a small stack that forks, as the landing overview says it, and the root alone from Safe Push Root Only`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should start the whole stack from the island's ⋯ and with S on its header and on its root's row, the popover at the header`)
