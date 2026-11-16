## Why

The repository action adds its own indentation, then shifts left when completed. The equally emphasized upcoming step and repository-only footer obscure the sequence.

## What Changes

- Align all step states with a shared layout and a borderless full-row repository action.
- Retain step 1's number and put its completion check beside the title.
- Mute the upcoming prefix step, explain its dependency, and use a footer that covers both tasks.
- Use one progress-color rule: primary numbers and titles for the active step, muted upcoming steps, and neutral completed steps with a success check.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `repository/onboarding`: welcome step layout, completion feedback and guidance.

## Impact

Welcome card presentation, focused Chromium/WebKit coverage, HTML snapshots and UX documentation. No backend, bindings, settings or onboarding lifecycle changes.
