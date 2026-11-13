## MODIFIED Requirements

### Requirement: Search: A row matches by every word of what it shows

A row MUST match when every word (split at white space) is, ignoring case, in what it shows or its branches: the
title, `#123`, the author, the meta of how the user takes part, the head and the base branch, its labels; never in its
URL or its description, whose words every row has. A pasted link of a row of the view MUST show that row.

#### Scenario: The words of a row
- **WHEN** the user types words
- **THEN** a row matches when every word (split at white space) is, ignoring case, in its title, `#123`, its author,
  the meta of how the user takes part (the teams asked, the reviewers, "you approved", "requested by dave", "changes
  requested by alice"), its head or its base branch, or its labels on GitHub
- **AND** never in its URL or its description, whose words every row has
  [@test] src/presenters/reviews.test.ts (`matches a row by every word of what it shows and its branches, ignoring case, never by its URL`, `matches a request by name by who asked, and a returned pull request by who handed it back and its branch`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

#### Scenario: A pasted link of a row
- **WHEN** the user pastes a link of a row of the view
- **THEN** that row shows
  [@test] src/presenters/reviews.test.ts (`finds the shown row a number or a pasted link names, never another number that starts the same`)
