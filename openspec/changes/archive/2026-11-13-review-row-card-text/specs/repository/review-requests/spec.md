## MODIFIED Requirements

### Requirement: Card: A row shows the unread dot, the title and the meta

A row, 36 px, MUST show the unread dot, the title (`font-medium`, the issue key it starts with muted and regular, where
Rust split the title, `titleLead`), the labels of its pull request as muted chips (`Badge variant="secondary"`, the
first two, then "+2"), and the meta "#101 · carol · 2 files · +12 −3", which gives way before the title: the title is
cut only once the meta has no room left, the labels after it (muted line counts, as in every row); the labels and the
meta are hidden below `sm`.

#### Scenario: Someone else than the author asked
- **WHEN** someone else than the author asked for the review, as for a pull request a bot opened
- **THEN** the meta says "#830 · agent-bot · requested by dave · …": that one waits for the user
  [@test] src/lib/review-requests.test.ts (`names who asked for the review when it is not the author`)

#### Scenario: Another base than the default branch
- **WHEN** the base of the pull request is not the default branch
- **THEN** the meta ends with "· → release/253"
- **AND** a layer of a stack leaves it out: its base is the layer below it, and the header of the stack names the base of its bottom
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`, `should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)

#### Scenario: The labels of a pull request
- **WHEN** the pull request has labels on GitHub
- **THEN** its row shows the first two as muted chips after the title, then "+2" for the rest
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)
