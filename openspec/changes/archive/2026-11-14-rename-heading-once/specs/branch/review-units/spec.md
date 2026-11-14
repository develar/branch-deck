## MODIFIED Requirements

### Requirement: In the Review Window: A body changed in a rename the index has not proven is worded as the rename

The heading of a unit with `renamed` MUST word the renames and their wait in place of "body changed" ("balance — only
renamed sumOf → totalOf, not proven"), of one with `renamed_from` its old name and the wait after what else of its form
changed ("totalOf — renamed from sumOf, not proven"); the badge of the card of its name MUST say "Renamed, not proven"
or "Renamed, checking". A unit `behaviour` without either keeps "body changed". Where the note of that rename stands in
the row of the heading, it MUST NOT spell the rename again: its sites alone, and what else it tells.

#### Scenario: The heading of a rename no index proved
- **GIVEN** a review with its plan of a commit that renames a private `sumOf` to `totalOf`, and no index
- **WHEN** the review shows the diffs
- **THEN** the heading of a call that changed in the name alone says "balance — only renamed sumOf → totalOf, not proven"
- **AND** the heading of a call whose line also changed in another way says "half — body changed"
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] src/presenters/review-units.test.ts (`words a body that changed in renamed names alone as the renames, not proven or checking, as Rust decided`)

#### Scenario: The heading of a declaration renamed
- **GIVEN** the same review
- **WHEN** it shows the declaration of `totalOf`
- **THEN** its heading says "totalOf — renamed from sumOf, not proven", and no heading names `sumOf`
- **AND** "… checking" while the index is asked, and "totalOf — parameters changed, renamed from sumOf, not proven" where its parameters changed too
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] src/presenters/review-units.test.ts (`words a declaration renamed where the index has not proven it as renamed from its old name, after what else of its form changed`)

#### Scenario: The row of a declaration renamed says the rename once
- **GIVEN** the same review, the block of the declaration of `totalOf` starting with the note of its rename
- **WHEN** the review shows it
- **THEN** its row says "totalOf — renamed from sumOf, not proven · 3 sites", the note its sites alone, and a screen reader hears the note in full
- **AND** in the pull request of the helper "totalOf — renamed from sumOf, not proven · 4 sites"; a note of another rename keeps its two spellings
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a body that changed in a rename no index proved says it is the rename, not proven, and a body with more beside it says it changed`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say of the calls and the declaration of a helper renamed where no index proves it that they are the rename, not proven, as the header says of its file`)
  [@test] src/presenters/diff-marks.test.ts (`says of a rename the heading of its row names already its sites alone, and names it in full to a screen reader`)

#### Scenario: A pull request that renames a private helper
- **GIVEN** the review of a pull request whose one commit renames the private `sumOf` of a ledger to `totalOf`, and no index
- **WHEN** its changes show
- **THEN** the header says "4 changes to read in 1 file · 1 file not proven", and the headings of its three calls "balance — only renamed sumOf → totalOf, not proven", "balanceOn — …" and "averageAmount — …"
- **AND** the heading of the helper says "totalOf — renamed from sumOf, not proven"; no heading says "body changed", "new" or "removed"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say of the calls and the declaration of a helper renamed where no index proves it that they are the rename, not proven, as the header says of its file`)

#### Scenario: The card of the name
- **WHEN** a mention of the story names such a unit
- **THEN** its card says "Renamed, not proven", "Renamed, checking" while the index is asked, the same of a declaration renamed, and "Body changed" of a unit without either
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)
