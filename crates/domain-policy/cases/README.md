# Scenario cases

The table of what the app and `bd` answer for the same situation: the next step of the subject and the decision of
its start, one expectation per case that both sides answer ([Rust as the source of the domain
rules](../../../docs/design/rust-domain-authority.md#таблица-сценариев)). Data only: the rules of this crate (`src/`)
don't read it, the test does. Every expectation is today's answer, never the wished one; where `bd` answers otherwise
by design, the case says so (`asymmetry`) beside the answer of `bd`.

One test reads every `*.json` file here, `crates/bd-remote/src/scenario_test.rs`, one test function per `group`, and
names the case id in each failure. It builds one repository of `given` per case with the fixtures of `bd-cli`, the
fake Patronus and the fake GitHub, and asks both sides of it:

- **The app**: the table its sync folds (`TableState`) and the facts its session holds (`FactStore`), each read of the
  same fakes as the session's host reads them, in the order its scheduler asks them after a sync: the robots, the
  problems of the failed ones, the Patronus view with the landing needs, the details of the robot of the subject (its
  profile), the pull requests, the rules of the bases of the approved ones, the merge profiles of the target, and the
  review readiness `ReadinessFact::update` asks for the shown rows whose step waits for it and for the subject the
  click needs. Then `repository_model::derive`: `next_steps_of` for the step, and the plan of the start the case asks
  as the app's `prepare` answers it (`operations::land::plan_of` over the sources of the session).
- **`bd`**: `safe_push()` itself with `--check` (the function the command runs: its preamble, `plan()` with the plan
  of `operations::land::prepare_one_shot`, the decision of `domain_policy::land::decide`, the output of a check), whose
  output carries the plan and the step its words tell (`SafePushOutput.plan`, `.step`); then `bd safe-push` without
  `--check`, which must start (through the API or from a pull request, canceling the robot the plan cancels) when
  nothing refuses it and the flags of the case accept every confirmation (`domain_policy::land::unaccepted`), and
  refuse otherwise.

The flags of a case are the intent of both sides: the popover's Start Anyway and its line of the robot a start
replaces accept what `--allow-missing`, `--allow-dangling`, `--allow-unknown-rules` and `--cancel-running` accept.

The fixed test names, by group:

| `group` | Test |
| --- | --- |
| `modes-and-routes` | `the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd` |
| `profiles` | `the_safe_push_profile_cases_answer_alike_in_the_app_and_bd` |
| `review` | `the_safe_push_review_cases_answer_alike_in_the_app_and_bd` |
| `next-step-rows` | `the_next_step_row_cases_answer_alike_in_the_app_and_bd` |
| `stack-and-whole-stack` | `the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd` |
| `landing` | `the_landing_cases_answer_alike_in_the_app_and_bd` |
| `discrepancies` | `the_discrepancy_cases_name_every_id_as_the_register_does` |

`the_cases_are_well_formed_and_name_every_row_of_the_next_step_table` checks that the ids are unique and of kebab
case, that every group has its test (a new group gets its test in the same commit), and that every row of the table of
[Next Step](../../../spec/branch/next-step.spec.md#the-table) is named by a case.

`safe-push.json` holds the first three groups, `next-step.json` the next two: one case per row of the table of
[Next Step](../../../spec/branch/next-step.spec.md#the-table) and per bullet of its stacked branch and its whole stack.
`landing.json` holds the rest of the Safe Push matrix: robots around the branch (on the remote branch rebuilt since,
of a branch built on it, of an older commit with and without a push, `--cancel-running`), the parked member of a whole
stack, a remote branch behind, the retarget of an ancestor's pull request, missing declarations and dangling references
with their `--allow-*`. `landing-routes.json` holds the cases of a repository that lands by a push into the target
(`given.route`, group `modes-and-routes`): there the side of `bd` is `bd land` (`land()` with `--check`, then the
command without it, which must land, `landed: true`, when nothing refuses it and the flags accept every confirmation).
`discrepancies.json` is the registry (below).


## A case

```json
{
  "id": "last-robot-profile",
  "group": "profiles",
  "title": "the profile of the last robot, which failed on an older commit, is the default; the branch unpushed since, its slot starts anew",
  "spec": ["spec/branch/patronus.spec.md#starting-a-safe-push-or-a-dry-run", "spec/cli/agent-cli.spec.md#bd-safe-push"],
  "given": { "robot": { "state": "failure", "on": "older" }, "profile": { "lastRobot": "lsc" } },
  "expect": {
    "step": { "row": "11", "kind": "safePush" },
    "land": { "confirmations": ["nonDefaultProfile"], "start": "api:lsc", "pushes": ["auth"] }
  }
}
```

- `id`: unique over all files, kebab-case; `title`: the situation in words; `spec`: the requirements it pins, each
  anchor a heading of its file (GitHub's slug). A row of the Next Step table is
  `spec/branch/next-step.spec.md#the-table:<row>` (`1`, `1′`, `2`, `3`, `5`, `5′`, `6`, `7`, `8`, `9`, `10`, `8′`, `10′`,
  `11`).
- `expect.step`: a subset of the next step of the subject (`domain_types::next_step::NextStep`): `row` and `kind` (the
  tag of `StepKind`) always, `safePush` and `dryRun` the blocker of each start (the code of `StepBlocker`, of a `rule`
  the code of its `Reason`), `null` where left out; `null` without a step (no row of the subject).
- `expect.land`: a subset of the decision of the start (`domain_types::landing::LandDecision`): `blockers` (the codes
  of the `Reason`s, in order), `confirmations` (the kinds), `waits` (the kinds of the facts it waits for, `<kind>:failed`
  where the read failed), `start` (`api`, `api:<profile>`, `comment:<number>`, `createPullRequest`), `pushes` (the
  branches of its push, `null` without one), `force`, `retargets` (the numbers), `cancels` (the robot), `lands`, `into`
  (the target a push of the copy lands into, `PlannedEffect::PushBaseline`; `null` without one).
  `blockers` and `confirmations` are `[]` where left out, and so is `waits` of a decision no blocker refuses: one that
  appears fails every case it appears in. `null` without a decision (no subject, no Patronus view).
- `asymmetry` (optional): `{ "kind": "byDesign", "why": "…" }`, where `bd` answers otherwise by design (the spec or the
  doc it names), always with `bd`: its own `step` or `land` instead of `expect`'s, or `error`, the code of the error of
  the command that refuses before it decides anything (what the table alone refuses, a target Patronus doesn't protect,
  a repository not on Space, an argument). `bd` stands only beside an `asymmetry`, and an `asymmetry` only with it.

## The registry: `discrepancies.json`

One entry of each id of the registry of the design doc: `D1`, `D2`, `D3a`, `D3b`, `D4`–`D15`, each once, its id as
`discrepancy` (no case outside the registry has one: a case answers one expectation). An entry is `exercised` (the
default `kind`: `given` and `expect` as any case, both sides answering alike, so it is `closed`) or `recorded`: a
discrepancy that no decision of either side reaches (the order of a retarget and a push, a retry, a request, the IPC),
with `today: { app, bd, why }`, where each side does it today, each a list of `path#text` or `path:line#text` (a test, a
function, a line of text) from the repository root: the text must be in the file, with a line on that line, so a
pointer never goes stale silently (a bare `path:line` is refused). A recorded entry has no `given`, `expect` or `bd`.

A discrepancy that a stage closed keeps its entry, the key of its id, with `closed: "<stage>"` (a `### <stage>` of the
design doc, whose register row says «закрыт»); `today` of a recorded entry names where each side does it now. Only an
entry of the registry is closed.

The test of the registry checks each id once, every pointer of `today`, no recorded or closed case outside it, and that
a closed entry names a stage whose row says «закрыт»; every test checks every `spec` anchor of its cases. D8 is
recorded: the vocabulary has no stored stack parent, so its case is a test of its own,
`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8` (the placement, the sync of the fixture,
`whole_stack`, `bd safe-push A --whole-stack --check`).

## `given`: the closed vocabulary

The schema refuses a key outside it (`Given`, `deny_unknown_fields`). A key is optional; the default is in brackets.
The branch is `auth` (`test-user/virtual/auth`), the target `master`, the GitHub repository `acme/monorepo`, the pull
request #7, the pull request of a whole stack #12. Each value is what the fixtures make of the repository and the
fakes; both sides read that.

| Key | Values | The repository and the fakes |
| --- | --- | --- |
| `mode` | `safePush` · `dryRun` [`safePush`] | the mode of the start (`--patronus-dry-run`) |
| `branch` | a branch of `commits` [`auth`] | the branch the case starts; of the whole stack, the branch whose stack it is |
| `subject` | `branch` · `wholeStack` [`branch`] | the start of the branch, or of its whole stack (`--whole-stack`): the stack branch of its root while the stack forks, else the last branch of the straight chain |
| `commits` | `(prefix) subject` strings, `(parent/child)` stacked [`["(auth) Add auth"]`]; `[]` only with `dangling` | `f<i>.txt` of each, as `commit_subjects`; stacks on with a parent |
| `target` | `protected` · `unprotected` [`protected`] | `robot-start-supported` answers `supported: false` |
| `remote` | `none` · `upToDate` · `behind` · `unpushed` · `rebuiltOnNewerBaseline` [`none`] | `bd push`; `unpushed` one commit after it; `behind` a commit of someone else on the remote branch, fetched; `rebuiltOnNewerBaseline` the baseline moved and the branch rebased onto it (the same commits, not a fast-forward) |
| `robot` | `{ state: running·failure·canceled·success, on: tip·older·remote·ancestor:<b>·child:<b>·stack, reason: approvals, fromPullRequest }`, or a list | the robot list, on the tips the sync built (`older`: `0000`); the i-th robot is `robot-<i>`, the one on an older commit `older`; the first started last (the newest), each next a minute earlier, the finishes in the order of the list; the problems of a robot say it failed for the approvals only with `reason: approvals` |
| `sync` | `running` | after the reads, a commit of the branch and the app's next sync past its grouping: its rows sync (`bd` syncs itself first) |
| `conflicts` | a branch | its first commit changes `config.txt`, which only an unassigned commit creates |
| `notOnSpace` | `true` | `origin` set to the bare repository |
| `pullRequest` | `{ of [the branch], base [master], draft [false], approvers [[]], reviewers [[]], changesRequested [[]], state [open] }` | the GitHub search answer (`latestReviews`, `reviewRequests`); a `merged` one at the tip the sync builds (merged with the commits of the branch); GitHub retargets it when asked |
| `stackPullRequest` | `true` | the open #12 of `{prefix}/stack/<root>` in the search |
| `readiness` | `notRequired` · `approved` · `missing` · `unknown` · `unavailable` · `unlisted` · `space` · `notAsked` [`notRequired`] | the pre-check answer, see below |
| `profile` | `{ chosen: <id>·none, lastRobot: lsc, skipsReview, newApi }` | the profile of the start (`--profile`); the last robot's options |
| `flags` | `withoutApprovals` · `cancelRunning` · `createPullRequest` · `allowMissing` · `allowDangling` · `allowUnknownRules` · `withoutChecks` · `bypassRules` | the flags of `bd` of the same names, the choices of the popover (`Land`); `withoutChecks` and `bypassRules` the ones of `bd land` |
| `route` | `push` | `branchdeck.landing push`: the repository lands by a push of the copy into the target, `bd land` its side |
| `permission` | `read` · `write` | `remote.origin.pushurl` github.com/acme/monorepo, and the listing of the pull requests names `viewerPermission: READ` or `WRITE` |
| `rules` | `keep` · `dismiss` · `unknown` · `statusChecks` [`keep`] | the fake GitHub's `GET rules/branches/master`: `keep` none, `dismiss` the pull request rule `dismiss_stale_reviews_on_push`, `unknown` 500, `statusChecks` the rule `required_status_checks` |
| `parked` | `self` · `builtOn` · a branch | `bd park` of the branch, of its parent or of the branch named |
| `stackBranch` | `notBuilt` | the last branch gets a stack problem (`(<b>/<b>) Loop`) |
| `landed` | `inBaseline` · `byContent` | `inBaseline`: origin/master gets every commit of the branch rebuilt (`cherry-pick -x`), fetched, HEAD keeps them until the update; `byContent`: `land_auth_by_content` after `bd push` (`LANDED_SUBJECTS` only), one commit left |
| `landingNeeds` | `true` | `(codec) Add controlUrl`, then the branch uses it (Kotlin) |
| `dangling` | `removed` (branch `fake-agent`) · `changed` (branch `suspend`), with `commits: []` | `commit_dangling_reference` · `commit_changed_declaration` |

A value the fixtures don't map fails the case with what it needs.

A merged robot lands by the hashes of the tips the sync built: of the branch itself "Merged by Patronus", of a branch
built on it (`child:<b>`) or of the stack branch (`stack`) "Landed in master with …"; a running one on `tip` holds the
branch, on `child:<b>` lands it, on `ancestor:<b>` lands a part of it (a Dry Run doesn't wait for it).

### `readiness`, `profile` and `robot`

| `readiness` | What the fakes answer |
| --- | --- |
| `notRequired` | the pre-check needs no approval |
| `approved` | with a pull request: Patronus evaluated it, IJPL Code Platform approved |
| `missing` | the pre-check needs approvals (the group Air Plugin Release Team, Alice suggested); with a pull request evaluated, nobody of IJPL Code Platform approved (Ilya Korennoy suggested) |
| `unknown` | a pull request with an approval, the review required, Patronus doesn't evaluate it |
| `unavailable` | the pre-check answers 500 |
| `unlisted` | GitHub's search of the pull requests fails; the pre-check needs no approval |
| `space` | no GitHub repository; the pre-check needs approvals (a Space merge request) |
| `notAsked` | the pre-check needs no approval; the app's session didn't ask it yet, `bd` always asks |

GitHub is configured (`branchdeck.githubRepository`, a token) for every value but `space`.

- `profile.chosen`: the profile of the start; `none` is the default checks.
- `profile.lastRobot`: the profile of the last robot of the branch (`lsc`, the only one the fake names), the default of
  the start; it needs `robot`. Patronus says that a profile skips the review only of this one: by the robot's options,
  or with `newApi` (Settings → Patronus) by the robot list, which says `skipsReview: true`. `skipsReview` without
  `lastRobot` is refused.
- `robot.on: older`: the robot ran on another commit (`sourceRevision` `0000`); `tip` on the branch's tip; `remote`
  on the remote branch, which `remote: rebuiltOnNewerBaseline` rebuilt since (a start would test the same commits).
