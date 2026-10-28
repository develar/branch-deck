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
  - ../../crates/domain-policy/src/land.rs
  - ../../crates/domain-policy/src/land_direct.rs
  - ../../crates/domain-policy/src/landed.rs
  - ../../crates/domain-policy/src/next_step.rs
  - ../../crates/domain-policy/src/landing_node.rs
  - ../../crates/domain-policy/src/actions.rs
  - ../../crates/domain-types/src/landing.rs
  - ../../crates/sync-core/src/baseline_push.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/operations/src/land.rs
  - ../../crates/operations/src/land_execute.rs
  - ../../crates/operations/src/land_session.rs
  - ../../crates/operations/src/engine.rs
  - ../../crates/operations/src/witness.rs
  - ../../crates/repository-model/src/derive.rs
  - ../../crates/repository-model/src/facts.rs
  - ../../crates/repository-model/src/session.rs
  - ../../crates/bd-remote/src/land_command.rs
  - ../../crates/bd-remote/src/status.rs
  - ../../crates/sync-core/src/grouping.rs
  - ../../src/presenters/next-step.ts
  - ../../src/presenters/land.ts
  - ../../src/presenters/actions.ts
---

# Landing Routes

Status: Draft (implemented: the route, its facts and its gates, Push to master with its step, its actions and `bd land`; the merge of a pull request is not)
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
  not lose Safe Push to a missing key. Patronus named off Space must be refused (`notOnSpace`). One function names the
  default (`LandingSetting::default_for`), for the rule and for Settings, whose read of the repository's settings
  tells it with its reason (`RepositoryConfig.landingDefault`: the route and whether `origin` pushes to Space) from the
  push URL of `origin` it reads for the detected GitHub repository, no process of its own.
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`the_setting_names_the_route_and_unset_it_follows_the_host`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_default_route_follows_the_host_of_origin_in_the_one_read_of_its_push_url`)

### What a sync reads for it

- A sync must read the route's facts of git without a process of its own: the setting with its config, and the push
  URL of each remote from its listing of the remotes (`git remote -v`, one process as `git remote` was, with
  `pushInsteadOf` applied). It sends them in `BranchesGrouped` (`landing`): the setting, whether `origin` pushes to
  Space, the github.com repository the remote of the baseline pushes to, the branch of the baseline there (`master` of
  `origin/master`) and its remote (`origin`; none of both for a local baseline). The session keeps them in the header
  of its view. The grouping of `bd status` reads them from the same listing (`BranchGrouping.landing`).
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
  reason (`Reason`, worded by the page in `src/presenters/reasons.ts` and by `bd` in `refusal_words.rs`). The view of the
  session carries it as a part of its own (`RepositoryView.route`), patched when it changes.
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`the_facts_of_the_session_read_as_the_rule_takes_them`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_push_into_the_target_lands_its_subject_and_a_refusal_of_the_server_stays_for_the_gate`)
  [@test] ../../src/presenters/reasons.test.ts (`words every reason of the catalog Rust writes as the page says it`)
- A push must be refused below write access (`noWriteAccess`), for rules no unsigned copy passes
  (`baselineRequiresSignatures`), and for a writer by a rule of a pull request, of the merge queue or of restricted
  updates (`baselineRequiresPullRequest`, `baselineRequiresMergeQueue`, `baselineRestrictsUpdates`); a maintainer or an
  admin, whom a ruleset may let bypass them (the API doesn't tell), confirms them instead, and so does a user GitHub
  named no permission of. Required status checks, which a push lands without, are a confirmation
  (`baselineRequiresStatusChecks`). A local baseline has no remote branch to push into (`localBaseline`).
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`a_push_is_gated_by_the_write_access_and_the_rules_of_the_target`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)
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

### Push to master

- One intent lands a subject on every route (`Intent::Land`): its plan decides by the route of the repository as the
  session holds it (`domain_policy::land::decide`, the dispatcher; `land_direct` the tail of a push), both tails reading
  the part of the subject of `land` (what lands, the table, the robots, the landing gate, the target). On the push
  route the start is one push of the copy of the subject into the target, a fast-forward (`PlannedEffect::PushBaseline`):
  the tip of the virtual branch of a piece (the branch with what it is built on, more than its row a confirmation), of a
  whole stack its stack branch, only once the sync built it (`stackNotBuilt`). The route is in the fingerprint of the
  plan: a setting changed between the click and the run is `plan_changed`.
  [@test] ../../crates/domain-policy/src/land_direct_test.rs (`a_piece_lands_by_a_push_of_its_copy_into_the_target_and_says_it_lands_more_than_its_row`, `a_whole_stack_pushes_its_stack_branch_only_once_the_sync_built_it`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)
- A push must be refused, in this order: a Dry Run (a robot of Patronus, `dryRunNeedsPatronus`), the refusal of the gate,
  what the table refuses (not built, parked), what landed it whole, a robot on its commit or one that lands it, what the
  landing gate refuses unless allowed, a conflict with the newest target; it waits for the facts of the gate, the copy,
  the pull requests GitHub lists and the landing gate, never reading one as "no".
  [@test] ../../crates/domain-policy/src/land_direct_test.rs (`the_gate_of_the_route_refuses_waits_or_asks_and_a_dry_run_needs_patronus`, `the_subject_refuses_in_the_order_of_the_table_what_landed_it_the_landing_gate_and_the_target`)
- A push runs no CI: what CI did not pass on the commit it lands must be confirmed (`unchecked`: no open pull request,
  one that shows another commit, failing or running checks, GitHub listing none; a commit without checks asks nothing),
  so must the changes requested on its pull request and what the gate asks (`routeRule`); `bd land` accepts them only by
  `--without-checks` and `--bypass-rules`.
  [@test] ../../crates/domain-policy/src/land_direct_test.rs (`the_checks_that_did_not_pass_on_the_tip_and_the_changes_requested_are_confirmations`, `bd_accepts_the_checks_and_what_the_gate_asks_only_by_their_flags`)
- The push must go by the first push URL of the remote of the baseline that its plan names
  (`PlannedEffect::PushBaseline.remote`, whose remote-tracking ref of the target the start reserves; a baseline that
  tracks another remote or branch by the time it runs pushes nothing), `<tip>:refs/heads/<target>`, without `+` and
  without a lease, after the size check, with the configured remote of the URL cleared so its remote-tracking ref stays
  (only the fetch of the baseline writes it); the witness under the repository lock holds the tip of the copy, the
  remote-tracking ref of the baseline the copies were built on and the checked out branch at the HEAD the sync grouped.
  [@test] ../../crates/sync-core/src/baseline_push_test.rs (`a_copy_built_on_the_target_lands_as_a_fast_forward_and_the_tracking_ref_stays`, `a_baseline_that_tracks_another_remote_than_the_plan_named_pushes_nothing`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_push_into_the_baseline`)
  [@test] ../../crates/operations/src/land_execute_test.rs (`a_push_into_the_target_lands_the_copy_as_a_fast_forward_and_tells_what_it_landed`)
- A target that moved must refuse the push as no fast-forward: the start fetches the baseline (`fetch_baseline`, joining
  a fetch that runs), asks the sync that builds the copy anew, and fails as `baseline_moved`, pushing nothing again by
  itself. A refusal of the server (`[remote rejected]`: a hook, a protected branch) must fail it as `baseline_refused` in
  the server's words, which the session keeps: the gate asks about them before the next push
  (`baselineRefusedPush`), and a push that lands forgets them.
  [@test] ../../crates/sync-core/src/baseline_push_test.rs (`a_target_that_moved_refuses_the_copy_as_no_fast_forward`, `the_server_refusing_the_push_is_said_in_its_words_and_a_large_push_never_goes`)
  [@test] ../../crates/operations/src/land_execute_test.rs (`a_target_that_moved_is_fetched_and_synced_and_the_push_is_baseline_moved`, `a_refusal_of_the_server_is_baseline_refused_in_its_words_and_goes_into_the_facts`)
  [@test] ../../crates/domain-policy/src/landing_route_test.rs (`a_refusal_of_the_server_in_this_session_is_asked_before_the_next_push`)
- A push that landed must land its subject at once, each branch of what it landed at the tip it pushed, until a fetch
  shows it (`landed_state`: "Landed in origin/master"): a second click pushes nothing. Then Update Project with its fetch
  follows as an operation of its own (`land_direct::update_after`, run by the engine), whose replay drops the originals
  whose copies landed; the caller of the start hears its steps and its outcome on the channel of the start, after the
  start's own (the toast of the push tells how the update went), and the answer of the start comes after it; `bd land`
  names `bd update` instead.
  [@test] ../../crates/domain-policy/src/landed_test.rs (`a_push_of_this_session_landed_the_subject_at_the_tip_it_pushed`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_push_into_the_target_lands_its_subject_and_a_refusal_of_the_server_stays_for_the_gate`)
  [@test] ../../crates/domain-policy/src/land_direct_test.rs (`the_update_follows_only_a_push_into_the_target_that_landed`)
  [@test] ../../crates/operations/src/update_test.rs (`a_push_into_the_target_is_followed_by_an_update_of_its_own_that_drops_the_original_whose_copy_landed`)
  [@test] ../../crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`)
- The next step of a ready subject must be "Push to master" (the row `push` of
  [Next Step](next-step.spec.md#a-push-into-the-target)), its node ready, the user's move, and its action
  `push-to-baseline` of a branch and of a whole stack that forks, none of the robot's starts; a refusal of the gate
  disables it with its reason and the node waits.
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`a_push_into_the_target_is_the_step_of_its_route_and_its_gate_holds_it_back`)
  [@test] ../../crates/domain-policy/src/landing_node_test.rs (`a_push_into_the_target_is_ready_and_a_refusal_of_its_gate_waits`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_push_into_the_target_offers_its_push_with_the_reason_of_its_step_and_no_start_of_the_robot`)
  [@test] ../../src/presenters/next-step.test.ts (`row push: Push to master by the branches it lands, its start disabled with the refusal of the gate of the route`)
- `bd land` must land by the same plan ([Agent CLI](../cli/agent-cli.spec.md#bd-land)), its sync and its trial with no
  fetch and no check against the newest target (a push is its own), and `bd status` must decide the steps by the same
  route over what its grouping read of git, GitHub not asked: the gate of a push into a repository on GitHub waits.
  [@test] ../../crates/bd-remote/src/land_test.rs (`a_moved_target_is_baseline_moved_and_a_refusal_of_the_server_baseline_refused`, `bd_status_decides_the_steps_by_the_route_of_the_repository`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_land_check_by_a_push`, `a_land_by_a_push`)
