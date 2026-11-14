## MODIFIED Requirements

### Requirement: Conflict and blocked dialog: "Copy Prompt for Agent" copies and confirms in place

The primary button "Copy Prompt for Agent" MUST copy a prompt and confirm in place: it says "Copied" (`CopyCheckIcon`)
for a moment, as a copy button does, in the width of its words (`WidestLabel`), so the footer stays as it is, and no
toast covers the dialog. Both cases SHALL offer it: an overlap of
uncommitted changes with the upstream needs the same merge as a conflict.

#### Scenario: Copy the prompt of a conflict
- **WHEN** the user clicks "Copy Prompt for Agent"
- **THEN** a prompt is copied, and the button says "Copied" (`CopyCheckIcon`) for a moment in its own width, with no
  toast over the dialog
  [@test] tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)

#### Scenario: A blocked update
- **WHEN** the dialog shows a block
- **THEN** it offers "Copy Prompt for Agent" too
