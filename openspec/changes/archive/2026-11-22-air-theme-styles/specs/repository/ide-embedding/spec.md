## MODIFIED Requirements

### Requirement: Page: The theme of the IDE maps onto the tokens of the page

The page MUST map the theme and the styles of `hostContext` onto its semantic tokens, and follow each
`host-context-changed`. The canvas is the background of the place; the cards, the popups and the fields are the raised
surface of the IDE; the separator is the separator of the IDE. A token whose meaning the IDE lacks stays the page's:
`--info`, the violet of merged, and `--primary`. The `styles.variables` of a context MUST be the whole set: a variable
that a later set lacks leaves the page.

#### Scenario: Light to Dark
- **WHEN** the IDE switches from Light to Dark
- **THEN** the page shows the dark tokens without a reload
  [@test] tests/e2e/air/air-page.spec.ts (`the page shows the theme of the IDE and follows its switch from Light to Dark without a reload`)

#### Scenario: The styles all
- **GIVEN** the app file has `styles: all`, so Air sends every variable
- **WHEN** the page shows the theme of the IDE
- **THEN** `--info` is the violet of the page, and `--border` is the separator of the IDE
  [@test] tests/e2e/air/air-page.spec.ts (`with the styles all of the app file, the violet of merged stays the page's, and the lines of the rows are the separator of the IDE`)

#### Scenario: The styles neutral, then none
- **GIVEN** the app file has `styles: neutral`, so Air sends no status tone
- **WHEN** the page shows the theme of the IDE
- **THEN** `--success` and `--destructive` are the page's, and `--canvas` and `--card` the IDE's
- **AND** after a change of the field to `none` without a reload, every token is the page's
  [@test] tests/e2e/air/air-page.spec.ts (`with the styles neutral of the app file, the status tones are the page's and the surfaces the IDE's, and a change to none without a reload makes every token the page's`)

### Requirement: Install: The app file is the example of the contract with this binary

The app file MUST be the example app file of the contract with the command of this binary: the app binary with
`cli mcp`; `brd`, on Windows `brd.exe` beside the app, with `mcp`. A binary that macOS runs translocated is refused, as
the install of the agent CLI refuses it. The file MUST have `styles: neutral`, but an install MUST keep the value
`all`, `neutral` or `none` that the file has, and the status MUST compare the file with the text of that value.

#### Scenario: The app file
- **WHEN** the install writes the app file
- **THEN** it reads as `docs/design/ide-embedding/protocol/examples/branch-deck.yaml` with the command of this binary
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_app_file_is_the_example_of_the_contract_with_the_command_of_this_binary`)

#### Scenario: The command of each binary
- **WHEN** the binary is the app binary, or `brd`
- **THEN** the arguments are `cli mcp` (`mcp` on Windows), or `mcp`
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_runs_brd_mcp_with_the_app_binary_and_mcp_with_brd`)

#### Scenario: The styles of the user
- **GIVEN** the user set `styles: all` in the app file of this binary
- **WHEN** Settings → Tools reads the file, and the user clicks Install
- **THEN** the row says the file is current, and the install keeps `styles: all`
- **AND** a value that Air does not know is not current, and the install writes `neutral`
  [@test] crates/bd-cli/src/install_test.rs (`an_install_keeps_the_styles_the_user_set_in_the_ide_app_file`)
