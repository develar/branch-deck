---
name: Landing Routes
description: Requirements for the route of a repository into master, a robot of Patronus, a push of the copy into the target or a merge of the pull request on GitHub, named by `branchdeck.landing` or by the host of `origin`, and the gate of each route by the write access and the rules of the target.
style: plain-1
targets:
  - crates/domain-types/src/landing_route.rs
  - crates/domain-types/src/reason.rs
  - crates/domain-types/src/pull_request.rs
  - crates/domain-types/src/session.rs
  - crates/domain-types/src/lib.rs
  - crates/domain-policy/src/landing_route.rs
  - crates/sync-core/src/repository_config.rs
  - crates/sync-core/src/config_snapshot.rs
  - crates/sync-core/src/repo_facts.rs
  - crates/sync-core/src/sync.rs
  - crates/remote-facts/src/branch_rules.rs
  - crates/remote-facts/src/pull_requests.rs
  - crates/repository-model/src/pull_request_facts.rs
  - crates/repository-model/src/runtime.rs
  - crates/repository-model/src/table.rs
  - crates/repository-model/src/state.rs
  - crates/repository-model/src/patch.rs
  - crates/repository-model/src/refusal_words.rs
  - src/presenters/reasons.ts
  - src/model/patch.ts
  - src/lib/repository-config.ts
  - crates/domain-policy/src/land.rs
  - crates/domain-policy/src/land_direct.rs
  - crates/domain-policy/src/landed.rs
  - crates/domain-policy/src/next_step.rs
  - crates/domain-policy/src/landing_node.rs
  - crates/domain-policy/src/actions.rs
  - crates/domain-types/src/landing.rs
  - crates/sync-core/src/baseline_push.rs
  - crates/sync-core/src/remote_command.rs
  - crates/operations/src/land.rs
  - crates/operations/src/land_execute.rs
  - crates/operations/src/land_session.rs
  - crates/operations/src/run_frame.rs
  - crates/operations/src/update.rs
  - crates/operations/src/registry.rs
  - crates/operations/src/engine.rs
  - crates/operations/src/witness.rs
  - crates/operations/src/close.rs
  - crates/operations/src/journal.rs
  - crates/repository-model/src/derive.rs
  - crates/repository-model/src/facts.rs
  - crates/repository-model/src/landed_closes.rs
  - crates/repository-model/src/session.rs
  - crates/bd-remote/src/land_command.rs
  - crates/bd-remote/src/status.rs
  - crates/sync-core/src/grouping.rs
  - src/presenters/next-step.ts
  - src/presenters/land.ts
  - src/presenters/actions.ts
  - src/presenters/outcome.ts
  - src/presenters/operations.ts
  - src/presenters/activity.ts
  - src/hooks/useSafePush.ts
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useWholeStackActions.ts
  - src/hooks/useStackNextStep.ts
  - src/components/branch/SafePushPopover.tsx
  - src/components/branch/SafePushButton.tsx
  - src/components/branch/NextStepSlot.tsx
  - src/components/branch/WholeStackSlot.tsx
  - src/components/branch/BranchPreviewActions.tsx
  - src/lib/push-to-baseline.ts
  - src/lib/landed-closes.ts
  - src/lib/review-window.ts
  - src/lib/branch-review.ts
  - src/types/sub-window.ts
  - src/stores/update-project.ts
  - crates/remote-facts/src/pull_request_writes.rs
  - crates/github/src/client.rs
  - crates/operations/src/auto_merge.rs
  - crates/domain-policy/src/news.rs
  - crates/domain-policy/src/refresh.rs
  - crates/remote-facts/src/scheduler_github.rs
  - src/lib/merge-pull-request.ts
  - src/lib/auto-merge-news.ts
  - crates/github/src/fake_repository.rs
  - crates/test-utils/src/remote_git.rs
---

# Landing Routes

Status: Draft (implemented: the route, its facts and its gates, Push to master with its step, its actions, its page and `brd land`, the closes of what landed by content, the route in Settings → Landing, the merge of a pull request and its auto-merge on GitHub with their steps, `brd land` and the news of an auto-merge)
Date: 2026-10-28

## Purpose

The work of a branch is done when it is in master ([Landing Work into Master](../../../../docs/design/landing.md)). In the
IntelliJ monorepo it gets there by a Safe Push of Patronus. Elsewhere Branch Deck had no way into master: a branch
outside Space had no next step, and an approved green pull request only waited.

A repository now has a route into master, one of three: a robot of Patronus, a push of the copy of the branch into the
branch of the baseline on its remote (a fast-forward of `master`), or a merge of its pull request on GitHub. The unit
of landing is a robot, a push or a merge. The route is a fact of the repository, named by its git config or by the
host of `origin`, and each route has a gate: what GitHub tells of the user's write access and of the rules of the
target, read before anything is pushed.

## Goals

- Name the route of a repository once, in its git config, read by the app and `brd` alike.
- Keep the monorepo on Safe Push without a setting, and give every other repository a route.
- Tell before a push whether the user may push into the target or merge there, from facts GitHub already gives.
- Never read a fact without an answer as "no".

## Non-goals

- Detect a push route. A push into master skips the robot and the CI of a pull request: only the user names it.
- Read classic branch protection. It needs admin rights; the server refuses a push it forbids.
- Judge a remote other than GitHub. Off GitHub (a GitLab remote, a mirror) there is no gate: the server answers.

## Requirements

### Requirement: Setting: The route is one of three values of `branchdeck.landing`

The route MUST be `branchdeck.landing` of the git config, `patronus`, `push` or `pull-request` (any case, trimmed); an
empty value is unset, another value is invalid with its reason, and an operation MUST refuse it as it refuses a wrong
`branchdeck.resolveByWords`.

#### Scenario: The three values, in any case and trimmed
- **WHEN** `branchdeck.landing` is `patronus`, `push` or `pull-request`, in any case, with spaces around it
- **THEN** it names that route
- **AND** an empty value is unset
  [@test] crates/domain-types/src/landing_route_test.rs (`the_setting_is_read_as_git_keeps_it_and_written_back_the_same`)

#### Scenario: Another value is invalid with its reason
- **WHEN** `branchdeck.landing` holds another value
- **THEN** it is invalid with its reason
  [@test] crates/sync-core/src/repository_config_test.rs (`the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset`)

#### Scenario: An operation refuses an invalid route
- **WHEN** an operation reads an invalid `branchdeck.landing`
- **THEN** it refuses it as it refuses a wrong `branchdeck.resolveByWords`
  [@test] crates/sync-core/src/config_snapshot_test.rs (`the_route_comes_from_git_config_and_is_none_without_it`)

### Requirement: Setting: The route is read and changed with the other keys of Settings

The route MUST be read with the other keys of Settings (Branches, Landing, Agents) in one `git config` and in the one
read of an operation (`ConfigSnapshot`). A change MUST go through `set_repository_config` like the other keys.

#### Scenario: The one read of an operation
- **WHEN** an operation reads its config
- **THEN** the route comes from `git config` in its one read (`ConfigSnapshot`), none without the key
  [@test] crates/sync-core/src/config_snapshot_test.rs (`the_route_comes_from_git_config_and_is_none_without_it`)

#### Scenario: The read and the change of Settings
- **WHEN** Settings reads or changes the route
- **THEN** it is read with the other keys of Settings (Branches, Landing, Agents) in one `git config`
- **AND** a change goes through `set_repository_config` like the other keys

### Requirement: Setting: A `push` is taken only from the repository's own config

`push` MUST be taken only from the repository's own config (`.git/config`, the worktree config, `git -c`): a `push` of
the global or the system config is unset, with the reason, for Settings and for every operation. A push into master of
every repository of the user that names no route would be a trap.

#### Scenario: A push of the global config in Settings
- **GIVEN** `branchdeck.landing` is `push` in the global or the system config only
- **WHEN** Settings reads the route
- **THEN** it is unset, with the reason
- **AND** a `push` of the repository's own config or of the worktree config is taken
  [@test] crates/sync-core/src/repository_config_test.rs (`the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset`)

#### Scenario: A push of the global config in an operation
- **GIVEN** `branchdeck.landing` is `push` in the global or the system config only
- **WHEN** an operation reads the route
- **THEN** it is unset
  [@test] crates/sync-core/src/config_snapshot_test.rs (`a_push_route_of_the_global_config_is_none`)

### Requirement: Setting: A route is written only to the repository's own config

A write of the route MUST go to the repository's own config: a global write is refused for every value, since it
would apply to every repository without a route of its own (a `push` pushes into master, a `patronus` refuses every
repository off Space, a `pull-request` takes Safe Push from the ones on Space). A hand-written global value SHALL still
be removable.

#### Scenario: A global write is refused, a global value removed
- **WHEN** a write of `push`, `patronus` or `pull-request` names the global config
- **THEN** it is refused and the global config stays as it was
- **AND** a write to the repository's own config is taken
- **AND** a hand-written global value can still be removed
  [@test] crates/sync-core/src/repository_config_test.rs (`a_route_is_written_only_to_the_local_config`)

### Requirement: Setting: The pull request rule of the agents follows the route

The rule of the agents that pushes the branch after the pick and opens its pull request (`branchdeck.agentPullRequest`)
MUST follow the route: under Push it is off with its reason, in `brd workspace`, `brd lease get` and Settings → Agents
([brd workspace](../../cli/workspace/spec.md#requirements)).

#### Scenario: Only the push route turns the rule off
- **WHEN** the route is Push
- **THEN** `branchdeck.agentPullRequest` is off with its reason
- **AND** under Patronus or the pull request it is not
  [@test] crates/domain-types/src/landing_route_test.rs (`only_the_push_route_turns_the_agent_pull_request_off`)

#### Scenario: `brd workspace` and `brd lease get` under Push
- **WHEN** `brd workspace`, `brd lease get` or Settings → Agents tells the rule under the push route
- **THEN** it is off with its reason, and a `push` of the global config turns nothing off
  [@test] crates/bd-lease/src/workspace_test.rs (`the_push_route_turns_the_pull_request_rule_off_and_a_global_push_route_does_not`)

### Requirement: Setting: Unset, the route follows the host of `origin`

Unset, the route MUST be Patronus when the push URL of `origin` is on Space (`git.jetbrains.team`), and the pull
request elsewhere; never a push. This is an exception to "a rule changes only by an explicit value"
([Agent Workspace](../../cli/workspace/spec.md)): the host of `origin` does not change by itself, and the monorepo MUST
NOT lose Safe Push to a missing key.

#### Scenario: Unset on Space and elsewhere
- **GIVEN** `branchdeck.landing` is unset
- **WHEN** the push URL of `origin` is on Space (`git.jetbrains.team`)
- **THEN** the route is Patronus
- **AND** elsewhere it is the pull request, never a push
  [@test] crates/domain-policy/src/landing_route_test.rs (`the_setting_names_the_route_and_unset_it_follows_the_host`)

### Requirement: Setting: Patronus named off Space is refused

Patronus named off Space MUST be refused (`notOnSpace`).

#### Scenario: Patronus off Space
- **GIVEN** `branchdeck.landing` is `patronus`
- **WHEN** the push URL of `origin` is not on Space
- **THEN** the route is refused (`notOnSpace`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`the_setting_names_the_route_and_unset_it_follows_the_host`)

### Requirement: Setting: One function names the default route

One function MUST name the default (`LandingSetting::default_for`), for the rule and for Settings, whose read of the
repository's settings tells it with its reason (`RepositoryConfig.landingDefault`: the route and whether `origin`
pushes to Space) from the push URL of `origin` it reads for the detected GitHub repository, no process of its own.

#### Scenario: Settings tells the default and its reason
- **WHEN** Settings reads the repository's settings with the route unset
- **THEN** `RepositoryConfig.landingDefault` names the route of `LandingSetting::default_for` and whether `origin` pushes to Space
- **AND** it comes from the push URL of `origin` read for the detected GitHub repository, no process of its own
  [@test] crates/sync-core/src/repository_config_test.rs (`the_default_route_follows_the_host_of_origin_in_the_one_read_of_its_push_url`)

### Requirement: What a sync reads for it: The facts of git of the route need no process of their own

A sync MUST read the route's facts of git without a process of its own: the setting with its config, and the push URL
of each remote from its listing of the remotes (`git remote -v`, one process as `git remote` was, with
`pushInsteadOf` applied).

#### Scenario: The push URLs come with the listing of the remotes
- **WHEN** a sync lists the remotes with `git remote -v`
- **THEN** each remote comes with its push URL, `pushInsteadOf` applied
- **AND** the listing is one process, as `git remote` was
  [@test] crates/sync-core/src/repo_facts_test.rs (`the_remotes_come_with_their_push_urls`, `the_landing_remote_is_read_with_the_facts_of_a_sync`)

### Requirement: What a sync reads for it: The sync sends the facts of the route in `BranchesGrouped`

A sync MUST send the route's facts in `BranchesGrouped` (`landing`): the setting, whether `origin` pushes to Space, the
github.com repository the remote of the baseline pushes to, the branch of the baseline there (`master` of
`origin/master`) and its remote (`origin`; none of both for a local baseline). The session MUST keep them in the header
of its view.

#### Scenario: The facts a sync sends
- **WHEN** a sync groups the branches
- **THEN** `BranchesGrouped` carries in `landing` the setting, whether `origin` pushes to Space, the github.com repository the remote of the baseline pushes to, the branch of the baseline there and its remote
- **AND** a local baseline has neither the branch nor the remote
  [@test] crates/sync-core/src/repo_facts_test.rs (`the_landing_remote_is_read_with_the_facts_of_a_sync`)

#### Scenario: The session keeps them in the header of its view
- **WHEN** the session takes `BranchesGrouped`
- **THEN** the header of its view holds the facts of the route
  [@test] crates/repository-model/src/session_test.rs (`a_push_route_reads_the_rules_of_the_target`)

### Requirement: What a sync reads for it: The grouping of `brd status` reads the same facts

The grouping of `brd status` MUST read the route's facts from the same listing of the remotes
(`BranchGrouping.landing`).

#### Scenario: `brd status` groups the branches
- **WHEN** `brd status` groups the branches
- **THEN** `BranchGrouping.landing` holds the facts of the route, read from the same listing of the remotes

### Requirement: Facts of GitHub: The permission of the token comes with the listing of the pull requests

What the token may do in the repository (`viewerPermission`: `ADMIN`, `MAINTAIN`, `WRITE`, `TRIAGE`, `READ`) MUST come
from the listing of the pull requests, the same query, no request of its own.

#### Scenario: The listing answers the permission
- **WHEN** the session lists the pull requests of the repository the remote pushes to
- **THEN** the same query answers `viewerPermission`, no request of its own
- **AND** a token asked again without teams still answers it
  [@test] crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`, `a_token_without_read_org_is_asked_again_without_teams`)

#### Scenario: The values of the permission
- **WHEN** GitHub answers `ADMIN`, `MAINTAIN`, `WRITE`, `TRIAGE` or `READ`
- **THEN** each is read as that permission of the token
  [@test] crates/domain-types/src/landing_route_test.rs (`a_permission_of_github_writes_from_write_and_bypasses_from_maintain`)

### Requirement: Facts of GitHub: The permission goes with the listing

The permission MUST go with the listing: kept through a failure GitHub recovers from, none without a listing to stand
on, never "no access"; with it the repository of the listing.

#### Scenario: A failure keeps it, no listing has none
- **GIVEN** a listing answered the permission and its repository
- **WHEN** a read fails that GitHub recovers from
- **THEN** both are kept
- **AND** without a listing to stand on there is none, never "no access"
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`the_permission_of_the_token_goes_with_the_listing`)

### Requirement: Facts of GitHub: A patch carries the permission and the repository

A patch MUST carry the permission and the repository of the listing with the head of the answer.

#### Scenario: The patch of a listing
- **WHEN** the session applies a listing of the pull requests
- **THEN** its patch carries the repository and the permission of the token with it
  [@test] crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)

### Requirement: Facts of GitHub: The rules of a branch tell what a push into it meets

The rules of a branch (`GET rules/branches/<branch>`, the active rulesets) MUST tell what a push into it meets: a pull
request (`pull_request`), signed commits (`required_signatures`), status checks (`required_status_checks`), the merge
queue (`merge_queue`), restricted updates (`update`), besides the dismissal of stale reviews.

#### Scenario: The rules a push meets, by their types
- **WHEN** the rules of a branch hold `pull_request`, `required_signatures`, `required_status_checks`, `merge_queue` or `update`
- **THEN** each is found as what a push into the branch meets
  [@test] crates/remote-facts/src/branch_rules_test.rs (`the_rules_a_push_into_the_branch_meets_are_found_by_their_types`)

#### Scenario: The dismissal of stale reviews among them
- **WHEN** a rule of a pull request dismisses stale reviews on a push
- **THEN** it is found among the rules of the branch
  [@test] crates/remote-facts/src/branch_rules_test.rs (`a_pull_request_rule_that_dismisses_stale_reviews_on_push_is_found_among_the_rules_of_the_branch`)

### Requirement: Facts of GitHub: The session reads the rules of the target of a push

The session MUST read the rules of the target while the route is a push into a repository on GitHub, besides those of
the bases of the approved pull requests, once per base per session as those.

#### Scenario: A push route on GitHub
- **WHEN** the route is a push into a repository on GitHub
- **THEN** the session reads the rules of its target besides those of the bases of the approved pull requests, once per base per session
  [@test] crates/repository-model/src/session_test.rs (`a_push_route_reads_the_rules_of_the_target`)

#### Scenario: No rules of the target on another route or off GitHub
- **WHEN** the route is unset, the pull request, or a push into a repository on Space
- **THEN** no rules of the target are read for it
  [@test] crates/domain-policy/src/landing_route_test.rs (`the_rules_of_the_target_are_read_only_for_a_push_on_github`)

### Requirement: Gates: One rule decides the route and its gate

One rule MUST decide the route and its gate for the app and `brd` (`domain_policy::landing_route::route`): open, a wait
for a fact (with the reason of a read that failed), confirmations the user accepts, or a refusal with its reason
(`Reason`, worded by the page in `src/presenters/reasons.ts` and by `brd` in `refusal_words.rs`).

#### Scenario: The facts of the session as the rule takes them
- **WHEN** the rule reads the facts of the session
- **THEN** a fact without an answer is a wait, a read that failed a wait with its reason, and a known fact is decided on
  [@test] crates/domain-policy/src/landing_route_test.rs (`the_facts_of_the_session_read_as_the_rule_takes_them`)

#### Scenario: The page words every reason
- **WHEN** the gate refuses or asks with a `Reason`
- **THEN** the page words it in `src/presenters/reasons.ts`, and `brd` in `refusal_words.rs`
  [@test] src/presenters/reasons.test.ts (`words every reason of the catalog Rust writes as the page says it`)

### Requirement: Gates: The view of the session carries the route

The view of the session MUST carry the route and its gate as a part of its own (`RepositoryView.route`), patched when it
changes.

#### Scenario: The route changes
- **WHEN** the route or its gate changes in the session
- **THEN** `RepositoryView.route` is patched
  [@test] crates/repository-model/src/session_test.rs (`a_push_into_the_target_lands_its_subject_and_a_refusal_of_the_server_stays_for_the_gate`)

### Requirement: Gates: A push is refused below write access

A push MUST be refused below write access (`noWriteAccess`).

#### Scenario: A reader or a triager
- **WHEN** the token may only read or triage the repository the push goes into
- **THEN** the push is refused (`noWriteAccess`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_is_gated_by_the_write_access_and_the_rules_of_the_target`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Gates: A push is refused for rules no unsigned copy passes

A push MUST be refused for rules no unsigned copy passes (`baselineRequiresSignatures`).

#### Scenario: The target requires signed commits
- **WHEN** the rules of the target require signed commits
- **THEN** the push is refused (`baselineRequiresSignatures`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_is_gated_by_the_write_access_and_the_rules_of_the_target`)

### Requirement: Gates: Rules a maintainer may bypass refuse a writer and ask a maintainer

A push MUST be refused for a writer by a rule of a pull request, of the merge queue or of restricted updates
(`baselineRequiresPullRequest`, `baselineRequiresMergeQueue`, `baselineRestrictsUpdates`); a maintainer or an admin,
whom a ruleset may let bypass them (the API doesn't tell), MUST confirm them instead, and so does a user GitHub named
no permission of.

#### Scenario: A writer meets a rule
- **GIVEN** the token may write
- **WHEN** the target requires a pull request, the merge queue or restricts updates
- **THEN** the push is refused (`baselineRequiresPullRequest`, `baselineRequiresMergeQueue`, `baselineRestrictsUpdates`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_is_gated_by_the_write_access_and_the_rules_of_the_target`)

#### Scenario: A maintainer, an admin or no permission named
- **WHEN** a maintainer or an admin meets those rules, or GitHub named no permission of the user
- **THEN** the gate asks the user to confirm them instead, never refuses
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_is_gated_by_the_write_access_and_the_rules_of_the_target`)

### Requirement: Gates: Required status checks are a confirmation

Required status checks, which a push lands without, MUST be a confirmation (`baselineRequiresStatusChecks`).

#### Scenario: The target requires status checks
- **WHEN** the target requires status checks
- **THEN** the gate asks the user to confirm them (`baselineRequiresStatusChecks`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_is_gated_by_the_write_access_and_the_rules_of_the_target`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Gates: A local baseline has no remote branch to push into

A push from a local baseline MUST be refused: it has no remote branch to push into (`localBaseline`).

#### Scenario: A local baseline
- **WHEN** the baseline is local
- **THEN** the push is refused (`localBaseline`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_elsewhere_than_the_repository_of_the_facts_has_no_gate`)

### Requirement: Gates: A gate waits for the facts of GitHub, never "no"

A gate MUST wait for the permission and for the rules while they have no answer, and while their read failed, with the
reason; never "no". The permission MUST be read first: below write access no rule is waited for.

#### Scenario: No answer yet
- **WHEN** the permission or the rules have no answer
- **THEN** the gate waits for that fact
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_waits_for_the_facts_of_github_and_never_reads_a_missing_one_as_no`)

#### Scenario: A read that failed
- **WHEN** the read of the permission or of the rules failed
- **THEN** the gate waits for it with the reason, never "no"
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_waits_for_the_facts_of_github_and_never_reads_a_missing_one_as_no`)

#### Scenario: Below write access no rule is waited for
- **GIVEN** the rules have no answer yet
- **WHEN** the permission is below write access
- **THEN** the push is refused, no rule waited for
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_waits_for_the_facts_of_github_and_never_reads_a_missing_one_as_no`)

### Requirement: Gates: The facts of GitHub count only for the repository pushed to

The facts of GitHub MUST count only for the repository the remote of the baseline pushes to (compared without case):
a push to another host, to a repository the listing doesn't describe, or off GitHub has no gate, and the server
answers.

#### Scenario: Another host, another repository, off GitHub
- **WHEN** the push goes to another host, to a repository the listing doesn't describe, or off GitHub
- **THEN** it has no gate, and the server answers
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_elsewhere_than_the_repository_of_the_facts_has_no_gate`)

#### Scenario: The repository in another case
- **WHEN** GitHub names the repository the remote pushes to in another case
- **THEN** its facts count for it
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_push_elsewhere_than_the_repository_of_the_facts_has_no_gate`)

### Requirement: Gates: A merge of the pull request needs write access

A merge of the pull request MUST need write access to the repository `origin` pushes to; without it the route stays
and its merge is refused (`noWriteAccess`: others merge, the step waits as today).

#### Scenario: A writer and a triager
- **WHEN** the route is the pull request
- **THEN** with write access the gate of its merge is open
- **AND** below write access the route stays and its merge is refused (`noWriteAccess`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_merge_needs_write_access_to_the_repository_origin_pushes_to`)

### Requirement: Gates: A mirror or a repository off GitHub has no merge

A pull request of a mirror `origin` doesn't push to MUST be refused, as it merges nothing into its target
(`gitHubMirror`), and off GitHub there is none (`notOnGitHub`). Both MUST be decided on what git names before GitHub
answers (`LandingRemote::github`), so no merge of them is offered while GitHub is asked, nor taken back after.

#### Scenario: A mirror and off GitHub
- **WHEN** the route is the pull request and its pull requests are of a mirror `origin` doesn't push to
- **THEN** the merge is refused (`gitHubMirror`)
- **AND** off GitHub it is refused (`notOnGitHub`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_merge_needs_write_access_to_the_repository_origin_pushes_to`)

#### Scenario: Before GitHub answers
- **WHEN** GitHub hasn't answered, and git names no GitHub repository of the branches, or one `origin` doesn't push to
- **THEN** the merge is refused at once (`notOnGitHub`, `gitHubMirror`)
- **AND** for the repository `origin` pushes to it waits for the permission
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_merge_is_refused_before_any_answer_where_git_names_no_repository_origin_pushes_to`)

### Requirement: Push to master: One intent lands a subject on every route

One intent MUST land a subject on every route (`Intent::Land`): its plan decides by the route of the repository as the
session holds it (`domain_policy::land::decide`, the dispatcher; `land_direct` the tail of a push), both tails reading
the part of the subject of `land` (what lands, the table, the robots, the landing gate, the target).

#### Scenario: The app and `brd` decide alike
- **WHEN** the app or `brd` plans the landing of a subject
- **THEN** the dispatcher decides by the route the session holds, the same cases answering alike in both
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Push to master: On the push route the start is one push of the copy

On the push route the start MUST be one push of the copy of the subject into the target, a fast-forward
(`PlannedEffect::PushBaseline`): the tip of the virtual branch of a piece (the branch with what it is built on, more
than its row a confirmation), of a whole stack its stack branch, only once the sync built it (`stackNotBuilt`).

#### Scenario: A piece
- **WHEN** a piece lands on the push route
- **THEN** the start pushes the tip of its virtual branch into the target as a fast-forward (`PlannedEffect::PushBaseline`)
- **AND** that it lands more than its row (the branch with what it is built on) is a confirmation
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_piece_lands_by_a_push_of_its_copy_into_the_target_and_says_it_lands_more_than_its_row`)

#### Scenario: A whole stack
- **WHEN** a whole stack lands on the push route
- **THEN** the start pushes its stack branch, only once the sync built it (`stackNotBuilt` before)
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_whole_stack_pushes_its_stack_branch_only_once_the_sync_built_it`)

### Requirement: Push to master: The route is in the fingerprint of the plan

The route MUST be in the fingerprint of the plan: a setting changed between the click and the run is `plan_changed`.

#### Scenario: The setting changed after the click
- **WHEN** the route changes between the click and the run
- **THEN** the start is `plan_changed`

### Requirement: Push to master: A push is refused in the order of its rule

A push MUST be refused, in this order: a Dry Run (a robot of Patronus, `dryRunNeedsPatronus`), the refusal of the gate,
what the table refuses (not built, parked), what landed it whole, a robot on its commit or one that lands it, what the
landing gate refuses unless allowed, a conflict with the newest target.

#### Scenario: A Dry Run and the refusal of the gate
- **WHEN** the start of a push is a Dry Run, or the gate of the route refuses
- **THEN** a Dry Run is refused first (`dryRunNeedsPatronus`), then the refusal of the gate
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_gate_of_the_route_refuses_waits_or_asks_and_a_dry_run_needs_patronus`)

#### Scenario: The refusals of the subject in their order
- **WHEN** the table refuses the subject (not built, parked), something landed it whole, a robot is on its commit or lands it, the landing gate refuses it unless allowed, or it conflicts with the newest target
- **THEN** the push is refused by the first of these in that order
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_subject_refuses_in_the_order_of_the_table_what_landed_it_the_landing_gate_and_the_target`)

### Requirement: Push to master: A push waits for its facts, never "no"

A push MUST wait for the facts of the gate, the copy, the pull requests GitHub lists and the landing gate, never
reading one as "no".

#### Scenario: A fact without an answer
- **WHEN** a fact of the gate, the copy, the pull requests GitHub lists or the landing gate has no answer
- **THEN** the push waits for it, never reading it as "no"
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_gate_of_the_route_refuses_waits_or_asks_and_a_dry_run_needs_patronus`)

### Requirement: Push to master: What CI did not pass is confirmed

A push runs no CI: what CI did not pass on the commit it lands MUST be confirmed (`unchecked`), so MUST the changes
requested on its pull request and what the gate asks (`routeRule`); a commit without checks asks nothing.

#### Scenario: What CI did not pass and the changes requested
- **WHEN** the commit a push lands has no open pull request, one that shows another commit, failing or running checks, or GitHub lists none
- **THEN** it is a confirmation (`unchecked`)
- **AND** the changes requested on its pull request are a confirmation too
- **AND** a commit without checks asks nothing
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_checks_that_did_not_pass_on_the_tip_and_the_changes_requested_are_confirmations`)

#### Scenario: What the gate asks
- **WHEN** the gate of the route asks a confirmation
- **THEN** the push asks it (`routeRule`)
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_gate_of_the_route_refuses_waits_or_asks_and_a_dry_run_needs_patronus`)

### Requirement: Push to master: `brd land` accepts the confirmations only by their flags

`brd land` MUST accept what CI did not pass and what the gate asks only by `--allow-checks-not-passed` and
`--bypass-rules`.

#### Scenario: The flags of `brd land`
- **WHEN** `brd land` meets what CI did not pass or what the gate asks
- **THEN** it accepts them only by `--allow-checks-not-passed` and `--bypass-rules`
  [@test] crates/domain-policy/src/land_direct_test.rs (`bd_accepts_the_checks_and_what_the_gate_asks_only_by_their_flags`)

### Requirement: Push to master: The push goes by the remote its plan names

The push MUST go by the first push URL of the remote of the baseline that its plan names
(`PlannedEffect::PushBaseline.remote`, whose remote-tracking ref of the target the start reserves),
`<tip>:refs/heads/<target>`, without `+` and without a lease, after the size check, with the configured remote of the
URL cleared so Git itself moves no remote-tracking ref.

#### Scenario: A copy built on the target lands as a fast-forward
- **WHEN** the push of a copy built on the target runs
- **THEN** it pushes `<tip>:refs/heads/<target>` by the first push URL of the remote, without `+` and without a lease, after the size check
- **AND** Git itself moves no remote-tracking ref
  [@test] crates/sync-core/src/baseline_push_test.rs (`a_copy_built_on_the_target_lands_as_a_fast_forward_and_moves_the_tracking_ref_by_cas`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_push_into_the_baseline`)
  [@test] crates/operations/src/land_execute_test.rs (`a_push_into_the_target_lands_the_copy_as_a_fast_forward_and_tells_what_it_landed`)

#### Scenario: A push too large for the size check
- **WHEN** the push fails the size check
- **THEN** it never goes
  [@test] crates/sync-core/src/baseline_push_test.rs (`the_server_refusing_the_push_is_said_in_its_words_and_a_large_push_never_goes`)

#### Scenario: The baseline tracks another remote by the time it runs
- **WHEN** the baseline tracks another remote or branch than the plan named by the time the push runs
- **THEN** it pushes nothing
  [@test] crates/sync-core/src/baseline_push_test.rs (`a_baseline_that_tracks_another_remote_than_the_plan_named_pushes_nothing`)

### Requirement: Push to master: The tracking ref moves by one CAS after the push

After a successful fast-forward, the tracking ref MUST move from the witness tip to the pushed tip by one CAS under
`fetch.lock`, with a `branch-deck: baseline push` reflog message.

#### Scenario: The push landed
- **WHEN** the fast-forward succeeded
- **THEN** the tracking ref moves from the witness tip to the pushed tip by one CAS under `fetch.lock`, with a `branch-deck: baseline push` reflog message
  [@test] crates/sync-core/src/baseline_push_test.rs (`a_copy_built_on_the_target_lands_as_a_fast_forward_and_moves_the_tracking_ref_by_cas`)

#### Scenario: The tracking ref moved since the witness
- **WHEN** the tracking ref moved since the witness
- **THEN** it is not overwritten
  [@test] crates/sync-core/src/baseline_push_test.rs (`a_tracking_ref_that_moved_since_the_witness_is_not_overwritten`)

### Requirement: Push to master: The witness of a push

The witness under the repository lock MUST hold the tip of the copy, the remote-tracking ref of the baseline the
copies were built on and the checked out branch at the HEAD the sync grouped.

#### Scenario: The witness of the push
- **WHEN** the push takes the repository lock
- **THEN** its witness holds the tip of the copy, the remote-tracking ref of the baseline the copies were built on and the checked out branch at the HEAD the sync grouped

### Requirement: Push to master: A target that moved refuses the push as no fast-forward

A target that moved MUST refuse the push as no fast-forward: the start fetches the baseline (`fetch_baseline`, joining
a fetch that runs), asks a sync, and fails as `baseline_moved`, pushing nothing again by itself.

#### Scenario: The target moved since the copy was built
- **WHEN** the target moved since the copy was built on it
- **THEN** the push is refused as no fast-forward
  [@test] crates/sync-core/src/baseline_push_test.rs (`a_target_that_moved_refuses_the_copy_as_no_fast_forward`)

#### Scenario: The start fetches, asks a sync and fails
- **WHEN** the push of a start is refused as no fast-forward
- **THEN** the start fetches the baseline (`fetch_baseline`, joining a fetch that runs) and asks a sync
- **AND** it fails as `baseline_moved`, pushing nothing again by itself
  [@test] crates/operations/src/land_execute_test.rs (`a_target_that_moved_is_fetched_and_synced_and_the_push_is_baseline_moved`)

### Requirement: Push to master: Update Project follows a target that moved

The copies are built on the fork point of the checked out branch, so Update Project MUST follow a target that moved as
after a push that landed (`land_direct::update_after`): the update onto the moved target builds the copy anew on it,
and the step offers its push again; `brd land` names `brd update`.

#### Scenario: The update after a target that moved
- **WHEN** the push failed as `baseline_moved`
- **THEN** Update Project follows it, which builds the copy anew on the moved target
- **AND** the step offers its push again
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_update_follows_a_push_into_the_target_that_landed_or_found_it_moved`)

#### Scenario: `brd land` and a target that moved
- **WHEN** a push of `brd land` failed as `baseline_moved`
- **THEN** it names `brd update`
  [@test] crates/bd-remote/src/land_test.rs (`a_moved_target_is_baseline_moved_and_a_refusal_of_the_server_baseline_refused`)

### Requirement: Push to master: A refusal of the server is `baseline_refused` in its words

A refusal of the server (`[remote rejected]`: a hook, a protected branch) MUST fail the push as `baseline_refused` in
the server's words, which the session keeps: the gate asks about them before the next push (`baselineRefusedPush`),
and a push that lands forgets them.

#### Scenario: The server refuses the push
- **WHEN** the server answers `[remote rejected]`, by a hook or a protected branch
- **THEN** the push fails as `baseline_refused` in the server's words
- **AND** the target stays as it was
  [@test] crates/sync-core/src/baseline_push_test.rs (`the_server_refusing_the_push_is_said_in_its_words_and_a_large_push_never_goes`)
  [@test] crates/operations/src/land_execute_test.rs (`a_refusal_of_the_server_is_baseline_refused_in_its_words_and_goes_into_the_facts`)

#### Scenario: The gate asks about it before the next push
- **GIVEN** the server refused a push in this session
- **WHEN** the next push is planned
- **THEN** the gate asks about the server's words (`baselineRefusedPush`)
- **AND** a push that lands forgets them
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_refusal_of_the_server_in_this_session_is_asked_before_the_next_push`)

### Requirement: Push to master: A fetched baseline ahead of the fork point refuses the push

A fetched baseline with commits the fork point of the checked out branch lacks (the check of the update of the last
sync: its `base` is not the head of the baseline) MUST refuse a push the gate would let go or ask about
(`baselineAhead`, "origin/master has new commits: Update Project rebuilds the copy on the new baseline"). A gate that
refuses for another reason or waits keeps its answer.

#### Scenario: The fetched baseline has new commits
- **GIVEN** the `base` of the update of the last sync is not the head of the fetched baseline
- **WHEN** the gate would let the push go or ask about it
- **THEN** the push is refused (`baselineAhead`, "origin/master has new commits: Update Project rebuilds the copy on the new baseline")
- **AND** a target moved outside the session takes Update Project (row push′ of [Next Step](../next-step/spec.md#a-push-into-the-target))
- **AND** a gate that refuses for another reason or waits keeps its answer
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_fetched_baseline_ahead_of_the_fork_point_refuses_the_push_until_the_update`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Push to master: A landing of this session on the target lifts the refusal of a baseline ahead

The refusal of a baseline ahead MUST NOT apply while an operation in this session holds the same target ref for a
landing: that landing will move the target and rebuild the copies, so the Push to master action stays enabled and
queues behind it.

#### Scenario: Another landing on the same target ref is in flight
- **GIVEN** an operation in this session holds the same target ref for a landing
- **WHEN** the fetched baseline is ahead of the fork point
- **THEN** Push to master stays the next step, enabled, and queues behind it
- **AND** only an operation that holds the target ref counts
  [@test] crates/repository-model/src/derive_test.rs (`the_landing_route_sees_only_an_operation_holding_its_target_ref`)
  [@test] crates/domain-policy/src/next_step_test.rs (`a_push_to_master_stays_the_next_step_while_another_landing_is_in_flight`)

### Requirement: Push to master: `brd land` refuses a push onto a baseline ahead

`brd land` MUST refuse a push onto a fetched baseline ahead of the fork point as `baseline_moved` with `brd update`
next, before anything is sent.

#### Scenario: A fetched target ahead of the branch
- **WHEN** `brd land` pushes a branch whose fetched target is ahead of its fork point
- **THEN** it refuses as `baseline_moved` with `brd update` next, nothing sent
  [@test] crates/bd-remote/src/land_test.rs (`a_fetched_target_ahead_of_the_branch_refuses_the_push_before_it_is_sent`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Push to master: A push that landed lands its subject at once

A push that landed MUST land its subject at once, each branch of what it landed at the tip it pushed (`landed_state`:
"Landed in origin/master"): a second click pushes nothing.

#### Scenario: The subject is landed at the tip it pushed
- **WHEN** a push of this session landed
- **THEN** each branch of what it landed is landed at the tip it pushed ("Landed in origin/master")
- **AND** a copy rebuilt since is not
  [@test] crates/domain-policy/src/landed_test.rs (`a_push_of_this_session_landed_the_subject_at_the_tip_it_pushed`)
  [@test] crates/repository-model/src/session_test.rs (`a_push_into_the_target_lands_its_subject_and_a_refusal_of_the_server_stays_for_the_gate`)

### Requirement: Push to master: Update Project without a fetch follows a push that landed

After the successful fast-forward CAS moved the tracking ref, Update Project MUST run without a fetch under the same
operation guard until its sync ends (`land_direct::update_after`, run by the engine), and its replay drops the
originals whose copies landed. `brd land` names `brd update` instead.

#### Scenario: The update after the push
- **WHEN** a push into the target landed
- **THEN** Update Project runs without a fetch (`land_direct::update_after`, run by the engine)
- **AND** its replay drops the originals whose copies landed
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_update_follows_a_push_into_the_target_that_landed_or_found_it_moved`)
  [@test] crates/operations/src/update_test.rs (`a_push_into_the_target_is_followed_by_an_update_of_its_own_that_drops_the_original_whose_copy_landed`)

### Requirement: Push to master: A second push waits for the first and runs on the plan it left

A push into the target started while another landing of the session holds its target ref (`refs/remotes/origin/master`)
MUST wait in the queue until that landing's update and sync end, then run on the plan they left
(`Accepted::decided_again`) while the user accepted every confirmation of it: one that asks more is `plan_changed`.

#### Scenario: Two pushes land one after the other
- **GIVEN** a push into the target runs its update under its operation guard
- **WHEN** a second push into the same target is started
- **THEN** it is queued behind the first
- **AND** once the update and the sync of the first end, it is planned again and pushes, both landing in the target
  [@test] crates/operations/src/update_test.rs (`two_pushes_into_the_target_run_one_after_the_other_with_the_second_replanned`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should queue a second push to master until the first landing's update and sync finish`)

#### Scenario: The plan the wait left asks a new confirmation
- **GIVEN** a push waited in the queue
- **WHEN** the plan the wait left asks a confirmation the user did not accept
- **THEN** it takes that plan's fingerprint but not the confirmation, and is refused as `plan_changed`
  [@test] crates/operations/src/land_execute_test.rs (`a_replanned_landing_still_requires_a_new_confirmation`)

### Requirement: Push to master: A waited push whose target ref alone moved updates and plans once more

A push that waited MUST, when the only ref that moved since its plan is the remote-tracking ref of its target, run
Update Project with its fetch under its guard and plan again once (`witnessed_or_retry_target`: `baseline_moved`); any
other moved ref, and any moved ref of a push that did not wait, MUST be `plan_changed`.

#### Scenario: Only the target ref moved
- **GIVEN** a push waited in the queue
- **WHEN** the remote-tracking ref of its target is the only ref of its witness that moved
- **THEN** the start updates the project with its fetch, plans again and runs that plan once
- **AND** when another ref moved too, or the push did not wait, it is `plan_changed`
  [@test] crates/operations/src/land_execute_test.rs (`only_a_waited_landing_with_its_target_ref_moved_gets_one_retry_signal`)

#### Scenario: `brd land` after a push that landed
- **WHEN** a push of `brd land` landed
- **THEN** it names `brd update` instead of running it
  [@test] crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`)

### Requirement: Push to master: The caller hears the start, then the update, as one operation

The caller MUST hear the start's outcome first, then the update outcome on the same channel; the journal keeps one
operation.

#### Scenario: The outcomes of a push and its update
- **WHEN** a push into the target landed and its update ran
- **THEN** the caller hears the start's outcome, then the update outcome on the same channel
- **AND** the journal keeps one operation
  [@test] crates/operations/src/update_test.rs (`a_push_into_the_target_is_followed_by_an_update_of_its_own_that_drops_the_original_whose_copy_landed`)

### Requirement: Push to master: The next step of a ready subject is Push to master

The next step of a ready subject MUST be "Push to master" (the row `push` of
[Next Step](../next-step/spec.md#a-push-into-the-target)), its node ready, the user's move, and its action
`push-to-baseline` of a branch and of a whole stack that forks, none of the robot's starts; a refusal of the gate
disables it with its reason and the node waits.

#### Scenario: The step and its node
- **WHEN** a subject on the push route is ready
- **THEN** its next step is "Push to master", the user's move, its node ready
- **AND** a refusal of the gate holds it back and the node waits
  [@test] crates/domain-policy/src/next_step_test.rs (`a_push_into_the_target_is_the_step_of_its_route_and_its_gate_holds_it_back`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`a_push_into_the_target_is_ready_and_a_refusal_of_its_gate_waits`)

#### Scenario: Its action
- **WHEN** the menus of a branch or of a whole stack that forks on the push route are built
- **THEN** they offer `push-to-baseline` with the reason of its step, none of the robot's starts
  [@test] crates/domain-policy/src/actions_test.rs (`a_push_into_the_target_offers_its_push_with_the_reason_of_its_step_and_no_start_of_the_robot`)

#### Scenario: The page words the step
- **WHEN** the page words the row `push`
- **THEN** it says Push to master by the branches it lands, its start disabled with the refusal of the gate of the route
  [@test] src/presenters/next-step.test.ts (`row push: Push to master by the branches it lands, its start disabled with the refusal of the gate of the route`)

### Requirement: Push to master: `brd land` lands by the same plan

`brd land` MUST land by the same plan ([Agent CLI](../../cli/agent-cli/spec.md#brd-land)), its sync and its trial with
no fetch and no check against the newest target (a push is its own).

#### Scenario: The plan of `brd land`
- **WHEN** `brd land` lands a branch on the push route
- **THEN** it runs the same plan, a moved target `baseline_moved` and a refusal of the server `baseline_refused`
  [@test] crates/bd-remote/src/land_test.rs (`a_moved_target_is_baseline_moved_and_a_refusal_of_the_server_baseline_refused`)

#### Scenario: No fetch and no check against the newest target
- **WHEN** `brd land` checks or lands by a push
- **THEN** its sync and its trial run with no fetch and no check against the newest target
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_land_check_by_a_push`, `a_land_by_a_push`)

### Requirement: Push to master: `brd status` decides the steps by the same route

`brd status` MUST decide the steps by the same route over what its grouping read of git, GitHub not asked: the gate of
a push into a repository on GitHub waits.

#### Scenario: The steps of `brd status`
- **WHEN** `brd status` runs in a repository with a route
- **THEN** it decides the steps by that route over what its grouping read of git, GitHub not asked
- **AND** the gate of a push into a repository on GitHub waits
  [@test] crates/bd-remote/src/land_test.rs (`bd_status_decides_the_steps_by_the_route_of_the_repository`)

### Requirement: Merge: On the pull-request route a start merges the pull request on GitHub

On the pull-request route a start MUST merge the open pull request of the subject on GitHub (`Intent::Land`, the tail
`land_direct::decide_merge`): of a branch its own, of a whole stack the one of its stack branch. A Dry Run MUST be
refused (`dryRunNeedsPatronus`).

#### Scenario: The merge of a branch's pull request
- **WHEN** a start lands a branch on the pull-request route
- **THEN** it merges the open pull request of the branch on GitHub
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_clean_pull_request_merges_by_the_first_method_the_repository_allows_and_an_unstable_one_asks_first`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: The merge of a whole stack
- **WHEN** a start lands a whole stack on the pull-request route
- **THEN** it merges the pull request of its stack branch

#### Scenario: A Dry Run
- **WHEN** the start is a Dry Run
- **THEN** it is refused (`dryRunNeedsPatronus`)
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_head_behind_the_copy_is_pushed_then_auto_merged_and_without_a_pull_request_nothing_merges`)

### Requirement: Merge: The start merges on GitHub only while no robot lands the repository

The start MUST merge on GitHub only while no robot of Patronus lands the repository and a merge there lands into its
target (`land::merges_on_github`: Patronus answers that the repository is not its, or, before it answers, `origin` is
not on Space (`land::patronus_lands`); the gate refuses neither a mirror nor a repository off GitHub); otherwise the
start stays the robot's, as before the routes.

#### Scenario: The step by what GitHub and Patronus tell
- **WHEN** Patronus answers that the repository is not its and the gate refuses neither a mirror nor a repository off GitHub
- **THEN** the next step merges the pull request
- **AND** while Patronus lands the repository the start stays the robot's
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merge_of_the_pull_request_is_the_step_of_its_route_by_what_github_tells`)

#### Scenario: Before Patronus answers
- **WHEN** Patronus hasn't answered and `origin` is not on Space
- **THEN** the merge is the start already, waiting for GitHub, as it stays after the answer
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)

### Requirement: Merge: What GitHub tells of the pull request comes with the listing

What GitHub tells of the pull request MUST come with the listing, the same query, no request of its own: whether it can
merge it now (`mergeStateStatus`: `CLEAN`, `UNSTABLE`, `BLOCKED`, `BEHIND`, `DIRTY`, `DRAFT`, `HAS_HOOKS`, `UNKNOWN`),
the method of an auto-merge enabled on it (`autoMergeRequest`), and of the repository the methods it allows
(`rebaseMergeAllowed`, `mergeCommitAllowed`, `squashMergeAllowed`, `autoMergeAllowed`).

#### Scenario: The listing answers the merge state, the auto-merge and the methods
- **WHEN** the session lists the pull requests
- **THEN** each comes with `mergeStateStatus` and `autoMergeRequest`, and the repository with `rebaseMergeAllowed`, `mergeCommitAllowed`, `squashMergeAllowed` and `autoMergeAllowed`
- **AND** no request of its own asks them
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_merge_state_the_auto_merge_and_the_merge_methods_come_with_the_listing`)

#### Scenario: The session keeps the methods with the listing
- **WHEN** the session applies a listing
- **THEN** the methods of the repository go with it
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_merge_and_the_auto_merge_go_into_the_pull_request_at_once_and_the_methods_with_the_listing`)

### Requirement: Merge: A merge state GitHub still computes is asked again and waited for

GitHub computes the state lazily: an open pull request it answered `UNKNOWN` of MUST make the session list once more a
moment later, and `brd` list again once; the start MUST wait for it (`mergeState`), never "no".

#### Scenario: GitHub answered `UNKNOWN`
- **WHEN** GitHub answered `UNKNOWN` of an open pull request
- **THEN** the session lists once more a moment later, and `brd` lists again once
  [@test] crates/domain-policy/src/refresh_test.rs (`an_open_pull_request_whose_merge_state_github_computes_asks_the_list_again`)

#### Scenario: The start waits for the state
- **WHEN** the merge state is `UNKNOWN` or not answered
- **THEN** the start waits for it (`mergeState`), never "no"
  [@test] crates/domain-policy/src/land_direct_test.rs (`what_holds_the_merge_back_refuses_it_running_checks_enable_the_auto_merge_and_an_unknown_state_waits`)

### Requirement: Merge: The method is the first the repository allows

The method MUST be the first the repository allows of rebase, a merge commit and squash (the rebase while GitHub named
none): a rebase keeps author, time and message, so the update pairs the commits that landed with their originals and
drops them; a merge commit keeps the copies; squash pairs none, so the update after it may stop on a conflict, which
the start asks first (`squashOnly`).

#### Scenario: Rebase, a merge commit, squash
- **WHEN** the repository allows rebase, or only a merge commit, or only squash, or GitHub named none
- **THEN** the merge goes by the first allowed of rebase, a merge commit and squash, the rebase while GitHub named none
- **AND** a squash only is asked first (`squashOnly`)
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_clean_pull_request_merges_by_the_first_method_the_repository_allows_and_an_unstable_one_asks_first`)

### Requirement: Merge: A pull request GitHub can merge is merged now

A start MUST merge now a pull request GitHub can merge (`CLEAN`, `HAS_HOOKS`), and one whose checks the base does not
require fail or run (`UNSTABLE`) once the user accepts it (`checksNotRequired`, `--allow-checks-not-passed` of
`brd land`); changes someone requested that hold nothing back are a confirmation too.

#### Scenario: `CLEAN` and `HAS_HOOKS`
- **WHEN** GitHub answers `CLEAN` or `HAS_HOOKS`
- **THEN** the start merges now
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_clean_pull_request_merges_by_the_first_method_the_repository_allows_and_an_unstable_one_asks_first`)

#### Scenario: `UNSTABLE` once accepted
- **WHEN** GitHub answers `UNSTABLE`, checks the base does not require failing or running
- **THEN** the start merges once the user accepts it (`checksNotRequired`, `--allow-checks-not-passed` of `brd land`)
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_clean_pull_request_merges_by_the_first_method_the_repository_allows_and_an_unstable_one_asks_first`)

#### Scenario: Changes requested that hold nothing back
- **WHEN** someone requested changes that hold nothing back
- **THEN** they are a confirmation

### Requirement: Merge: The auto-merge is enabled while required checks run

A start MUST enable the auto-merge, GitHub merging once the checks pass, while required checks still run (`BLOCKED` by
them) where the repository allows it, and after the push of the branch (of a whole stack its stack branch) when the
pull request shows another commit than the copy: the push restarts its checks, a merge now would merge the old head.
One whose auto-merge is enabled MUST start nothing.

#### Scenario: Required checks still run
- **WHEN** required checks still run (`BLOCKED` by them) and the repository allows the auto-merge
- **THEN** the start enables the auto-merge
  [@test] crates/domain-policy/src/land_direct_test.rs (`what_holds_the_merge_back_refuses_it_running_checks_enable_the_auto_merge_and_an_unknown_state_waits`)

#### Scenario: The pull request shows another commit than the copy
- **WHEN** the pull request shows another commit than the copy and the repository allows the auto-merge
- **THEN** the start pushes the branch (of a whole stack its stack branch), then enables the auto-merge on the pushed commit
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_head_behind_the_copy_is_pushed_then_auto_merged_and_without_a_pull_request_nothing_merges`)

#### Scenario: The auto-merge is enabled already
- **WHEN** the auto-merge of the pull request is enabled
- **THEN** the start starts nothing
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_head_behind_the_copy_is_pushed_then_auto_merged_and_without_a_pull_request_nothing_merges`)

### Requirement: Merge: What holds the merge back refuses it with its reason

A start MUST refuse, with its reason: no open pull request into the target (`noPullRequestToMerge`: Create Pull Request
first; `mergedIntoBranch` when GitHub merged the branch's pull request into another branch, a parent's, which the
target lacks), one into another base (`otherBase`), a draft (`mergeDraft`), changes requested, missing approvals,
failing or running checks, an unnamed hold, a head behind, a conflict, another head.

#### Scenario: No pull request, or one into another base
- **WHEN** the branch has no open pull request into the target, or one into another base
- **THEN** the start is refused (`noPullRequestToMerge`: Create Pull Request first; `otherBase`)
- **AND** when GitHub merged the branch's pull request into another branch, a parent's, which the target lacks, it is `mergedIntoBranch`
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_head_behind_the_copy_is_pushed_then_auto_merged_and_without_a_pull_request_nothing_merges`)

#### Scenario: What GitHub holds the merge back by
- **WHEN** the pull request is a draft (`mergeDraft`), has changes requested (`mergeChangesRequested`), lacks approvals the base requires (`mergeNeedsApprovals`), has required checks that fail (`mergeChecksFail`) or run without auto-merge (`mergeChecksRunning`), is held by what GitHub doesn't name (`mergeBlocked`), its head behind its base (`mergeBehind`: the update of the branch and its push), or conflicts (`mergeConflict`)
- **THEN** the start is refused with that reason
  [@test] crates/domain-policy/src/land_direct_test.rs (`what_holds_the_merge_back_refuses_it_running_checks_enable_the_auto_merge_and_an_unknown_state_waits`)

#### Scenario: Another head without auto-merge
- **WHEN** the pull request shows another head and the repository allows no auto-merge
- **THEN** the start is refused (`mergeHeadBehind`: the push first)
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_head_behind_the_copy_is_pushed_then_auto_merged_and_without_a_pull_request_nothing_merges`)

### Requirement: Merge: The merge is `mergePullRequest` with the head the plan saw

The merge MUST be GitHub's `mergePullRequest` with the method and the head the plan saw (`expectedHeadOid`,
`PlannedEffect::MergePullRequest`), after one read of the node of the pull request and never sent twice; a head that
moved since sends nothing (`plan_changed`), and GitHub's refusal is `not_mergeable` in its words.

#### Scenario: The merge is sent once
- **WHEN** a merge runs
- **THEN** it reads the node of the pull request once, then sends `mergePullRequest` once with the method and `expectedHeadOid`
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_merge_reads_the_node_then_sends_the_mutation_once_with_the_head_and_a_moved_head_sends_nothing`)
  [@test] crates/operations/src/land_execute_test.rs (`a_merge_of_the_pull_request_goes_into_the_facts_and_a_moved_head_or_a_refusal_of_github_merges_nothing`)

#### Scenario: A head that moved, a refusal of GitHub
- **WHEN** the head moved since the plan, or GitHub refuses the merge
- **THEN** a moved head sends nothing (`plan_changed`)
- **AND** GitHub's refusal is `not_mergeable` in its words, nothing merged
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_merge_reads_the_node_then_sends_the_mutation_once_with_the_head_and_a_moved_head_sends_nothing`)
  [@test] crates/operations/src/land_execute_test.rs (`a_merge_of_the_pull_request_goes_into_the_facts_and_a_moved_head_or_a_refusal_of_github_merges_nothing`)

### Requirement: Merge: The auto-merge is `enablePullRequestAutoMerge` with the head the plan saw

The auto-merge MUST be GitHub's `enablePullRequestAutoMerge` with the method and the head the plan saw
(`EnableAutoMerge`), after one read of the node of the pull request and never sent twice.

#### Scenario: The auto-merge is enabled on the head
- **WHEN** the start enables the auto-merge
- **THEN** it sends `enablePullRequestAutoMerge` once on the head the plan saw
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`the_auto_merge_is_enabled_on_the_head_and_disabled_by_the_node`)

#### Scenario: The auto-merge after the push of the branch
- **WHEN** the start pushed the branch before the auto-merge
- **THEN** the auto-merge goes on the pushed commit
  [@test] crates/operations/src/land_execute_test.rs (`the_auto_merge_after_the_push_of_the_branch_goes_on_the_pushed_commit`)

### Requirement: Merge: A merge and an auto-merge hold their field of the pull request

A merge MUST hold the state of the pull request (`PullRequestField::State`), an auto-merge its auto-merge
(`PullRequestField::AutoMerge`); both run without Patronus.

#### Scenario: What a merge and an auto-merge hold
- **WHEN** a merge or an auto-merge runs
- **THEN** a merge holds `PullRequestField::State`, an auto-merge `PullRequestField::AutoMerge`
- **AND** both run without Patronus

### Requirement: Merge: A merge by the click goes into the session at once

A merge by the click MUST go into the session at once (`PullRequestWrite::Merged`): its subject is landed by the pull
request ("Landed with #7"), and the pull requests of what it landed close by content as a merged pull request's
([Closing what landed](#closing-what-landed)).

#### Scenario: The merge goes into the pull request at once
- **WHEN** a merge by the click succeeded
- **THEN** the session holds the pull request merged at once (`PullRequestWrite::Merged`), its subject landed by it ("Landed with #7")
- **AND** the pull requests of what it landed close by content as a merged pull request's
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_merge_and_the_auto_merge_go_into_the_pull_request_at_once_and_the_methods_with_the_listing`)

### Requirement: Merge: Update Project with its fetch follows a merge

After a merge by the click, Update Project with its fetch MUST follow as an operation of its own
(`land_direct::update_after`), as after a push; `brd land` names `brd update` instead.

#### Scenario: The update after a merge
- **WHEN** a merge by the click succeeded
- **THEN** Update Project with its fetch follows as an operation of its own
- **AND** `brd land` names `brd update` instead
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_update_follows_a_merge_of_the_pull_request_and_no_auto_merge`)

### Requirement: Merge: An enabled auto-merge goes into the session and is news once merged

An enabled auto-merge MUST go into the session too (`PullRequestWrite::AutoMerge`), and run no update: once a listing
shows the pull request merged, the session tells it as news (`NewsEvent::AutoMerged`, `news::auto_merged`).

#### Scenario: The auto-merge runs no update
- **WHEN** the start enabled the auto-merge
- **THEN** the session holds it at once (`PullRequestWrite::AutoMerge`) and no update follows
  [@test] crates/domain-policy/src/land_direct_test.rs (`the_update_follows_a_merge_of_the_pull_request_and_no_auto_merge`)
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`a_merge_and_the_auto_merge_go_into_the_pull_request_at_once_and_the_methods_with_the_listing`)

#### Scenario: GitHub merged it by its auto-merge
- **WHEN** a listing shows the pull request merged by its auto-merge
- **THEN** the session tells it as news once (`NewsEvent::AutoMerged`, `news::auto_merged`)
  [@test] crates/domain-policy/src/news_test.rs (`a_pull_request_github_merged_by_its_auto_merge_is_news_once`)

### Requirement: Merge: A layer of a stack of GitHub merges with the layers below it

A layer of a stack of GitHub ([Stacks of GitHub](../stacked-branches/spec.md#stacks-of-github)) MUST merge with the
layers below it in one asynchronous merge of its own pull request (`merge-async` with the head the plan saw,
`PlannedEffect::MergeStack`, the layers below named), its result asked until GitHub merged it, queued it or refused it:
a piece in one move. Once GitHub merged it, the layer and each layer below go into the session as merged.

#### Scenario: One asynchronous merge of the layer
- **WHEN** a layer of a stack of GitHub lands
- **THEN** the start sends one `merge-async` of its pull request with the head the plan saw (`PlannedEffect::MergeStack`), the layers below named
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_layer_of_a_stack_merges_with_the_layers_below_it_and_the_first_one_that_cant_holds_it`)

#### Scenario: Its result is asked until GitHub merged it
- **WHEN** the asynchronous merge was sent
- **THEN** its result is asked until GitHub merged it, queued it or refused it
- **AND** once merged, the layer and each layer below go into the session as merged
  [@test] crates/operations/src/pull_request_test.rs (`the_merge_of_a_piece_of_a_stack_asks_its_result_until_github_merged_it`)
  [@test] tests/e2e/stacked-branches/github-stacks.spec.ts (`should merge a piece of a stack of GitHub with the layers below it in one asynchronous merge from the slot of the row`)

### Requirement: Merge: The first layer below that a merge can't take holds the layer

The first layer below that a merge can't take now MUST hold the layer, by that layer's number and reason (approvals, a
conflict, a state GitHub still computes: a wait); GitHub has no auto-merge of a stack, so required checks that still
run hold it.

#### Scenario: A layer below can't be merged now
- **WHEN** a layer below lacks approvals, conflicts, or has a state GitHub still computes
- **THEN** the first such layer holds the merge by its number and reason (`mergeNeedsApprovals`, `mergeConflict`), a state still computed a wait
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_layer_of_a_stack_merges_with_the_layers_below_it_and_the_first_one_that_cant_holds_it`)

#### Scenario: Required checks that still run
- **WHEN** required checks of the layer still run
- **THEN** the merge is held (`mergeChecksRunning`), no auto-merge enabled
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_layer_of_a_stack_merges_with_the_layers_below_it_and_the_first_one_that_cant_holds_it`)

### Requirement: Merge: A pull request into the branch below outside a stack is not merged

A pull request into the branch below it that GitHub lists in no stack would merge into that branch, landing nothing:
it MUST NOT be merged. The step, which knows the branch below (`RepositoryView.githubLayers`), MUST hold it with "#9
goes into login outside a stack of GitHub: its merge would land nothing, link the stack first" (`layerNotLinked`); the
start, which reads its base alone (`start_base`), refuses it as `otherBase`, as one into a virtual branch.

#### Scenario: The step of a layer outside a stack
- **WHEN** the pull request of the layer goes into the branch below it and GitHub lists it in no stack
- **THEN** its step is Link GitHub Stack (row 5 of [Next Step](../next-step/spec.md#the-table)), its merge held with "#9 goes into login outside a stack of GitHub: its merge would land nothing, link the stack first" (`layerNotLinked`)
- **AND** once GitHub lists it in the stack, the step merges it
  [@test] crates/domain-policy/src/next_step_test.rs (`row_5_links_the_stack_of_a_layer_github_lists_in_none`)

#### Scenario: The start of a layer outside a stack
- **WHEN** a start merges a layer GitHub lists in no stack
- **THEN** it is refused as `otherBase`, a pull request into a virtual branch, as any such (row 5 of [Next Step](../next-step/spec.md#the-table): Retarget #7 to master)
- **AND** the merge retargets nothing
  [@test] crates/domain-policy/src/land_direct_test.rs (`a_layer_outside_a_stack_is_not_merged`)

### Requirement: Merge: Cancel Auto-Merge disables the auto-merge

Cancel Auto-Merge MUST disable the auto-merge of the open pull request (`Intent::DisableAutoMerge`,
`disablePullRequestAutoMerge`), the action `cancel-auto-merge` of a branch and of a whole stack while its step is
`merge′`; the merge MUST be `merge-pull-request` in place of the robot's starts.

#### Scenario: Cancel Auto-Merge
- **WHEN** the user chooses Cancel Auto-Merge on a pull request that has an auto-merge
- **THEN** `disablePullRequestAutoMerge` disables it (`Intent::DisableAutoMerge`)
  [@test] crates/operations/src/auto_merge_test.rs (`cancel_auto_merge_disables_the_auto_merge_of_an_open_pull_request_that_has_one`)
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`the_auto_merge_is_enabled_on_the_head_and_disabled_by_the_node`)

#### Scenario: The actions of the merge route
- **WHEN** the menus of a branch or of a whole stack on the merge route are built
- **THEN** they offer `merge-pull-request` in place of the robot's starts
- **AND** `cancel-auto-merge` while the step is `merge′`
  [@test] crates/domain-policy/src/actions_test.rs (`a_merge_of_the_pull_request_offers_its_merge_and_cancel_auto_merge_while_github_merges_it`)

### Requirement: Merge: `brd land` merges by the same plan

`brd land` MUST merge by the same plan ([Agent CLI](../../cli/agent-cli/spec.md#brd-land)): `pull_request` names the
pull request, `auto_merge` an auto-merge enabled (now or before), `landed` a merge; its refusals `not_mergeable`,
`merge_blocked` and `no_pull_request`, a mirror or a repository off GitHub `landing_route_unsupported`. On Space the
pull-request route is the Safe Push, as in the app while Patronus lands the repository.

#### Scenario: The report of `brd land`
- **WHEN** `brd land` lands on the pull-request route
- **THEN** its report names `pull_request`, `auto_merge` (now or before) or `landed`
- **AND** its refusals are `not_mergeable`, `merge_blocked` and `no_pull_request`, a mirror or a repository off GitHub `landing_route_unsupported`
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: The pull-request route on Space
- **WHEN** `brd land` runs on Space with the pull-request route while Patronus lands the repository
- **THEN** it starts the Safe Push, as the app does

### Requirement: Closing what landed: One rule closes the pull requests that hold nothing more

Whatever landed a branch, the open pull requests that hold nothing the branch lacks (the head is the branch's tip, or
its remote branch has no commit the branch lacks) MUST close, by one rule (`domain_policy::landed::pull_requests_to_close`,
`Landing`).

#### Scenario: A merged robot
- **WHEN** a robot of Patronus merged
- **THEN** the pull requests close as before ([Patronus](../patronus/spec.md#merged)): of the branches it landed whole, not of its own branch
  [@test] crates/domain-policy/src/landed_test.rs (`closes_the_pull_requests_of_the_branches_a_merged_robot_landed_whole_not_of_its_own_branch`)

### Requirement: Closing what landed: A push closes the pull requests that show an older version

Of a push into the target, the pull request of every branch it landed, its own too, that shows an older version of it
(a head the push left out) MUST close, while one into the target whose head is its branch's tip in the history of the
copy is GitHub's, which merges it once its head is in the target.

#### Scenario: A head the push left out and one in the history of the copy
- **WHEN** a push into the target landed branches
- **THEN** each pull request that shows an older version of its branch, its own too, closes
- **AND** one into the target whose head is its branch's tip in the history of the copy is left to GitHub (a piece, the last branch of a straight chain)
- **AND** of a stack that forks every branch's closes, as a stack branch copies the branches anew
  [@test] crates/domain-policy/src/landed_test.rs (`a_push_closes_the_pull_requests_whose_head_it_left_out_and_leaves_those_in_the_history_of_its_copy_to_github`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should close the pull requests of what a push of a stacked piece landed that show an older version of a branch, its own too`)

### Requirement: Closing what landed: A push closes the layers of a stack of GitHub above the bottom

Of a push into the target, a layer of a stack of GitHub above the bottom MUST close too: it goes into the branch below
it, which GitHub never marks merged.

#### Scenario: The layers above the bottom
- **WHEN** a push landed the layers of a stack of GitHub
- **THEN** the pull request of each layer above the bottom closes
  [@test] crates/domain-policy/src/landed_test.rs (`a_push_closes_the_layers_of_a_stack_of_github_above_the_bottom`)

### Requirement: Closing what landed: The fetched baseline and a merged pull request close what they have

Of the fetched baseline and of a pull request GitHub merged with the commits of its branch (that branch with what it
is built on, of a whole stack every branch), every open pull request of what it has MUST close.

#### Scenario: The fetched baseline
- **WHEN** the fetched baseline has every commit of branches
- **THEN** every open pull request of what it has that holds nothing more closes
  [@test] crates/domain-policy/src/landed_test.rs (`the_fetched_baseline_closes_every_open_pull_request_of_what_it_has_that_holds_nothing_more`)

#### Scenario: A pull request GitHub merged
- **WHEN** GitHub merged a pull request with the commits of its branch
- **THEN** every open pull request of that branch with what it is built on, of a whole stack every branch, closes
  [@test] crates/domain-policy/src/landed_test.rs (`names_the_pull_request_github_merged_with_the_commits_of_the_branch`)

### Requirement: Closing what landed: The session tells each landing without a robot once

The session MUST tell each landing without a robot once as news (`NewsEvent::Landed`, with the pull requests it closes)
and keep what landed them for the plan of the close; a pull request is told once by any landing, a robot's merge too
(`repository_model::landed_closes`).

#### Scenario: A push into the target
- **WHEN** a push into the target ends
- **THEN** it is told over the table before its update drops the branches, the branches it landed then judged by it
- **AND** the fetch after it, which shows them in the baseline, tells nothing again
  [@test] crates/repository-model/src/landed_closes_test.rs (`a_push_tells_the_pull_requests_it_landed_that_show_an_older_version_and_the_fetch_after_it_tells_nothing_again`)

#### Scenario: The fetched baseline and a merged pull request
- **WHEN** the fetched baseline or a merged pull request landed branches
- **THEN** the fetched baseline is told once per branch and tip, a merged pull request once per head
  [@test] crates/repository-model/src/landed_closes_test.rs (`the_fetched_baseline_and_a_merged_pull_request_tell_the_open_pull_requests_of_what_they_have_once`)

### Requirement: Closing what landed: The page asks the close of the news at once

The page MUST ask the close of the news at once (`CloseLandedPullRequests` with `LandingKey::Told`, no timer of its
own). A close that fails is "Cannot close pull request #5".

#### Scenario: The news of a landing
- **WHEN** the page hears the news of a landing without a robot
- **THEN** it asks the session to close what it landed at once (`CloseLandedPullRequests` with `LandingKey::Told`), no timer of its own
- **AND** each close that failed toasts "Cannot close pull request #5"
  [@test] src/lib/landed-closes.test.ts (`asks the session to close what each landing without a robot landed, and toasts each close that failed`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should close the pull requests of what a push of a stacked piece landed that show an older version of a branch, its own too`)

### Requirement: Closing what landed: Each close carries the comment Rust words

Each close MUST carry the comment Rust words, which names what landed the branch; Activity says each close by its
comment.

#### Scenario: The comments of the closes
- **WHEN** a landing without a robot closes a pull request
- **THEN** its comment is "Landed in master as 4a64f53 with the push of user/virtual/login, which is built on this branch." (an ancestor), "… with the push of user/virtual/login." (its own), "… with the push of the whole air stack (user/stack/air), which holds this branch.", "Landed in master: origin/master has every commit of this branch.", or "Landed in master with #7, the pull request of user/virtual/login, which is built on this branch."
  [@test] crates/operations/src/close_test.rs (`the_closes_of_a_landing_without_a_robot_name_what_landed_each_branch`)

#### Scenario: Activity says each close
- **WHEN** Activity lists the closes of a landing without a robot
- **THEN** it says each by its comment, which no toast says
  [@test] src/presenters/activity.test.ts (`says each close of a landing without a robot by its comment, which no toast says`)

### Requirement: Closing what landed: `brd land` lists what its push closes and closes none

`brd land` MUST list the pull requests its push closes as `closes`, by the same rule, and close none: the app's session
closes them ([Agent CLI](../../cli/agent-cli/spec.md#brd-land)).

#### Scenario: The closes of `brd land`
- **WHEN** a push of `brd land` landed
- **THEN** its report lists the pull requests the push closes as `closes`, by the same rule
- **AND** it closes none

### Requirement: The page: The view carries the route with its gate

The view of the session MUST carry the route with its gate (`RepositoryView.route`, `derive::landing_route_of`), so the
page knows a repository that lands by a push without Patronus: its start runs where no robot answers, and its words are
a push's.

#### Scenario: A repository that lands by a push
- **WHEN** the route of the repository is a push
- **THEN** `RepositoryView.route` carries it with its gate, so its start runs where no robot answers and its words are a push's
  [@test] crates/repository-model/src/session_test.rs (`a_push_into_the_target_lands_its_subject_and_a_refusal_of_the_server_stays_for_the_gate`)

### Requirement: The page: The label names the branch of the baseline on its remote

The label of the push MUST name the branch of the baseline on its remote: "Push to master", "Push to main" ("· 3" for
what it is built on).

#### Scenario: The label of the push
- **WHEN** the baseline is `master` or `main` on its remote
- **THEN** the label is "Push to master" or "Push to main", with "· 3" for what it is built on

### Requirement: The page: Push to master is one start on every surface of the Safe Push

Push to master MUST be one start on every surface that offers the Safe Push. Each MUST run the plan of `prepare`: it
pushes at once when it asks nothing, else opens its popover at the slot, which Enter starts.

#### Scenario: The slot of the row
- **WHEN** the user clicks the slot of the row
- **THEN** the push runs the plan of `prepare`: at once when it asks nothing, else its popover opens at the slot, asking what CI did not pass, and Enter starts it
- **AND** the update follows it
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push the copy into master from the slot of the row, the popover asking what CI did not pass, and update after it`)

#### Scenario: S on the row
- **WHEN** the user presses S on the row
- **THEN** the popover of Push to master opens
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should open the popover of Push to master with S on the row`)

#### Scenario: The menus of the row
- **WHEN** the user chooses "Push to master" of the "⋯" or the right-click menu
- **THEN** the push starts
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push into master from the menu of the row`)

#### Scenario: The branch preview
- **WHEN** the user starts it from the footer of the branch preview, or presses S there
- **THEN** the push starts
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push into master from the footer of the branch preview`, `should push into master with S in the branch preview`)

#### Scenario: The review window
- **WHEN** the user starts it from the title bar of the review window
- **THEN** the review window asks the main window (`push-to-baseline`), which pushes
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push into master when the review window asks the main window`)

#### Scenario: A whole stack
- **WHEN** the user starts it from the header of the island of a whole stack, or from "Whole Stack ›" of the menu of its root
- **THEN** its stack branch is pushed
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push the stack branch of a whole stack into master from the header of its island`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push a whole stack into master from Whole Stack of the menu of its root`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_whole_stack_off_the_robot_offers_the_push_or_the_merge_of_its_stack_branch_with_the_reason_of_its_step`)

### Requirement: The page: The popover of a push says what one push lands

The popover of a push MUST say what one push lands, then a line per confirmation, and that a push runs no CI. No Dry
Run, no profile, no review; its start MUST be "Push to master".

#### Scenario: What one push lands
- **WHEN** the popover of a push opens
- **THEN** it says what the push lands: "Pushes host-git into master: 3 commits.", a piece of a stack "Pushes oauth with auth and login, which it is built on: 3 branches · 12 commits into master.", a whole stack "Pushes 7 branches · 14 commits into master in one push: …" with the stack branch it pushes
- **AND** it shows no Dry Run, no profile, no review, and its start is "Push to master"

#### Scenario: A line per confirmation
- **WHEN** the push asks confirmations
- **THEN** the popover says what CI did not pass on the commit it lands ("JIRA-456 has no open pull request: no CI ran on the commit it lands.", the checks of #7 failing or still running, #7 showing another commit), the changes requested, what the gate asks ("master requires status checks: a push lands without them.", a refusal of the server before)
- **AND** that a push runs no CI
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should ask about the required status checks of master and push by the GitHub URL of origin`)

### Requirement: The page: A push that landed is one toast that follows its update

A push that landed MUST be one toast that follows the update after it in place: "Pushed JIRA-456 into master · 3
commits", "Updating the project…", then what the update did, or Update Project in the toast when the update did not
move the branch.

#### Scenario: The push and its update
- **WHEN** a push landed and its update runs
- **THEN** one toast says "Pushed JIRA-456 into master · 3 commits", "Updating the project…", then what the update did ("JIRA-456 landed in origin/master · Updated master: …")
- **AND** Update Project in the toast when the update did not move the branch (local changes, a conflict, a failure, an update that did not run)
  [@test] src/presenters/outcome.test.ts (`says what it pushed, a target that moved, the words of the server that refused it, and the push again of a plan that changed`, `says the update that follows it: running, what it did, or Update Project where it did not move the branch`)

### Requirement: The page: A target that moved is a notice that follows its update

A target that moved MUST be a notice in the same way: "master moved: JIRA-456 is rebuilt on the new commits", nothing
pushed, the update, then "Push to master again", which the step offers.

#### Scenario: The target moved
- **WHEN** the push found the target moved
- **THEN** the notice says "master moved: JIRA-456 is rebuilt on the new commits", nothing pushed, then the update
- **AND** then "Push to master again", which the step offers
  [@test] src/presenters/outcome.test.ts (`says what it pushed, a target that moved, the words of the server that refused it, and the push again of a plan that changed`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should say that master moved, push nothing, and offer the push of the copy rebuilt on it`)

### Requirement: The page: A refusal of the server is an error in its words

A refusal of the server MUST be an error in its words: "Cannot push JIRA-456 into master" with "The server refused the
push into master: …".

#### Scenario: The server refused the push
- **WHEN** the server refused the push
- **THEN** the error says "Cannot push JIRA-456 into master" with "The server refused the push into master: …"
- **AND** the gate asks about it before the next push
  [@test] src/presenters/outcome.test.ts (`says what it pushed, a target that moved, the words of the server that refused it, and the push again of a plan that changed`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should say in the words of the server why it refused the push, and ask about it before the next push`)

### Requirement: The page: Activity and the slot say the operation as a push

Activity MUST say the same (`presentLandOutcome`), a start refused before its first effect as a push too ("Cannot push
auth into master", by the target its journal entry names, `JournalEntry.into`). The slot MUST say the operation as a
push ("Push of auth into master", by the remote-tracking ref it holds).

#### Scenario: A start refused before its first effect
- **WHEN** a start of the push route is refused before its first effect
- **THEN** its journal entry names its target (`JournalEntry.into`)
- **AND** Activity says "Cannot push auth into master", not a Safe Push
  [@test] src/presenters/activity.test.ts (`says a start of the push route refused before its first effect as a push into its target, not a Safe Push`)
  [@test] crates/operations/src/journal_test.rs (`a_start_of_the_push_route_is_journaled_with_its_target_also_when_refused_before_its_first_effect`)

#### Scenario: The slot of a running push
- **WHEN** a push runs
- **THEN** the slot says "Push of auth into master", by the remote-tracking ref it holds

### Requirement: The page: A refusal of the gate disables Push to master with its reason

A refusal of the gate MUST disable Push to master on every surface with its reason: below write access "You have no
write access to acme/monorepo: someone who has lands it into master".

#### Scenario: No write access
- **WHEN** the token has no write access to the repository
- **THEN** Push to master is disabled on every surface with "You have no write access to acme/monorepo: someone who has lands it into master"
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should disable Push to master with the reason when the token has no write access to the repository`)

### Requirement: The page: Merge #7 is one start on the surfaces of the row

Merge #7 MUST be one start on the surfaces of the row: the slot, S on the row, the "⋯" and right-click menus, the footer
of the branch preview, the title bar of the review window, and of a whole stack the header of its island and "Whole
Stack ›" of the menu of its root, the merge of the pull request of its stack branch. "Auto-Merge #7" MUST be the same
while required checks run.

#### Scenario: The slot of the row
- **WHEN** the user clicks the slot of a row whose plan asks nothing
- **THEN** it merges at once, and the update follows it
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge the approved green pull request from the slot of the row by rebase, and update after it`)

#### Scenario: S on the row
- **WHEN** the user presses S on the row
- **THEN** its popover says "GitHub merges #7 of JIRA-456 into master by rebase: 3 commits." and what it asks, its start "Merge #7"
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should open the popover of Merge #7 with S on the row, naming the method`)

#### Scenario: The menus of the row
- **WHEN** the user chooses Merge in the "⋯" or the right-click menu
- **THEN** the pull request is merged
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge from the menu of the row`)

#### Scenario: The branch preview
- **WHEN** the user starts it from the footer of the branch preview
- **THEN** the pull request is merged
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge from the footer of the branch preview`)

#### Scenario: The review window
- **WHEN** the user starts it from the title bar of the review window
- **THEN** the review window asks the main window (`merge-pull-request`), which merges
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge when the review window asks the main window`)

#### Scenario: The review window while the auto-merge is on
- **WHEN** the auto-merge of the pull request is on
- **THEN** the title bar of the review window says "GitHub merges the pull request of JIRA-456 once its checks pass"

#### Scenario: A whole stack
- **WHEN** the user starts it from the header of the island of a whole stack, or from "Whole Stack ›" of the menu of its root
- **THEN** the pull request of its stack branch is merged, which always asks first ("GitHub merges #12 of the whole air stack with the 6 branches it is built on into master by rebase: 14 commits.", the push of its stack branch the remote lacks)
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge the pull request of the stack branch of a whole stack from the header of its island`)
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge a whole stack from Whole Stack of the menu of its root`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_whole_stack_off_the_robot_offers_the_push_or_the_merge_of_its_stack_branch_with_the_reason_of_its_step`)

#### Scenario: The rows of a merge
- **WHEN** the page words the step of a merge
- **THEN** it says Merge #7 or "Auto-Merge #7" by the method
  [@test] src/presenters/next-step.test.ts (`rows of a merge: Merge #7 and Auto-Merge #7 by the method, merge′ merges by itself, merge″ says why the merge waits, pr creates the pull request`)

### Requirement: The page: A merge is one toast that follows its update

A merge MUST be one toast that follows the update after it in place, as a push's: "Merged #7 of JIRA-456 into master ·
3 commits", then what the update did.

#### Scenario: The merge and its update
- **WHEN** the merge succeeded and its update runs
- **THEN** one toast says "Merged #7 of JIRA-456 into master · 3 commits", then what the update did
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge the approved green pull request from the slot of the row by rebase, and update after it`)
  [@test] src/presenters/outcome.test.ts (`says what GitHub merged, an auto-merge it waits with, and its refusal in its words`)

### Requirement: The page: An auto-merge is said until GitHub merged it

An auto-merge MUST be "Auto-Merge of #7 is on", its slot "Merges when checks pass", its node landing, Cancel Auto-Merge
in the menus ("Canceled the auto-merge of #7"); once GitHub merged it the same toast says "Merged #7 into master" with
Update Project.

#### Scenario: The auto-merge is on, then GitHub merged it
- **WHEN** the start enabled the auto-merge while the required checks run
- **THEN** the toast says "Auto-Merge of #7 is on", its slot "Merges when checks pass", its node landing
- **AND** once GitHub merged it the same toast says "Merged #7 into master" with Update Project
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should enable the auto-merge while the required checks run, and offer Update Project once GitHub merged it`)
  [@test] src/presenters/outcome.test.ts (`says what GitHub merged, an auto-merge it waits with, and its refusal in its words`)

#### Scenario: Cancel Auto-Merge in the menus
- **WHEN** the user chooses Cancel Auto-Merge in the menu of the row
- **THEN** the auto-merge is canceled, "Canceled the auto-merge of #7"
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should cancel the auto-merge from the menu of the row`)

### Requirement: The page: What holds the merge back disables Merge with its reason

What holds the merge back MUST disable Merge with its reason, the slot naming it ("Awaiting Approval", "Conflict",
"Update Needed").

#### Scenario: The pull request waits for approvals
- **WHEN** the pull request waits for the approvals its base requires
- **THEN** Merge is disabled with the reason, the slot saying "Awaiting Approval"
- **AND** a conflict says "Conflict", a head behind "Update Needed"
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should disable Merge with the reason while the pull request waits for the approvals its base requires`)
  [@test] src/presenters/next-step.test.ts (`rows of a merge: Merge #7 and Auto-Merge #7 by the method, merge′ merges by itself, merge″ says why the merge waits, pr creates the pull request`)

### Requirement: The page: A rebase merge of GitHub is followed by an update that drops the originals

The fake GitHub of the tests merges a pull request as a fast-forward of the remote (`Merger`); a rebase merge of GitHub
writes the copy again (its author, time and message kept, GitHub the committer), and the update after it MUST drop the
originals all the same, naming them as landed.

#### Scenario: The update after a merge by rebase
- **GIVEN** GitHub wrote the copy again by a rebase merge, its author, time and message kept, GitHub the committer
- **WHEN** the update after it runs
- **THEN** it drops the originals all the same, naming them as landed
  [@test] crates/operations/src/update_test.rs (`a_merge_by_rebase_is_followed_by_an_update_that_drops_the_original_whose_copy_github_wrote_again`)

### Requirement: The page: A pull request merged into a parent's branch offers Create Pull Request…

A branch whose pull request GitHub merged into another branch than master, a parent's virtual branch, MUST offer Create
Pull Request… into master in its slot, Merge disabled with why ("#7 was merged into user-name/virtual/core, not master:
create a pull request into master to land it"), and the dialog MUST create it.

#### Scenario: Merged into a parent's branch
- **WHEN** GitHub merged the pull request of a branch into a parent's virtual branch
- **THEN** its slot offers Create Pull Request… into master, Merge disabled with "#7 was merged into user-name/virtual/core, not master: create a pull request into master to land it"
- **AND** the dialog creates it
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should offer Create Pull Request… into master when GitHub merged the pull request into a parent's branch, saying why, and create it`)

### Requirement: The page: A merged pull request the branch did not land by is no live one

The merged pull request into a parent's branch MUST be no live one a new one would repeat, as nothing of the branch
landed: only one by which the branch landed is.

#### Scenario: A new pull request repeats a merged one
- **WHEN** the dialog creates a pull request of a branch whose pull request was merged
- **THEN** it may repeat the merged one while the branch did not land by it, as into a parent's branch
  [@test] src/lib/pull-request-dialog.test.ts (`lets a new pull request repeat a merged one only while the branch did not land by it, as into a parent's branch`)

### Requirement: The page: Settings → Landing names the route

Settings → Landing MUST name the route of the selected repository ("Land by": Safe Push · Push · Pull Request) and
where it comes from, on the tab of the repository only: a route is never written to the global config
([Settings](../../repository/settings/spec.md#landing)).

#### Scenario: The route chosen in Settings
- **WHEN** the user chooses Push in "Land by" on the tab of the repository
- **THEN** the repository lands by a push into master, the main window taking the step
- **AND** the default comes back once the choice is removed
  [@test] tests/e2e/settings/landing-settings.spec.ts (`should land the repository by a push into master once chosen, the main window taking the step, and back to the default`)
