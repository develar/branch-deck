## ADDED Requirements

### Requirement: One window: The header of a pull request says what it leaves to read

The header of the review of a pull request, and of each layer of a stack, MUST say what the pull request leaves to
read as Rust proved it from the marks of its diffs ([Range Reading](../range-reading/spec.md)), with the author's word
the diff proves wrong; a repository that turns the review plan off shows none.

#### Scenario: A pull request of formatting alone
- **WHEN** the window reviews a pull request that changes the spaces between tokens alone
- **THEN** its header says "Nothing to read · formatting"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say a pull request of formatting alone leaves nothing to read, its label of a refactoring then holding`)
