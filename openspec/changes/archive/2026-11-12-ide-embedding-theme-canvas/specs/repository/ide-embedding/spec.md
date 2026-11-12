## MODIFIED Requirements

### Requirement: Page: The theme of the IDE maps onto the tokens of the page

The page MUST map the theme and the styles of `hostContext` onto its semantic tokens, and follow each
`host-context-changed`. The canvas is the first background of the IDE, the background of the place, so the page is one
surface with its tool window; the cards, the popups and the fields are the second, the raised surface, so the canvas is
darker than the cards in both themes.

#### Scenario: Light to Dark
- **WHEN** the IDE switches from Light to Dark
- **THEN** the page shows the dark tokens without a reload
  [@test] tests/e2e/air/air-page.spec.ts (`the page shows the theme of the IDE and follows its switch from Light to Dark without a reload`)
