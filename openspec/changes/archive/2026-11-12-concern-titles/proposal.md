# Proposal

## Why

A step of the concerns, the task of a range of one commit or of commits without a body, was titled by a bare name of
its evidence: `probe` for a function a commit adds and calls, `read_head` for a function whose signature it changes.
The name said which identifier joined the parts, never what the change does to it ("The name of a token concern is
a word, not a sentence", a risk of [Review Concerns](../../specs/branch/review-concerns/spec.md)).

## What Changes

- A concern says what it does to its name (`change`): what the change does to the declaration of that name among its
  units (of a token, the one so named): `adds`, `removes`, `form` with the parts of its form that changed, `body`;
  `replaces` of a replacement; of its one file or its directory `adds` or `removes` where every part adds or deletes
  a file whole; else `mixed`.
- The page titles the step of a concern by it, a sentence set as text: "Add probe", "Change the parameters of open",
  "Change the signature of read_head", "Change open"; a concern of `mixed`, a replacement, the root and Other changes
  keep the name alone, as code.
- The name stays the token where the uses of a declaration joined the concern more often: measured on the lab, a
  name by that declaration was less often in the subject of its commit. A verb from the tokens alone (all their
  changed lines add them) was wrong most of the time ("Add over"): the verb comes from a declaration.
- `brd review-plan` prints `"schema": 34`, the `change` of each concern. The groups and the names of the 739 ranges
  of the lab are those before.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-concerns`: what a concern does to its name, its title in the window, the token that names a
  concern the uses of a declaration joined more often, the measurements.
- `branch/review-tour`: the title of a step of the concerns no section of the body names.
- `branch/review-plan`: the schema version.

## Impact

- `crates/review-plan/src/concerns.rs` (`ConcernChange`, `name_of`).
- `crates/bd-rewrite/src/review_plan_command.rs` (`REVIEW_PLAN_SCHEMA_VERSION` 34).
- `src/presenters/review-concerns.ts` (`presentConcernTitle`), `src/presenters/review-steps.ts`, `src/lib/bindings.ts`.
- `docs/contributing/agent-cli.md`, `docs/design/git-operations.md`, `docs/design/change-review.md`.
- The lab (`~/projects/review-concerns-lab`): `title_report.py`, the `change` of a group in `groupers/bd_concerns.py`.
