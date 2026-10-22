# Scenario cases

The table of today's behaviour of both sides: what the app decides (`getBranchNextStep`, `getSafePushDecision`) and
what `bd` answers (`plan()` of `bd safe-push`, then the start itself) for the same situation. Data only: no crate reads
this directory yet ([Rust as the source of the domain rules](../../../docs/design/rust-domain-authority.md),
stage `s0c-scenarios`). Every expectation is today's answer, never the wished one; a case where the sides differ names
the discrepancy of the design doc's registry (`D1`–`D10`) or says why they differ (`asymmetry`). When a later stage
makes a rule one, the case keeps one expectation and loses its `discrepancy`.

Two tests read every `*.json` file here, one test function per `group`, and name the case id in each failure:

| Side | Test | Adapter |
| --- | --- | --- |
| `bd` | `crates/bd-remote/src/scenario_test.rs` | `Given` (serde, `deny_unknown_fields`) → a `Space` of the bd-cli fixtures with the fake Patronus and the fake GitHub → the preamble of `safe_push()` → `plan()` → `check_output`; then `bd safe-push` without `--check` |
| app | `src/lib/scenario-cases.test.ts` | `src/test-support/scenario-case.ts` (zod, strict) → `src/test-support/scenario-facts.ts` → `NextStepFacts` → `getBranchNextStep` → `SafePushFacts` → `getSafePushDecision` |

The fixed test names, by group:

| `group` | Rust | Vitest |
| --- | --- | --- |
| `modes-and-routes` | `the_safe_push_mode_and_route_cases_answer_as_bd_answers_today` | `the safe push mode and route cases answer as the app answers today` |
| `profiles` | `the_safe_push_profile_cases_answer_as_bd_answers_today` | `the safe push profile cases answer as the app answers today` |
| `review` | `the_safe_push_review_cases_answer_as_bd_answers_today` | `the safe push review cases answer as the app answers today` |

A new group gets its pair of tests in the same commit; a group without its tests fails both (each test checks that it
knows every group of the files).

## A case

```json
{
  "id": "skips-review-profile",
  "group": "review",
  "title": "the profile of the last robot, which failed on an older commit, skips the review; the approvals missing",
  "spec": ["spec/branch/patronus.spec.md#safe-push-without-review"],
  "discrepancy": "D2",
  "given": { "readiness": "missing", "robot": { "state": "failure", "on": "older" }, "profile": { "lastRobot": "lsc", "skipsReview": true } },
  "app": { "step": { "kind": "restart" }, "decision": { "kind": "confirm", "reasons": ["profile"] } },
  "bd": { "check": { "refusals": ["approvals_missing"], "profile": "lsc" }, "start": { "error": "approvals_missing" } }
}
```

- `id`: unique over all files, kebab-case; `title`: the situation in words; `spec`: the requirements it pins.
- `discrepancy`: `null`, or the id of the registry (`D1`–`D10`) whose two answers the case records.
- `asymmetry` (optional): `{ "kind": "byDesign" | "unregistered", "why": "…" }` for sides that differ without a registry
  id: `byDesign` names where the difference is meant (a spec, a doc comment), `unregistered` is a finding the registry
  lacks yet, for the planner to give an id.
- `app.step` (optional): a subset of `getBranchNextStep`: `kind`, `safePushBlocker`, `dryRunBlocker`.
- `app.decision`: a subset of `getSafePushDecision`: `kind` and `reason`, `reasons`, `what` or `offersPush`.
- `bd.check`: a subset of the `--check` output: `refusals` (the codes, in order), `push_needed`, `profile` (the id or
  `null`), `pull_request` (the number or `null`), `readiness` (`review.readiness` or `null`), `lands`, `retargeted`
  (the numbers); or `{ "error": code }` when `plan()` fails.
- `bd.start` (optional): the same command without `--check`: `{ "started_via": "api" | "pull_request" }` or
  `{ "error": code }`.

The outcomes compare by class: an app `refuse` and a `bd` refusal or error refuse; an app `start`, `confirm` or
`choose` and a `bd` start start (the agent asks the user what the popover asks, after `--check`); `wait` is neither.
The class of `bd` is that of `start` when given, else of `check`. Sides of different classes carry a `discrepancy` or
an `asymmetry`, sides of one class none: the Vitest checks it for every case.

## `given`: the closed vocabulary

Both schemas refuse a key outside it. A key is optional; the default is in brackets. The branch is `auth`
(`test-user/virtual/auth`), the target `master`, the GitHub repository `acme/monorepo`, the pull request #7.

| Key | Values | `bd` (the fixtures) | app (the facts) |
| --- | --- | --- | --- |
| `mode` | `safePush` · `dryRun` [`safePush`] | `--patronus-dry-run` | `SafePushFacts.mode`; the step's `dryRunBlocker` |
| `via` | `click` · `key` [`click`] | — (a command is a confirmed start) | `SafePushFacts.via` |
| `subject` | `branch` · `wholeStack` [`branch`] | `--whole-stack` | the whole stack's facts |
| `commits` | `(prefix) subject` strings [`["(auth) Add auth"]`] | `Space::new` | `commitCount`, the ancestors |
| `target` | `protected` · `unprotected` [`protected`] | `robot-start-supported` answers `supported: false` | `options.supported` |
| `remote` | `none` · `upToDate` · `behind` · `unpushed` · `rebuiltOnNewerBaseline` [`none`] | `bd push` and commits around it | `startPush` |
| `robot` | `{ state: running·failure·canceled·success, on: tip·older·ancestor:<b>·child:<b>·stack }` | the robot list (id `older`) | `robot` slot, `restart`, `replaces` |
| `pullRequest` | `{ base [master], draft [false], approvers [[]], reviewers [[]], state [open] }` | the GitHub search answer | `pullRequest`, `fromPullRequest` |
| `readiness` | `notRequired` · `approved` · `missing` · `unknown` · `unavailable` · `unlisted` · `space` [`notRequired`] | the pre-check answer, see below | `readiness`, `SafePushFacts.review` |
| `profile` | `{ chosen: <id>·none, lastRobot: lsc, skipsReview, newApi }` | `--profile`; the last robot's options | `ask.profileId`; `options` |
| `flags` | `withoutApprovals` · `cancelRunning` · `createPullRequest` · `allowMissing` · `allowDangling` | the flags of the same names | `withoutApprovals` = `ask.bypassReview`; the others have no input of the decision |
| `rules` | `keep` · `dismiss` · `unknown` · `loading` | — (`bd` reads no branch rules: D1, D9) | `SafePushFacts.dismiss` |
| `parked` | `true` | `bd park auth` | `parked` |
| `stackBranch` | `built` · `conflict` · `notBuilt` | the stack branch of the sync | the whole stack's step |
| `landed` | `true` | a merged Safe Push landed it | `landedWith` |
| `landingNeeds` | `true` | a name another branch adds | `landingNeeds`, `landingRefusals` |

A value the adapters don't map yet fails the case with "not mapped yet": the stage maps it with the first case that
needs it. Mapped in `s0c-1`: `mode`, `via`, `commits` (one branch), `target`, `remote` (`none`, `upToDate`), `robot`
(`failure` on `older`), `pullRequest`, `readiness`, `profile`, `flags`.

### How each side reads `readiness`, `profile` and `robot`

| `readiness` | `bd`: what the fakes answer | app: `readiness` · `review` |
| --- | --- | --- |
| `notRequired` | the pre-check needs no approval | `notRequired` · loaded |
| `approved` | with a pull request: Patronus evaluated it, IJPL Code Platform approved | `approved` · loaded |
| `missing` | the pre-check needs approvals; with a pull request evaluated, nobody approved | `missing` (IJPL Code Platform, Ilya Korennoy suggested) · loaded |
| `unknown` | a pull request with an approval, the review required, Patronus doesn't evaluate it | `unknown` (`approvalsNotEvaluated`) · loaded |
| `unavailable` | the pre-check answers 500 | `unavailable` (`error`) · loaded |
| `unlisted` | GitHub's search of the pull requests fails; the pre-check needs no approval | no pull requests listed · `unlisted` |
| `space` | no GitHub repository; the pre-check needs approvals (a Space merge request) | no pull requests listed · `space` (required) |

GitHub is configured (`branchdeck.githubRepository`, a token) for every value but `space`.

- `profile.chosen`: `--profile <id>` and the profile the popover chose; `none` is the default checks.
- `profile.lastRobot`: the profile of the last robot of the branch (`lsc`, the only one the fake names), the default of
  both sides; it needs `robot`. Patronus says that a profile skips the review only of this one: by the robot's options,
  or with `newApi` (Settings → Patronus) by the robot list, which says `skipsReview: true`. `skipsReview` without
  `lastRobot` is refused.
- `robot.on: older`: the robot ran on another commit (`sourceRevision` `0000`); `tip` on the branch's tip.
