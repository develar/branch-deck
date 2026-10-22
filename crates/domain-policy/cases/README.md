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
| `landing` | `the_landing_cases_answer_as_bd_answers_today` | `the landing cases answer as the app answers today` |
| `discrepancies` | `the_discrepancy_cases_name_d1_to_d10_as_the_register_does` | `the discrepancy cases name d1 to d10 as the register does` |

`safe-push.json` holds the first three groups, `next-step.json` the next two: one case per row of the table of
[Next Step](../../../spec/branch/next-step.spec.md#the-table) and per bullet of its stacked branch and its whole stack.
`landing.json` holds the rest of the Safe Push matrix: robots around the branch (on the remote branch rebuilt since,
of a branch built on it, of an older commit with and without a push, `--cancel-running`), the parked member of a whole
stack, a remote branch behind, the retarget of an ancestor's pull request, missing declarations and dangling references
with their `--allow-*`. `discrepancies.json` is the registry (below).

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
- `discrepancy`: `null`, or the id of the registry (`D1`, `D2`, `D3a`, `D3b`, `D4`–`D10`) whose two answers the case
  records: only in the registry, or as the `twin` of its entry.
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
- `bd.start` (optional): the same command without `--check`: `{ "started_via": "api" | "pull_request", "canceled" }`
  (`canceled`: it canceled the running Safe Push of an older commit first) or `{ "error": code }`.

The outcomes compare by class: an app `refuse` and a `bd` refusal or error refuse; an app `start`, `confirm` or
`choose` and a `bd` start start (the agent asks the user what the popover asks, after `--check`); `wait` is neither.
The class of `bd` is that of `start` when given, else of `check`. Sides of different classes carry a `discrepancy` or
an `asymmetry`; an `asymmetry` stands only there. A `discrepancy` may stand where the classes agree: both start, one of
them for a reason the registry names wrong (D9: the app reads a failed read of the rules as "keeps the approvals", bd
never reads them). The Vitest checks it for every case.

## The registry: `discrepancies.json`

One entry of each id of the registry of the design doc: `D1`, `D2`, `D3a`, `D3b`, `D4`–`D10`, each once. An entry is
`exercised` (the default `kind`: `given`, `app` and `bd` as any case) or `recorded`: a discrepancy that no decision of
either side reaches (the order of a retarget and a push, a retry, a request, the IPC), with `today: { app, bd, why }`,
where each side does it today, each a list of `path:line` or `path#name` (a test, a function, a line of text) from the
repository root; a recorded entry has no `given`, `app` or `bd`. `twin` names the exercised case of another group with
the same `discrepancy` (D2: `skips-review-profile`, D3b: `row-5-prime-another-base`): a case outside the registry
carries a discrepancy only as the twin of its entry. An exercised case whose classes agree carries `today.why`, what
differs (D9: both start, the app by a failed read of the rules, `bd` because it never reads them).

Both tests check the registry: each id once, the twins, no recorded case outside it; the Rust one also that every
pointer of `today` resolves (the file exists, the line is in it, the name is in it), the Vitest that a discrepancy whose
classes agree says why. D8 is recorded: the vocabulary has no stored stack parent, so its case is a test of each side
of its own, `the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8` (Rust: the placement, the
sync of the fixture, `whole_stack_of`, `bd safe-push A --whole-stack --check`) and `the whole stack of a blocked branch
with an inactive declared parent is d8` (Vitest: the same table in `branchSyncStore`, `stackMainLineIn`,
`getStackNextStepNow`).

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
| `remote` | `none` · `upToDate` · `behind` · `unpushed` · `rebuiltOnNewerBaseline` [`none`] | `bd push`; `unpushed` one commit after it; `behind` a commit of someone else on the remote branch, fetched; `rebuiltOnNewerBaseline` the baseline moved and the branch rebased onto it | `startPush` (`behind` force, `rebuiltOnNewerBaseline` none), the remote status, the `dismiss` fact |
| `robot` | `{ state: running·failure·canceled·success, on: tip·older·remote·ancestor:<b>·child:<b>·stack, reason: approvals, fromPullRequest }`, or a list | the robot list after a sync, on the tips it built (`older`: `0000`) | `robot` slot (`getSafePushSlot`), `restart`, `replaces`, `startBlocker`, `landingPart`, `held`, `landedWith` |
| `sync` | `running` | — (`bd` syncs before it plans) | `syncing` |
| `conflicts` | a branch | its first commit changes `config.txt`, which only an unassigned commit creates | `hasError` of it, `blocked` of a branch built on it, the stack branch `Conflict` |
| `notOnSpace` | `true` | `origin` set to the bare repository | `patronusAvailable: false` |
| `pullRequest` | `{ of [the branch], base [master], draft [false], approvers [[]], reviewers [[]], changesRequested [[]], state [open] }` | the GitHub search answer (`latestReviews`, `reviewRequests`) | `pullRequest`, `fromPullRequest` of `of` only |
| `stackPullRequest` | `true` | the open #12 of `{prefix}/stack/<root>` in the search | `stackPullRequest`; of the whole stack its `pullRequest` |
| `readiness` | `notRequired` · `approved` · `missing` · `unknown` · `unavailable` · `unlisted` · `space` · `notAsked` [`notRequired`] | the pre-check answer, see below | `readiness`, `SafePushFacts.review` |
| `profile` | `{ chosen: <id>·none, lastRobot: lsc, skipsReview, newApi }` | `--profile`; the last robot's options | `ask.profileId`; `options` |
| `flags` | `withoutApprovals` · `cancelRunning` · `createPullRequest` · `allowMissing` · `allowDangling` | the flags of the same names | `withoutApprovals` = `ask.bypassReview`; the others have no input of the decision |
| `rules` | `keep` · `dismiss` · `unknown` · `loading` | — (`bd` reads no branch rules: D1, D9) | `SafePushFacts.dismiss`: `unknown` is `false`, as `readDismissesStaleReviews` reads a failed answer |
| `parked` | `self` · `builtOn` · a branch | `bd park` of the branch, of its parent or of the branch named | `parked`; of the whole stack `wholeStackParkedState` |
| `stackBranch` | `notBuilt` | the last branch gets a stack problem (`(<b>/<b>) Loop`) | the stack branch `Blocked` by it |
| `landed` | `inBaseline` · `byContent` | `origin/master` at HEAD; `land_auth_by_content` after `bd push` (`LANDED_SUBJECTS` only) | `landedIn`; one commit left |
| `landingNeeds` | `true` | `(codec) Add controlUrl`, then the branch uses it (Kotlin) | `landingNeeds`, `landingRefusals` |
| `dangling` | `removed` (branch `fake-agent`) · `changed` (branch `suspend`), with `commits: []` | `commit_dangling_reference` · `commit_changed_declaration` | the fixture's commits; a `danglingReference` in `landingNeeds` |

A value the adapters don't map yet fails the case with "not mapped yet": the stage maps it with the first case that
needs it. Not mapped yet: a decision of the whole stack; robots of the whole stack other than merged ones on the app's side.

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
- `robot.on: older`: the robot ran on another commit (`sourceRevision` `0000`); `tip` on the branch's tip; `remote`
  on the remote branch, which `remote: rebuiltOnNewerBaseline` rebuilt since (a start would test the same commits).
