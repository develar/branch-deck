## MODIFIED Requirements

### Requirement: The page: A refusal of the server is an error in its words

A refusal of the server MUST be an error in its words: "Cannot push JIRA-456 into master" with "The server refused the
push into master: …". Words longer than 4 lines MUST show as 4 lines with "Show More", which shows the rest in the toast,
scrolled within it, so the title and the close button of the toast stay in the window (every toast, `ClampedToastDescription`).

#### Scenario: The server refused the push
- **WHEN** the server refused the push
- **THEN** the error says "Cannot push JIRA-456 into master" with "The server refused the push into master: …"
- **AND** the gate asks about it before the next push
  [@test] src/presenters/outcome.test.ts (`says what it pushed, a target that moved, the words of the server that refused it, and the push again of a plan that changed`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should say in the words of the server why it refused the push, and ask about it before the next push`)

#### Scenario: A refusal longer than the window
- **WHEN** the words of the server are longer than the window (a mirror that names every file it could not write)
- **THEN** the toast shows 4 lines of them with "Show More", its title and close button in the window
- **AND** "Show More" shows the rest in the toast, scrolled within it, and "Show Less" the 4 lines again
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should keep the title and the close button of a long refusal in the window, and show the rest with Show More`)

