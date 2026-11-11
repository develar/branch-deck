## MODIFIED Requirements

### Requirement: Conflict view: The missing commits header has a persisted diff toggle

The missing commits header MUST have the Inline/Unified/Split toggle, persisted: a stored choice keeps winning over the
Inline default.

#### Scenario: A stored diff view mode
- **GIVEN** a stored choice of the diff view mode
- **WHEN** the missing commits header shows
- **THEN** its Inline/Unified/Split toggle has the stored choice, over the Inline default
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should keep a stored diff view mode over the Inline default`)
