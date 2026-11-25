# Proposal

## Why

The detail under the headline of a refused start in the Safe Push popover was still the gate's whole sentence: each
commit by its quoted subject, the whole path of the use (`flow/src/main/kotlin/…/FlowCodecIntegrationEntryPoint.kt:3`),
and "Stack flow on codec to land it too.", which repeats the button below it. On a long path it took five lines.

## What Changes

- The detail says the first refusal short (`describeRefusalDetail`): the commits by their short hashes, their subjects
  in the hover, the file by its name and line, its path in the hover, then the count of the others; the fix only where
  no button of the popover runs it. The texts of `brd` stay as they are.

Capabilities: `branch/landing-plan`, `branch/patronus`.
