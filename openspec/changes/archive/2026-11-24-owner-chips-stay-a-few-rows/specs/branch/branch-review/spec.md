## ADDED Requirements

### Requirement: Code owners: The chips stay a few rows

The header of the files MUST show "All" and the chips after it in their order, eight at most, and the chosen chip
wherever it stands; the others MUST be under "N More", a list of them with a search by name. Owners of as many files
MUST come in the order of the numbers in their names.

#### Scenario: A change of tens of owners
- **WHEN** the files of a review have more owners than the header shows
- **THEN** the header shows "All" and the first eight chips, "N More" the others, and the tree of the files stays in view below
- **AND** owners of as many files come in the order of the numbers in their names
- **AND** all chips show when one more would be all "N More" holds
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the tree in view under the chips of tens of owners, the others under N More with their search`)
  [@test] src/lib/review-owners.test.ts (`shows the first chips in their order and keeps the others for N More`, `shows every chip where one more would be all N More holds`)

#### Scenario: An owner found under N More
- **WHEN** the user searches the owners under "N More" and chooses one
- **THEN** its chip shows in the header, pressed, and marks its files; "N More" holds one chip less
- **AND** once another chip is chosen, it goes back under "N More"
- **AND** each chip of the list has its count and the shield of a group that still needs its approval, and the trigger has the shield while one of them does
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the tree in view under the chips of tens of owners, the others under N More with their search`)
  [@test] src/lib/review-owners.test.ts (`shows the chosen chip of N More in the header, and N More no more`)
