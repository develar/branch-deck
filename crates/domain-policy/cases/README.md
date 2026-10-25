# Scenario cases

The table of today's behaviour of both sides: what the app decides (`getBranchNextStepOf`, `getStackNextStepNow`,
`readSafePush`) and what `bd` answers (`bd safe-push --check`, then the start itself) for the same situation. Data
only: the rules of this crate (`src/`) don't read it yet, the two tests below do ([Rust as the source of the domain
rules](../../../docs/design/rust-domain-authority.md), stage `s0c-scenarios`). Every expectation is today's answer, never the wished one; a case where the sides differ names
the discrepancy of the design doc's registry (`D1`–`D15`) or says why they differ by design (`asymmetry`). When a later
stage makes a rule one, the case keeps one expectation and loses its `discrepancy`.

Two tests read every `*.json` file here, one test function per `group`, and name the case id in each failure:

| Side | Test | Adapter |
| --- | --- | --- |
| `bd` | `crates/bd-remote/src/scenario_test.rs` | `Given` (serde, `deny_unknown_fields`) → a `Space` of the bd-cli fixtures with the fake Patronus and the fake GitHub → `safe_push()` itself with `--check` (the function the command runs: its preamble, `plan()`, the output of a check) → the projection of the output; then `bd safe-push` without `--check` |
| app | `src/lib/scenario-cases.test.ts` | `src/test-support/scenario-case.ts` (zod, strict) → `src/test-support/scenario-facts.ts` seeds the stores as the sync, the repository view, Patronus, GitHub and the review scan leave them (`seedScenario`), what Rust computes of them from `facts/<id>.json` → the app's own readers: `getBranchNextStepOf` (`getStackNextStepNow` of the whole stack) for the step, `readSafePush` for the decision |

No adapter decides for its side. The Rust side builds a repository of `given` with the fixtures, once per case. What
the app's stores hold of the Rust core — the tips the sync left, the robots as the list answers them, in its order,
the repository view (`patronus_view` with the landing needs, as `bd status` reads it), the review readiness of the
branch and of its whole stack, and the open pull requests as the view of the session reads them (`openPullRequests`:
where each goes, a draft, the code owners Request Review… offers; `open_pull_requests_view`) — the group test computes of that same repository before it checks and starts, and
writes to `facts/<id>.json`, one file per exercised case (`assert_facts_kept`; the ids named `<branch>-tip`,
`<branch>-remote`, `stack-<root>-tip`, `<branch>-c<i>` for the i-th original commit, the robots with their start and
finish times settled by rank, the robot pages under `https://patronus`). The TS side seeds those files. What it still
models itself, in the shape of the stores, is the input the app would have read from the sync and the baseline check,
not a rule of a decision: the table of the sync (`Branch` with its status, its count and its `remoteStatus`, the
`StackBranch`), the landing report of `landed`, the `PullRequest`s and the rules of the target the session read (`facts.rules`); a rule that
starts reading more of them (the commits of the landing report, `commitsRestacked`) moves that part into the facts
first. The rule of the start lives only in the app (`getPullRequestStart`): the facts record the tip and the pull
request Rust asked with, and `seedReadiness` refuses an answer whose input the app computes otherwise (`the readiness
facts asked with another start or tip than the app computes now`). So a change of a rule of the app reddens the Vitest,
a change of `plan()`, of the sync, of `patronus_view`, of the robot list or of the review scan the Rust group test:
`UPDATE_SCENARIO_FACTS=1 cargo nextest run -p bd-remote -E 'test(/scenario/)'` rewrites the files (and removes those
of cases that are gone or no longer exercised), and the diff is reviewed like code, because the Vitest answers from
them. A file records the `given` it was computed from: the Vitest refuses facts of another `given`, and a case without
its file fails with the command.

The fixed test names, by group:

| `group` | Rust | Vitest |
| --- | --- | --- |
| `modes-and-routes` | `the_safe_push_mode_and_route_cases_answer_as_bd_answers_today` | `the safe push mode and route cases answer as the app answers today` |
| `profiles` | `the_safe_push_profile_cases_answer_as_bd_answers_today` | `the safe push profile cases answer as the app answers today` |
| `review` | `the_safe_push_review_cases_answer_as_bd_answers_today` | `the safe push review cases answer as the app answers today` |
| `next-step-rows` | `the_next_step_row_cases_answer_as_bd_answers_today` | `the next step row cases answer as the app answers today` |
| `stack-and-whole-stack` | `the_stack_and_whole_stack_cases_answer_as_bd_answers_today` | `the stack and whole stack cases answer as the app answers today` |
| `landing` | `the_landing_cases_answer_as_bd_answers_today` | `the landing cases answer as the app answers today` |
| `discrepancies` | `the_discrepancy_cases_name_every_id_as_the_register_does` | `the discrepancy cases name every id as the register does` |

The Rust test of a group keeps the facts of its exercised cases too (the bd answers first, then the files), and
`no_facts_file_is_of_a_case_that_is_gone` keeps the directory to the exercised cases with a `given`.

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
  "discrepancy": null,
  "given": { "readiness": "missing", "robot": { "state": "failure", "on": "older" }, "profile": { "lastRobot": "lsc", "skipsReview": true } },
  "app": { "step": { "kind": "createPullRequest" }, "decision": { "kind": "confirm", "reasons": ["profile"] } },
  "bd": { "check": { "refusals": [], "profile": "lsc" }, "start": { "started_via": "api" } }
}
```

- `id`: unique over all files, kebab-case; `title`: the situation in words; `spec`: the requirements it pins. A row of
  the Next Step table is `spec/branch/next-step.spec.md#the-table:<row>` (`1`, `1′`, `2`, `3`, `5`, `5′`, `6`, `7`, `8`,
  `9`, `10`, `8′`, `10′`, `11`): the Vitest `cover every row of the next step table` fails on a row no case names.
- `discrepancy`: `null`, or the id of the registry (`D1`, `D2`, `D3a`, `D3b`, `D4`–`D15`) whose two answers the case
  records: only in the registry, or as one of the `twins` of its entry.
- `asymmetry` (optional): `{ "kind": "byDesign", "why": "…" }` for sides that differ where the difference is meant (a
  spec, a doc comment names it). There is no other kind: a difference the registry lacks gets its id in the same
  commit, so the table can't park a discrepancy.
- `app.step` (optional): a subset of `getBranchNextStep`: `kind`, `safePushBlocker`, `dryRunBlocker`.
- `app.decision` (optional, where the row feeds a start): a subset of `getSafePushDecision`: `kind` and `reason`,
  `reasons`, `what` or `offersPush`. Without it the step decides the class, so `app.step` names the blocker of the mode
  (`safePushBlocker`, of a Dry Run `dryRunBlocker`): `hidden` or a blocker refuses, else it starts.
- `bd.check`: a subset of the `--check` output: `refusals` (the codes, in order), `push_needed`, `profile` (the id or
  `null`), `pull_request` (the number or `null`), `readiness` (`review.readiness` or `null`), `rules`
  (`rules.dismiss_stale_reviews_on_push`: `true`, `false`, or `null` when GitHub didn't answer or bd didn't ask), `lands`, `retargeted`
  (the numbers); or `{ "error": code }` when `plan()` fails.
- `bd.start` (optional): the same command without `--check`: `{ "started_via": "api" | "pull_request", "canceled" }`
  (`canceled`: it canceled the running Safe Push of an older commit first) or `{ "error": code }`.

The outcomes compare by class: an app `refuse` and a `bd` refusal or error refuse; an app `start`, `confirm` or
`choose` and a `bd` start start (the agent asks the user what the popover asks, after `--check`); `wait` is neither.
The class of `bd` is that of `start` when given, else of `check`. Sides of different classes carry a `discrepancy` or
an `asymmetry`; an `asymmetry` stands only there. A `discrepancy` may stand where the classes agree: both start, one of
them for a reason the registry names wrong (D9 until s1-holes: the app read a failed read of the rules as "keeps the
approvals", bd never read them). The Vitest checks it for every case.

## The registry: `discrepancies.json`

One entry of each id of the registry of the design doc: `D1`, `D2`, `D3a`, `D3b`, `D4`–`D15`, each once. An entry is
`exercised` (the default `kind`: `given`, `app` and `bd` as any case) or `recorded`: a discrepancy that no decision of
either side reaches (the order of a retarget and a push, a retry, a request, the IPC), with `today: { app, bd, why }`,
where each side does it today, each a list of `path#text` or `path:line#text` (a test, a function, a line of text) from
the repository root: the text must be in the file, with a line on that line, so a pointer never goes stale silently (a
bare `path:line` is refused). A recorded entry has no `given`, `app` or `bd`. `twins` names the exercised cases of other
groups with the same `discrepancy` (D3b: `row-5-prime-another-base`, D11: the three row 5 cases, D15:
`dry-run-while-a-safe-push-of-a-child-lands-the-branch`): a case outside the registry carries a discrepancy only as a
twin of its entry. An exercised case whose classes agree carries `today.why`, what differs (D9 until s1-holes: both
started, the app by a failed read of the rules, `bd` because it never read them; now `bd` refuses `rules_unknown`).

A discrepancy that a stage closed keeps its entry, the key of its id, with `closed: "<stage>"` (a `### <stage>` of the
design doc, whose register row says «закрыт»): its former twins answer alike now and carry `discrepancy: null`, so the
entry has no `twins`; `today` of a recorded entry names where each side does it now, and an exercised one whose sides
agree needs no `today.why` (D2, `closed: "s1-holes"`: both sides skip the approvals for a profile that skips the review,
its former twin `skips-review-profile`; D1, exercised: both refuse a push that dismisses the approvals Patronus counts,
`bd` with `push_dismisses_approvals`). A closed entry whose classes still differ by design carries an `asymmetry` too
(D9: the app confirms `rulesUnknown` where `bd` refuses `rules_unknown`, which `--allow-unknown-rules` lifts, as the
review case `rules-unknown-allowed` shows). Only an entry of the registry is closed.

Both tests check the registry: each id once, the twins, no recorded or closed case outside it, no twins of a closed
entry; the Rust one also that every pointer of `today` resolves and that a closed entry names a stage whose row says
«закрыт», the Vitest that a discrepancy whose classes agree says why. Both check every `spec` anchor
of every case: the file exists, the slug after `#` is a heading of it (GitHub's slug: lower case, letters, digits and
hyphens), a `#the-table:<row>` names a row of the table. D8 is recorded: the vocabulary has no stored stack parent, so its case is a test of each side
of its own, `the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8` (Rust: the placement, the
sync of the fixture, `whole_stack_of`, `bd safe-push A --whole-stack --check`) and `the whole stack of a blocked branch
with an inactive declared parent is d8` (Vitest: the same table in `branchSyncStore`, placed as the view places it,
`mainLineOf`, `getStackNextStepNow`).

## `given`: the closed vocabulary

Both schemas refuse a key outside it. A key is optional; the default is in brackets. The branch is `auth`
(`test-user/virtual/auth`), the target `master`, the GitHub repository `acme/monorepo`, the pull request #7, the pull
request of a whole stack #12.

| Key | Values | `bd` (the fixtures) | app (the facts) |
| --- | --- | --- | --- |
| `mode` | `safePush` · `dryRun` [`safePush`] | `--patronus-dry-run` | the mode of the ask of `readSafePush`; the step's `dryRunBlocker` |
| `branch` | a branch of `commits` [`auth`] | the branch argument | `branchName`, `subject`; of the whole stack its root |
| `via` | `click` · `key` [`click`] | — (a command is a confirmed start) | the `via` of the ask |
| `subject` | `branch` · `wholeStack` [`branch`] | `--whole-stack` | the facts of the stack branch while the stack forks, else of its last branch |
| `commits` | `(prefix) subject` strings, `(parent/child)` stacked [`["(auth) Add auth"]`]; `[]` a branch in the table without commits | `f<i>.txt` of each, as `commit_subjects`; stacks on with a parent; `[]` no such branch | the branches of the table, each with its tip and count, stacked by the prefix |
| `target` | `protected` · `unprotected` [`protected`] | `robot-start-supported` answers `supported: false` | `options.supported` |
| `remote` | `none` · `upToDate` · `behind` · `unpushed` · `rebuiltOnNewerBaseline` [`none`] | `bd push`; `unpushed` one commit after it; `behind` a commit of someone else on the remote branch, fetched; `rebuiltOnNewerBaseline` the baseline moved and the branch rebased onto it | the `remoteStatus` of the branch and of the branches it is built on (`bd push` pushes the stack): never pushed, up to date, one commit ahead, one behind with a dropped commit, the same commits rebuilt (not a fast-forward) |
| `robot` | `{ state: running·failure·canceled·success, on: tip·older·remote·ancestor:<b>·child:<b>·stack, reason: approvals, fromPullRequest }`, or a list | the robot list after a sync, on the tips it built (`older`: `0000`); the i-th robot is `robot-<i>`, the one on an older commit `older`; the first started last (the newest), each next a minute earlier, the finishes in the order of the list | the robots and the `PatronusView` of the facts (what `patronus_view` computes of them: `landedBy`, `span`, `landingAncestor`, the stacks' `landingMember`, and what the rules decide over them, `landing`: `hold`, `start`, `dryRun`, `replaces`, `push`); `reason: approvals` in `needsReview` by the robot's id |
| `sync` | `running` | — (`bd` syncs before it plans) | `syncing` |
| `conflicts` | a branch | its first commit changes `config.txt`, which only an unassigned commit creates | `hasError` of it, `blocked` of a branch built on it, the stack branch `Conflict` |
| `notOnSpace` | `true` | `origin` set to the bare repository | `patronusAvailable: false` |
| `pullRequest` | `{ of [the branch], base [master], draft [false], approvers [[]], reviewers [[]], changesRequested [[]], state [open] }` | the GitHub search answer (`latestReviews`, `reviewRequests`) | `pullRequest`, `fromPullRequest` of `of` only |
| `stackPullRequest` | `true` | the open #12 of `{prefix}/stack/<root>` in the search | `stackPullRequest`; of the whole stack its `pullRequest` |
| `readiness` | `notRequired` · `approved` · `missing` · `unknown` · `unavailable` · `unlisted` · `space` · `notAsked` [`notRequired`] | the pre-check answer, see below | the answer of the facts (Rust asked as the app's scan asks, `readinessInputOf`) in the review-readiness store under the key of the branch's input, the availability of the pull requests |
| `profile` | `{ chosen: <id>·none, lastRobot: lsc, skipsReview, newApi }` | `--profile`; the last robot's options | `ask.profileId`; `options` |
| `flags` | `withoutApprovals` · `cancelRunning` · `createPullRequest` · `allowMissing` · `allowDangling` · `allowUnknownRules` | the flags of the same names | `withoutApprovals` = `ask.bypassReview`; the others have no input of the decision |
| `rules` | `keep` · `dismiss` · `unknown` · `loading` [`keep`] | the fake GitHub's `GET rules/branches/master`: `keep` none (`fake_github`), `dismiss` the pull request rule `dismiss_stale_reviews_on_push`, `unknown` 500; `loading` is `keep` (bd waits for its own read) | the rules of the target as the session read them (`facts.rules`, `seedRules` of `scenario-facts.ts`): `unknown` a failed answer (the start confirms `rulesUnknown`), `loading` no answer yet |
| `parked` | `self` · `builtOn` · a branch | `bd park` of the branch, of its parent or of the branch named | `parked` (`domain_policy::parked`); of the whole stack the `stackParked` of its root |
| `stackBranch` | `notBuilt` | the last branch gets a stack problem (`(<b>/<b>) Loop`) | the stack branch `Blocked` by it |
| `landed` | `inBaseline` · `byContent` | `origin/master` at HEAD; `land_auth_by_content` after `bd push` (`LANDED_SUBJECTS` only) | the landing report of the baseline check (`inBaseline`); one commit left (`byContent`) |
| `landingNeeds` | `true` | `(codec) Add controlUrl`, then the branch uses it (Kotlin) | `landingNeeds` of the branch in the view of the facts, known |
| `dangling` | `removed` (branch `fake-agent`) · `changed` (branch `suspend`), with `commits: []` | `commit_dangling_reference` · `commit_changed_declaration` | the fixture's commits; the `danglingReference` the landing gate finds, in the view of the facts |

A value the adapters don't map yet fails the case with "not mapped yet": the stage maps it with the first case that
needs it. Not mapped yet: a decision of the whole stack.

A merged robot lands by the hashes of the tips the sync built: of the branch itself "Merged by Patronus", of a branch
built on it (`child:<b>`) or of the stack branch (`stack`) "Landed in master with …"; a running one on `tip` holds the
branch, on `child:<b>` lands it, on `ancestor:<b>` lands a part of it (a Dry Run doesn't wait for it).

### How each side reads `readiness`, `profile` and `robot`

| `readiness` | `bd`: what the fakes answer | app: `readiness` · `review` |
| --- | --- | --- |
| `notRequired` | the pre-check needs no approval | `notRequired` · loaded |
| `approved` | with a pull request: Patronus evaluated it, IJPL Code Platform approved | `approved` · loaded |
| `missing` | the pre-check needs approvals (the group Air Plugin Release Team, Alice suggested); with a pull request evaluated, nobody of IJPL Code Platform approved (Ilya Korennoy suggested) | `missing` · loaded |
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
