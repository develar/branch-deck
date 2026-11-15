## MODIFIED Requirements

### Requirement: Reviews: Approve counts the files the window showed for the head

Approve MUST count how many of their files among all changes the window showed (the file on screen, as the footer
counts it; none while the overview of the plan is at the top or the plan is still read) for the head it shows; a
branch-wide group's files are all of them.

#### Scenario: The overview of the plan at the top
- **WHEN** the overview of the plan is at the top of the changes
- **THEN** Approve counts no file, and the first file J shows counts
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count no file for Approve while the overview of the plan is at the top of the changes, and the first file J shows`)

#### Scenario: The plan of a new head is still read
- **WHEN** a new head keeps the file on screen while the plan of the new head is still read
- **THEN** Approve counts none of the files of the new head, and the file on screen counts once the plan is read
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the opened files again from zero on a new head, and say when the approval counts for no group that needs one`)

#### Scenario: A branch-wide group
- **WHEN** the user's group is branch-wide
- **THEN** its files are all of them
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep every file visible for a branch-wide group and count it in Approve`)
