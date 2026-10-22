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
| app | `src/lib/scenario-cases.test.ts` | `src/test-support/scenario-case.ts` (zod, strict) → `src/test-support/scenario-facts.ts` → `NextStepFacts` (of the whole stack as `stackFactsOf` reads them) → `getBranchNextStep` → `SafePushFacts` → `getSafePushDecision` |

The fixed test names, by group:

| `group` | Rust | Vitest |
| --- | --- | --- |
| `modes-and-routes` | `the_safe_push_mode_and_route_cases_answer_as_bd_answers_today` | `the safe push mode and route cases answer as the app answers today` |
| `profiles` | `the_safe_push_profile_cases_answer_as_bd_answers_today` | `the safe push profile cases answer as the app answers today` |
| `review` | `the_safe_push_review_cases_answer_as_bd_answers_today` | `the safe push review cases answer as the app answers today` |
| `next-step-rows` | `the_next_step_row_cases_answer_as_bd_answers_today` | `the next step row cases answer as the app answers today` |
| `stack-and-whole-stack` | `the_stack_and_whole_stack_cases_answer_as_bd_answers_today` | `the stack and whole stack cases answer as the app answers today` |

`safe-push.json` holds the first three groups, `next-step.json` the last two: one case per row of the table of
[Next Step](../../../spec/branch/next-step.spec.md#the-table) and per bullet of its stacked branch and its whole stack.

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

- `id`: unique over all files, kebab-case; `title`: the situation in words; `spec`: the requirements it pins. A row of
  the Next Step table is `spec/branch/next-step.spec.md#the-table:<row>` (`1`, `1′`, `2`, `3`, `5`, `5′`, `6`, `7`, `8`,
  `9`, `10`, `8′`, `10′`, `11`): the Vitest `cover every row of the next step table` fails on a row no case names.
- `discrepancy`: `null`, or the id of the registry (`D1`–`D10`) whose two answers the case records.
- `asymmetry` (optional): `{ "kind": "byDesign" | "unregistered", "why": "…" }` for sides that differ without a registry
  id: `byDesign` names where the difference is meant (a spec, a doc comment), `unregistered` is a finding the registry
  lacks yet, for the planner to give an id.
- `app.step` (optional): a subset of `getBranchNextStep`: `kind`, `safePushBlocker`, `dryRunBlocker`.
- `app.decision` (optional, where the row feeds a start): a subset of `getSafePushDecision`: `kind` and `reason`,
  `reasons`, `what` or `offersPush`. Without it the step decides the class, so `app.step` names the blocker of the mode
  (`safePushBlocker`, of a Dry Run `dryRunBlocker`): `hidden` or a blocker refuses, else it starts.
- `bd.check`: a subset of the `--check` output: `refusals` (the codes, in order), `push_needed`, `profile` (the id or
  `null`), `pull_request` (the number or `null`), `readiness` (`review.readiness` or `null`), `lands`, `retargeted`
  (the numbers); or `{ "error": code }` when `plan()` fails.
- `bd.start` (optional): the same command without `--check`: `{ "started_via": "api" | "pull_request" }` or
  `{ "error": code }`.

The outcomes compare by class: an app `refuse` and a `bd` refusal or error refuse; an app `start`, `confirm` or
`choose` and a `bd` start start (the agent asks the user what the popover asks, after `--check`); `wait` is neither.
The class of `bd` is that of `start` when given, else of `check`. Sides of different classes carry a `discrepancy` or
an `asymmetry`; an `asymmetry` stands only there. A `discrepancy` may stand where the classes agree: both start, one of
them for a reason the registry names wrong (D9: the app reads a failed read of the rules as "keeps the approvals", bd
never reads them). The Vitest checks it for every case.

## `given`: the closed vocabulary

Both schemas refuse a key outside it. A key is optional; the default is in brackets. The branch is `auth`
(`test-user/virtual/auth`), the target `master`, the GitHub repository `acme/monorepo`, the pull request #7, the pull
request of a whole stack #12.

| Key | Values | `bd` (the fixtures) | app (the facts) |
| --- | --- | --- | --- |
| `mode` | `safePush` · `dryRun` [`safePush`] | `--patronus-dry-run` | `SafePushFacts.mode`; the step's `dryRunBlocker` |
| `branch` | a branch of `commits` [`auth`] | the branch argument | `branchName`, `subject`; of the whole stack its root |
| `via` | `click` · `key` [`click`] | — (a command is a confirmed start) | `SafePushFacts.via` |
| `subject` | `branch` · `wholeStack` [`branch`] | `--whole-stack` | the facts of the stack branch while the stack forks, else of its last branch |
| `commits` | `(prefix) subject` strings, `(parent/child)` stacked [`["(auth) Add auth"]`] | `f<i>.txt` of each, as `commit_subjects`; stacks on with a parent | `commitCount`, the ancestors (`landsBranches`, `ancestorCount`) |
| `target` | `protected` · `unprotected` [`protected`] | `robot-start-supported` answers `supported: false` | `options.supported` |
| `remote` | `none` · `upToDate` · `behind` · `unpushed` · `rebuiltOnNewerBaseline` [`none`] | `bd push`; `unpushed` one commit after it | `startPush`, the `dismiss` fact |
| `robot` | `{ state: running·failure·canceled·success, on: tip·older·ancestor:<b>·child:<b>·stack, reason: approvals, fromPullRequest }`, or a list | the robot list after a sync, on the tips it built (`older`: `0000`) | `robot` slot (`getSafePushSlot`), `restart`, `replaces`, `startBlocker`, `landingPart`, `held`, `landedWith` |
| `sync` | `running` | — (`bd` syncs before it plans) | `syncing` |
| `conflicts` | a branch | its first commit changes `config.txt`, which only an unassigned commit creates | `hasError` of it, `blocked` of a branch built on it, the stack branch `Conflict` |
| `notOnSpace` | `true` | `origin` set to the bare repository | `patronusAvailable: false` |
| `pullRequest` | `{ base [master], draft [false], approvers [[]], reviewers [[]], changesRequested [[]], state [open] }` | the GitHub search answer (`latestReviews`, `reviewRequests`) | `pullRequest`, `fromPullRequest` |
| `stackPullRequest` | `true` | the open #12 of `{prefix}/stack/<root>` in the search | `stackPullRequest`; of the whole stack its `pullRequest` |
| `readiness` | `notRequired` · `approved` · `missing` · `unknown` · `unavailable` · `unlisted` · `space` · `notAsked` [`notRequired`] | the pre-check answer, see below | `readiness`, `SafePushFacts.review` |
| `profile` | `{ chosen: <id>·none, lastRobot: lsc, skipsReview, newApi }` | `--profile`; the last robot's options | `ask.profileId`; `options` |
| `flags` | `withoutApprovals` · `cancelRunning` · `createPullRequest` · `allowMissing` · `allowDangling` | the flags of the same names | `withoutApprovals` = `ask.bypassReview`; the others have no input of the decision |
| `rules` | `keep` · `dismiss` · `unknown` · `loading` | — (`bd` reads no branch rules: D1, D9) | `SafePushFacts.dismiss`: `unknown` is `false`, as `readDismissesStaleReviews` reads a failed answer |
| `parked` | `self` · `builtOn` | `bd park` of the branch or of its parent | `parked` |
| `stackBranch` | `notBuilt` | the last branch gets a stack problem (`(<b>/<b>) Loop`) | the stack branch `Blocked` by it |
| `landed` | `inBaseline` · `byContent` | `origin/master` at HEAD; `land_auth_by_content` after `bd push` (`LANDED_SUBJECTS` only) | `landedIn`; one commit left |
| `landingNeeds` | `true` | `(codec) Add controlUrl`, then the branch uses it (Kotlin) | `landingNeeds`, `landingRefusals` |

A value the adapters don't map yet fails the case with "not mapped yet": the stage maps it with the first case that
needs it. Not mapped yet: `remote` `behind` and `rebuiltOnNewerBaseline`; a decision of the whole stack; robots of the
whole stack other than merged ones on the app's side.

A merged robot lands by the hashes of the tips the sync built: of the branch itself "Merged by Patronus", of a branch
built on it (`child:<b>`) or of the stack branch (`stack`) "Landed in master with …"; a running one on `tip` holds the
branch, on `child:<b>` lands it, on `ancestor:<b>` lands a part of it (a Dry Run doesn't wait for it).

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
| `notAsked` | the pre-check needs no approval: `bd` always asks | null, the start waits for it (`loading`) |

GitHub is configured (`branchdeck.githubRepository`, a token) for every value but `space`.

- `profile.chosen`: `--profile <id>` and the profile the popover chose; `none` is the default checks.
- `profile.lastRobot`: the profile of the last robot of the branch (`lsc`, the only one the fake names), the default of
  both sides; it needs `robot`. Patronus says that a profile skips the review only of this one: by the robot's options,
  or with `newApi` (Settings → Patronus) by the robot list, which says `skipsReview: true`. `skipsReview` without
  `lastRobot` is refused.
- `robot.on: older`: the robot ran on another commit (`sourceRevision` `0000`); `tip` on the branch's tip.
