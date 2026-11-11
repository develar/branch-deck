## MODIFIED Requirements

### Requirement: The sidebar and the diffs: The find works anywhere in the review window

⌘F (Ctrl+F off macOS) MUST work anywhere in the review window, including the sidebar; ⌘G and Shift+⌘G step while the
bar is open, including with focus in the sidebar. A ⌘F pressed before the diffs of the subject came MUST open the find
bar as they come.

#### Scenario: Find from the file tree
- **GIVEN** the focus is in the sidebar
- **WHEN** the user presses ⌘F, then ⌘G and Shift+⌘G
- **THEN** the find bar opens and steps through the hits
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should open find from the file tree and step results there with ⌘F and ⌘G @webkit`)

#### Scenario: Find before the diffs came
- **GIVEN** the tree shows the files of the review and its diffs are still read
- **WHEN** the user presses ⌘F in the tree
- **THEN** nothing opens and the focus stays in the tree
- **AND** the find bar opens as the diffs come, its field focused
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should open find by ⌘F pressed in the file tree before the diffs of the review came, once they come @webkit`)

#### Scenario: Find while the files are still read
- **GIVEN** neither the files nor the diffs of the review are read, and the frame of the review has the focus
- **WHEN** the user presses ⌘F
- **THEN** the find bar opens as the diffs come, its field focused
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should open find by ⌘F pressed while the review still reads its files, once its diffs come @webkit`)
