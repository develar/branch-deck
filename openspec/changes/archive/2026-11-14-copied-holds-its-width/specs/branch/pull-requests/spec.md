## MODIFIED Requirements

### Requirement: Chip: Hovering the chip opens the details

Hovering the chip MUST open the details: the number and the state, the title, the checks, the reviewers, the last
update, the notice about the head, "Open Pull Request" and "Copy Pull Request Link" (which says "Copied" in place, in
its own width, so the details keep their size and stay open under the pointer that copied), named as in the branch
menu.

#### Scenario: The details on hover
- **WHEN** the pointer hovers the chip
- **THEN** the details show the number and the state, the title, the checks, the reviewers, the last update and the
  notice about the head
- **AND** "Open Pull Request" and "Copy Pull Request Link", which says "Copied" in place
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`, `should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)

#### Scenario: The checks in the details
- **WHEN** the details show the checks
- **THEN** they say "1 failing (tests), 2 running, 9 passed" or "All 4 passed"
- **AND** the failed ones are named as a cut list, "5 failing (a, b, c and 2 more)", one without a name said as such,
  never "and 1 more": "2 failing (tests and a check without a name)"
  [@test] src/lib/pull-request.test.ts (`lists the failing checks first`)
  [@test] src/lib/check-counts.test.ts (`never counts one failed check left out: one without a name is said as such, in the counts of the checks too`)

#### Scenario: Copy Pull Request Link in the details
- **WHEN** the user clicks "Copy Pull Request Link" in the details
- **THEN** the link is copied, and the button says "Copied" in its own width and place
- **AND** the details stay open while the pointer stays on the button
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
