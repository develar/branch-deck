## MODIFIED Requirements

### Requirement: Popover: The refusal is one headline with its fix first

The Safe Push popover of a refused start MUST say the first edge in a headline of one line, then a short detail
wrapped inside it: commits by their short hashes, the file by its name and line (each whole in its hover), never a
sentence its fix button repeats; the fix focused, "Start Anyway" an `outline` button below; Enter MUST NOT start anyway.

#### Scenario: A long path
- **WHEN** the start of `flow` needs `codec`, nothing lands it, and its use is in a long path
- **THEN** the headline is "Uses Codec of codec in FlowCodecIntegrationEntryPoint.kt", the detail "1a2b3c4d of flow
  uses Codec (FlowCodecIntegrationEntryPoint.kt:3), which 5e6f7a8b of codec adds.", its whole path in the hover of
  the file, and "Stack flow on codec" has the focus
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should say a need nothing lands in one headline and a short detail, focus its fix before Start Anyway and keep the popover narrow`)
  [@test] src/presenters/landing.test.ts (`says the first refusal short: the commits by their hashes, the file by its name, each whole in its hover, the fix only without its button`)
