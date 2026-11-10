## MODIFIED Requirements

### Requirement: Chip: The branch menu opens the pull request and copies its link

The branch context menu MUST have "Open Pull Request #7" in its history group when the branch has a pull request, the
number saying which one it opens, also when the branch can't be modified, and "Copy Pull Request Link" in its copy
group: one item for every surface, the menu, the footer of the preview and the title bar of the review. "Open Pull
Request #7" shows `O`, which
opens the pull request on the focused row as in the branch preview
([Branch Row](../branch-row/spec.md#keyboard)).

#### Scenario: Open Pull Request from the branch menu
- **WHEN** the user chooses "Open Pull Request #7" in the branch context menu
- **THEN** the pull request opens
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)

#### Scenario: O on the focused row and Copy Pull Request Link
- **WHEN** the user presses `O` on the focused row, or chooses "Copy Pull Request Link"
- **THEN** the pull request opens, or its link is copied
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)

### Requirement: Branch preview: The preview shows the pull request and opens it with O

The meta line of the [branch preview](../branch-preview/spec.md) MUST end with the chip; the body MUST start with a
pull request section (the details of the chip popover) above the commits; `O` and the footer button "Open Pull Request #7"
(the item of the menu) open it. A branch without a pull request has neither.

#### Scenario: The preview of a branch with a pull request
- **WHEN** the preview of a branch with a pull request opens
- **THEN** its meta line ends with the chip and its body starts with the pull request section above the commits
- **AND** `O` and the footer button "Open Pull Request #7" open the pull request
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request in the branch preview and open it with O`)

#### Scenario: The preview of a branch without a pull request
- **WHEN** the preview of a branch without a pull request opens
- **THEN** it has neither the chip nor the section

### Requirement: Creating a pull request: The branch menu offers it without an open or merged pull request

The history group of the branch menu (the right-click and "⋯") MUST have "Create Pull Request…" when the branch has no
open or merged pull request and the GitHub repository is known, on the remote or not (the dialog pushes it); a branch
with one MUST NOT. A closed pull request doesn't count: the branch gets a new one, and "Open Pull Request #7" (its hover
"#7 is closed") still opens the closed one, beside it in the menu, the preview and the review window alike.

#### Scenario: A branch without a pull request
- **WHEN** the user opens the menu of a branch without a pull request
- **THEN** its history group has "Create Pull Request…"
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`, `should open the pull request of a pushed branch from the push toast`)

#### Scenario: A branch with only a closed pull request
- **WHEN** the branch has only a closed pull request
- **THEN** "Create Pull Request…" is offered
- **AND** "Open Pull Request #7", its hover "#7 is closed", opens the closed one beside it
  [@test] src/lib/push-run.test.ts (`offers a new pull request when the branch has only a closed one`)
