---
name: Landing Routes
description: Requirements for the route of a repository into master, a robot of Patronus, a push of the copy into the target or a merge of the pull request on GitHub, named by `branchdeck.landing` or by the host of `origin`, and the gate of each route by the write access and the rules of the target.
style: plain-1
targets:
  - ../../crates/domain-types/src/landing_route.rs
  - ../../crates/domain-types/src/reason.rs
  - ../../crates/domain-types/src/pull_request.rs
  - ../../crates/domain-types/src/session.rs
  - ../../crates/domain-types/src/lib.rs
  - ../../crates/domain-policy/src/landing_route.rs
  - ../../crates/sync-core/src/repository_config.rs
  - ../../crates/sync-core/src/config_snapshot.rs
  - ../../crates/sync-core/src/repo_facts.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/remote-facts/src/branch_rules.rs
  - ../../crates/remote-facts/src/pull_requests.rs
  - ../../crates/repository-model/src/pull_request_facts.rs
  - ../../crates/repository-model/src/runtime.rs
  - ../../crates/repository-model/src/table.rs
  - ../../crates/repository-model/src/state.rs
  - ../../crates/repository-model/src/patch.rs
  - ../../crates/repository-model/src/refusal_words.rs
  - ../../src/presenters/reasons.ts
  - ../../src/model/patch.ts
  - ../../src/lib/repository-config.ts
---

# Landing Routes

Status: Draft (implemented: the route, its facts and its gates; the steps, the actions and `bd land` that follow it are not)
Date: 2026-10-28

## Summary

The work of a branch is done when it is in master ([Landing Work into Master](../../docs/design/landing.md)). In the
IntelliJ monorepo it gets there by a Safe Push of Patronus. Elsewhere Branch Deck had no way into master: a branch
outside Space had no next step, and an approved green pull request only waited.

A repository now has a route into master, one of three: a robot of Patronus, a push of the copy of the branch into the
branch of the baseline on its remote (a fast-forward of `master`), or a merge of its pull request on GitHub. The unit
of landing is a robot, a push or a merge. The route is a fact of the repository, named by its git config or by the
host of `origin`, and each route has a gate: what GitHub tells of the user's write access and of the rules of the
target, read before anything is pushed.

## Goals

- Name the route of a repository once, in its git config, read by the app and `bd` alike.
- Keep the monorepo on Safe Push without a setting, and give every other repository a route.
- Tell before a push whether the user may push into the target or merge there, from facts GitHub already gives.
- Never read a fact without an answer as "no".

## Non-goals

- Detect a push route. A push into master skips the robot and the CI of a pull request: only the user names it.
- Read classic branch protection. It needs admin rights; the server refuses a push it forbids.
- Judge a remote other than GitHub. Off GitHub (a GitLab remote, a mirror) there is no gate: the server answers.

## Requirements

### Setting

- The route must be `branchdeck.landing` of the git config, `patronus`, `push` or `pull-request` (any case, trimmed),
  read with the other keys of Settings → Repository in one `git config` and in the one read of an operation
  (`ConfigSnapshot`); an empty value is unset, another value is invalid with its reason, and an operation refuses it as
  it refuses a wrong `branchdeck.stacks`. A change goes through `set_repository_config` like the other keys.
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset`)
  [@test] ../../crates/sync-core/src/config_snapshot_test.rs (`the_route_comes_from_git_config_and_is_none_without_it`)
  [@test] ../../crates/domain-types/src/landing_route_test.rs (`the_setting_is_read_as_git_keeps_it_and_written_back_the_same`)
- `push` must be taken only from the repository's own config (`.git/config`, the worktree config, `git -c`): a `push`
  of the global or the system config is unset, with the reason, for Settings and for every operation, and a global
  write of it is refused. A push into master of every repository of the user that names no route would be a trap.
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset`, `a_push_route_is_written_only_to_the_local_config`)
  [@test] ../../crates/sync-core/src/config_snapshot_test.rs (`a_push_route_of_the_global_config_is_none`)
- Unset, the route must be Patronus when the push URL of `origin` is on Space (`git.jetbrains.team`), and the pull
  request elsewhere; never a push. This is an exception to "a rule changes only by an explicit value"
  ([Agent Workspace](../cli/workspace.spec.md)): the host of `origin` does not change by itself, and the monorepo must
  not lose Safe Push to a missing key. Patronus named off Space must be refused (`notOnSpace`).
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`the_setting_names_the_route_and_unset_it_follows_the_host`)

### What a sync reads for it

- A sync must read the route's facts of git without a process of its own: the setting with its config, and the push
  URL of each remote from its listing of the remotes (`git remote -v`, one process as `git remote` was, with
  `pushInsteadOf` applied). It sends them in `BranchesGrouped` (`landing`): the setting, whether `origin` pushes to
  Space, the github.com repository the remote of the baseline pushes to, and the branch of the baseline there (`master`
  of `origin/master`; none for a local baseline). The session keeps them in the header of its view.
  [@test] ../../crates/sync-core/src/repo_facts_test.rs (`the_remotes_come_with_their_push_urls`, `the_landing_remote_is_read_with_the_facts_of_a_sync`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_push_route_reads_the_rules_of_the_target`)

### Facts of GitHub

- What the token may do in the repository (`viewerPermission`: `ADMIN`, `MAINTAIN`, `WRITE`, `TRIAGE`, `READ`) must come
  from the listing of the pull requests, the same query, no request of its own. It goes with the listing: kept through
  a failure GitHub recovers from, none without a listing to stand on, never "no access"; with it the repository of the
  listing. A patch carries both with the head of the answer.
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`, `a_token_without_read_org_is_asked_again_without_teams`)
  [@test] ../../crates/repository-model/src/pull_request_facts_test.rs (`the_permission_of_the_token_goes_with_the_listing`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)
  [@test] ../../crates/domain-types/src/landing_route_test.rs (`a_permission_of_github_writes_from_write_and_bypasses_from_maintain`)
- The rules of a branch (`GET rules/branches/<branch>`, the active rulesets) must tell what a push into it meets: a pull
  request (`pull_request`), signed commits (`required_signatures`), status checks (`required_status_checks`), the merge
  queue (`merge_queue`), restricted updates (`update`), besides the dismissal of stale reviews.
  [@test] ../../crates/remote-facts/src/branch_rules_test.rs (`the_rules_a_push_into_the_branch_meets_are_found_by_their_types`, `a_pull_request_rule_that_dismisses_stale_reviews_on_push_is_found_among_the_rules_of_the_branch`)
- The session must read the rules of the target while the route is a push into a repository on GitHub, besides those
  of the bases of the approved pull requests, once per base per session as those.
  [@test] ../../crates/repository-model/src/session_test.rs (`a_push_route_reads_the_rules_of_the_target`)
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`the_rules_of_the_target_are_read_only_for_a_push_on_github`)

### Gates

- One rule must decide the route and its gate for the app and `bd` (`domain_policy::landing_route::route`): open, a
  wait for a fact (with the reason of a read that failed), confirmations the user accepts, or a refusal with its
  reason (`Reason`, worded by the page in `src/presenters/reasons.ts` and by `bd` in `refusal_words.rs`).
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`the_facts_of_the_session_read_as_the_rule_takes_them`)
  [@test] ../../src/presenters/reasons.test.ts (`words every reason of the catalog Rust writes as the page says it`)
- A push must be refused below write access (`noWriteAccess`), for rules no unsigned copy passes
  (`baselineRequiresSignatures`), and for a writer by a rule of a pull request, of the merge queue or of restricted
  updates (`baselineRequiresPullRequest`, `baselineRequiresMergeQueue`, `baselineRestrictsUpdates`); a maintainer or an
  admin, whom a ruleset may let bypass them (the API doesn't tell), confirms them instead, and so does a user GitHub
  named no permission of. Required status checks, which a push lands without, are a confirmation
  (`baselineRequiresStatusChecks`). A local baseline has no remote branch to push into (`localBaseline`).
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`a_push_is_gated_by_the_write_access_and_the_rules_of_the_target`)
- A gate must wait for the permission and for the rules while they have no answer, and while their read failed, with
  the reason; never "no". The permission is read first: below write access no rule is waited for.
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`a_push_waits_for_the_facts_of_github_and_never_reads_a_missing_one_as_no`)
- The facts of GitHub must count only for the repository the remote of the baseline pushes to (compared without case):
  a push to another host, to a repository the listing doesn't describe, or off GitHub has no gate, and the server
  answers.
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`a_push_elsewhere_than_the_repository_of_the_facts_has_no_gate`)
- A merge of the pull request must need write access to the repository `origin` pushes to; without it the route
  stays and its merge is refused (`noWriteAccess`: others merge, the step waits as today). A pull request of a mirror
  `origin` doesn't push to merges nothing into its target (`gitHubMirror`), and off GitHub there is none
  (`notOnGitHub`).
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`a_merge_needs_write_access_to_the_repository_origin_pushes_to`)
