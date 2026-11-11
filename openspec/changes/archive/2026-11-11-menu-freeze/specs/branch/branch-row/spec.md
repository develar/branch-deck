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
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge, Create Pull Request… and Request Review… in their places while GitHub lists, the open pull request in the place of Create @webkit`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
  [@test] src/presenters/robot-actions.test.ts (`cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)

### Requirement: Hover and click: A submenu whose trigger moved from under a still pointer stays open

Content that moves the trigger of an open submenu from under a still pointer (a scroll of the menu by the wheel; an
open menu's list never moves, [Branch menu](#requirement-branch-menu-an-open-menu-doesnt-move)) MUST close nothing:
the submenu stays open until the pointer moves; a move away from it then closes it, a move into it keeps it
(`hoverClosesOnMove` of `src/lib/pointer-moves.ts`, in the submenu roots of `src/components/ui`).

#### Scenario: A scroll moves Copy from under the pointer
- **WHEN** "Copy ›" of a branch is open with the pointer resting on Copy at the end of a menu that scrolls, and the wheel scrolls the menu, so Copy goes down from under the pointer
- **THEN** "Copy ›" stays open, and the next move away from it closes it
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep its open submenu when a scroll moves its item from under a still pointer, until the pointer moves away`)

#### Scenario: The move into the submenu
- **WHEN** the pointer then moves into "Copy ›"
- **THEN** it stays open, and its item under the pointer is highlighted
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep its open submenu when a scroll moves its item from under a still pointer, and a move into it`)

### Requirement: Branch menu: An open menu doesn't move

While a menu shows, its list MUST NOT move (`frozenMenuGroups` of `src/lib/menu.ts`, in `MenuItems`, every menu of the
page): its items are the ones it opened with, each in its place as it changes; an item that leaves stays there,
disabled, until the menu closes, and one that comes shows at the next open. An item that takes the place of another
shares its slot (`MenuItem.slot`) and shows there. A group is known by its item that opens a submenu, else by its first
item, never by its place, so an open submenu stays mounted.

#### Scenario: A sync finds a conflict while Copy is open
- **WHEN** "Copy ›" of a branch is open with the pointer on its items, and a sync finds a conflict in the branch
- **THEN** the menu keeps its items in their places, without "Fix Conflict…", "Copy ›" stays open, and Copy Branch Name copies the name
- **AND** the next open of the menu leads with "Fix Conflict…"
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep the items of an open menu in their places as a sync finds a conflict, and lead with Fix Conflict… at the next open`)
  [@test] src/lib/menu.test.ts (`shows each item in its place as it is now, keeps an item that left there disabled and shows a new one at the next open`, `freezes an open submenu too, and a submenu that left is a disabled item that opens nothing`)

#### Scenario: An item that takes the place of another
- **WHEN** GitHub lists the open #7 of a branch while its menu shows "Create Pull Request…"
- **THEN** "Open Pull Request #7" shows in its place
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge, Create Pull Request… and Request Review… in their places while GitHub lists, the open pull request in the place of Create @webkit`)
  [@test] src/lib/menu.test.ts (`shows the item that took the slot of another in its place: Open Pull Request #7 where Create Pull Request… was`)

#### Scenario: An item that comes while the menu shows
- **WHEN** GitHub lists a closed pull request of a branch while its menu shows
- **THEN** the menu takes no item in, the item under the pointer stays, and "Open Pull Request #5" follows "Create Pull Request…" at the next open
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should show the open of a closed pull request GitHub lists while the menu shows at its next open, the items under the pointer where they were`)

## RENAMED Requirements

- FROM: `### Requirement: Branch menu: An open submenu stays open when the menu changes above it`
- TO: `### Requirement: Branch menu: An open menu doesn't move`
