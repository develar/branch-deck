# Code Map

**Status:** Active  
**Updated:** 2026-10-29  
**Tags:** [features, specs, crates, rules]

## Summary

Where each feature, rule and module lives: its spec, its entry code, the rule files of `domain-policy` and the
crates. [AGENTS.md](../../AGENTS.md) ("Where Things Are") keeps the short index and links here; the `//!` header
of each crate's `lib.rs` and of each rule file says the rest.

## Feature Index

Each feature's contract is its spec (paths under `spec/`, without `.spec.md`); read it before changing the feature.

| Feature | Specs | Entry code |
|---|---|---|
| Repository session: the table, the syncs, the remote tips and the facts of the remote services (robots, pull requests, readiness, review requests, read by the session's scheduler) of a worktree and branch prefix live in Rust; a window subscribes to its snapshot and patches, asks `sync` and `refresh_facts`, tells its focus and its demand (`set_demand`); `bd sync` folds its sync through the same `TableState`. The warm start: the frame of a session's last sync in the object cache, seeded before the main window is built and injected as `window.__BD_MODEL__` beside `__TAURI_STORE__`, its facts `Cached` and its remote actions `Disabled{Cached}` until the first sync | `repository/onboarding`, `branch/branch-review` ([Session facts](../design/git-operations.md#session-facts), [Warm start](../design/git-operations.md#warm-start), [Rendering Performance](../design/rendering-performance.md#loading)) | `crates/repository-model/src/session.rs`, `crates/repository-model/src/table.rs`, `crates/repository-model/src/facts.rs`, `crates/repository-model/src/warm_start.rs`, `crates/remote-facts/src/scheduler.rs`, `crates/app-api/src/session/view.rs`, `src-tauri/src/services.rs`, `src-tauri/src/warm_start.rs`, `src/model/store.ts` (`adoptCachedModel`), `src/model/demand.ts` |
| Integration status: archive the branches the baseline has (by default rebase/cherry-pick detection only; merge and squash by an environment variable) | `branch/branch-row` ([Integration Detection](../design/integration-detection.md)) | `crates/branch-integration/` |
| Context menus (the "⋯" menu on right-click, acting on the selection) | `branch/branch-row`, `commit/commit-row` | `crates/domain-policy/src/actions.rs` (the actions of a branch, a whole stack, a robot and a pull request, each enabled or disabled with its reason), `crates/repository-model/src/action_view.rs` (`RepositoryView.actions`), `src/presenters/actions.ts` (their words and looks), `src/hooks/useBranchContextActions.ts`, `src/components/shared/ContextMenuItems.tsx` |
| AI branch naming (`claude -p --model haiku`, after consent) | `branch/branch-name-suggestions` | `crates/sync-core/src/branch_name_suggestion.rs`, `src/components/ai/` |
| Squash and fixups (`bd squash`, `bd fold`) | `commit/squash`, `commit/fixups`, `cli/agent-cli` | `crates/operations/src/rewrite.rs` (the intents `Rewrite(Squash)`, `Rewrite(SquashFixups)`), `crates/history-rewrite/src/squash.rs`, `crates/history-rewrite/src/autosquash.rs`, `crates/bd-rewrite/src/squash_command.rs` |
| Message of a commit (`bd reword`; the app's edit of a message) | `cli/agent-cli`, `commit/commit-row` | `crates/operations/src/rewrite.rs` (the intent `Rewrite(Reword)`), `crates/history-rewrite/src/reword.rs`, `crates/history-rewrite/src/fixup_targets.rs`, `crates/bd-rewrite/src/reword_command.rs`, `src/hooks/useRewordCommit.ts` |
| Update Project (the intents `UpdateProject`, `FetchBaseline`, `DismissLanded`; `bd update`, resolve sessions, `bd undo update:`) | `repository/update-project`, `cli/update` | `crates/operations/src/update.rs`, `crates/update-core/src/update_project.rs`, `crates/update-core/src/resolve_session.rs`, `crates/git-ops/src/update_branch.rs`, `crates/git-ops/src/update_resolve.rs`, `src/stores/update-project.ts`, `src/lib/update-progress.ts` |
| Pick between worktrees (`bd pick --from <worktree> --follow`); the commits someone else pushed to a remote virtual branch (`bd pick --remote`, Pick Remote Commits) | `cli/pick`, `repository/outgoing` | `crates/operations/src/rewrite.rs` (the intent `Rewrite(PickRemote)` of Pick Remote Commits; `Pick` only records a run of `bd`), `crates/update-core/src/pick.rs`, `crates/git-ops/src/pick.rs`, `crates/sync-core/src/moved_remote.rs`, `crates/repository-model/src/moved_remote.rs`, `src/lib/git/pick-remote.ts` |
| Agent workspace (`bd workspace`): a lease and the `(branch)` subject by `branchdeck.agentLease` and `branchdeck.agentSubjectBranch`, Settings → Agents | `cli/workspace`, `repository/settings`, `repository/onboarding` | `crates/lease-pool/src/agent_workspace.rs`, `crates/bd-lease/src/workspace_command.rs`, `src/components/settings/AgentsPage.tsx` |
| Command-line tools: the `teamcity` and `youtrack` CLIs in Settings → Tools, installed and updated through Homebrew, winget or scoop, with their agent skills; the Setup dot on Settings (updates, expiring sign-ins) | `repository/cli-tools`, `repository/setup-attention`, `repository/settings` | `crates/cli-tools/`, `crates/sync-utils/src/known_tools.rs`, `crates/app-api/src/app/cli_tools.rs`, `src/components/settings/ToolsPage.tsx` |
| Landing routes: the route into master (`branchdeck.landing`: Patronus, a push into the target, a merge of the pull request; unset by the host of `origin`) and its gate of the write access and the rules of the target; Push to master, Merge #7 and Auto-Merge #7 (`bd land`), Cancel Auto-Merge | `branch/landing-routes`, `branch/next-step`, `repository/settings`, `cli/agent-cli` ([Landing](../design/landing.md#routes)) | `crates/domain-policy/src/landing_route.rs`, `crates/domain-policy/src/land_direct.rs`, `crates/domain-types/src/landing_route.rs`, `crates/sync-core/src/repository_config.rs`, `crates/sync-core/src/repo_facts.rs` (`landing_remote`), `crates/sync-core/src/baseline_push.rs`, `crates/operations/src/land_execute.rs` (`push_baseline`, `merge`), `crates/operations/src/auto_merge.rs`, `crates/remote-facts/src/pull_request_writes.rs` (`merge_pull_request`, `enable_auto_merge`, `disable_auto_merge`), `crates/bd-remote/src/land_command.rs`, `crates/remote-facts/src/branch_rules.rs`, `crates/remote-facts/src/pull_requests.rs` (`viewerPermission`, `mergeStateStatus`) |
| Lease pool (`bd lease`), its clean rules (`.branchdeck.toml`), the host slots that `bd-host serve` lends in process | `cli/lease` | `crates/lease-pool/src/lease.rs`, `crates/lease-pool/src/lease_clean.rs`, `crates/lease-pool/src/host_slots.rs`, `crates/bd-lease/src/lease_command.rs`, `crates/git-ops/src/lease_slot.rs` (state in `$GIT_COMMON_DIR/branch-deck/leases.json`) |
| Remote runs on another Mac (`bd-host`, Draft): a command of a checkout in a warm host slot, the agent under launchd, the upgrade | `cli/host`, `cli/lease` ([Host Runs](../design/host-runs.md)) | `crates/bd-host/`, `crates/host-client/`, `crates/host-agent/`, `crates/host-link/` |
| Landed commits | `repository/landed-commits` | `crates/domain-policy/src/landed.rs` (what landed a subject), `crates/operations/src/update.rs` (the intent `DismissLanded`), `crates/sync-core/src/landed.rs` (report in `branch-deck/landed.json`), `src/stores/landed.ts` |
| Landing overview, PR status, Create Pull Request, Request Review, the push | `repository/landing-overview`, `repository/outgoing`, `branch/pull-requests` | `crates/domain-policy/src/pull_request.rs` (where a pull request goes, the rules of its base), `crates/domain-policy/src/review.rs` (the candidates of Request Review, where the approvals count), `crates/domain-types/src/remote_status.rs` (what a push does by the remote status), `crates/domain-policy/src/push.rs` (the push orders), `crates/domain-policy/src/attention.rs` (the robot that needs the user), `crates/operations/src/pull_request.rs` (the intents `CreatePullRequest`, `RequestReview`, `Retarget`, each with its push first; `bd request-review`), `crates/operations/src/close.rs` (`ClosePullRequest`, `CloseLandedPullRequests`), `crates/operations/src/push.rs` (the intent `Push` of a branch, a stack, up to a commit, an unapplied branch; its network part for `bd push`), `remote_facts::pull_requests`, `remote_facts::pull_request_create`, `remote_facts::pull_request_text`, `repository_model::pull_request_facts` (the pull requests, rules and profiles of the session), `crates/github/`, `src/lib/pull-request-run.ts`, `src/lib/push-run.ts`, `src/stores/pull-requests.ts` |
| Patronus: robots, Safe Push and Dry Run, the next step, the failure prompt (`bd safe-push`, `bd robot`) | `branch/patronus`, `branch/next-step`, `branch/safe-push-failure-prompt`, `branch/whole-stack`, `cli/agent-cli` | `crates/domain-policy/` (the next step, the holds, the start and what landed: the [rule index](#rule-index)), `crates/operations/` (the plan of a start and its run, one for the app and `bd`: `land.rs`, `land_execute.rs`, `land_session.rs`, `robot_start.rs`; the restart and the cancel of a robot: `robot.rs`), `crates/repository-model/` (the Patronus view and the landing needs the session derives: `facts.rs`; the facts a push and a robot action are checked on: `push_guard.rs`), `remote_facts::scheduler` (when the session reads each fact: the robots, their details, the pull requests, the readiness, the review requests), `crates/trial-check/src/landing_gate.rs` (the landing gate of a start), `crates/patronus/`, `remote_facts::review_readiness` (the code-owner pre-check), `remote_facts::patronus_command`, `src/presenters/next-step.ts`, `src/presenters/land.ts`, `src/presenters/outcome.ts` (the toast of a start's outcome), `src/presenters/actions.ts` (the items of a robot), `src/lib/robot-run.ts` (its restart and cancel), `src/hooks/useSafePush.ts`, `src/stores/review-readiness.ts` |
| Safe Push statistics, the Landing view of the main window (Branches · Reviews · Landing, ⌘1/⌘2/⌘3): the user's robots of a period (30 days by default) against everyone else's, time lost, failure causes, start hours (`bd robot-stats`) | `repository/safe-push-statistics`, `repository/main-views` | `crates/remote-facts/src/robot_stats.rs`, `crates/remote-facts/src/robot_stats_aggregate.rs`, `crates/patronus/src/stats.rs`, `crates/bd-remote/src/robot_stats_command.rs`, `src/components/landing/LandingView.tsx` |
| GitHub sign-in (OAuth device flow); the sign-ins of the app's accounts as commands of the app (one per provider, a second call joins it) and their status, the fact `auth` of every session | `repository/github-sign-in` | `crates/github/src/token.rs`, `crates/remote-facts/src/sign_in.rs`, `crates/app-api/src/app/sign_in.rs`, `src/stores/auth.ts`, `src/lib/sign-in-run.ts` |
| YouTrack sign-in (Hub OAuth in the browser, refresh token; `sign_in` of the app), the issue popover, Mark Fixed | `repository/youtrack` | `crates/operations/src/issue.rs` (the intent `SetIssueState` of Mark Fixed), `crates/youtrack/`, `crates/remote-facts/src/sign_in.rs`, `src/stores/auth.ts`, `src/stores/youtrack-issues.ts` |
| Issue groups | `branch/issue-groups` | `crates/operations/src/rewrite.rs` (the intents `Rewrite(Move)` and `Rewrite(Rename)`, whose regroup writes `refs/branch-deck/issue-groups`), `crates/history-rewrite/src/issue_groups.rs` |
| Conflict fix (`bd check --fix`) | `branch/conflict-fix`, `commit/commit-row` | `crates/operations/src/rewrite.rs` (the intent `Rewrite(ApplyConflictFix)`), `crates/trial-check/src/conflict_fix.rs` |
| Move to a branch (drag and drop, menus, keys) | `branch/move-commits` | `crates/operations/src/rewrite.rs` (the intents `Rewrite(Move)`, `Rewrite(Ungroup)`), `crates/history-rewrite/src/move_plan.rs` |
| Stacked branches, the stack island, the whole stack | `branch/stacked-branches`, `branch/stack-island`, `branch/whole-stack`, `branch/branch-row` | `crates/domain-policy/src/stack.rs` (parents, children, descendants, ancestors, main line), `repository_model::derive` (`BranchView.structure`, the push orders `RepositoryView.pushes`), `crates/operations/src/push.rs` (the pushes of a stack), `crates/sync-core/src/stack_branch.rs`, `src/stores/stack-view.ts`, `src/lib/push-run.ts` |
| Stack parents (`bd restack`, `bd unstack`) | `branch/stack-parents` | `crates/operations/src/rewrite.rs` (the intents `Rewrite(Restack)`, `Rewrite(Unstack)`), `crates/history-rewrite/src/stack_parents.rs` |
| Parked branches: kept local, never pushed, not Your Move (`bd park`, `bd unpark`) | `branch/parked-branches` | `crates/domain-policy/src/parked.rs`, `crates/operations/src/rewrite.rs` (the intents `Rewrite(Park)`, `Rewrite(Unpark)`), `crates/history-rewrite/src/parked.rs`, `crates/sync-core/src/parked.rs` |
| Unapply and Reapply: a branch and its files out of HEAD, kept as `{prefix}/unapplied/<name>` in the Unapplied card, and back on top (`bd unapply`, `bd reapply`, `bd push --unapplied`) | `branch/unapply` | `crates/operations/src/rewrite.rs` (the intents `Rewrite(Unapply)`, `Rewrite(Reapply)`, `Rewrite(DeleteUnapplied)`), `crates/sync-core/src/unapply_branch.rs`, `crates/sync-core/src/reapply_branch.rs`, `crates/git-ops/src/unapply.rs`, `crates/git-ops/src/reapply.rs`, `src/components/branch/UnappliedBranchTableCard.tsx` |
| Operations across processes: the operations registry of a repository (an operation of the remote that needs what another process runs is `busy`, naming that operation; the operations of `bd` on the rows of the app; the stamp after a mutation of `bd`), a start whose answer was lost (Start Unknown, Forget This Start, `bd operation forget`), one fetch of the baseline for two processes (`fetch.lock`) | `cli/agent-cli`, `branch/patronus`, `repository/update-project` ([Operations registry](../design/git-operations.md#operations-registry), [Running Operations](../design/operations.md)) | `crates/domain-policy/src/admission.rs`, `crates/operations/src/forget.rs` (the intent `ForgetStart`), `crates/operations/src/registry_file.rs`, `crates/operations/src/registry_watch.rs`, `crates/operations/src/reconcile.rs`, `crates/sync-core/src/fetch_lock.rs`, `crates/bd-remote/src/operation_command.rs`, `crates/bd-core/src/operation_record.rs`, `src/presenters/operations.ts` |
| Activity log, its side panel and its window: read from the journals (the operations of the app and `bd` in the journal of their repository, the page's records in the journal of the app data directory; `read_activity`, `record_activity`, `clear_activity`) | `repository/activity` | `crates/operations/src/journal.rs`, `crates/app-settings/src/journals.rs`, `crates/app-api/src/app/activity.rs`, `src/stores/activity.ts`, `src/presenters/activity.ts`, `src/components/repository/ActivityPanel.tsx`, `src/pages/ActivityPage.tsx`, `src/lib/activity-window-link.ts` |
| Commit files, diffs, the commit review (`D`), the tree-sitter highlighting | `commit/commit-files`, `commit/commit-review` ([Diff View](../design/diff-view.md)) | `crates/file-diffs/`, `crates/code-symbols/src/highlight/`, `src/components/diff/` |
| Go to Declaration and Find Usages in a review (⌘-click, ⌘B, ⌥F7, Back), outside it through accio | `commit/symbol-navigation` | `crates/code-symbols/src/navigate.rs`, `crates/file-diffs/src/declarations.rs`, `crates/file-diffs/src/usages.rs`, `crates/file-diffs/src/accio.rs`, `src/hooks/useSymbolNavigation.ts`, `src/components/diff/DeclarationFileView.tsx` |
| Review plan (experimental): Mark Reviewed, the reviewed pair of a branch in `refs/branch-deck/reviewed/` (`bd mark-reviewed`), Since Your Review, the interdiff of a branch since then, the API surface with its accio fan-in, the risk per file and the dependency order of the files (`bd review-plan`) | `branch/review-plan` | `crates/review-plan/` (`mark_reviewed_core`, `review_plan_core`, `interdiff.rs`, `api.rs`, `fan_in.rs`, `risk.rs`, `order.rs`), `crates/trial-check/src/declaration_checks.rs` (`changed_declarations`), `crates/trial-check/src/symbol_dependencies.rs` (`file_links`), `crates/git-ops/src/reviewed_refs.rs`, `crates/bd-rewrite/src/mark_reviewed_command.rs`, `crates/bd-rewrite/src/review_plan_command.rs`, `crates/app-api/src/reads/review_plan.rs` (`get_review_plan`), `crates/app-api/src/app/mark_reviewed.rs` (`mark_branch_reviewed`), `crates/sync-core/src/repository_config.rs` (`branchdeck.reviewExperimental`), the review window: `src/hooks/useBranchReview.ts` with `src/hooks/useReviewPlan.ts`, `src/lib/review-plan.ts` (Mark Reviewed), `src/lib/range-review.ts` (`localSinceReview`), `src/components/review/ReviewPlanCard.tsx`, `src/presenters/review-plan.ts` |
| Command palette (⌘K: the views, Activity, the commands of the app, Go to a branch) | `repository/command-palette` | `src/components/command-palette/CommandPalette.tsx`, `src/lib/command-palette.ts` |
| Title bar and main menu | `repository/title-bar` | `src-tauri/src/title_bar.rs`, `src/components/shared/WindowTitleBar.tsx` |
| Your Turn (review requests, the pull request review) | `repository/review-requests`, `repository/robots-indicator` | `crates/domain-policy/src/news.rs` (the news of a session: review requests, approvals), `crates/operations/src/review.rs` (the intent `SubmitReview`), `crates/operations/src/review_fetch.rs` (`FetchReview`), `remote_facts::review_requests`, `repository_model::review_request_facts` (the review requests of the session, their news), `src/components/review/`, `src/stores/review-requests.ts` |
| Reviews, the second view of the main window (⌘2): every open pull request the user takes part in on GitHub (the teams' requests, the reviewed, the own), each once, and a pull request added by its link; read only while it shows | `repository/reviews`, `repository/main-views` | `crates/domain-policy/src/reviews.rs`, `remote_facts::reviews`, `repository_model::review_view` (the sections and the `pull:<n>` actions), `app_settings::linked_pull_requests`, `src/components/reviews/ReviewsView.tsx`, `src/presenters/reviews.ts`, `src/stores/reviews.ts` |
| Settings window; the settings of the app (one writer, versions, the branch prefix resolved in Rust) | `repository/settings` | `crates/app-settings/` (`SettingsService`, the one writer), `crates/sync-core/src/repository_config.rs`, `crates/app-api/src/app/settings.rs`, `src/stores/settings/persistent-store.ts`, `src/components/settings/` |
| Branch review | `branch/branch-review` | `crates/git-ops/src/ownership.rs`, `crates/app-api/src/remote/branch_review.rs`, `src/components/review/BranchReview.tsx` |
| Symbol dependencies | `commit/symbol-dependencies` ([Symbol Dependencies](../design/symbol-dependencies.md)) | `crates/code-symbols/`, `crates/trial-check/src/symbol_scan.rs` |
| Zoom | `repository/zoom` | `src-tauri/src/zoom.rs` |

The rows and the menus of the main window: `branch/branch-row`, `commit/commit-row` (start there). The `bd` commands:
`cli/agent-cli` and [Agent CLI](agent-cli.md). The `branch-deck` agent skill is generated by
`skill_text()` (`crates/bd-cli/src/agent_help.rs`) and installed by `bd install`; the repository has no skill file.

## Rule Index

Every rule of the domain is one file of `crates/domain-policy/src/` (a `//!` header names the rule and its spec, whose
`targets:` list it, `pnpm lint:specs`; a table test `<rule>_test.rs` beside it), a pure function of `domain-types` that
the app and `bd` both call: the session derives its answer into a part of the view (`repository_model::derive`, [The
landing of a session](../design/git-operations.md#the-landing-of-a-session)), which the page only words
(`src/presenters/`), and `bd` decides by the same function. A rule never goes into `src/lib` or `src/hooks`
(`pnpm lint:arch`, `rule-module`).

| Rule | File | Its answer in the view; in `bd` |
|---|---|---|
| The table as the rules read it, borrowed from the view or a grouping | `facts.rs` (`Repository`) | `derive::repository`; `bd`: `derive::grouped_rows` |
| The robot of a branch and of a whole stack | `robot_match.rs` | `robotId` (`facts::derive_patronus`); `bd`: the same of its one-shot session (`one_shot::OneShot`) |
| What keeps a branch out of a push, the push orders and their verdicts (Push Stack, the button, Push with Children and its stack branch, Push Up to Here) and what a start pushes (`start_push`) | `push.rs` | `RepositoryView.pushes` (`derive::push_view`), the `Push` effect of `land::decide`, the plan of the intent `Push` (`operations::push`); `bd push`, `bd safe-push` |
| The declared parent (`(parent/child)`, the stack parents), children, descendants, ancestors by effective base, main line, the whole stack and what is left of it | `stack.rs` | `BranchView.structure` (`derive::place`), `StackLanding.rest`; `bd status`, `bd safe-push --whole-stack`, `bd push --with-children` |
| Parked: the branch and what is built on it stay local | `parked.rs` | `BranchStructure.parked`; `bd push`, `bd safe-push` |
| The robot that needs the user: the failed robot of a branch or a whole stack as it is that nothing else holds (the landing overview, Attention, the Dock badge) | `attention.rs` | `RepositoryView.attention` (`derive::attention_of`); `bd`: none |
| What holds a push (a running Safe Push, a landed branch), the push an action can't make, what a restart does | `holds.rs` | `BranchPatronus.landing`, `StackPatronus.landing`, `PatronusView.restarts` (`derive::decide_landing`); `push_guard`, `operations::robot`, `bd push` |
| The start of a Safe Push or a Dry Run: its refusal, blockers, confirmations, waits, effects (retarget, cancel, push, start), its profile; the dispatcher of `decide` by the route of the repository, and the part of the subject both tails read | `land.rs` | `RepositoryView.lands` (`derive::lands_of`), the plan of a start (`prepare`, `operations::land`); `bd safe-push` (`prepare_one_shot`) |
| The start of a repository that lands by a push of the copy into the target: its blockers, the confirmations of what CI did not check and of the gate of the route, the push `PushBaseline`; of one that lands by a merge of the pull request on GitHub what GitHub tells of it (`merge_of`, the plan and the next step), the merge `MergePullRequest` or the auto-merge `EnableAutoMerge`; the update that follows a push that landed and a merge | `land_direct.rs` | `RepositoryView.lands`, the plan of a start; the engine's update after it (`engine::execute_land`); `bd land` |
| What landed a subject (the baseline, merged Safe Pushes, a merged pull request), which pull requests a merge closes | `landed.rs` | `RepositoryView.landedStates`; `bd status` (`landed_by`), `bd robot` (`closes`), `bd safe-push` (`landed`) |
| Where a pull request goes (the target, a virtual branch, another base), the pull request a robot starts from | `pull_request.rs` | `RepositoryView.openPullRequests`, the readiness the session asks; `bd safe-push` |
| The code-owner review: whom Request Review asks, what the readiness means, where the approvals count | `review.rs` | `PullRequestView.candidates`; `bd request-review`, `bd safe-push` |
| Whether a subject waits for approvals, which shown subjects the session asks | `readiness.rs` | `derive::wanted_readiness` |
| The running Safe Push that lands a missing declaration of the landing gate | `landing_needs.rs` | `BranchPatronus.landingNeeds` |
| The route of a repository into master (a robot, a push into the target, a merge of the pull request) by `branchdeck.landing` or the host of `origin`, and its gate of the write access and the rules of the target; the branch whose rules the session reads for it | `landing_route.rs` | the facts it reads: `TableHeader.landing`, `PullRequestsView.viewerPermission`, `FactsView.rules`; `bd`: none yet |
| The next step of a branch and of a whole stack | `next_step.rs` | `RepositoryView.nextSteps` (`derive::next_steps`); `bd status`, `bd safe-push --check` (`next_step`) |
| The node of a row and of a stack header | `landing_node.rs` | `NextStepView.node` |
| The actions of a branch, a whole stack, a robot and a pull request to review, each enabled or disabled with its reason | `actions.rs` | `RepositoryView.actions` (`action_view::actions_of`, `review_view::review_actions_of` by `pull:<n>`); `bd`: none yet |
| The sections of Reviews, each pull request the user takes part in once, the links that go | `reviews.rs` | `RepositoryView.reviews` (`review_view::reviews_view`); `bd`: none yet |
| The news of a session (a robot merged or failed, approvals arrived, review requests) | `news.rs` | `RepositoryView.news` |
| When a fact of the remote is asked again | `refresh.rs` | `remote_facts::scheduler` |
| How a new operation of the remote is admitted: in its session it joins the same intent, waits behind the newest one of its process that holds a resource it needs, runs beside the rest; across processes the record of another one that holds what it needs makes it busy (`holds_across`, over the registry file) | `admission.rs` | `RepositoryView.operations` (`operations::registry`); `bd`: `holds_across` (`ProcessRegistry::admit`) |
| The reasons the rules give: codes with their facts, no words | `reasons.rs` (types: `domain_types::reason`) | words: `src/presenters/reasons.ts`, `repository_model::refusal_words::start_refusal_text`, `bd-core/src/next_step_summary.rs` |

Outside `domain-policy`: the grouping of the commits (`crates/history-rewrite/src/commit_grouper.rs`), what a robot
landed and spans by content (`crates/sync-core/src/robot_landing.rs`, a fact of git), the Safe Push statistics
(`crates/remote-facts/src/robot_stats_aggregate.rs`), and what a push of one branch does by its remote status alone
(`crates/domain-types/src/remote_status.rs`: `RemoteStatusUpdate::push_action`, stored as `RemoteStatusUpdate::action`,
`needs_push`, `drops_remote_commits`, `pushes_again`; the sync sets it, and the crates of git reach no rule). The plan and its stages: [Rust as the Domain
Authority](../design/rust-domain-authority.md#индекс-правил).

## File Structure

```
src/
├── components/
│   ├── ui/          # shadcn/ui components (generated, some locally customised)
│   ├── layout/      # Layout primitives (Container, FieldGroup)
│   ├── shared/      # Shared app components (cards, inline forms, copy button, context menus, title bar)
│   ├── repository/  # Header, repository selector, welcome card, Your Turn
│   ├── branch/      # Branch tables, rows, inline actions, the Safe Push popover
│   ├── commit/      # Commit list, commit info, floating selection bar
│   ├── unassigned/  # Unassigned commits card and inline branch creator
│   ├── conflict/    # Conflict and missing-commits views
│   ├── diff/        # Diff views on the diff engine (lazy chunk)
│   ├── landing/     # The Landing view: the Safe Push statistics and their charts
│   ├── files/       # The directory tree of changed files
│   ├── review/      # Branch, range and pull request reviews
│   ├── ai/          # AI branch name suggestions (Claude Code)
│   ├── command-palette/ # The command palette of the main window (⌘K)
│   └── settings/    # The pages of the Settings window and their rows
├── hooks/           # React hooks (useX.ts)
├── model/           # The window's side of its repository session: the view and the patches it applies (store.ts, patch.ts), the parts of a row (`useRowPart`), the structure of a branch (structure.ts), the intents it runs (intents.ts: the only calls of `execute` and `cancel_operation`), what it shows and needs of the session (demand.ts, `set_demand`)
├── presenters/      # The words, icons and menu items of what Rust decided: next-step.ts, landing-node.ts, land.ts, actions.ts, reasons.ts; no rule, no store read (the caller passes what it selected)
├── stores/          # zustand stores; settings/ holds persisted, cross-window settings
├── lib/             # Utilities and the generated bindings.ts
├── pages/           # Route components (index, the sub-windows: reviews, conflict windows, settings, activity)
├── types/           # Shared TypeScript types
├── test-support/    # Helpers of the unit tests
├── router.ts        # react-router routes (createBrowserRouter)
├── App.tsx          # The root route
├── index.css        # Tailwind and the theme tokens
└── main.tsx         # App entry (store handlers, router)
src-tauri/src/
├── api.rs           # The Tauri wrappers of the commands of app-api and their registration (`typed_commands`, `raw_handler`), generated from its registry (`for_each_command!`): the app has no command or list of its own
├── desktop.rs       # The windows, the folder dialog and the restart of the app (`WindowHost`, `DialogHost`, `RestartHost` of app-api)
├── lib.rs           # The app's setup; `specta_builder` (the commands of `api.rs`, exported to bindings.ts by bindings_test.rs)
├── services.rs      # What the app's sessions run with: their start, `AppEnvironment`, `AppServices` (over the `AppContext`)
└── …                # title_bar.rs, menu.rs, zoom.rs, …
crates/
├── git-executor/    # GitCommandExecutor: the one place git starts, the counts of an operation's processes; the scan tests of the production sources (`source_scan_test.rs`): git (`git_usage_scan_test.rs`, `worktree_safety_test.rs`), nothing across the network (`concurrency_scan_test.rs`), no console window, `fs_retry`, the span of a thread
├── git-ops/         # The git of Branch Deck on objects and refs: one process per phase, never the working tree but the guarded update; one module per operation or shared reader and writer, no lock and no policy
├── history-rewrite/ # The rewrites of the checked out branch (regroup, reword, squash, fold), the grouping, the stores, the repository lock
├── sync-core/       # Branch sync, the pushes, the baseline and the cores of the other operations; no HTTP crate, no `file-diffs`, no `code-symbols`
├── trial-check/     # The trial checks (`bd check`, the conflict fix, the landing gate), `bd move-hunk`, the diff scan and the symbol summaries of the commits
├── review-plan/     # The plan of a review (experimental): Mark Reviewed and the reviewed pair of a branch, the interdiff since it, the API surface, its fan-in, the risk and the order of the files, for `bd` and the app
├── update-core/     # Update Project, `bd pick` and the resolve sessions of a conflicting `bd update`, and their undo
├── lease-pool/      # The lease pool of agent worktrees (`bd lease`), its clean rules and the agent workspace (`bd workspace`)
├── remote-facts/    # What GitHub and Patronus know of the branches, over HTTP: pull requests, review requests, branch rules, review readiness, the Patronus command, the Safe Push statistics; the sign-ins of the app's accounts (`sign_in`, one per provider, the reader of the fact `auth`); the scheduler of the session facts (`scheduler`, read through the hosts' `FactSources`)
├── repository-model/ # The model of a repository for the page and bd: the sessions (the table state, its syncs and patches, their facts) and the one-shot session of a `bd` command (`one_shot`), the table of the last sync, the Patronus view the session derives (one rule, the robot of each row) with the landing needs the host computes, what the rules decide of them for the view (`derive`, `action_view`), the guards of the app's push and robot actions; no trial check
├── operations/      # The engine of the operations of a repository: the in-process registry (the `operations` part of the view, admitted by `domain_policy::admission`), the registry of the repository between processes (`registry_file`, its watch `registry_watch`, the lost starts `reconcile` and `forget`), the journal of the operations that ended (`journal`), the frame every family runs in, the witness of a plan, the facts a plan needs and their reads (`needs`, `facts::fill`), every intent (`prepare`, `execute`, `cancel_operation`): the start (`land`, `land_execute`), the push (`push`), the robot (`robot`), the pull requests (`pull_request`, `close`, `review`), the issue (`issue`), the review fetch (`review_fetch`), the rewrites and their undo (`rewrite`), Update Project, Fetch and Dismiss Landed (`update`)
├── domain-types/    # The types that cross a process or window boundary (the cores, bd, the app): the commits, conflicts and update checks that git-ops builds, the requests and answers of the rewrite cores (`rewrite`), the branch names and the stack prefix syntax, `ErrorCode`, the landing refusals; serde, specta and schemars behind features, no IO; git-ops depends on it
├── domain-policy/   # The pure rules of the domain, one file per rule ([Rule Index](#rule-index): `facts`, `robot_match`, `push`, `stack`, `parked`, `attention`, `holds`, `land`, `land_direct`, `landed`, `pull_request`, `review`, `readiness`, `landing_needs`, `landing_route`, `next_step`, `landing_node`, `actions`, `admission`, `news`, `refresh`, `reasons`), only domain-types: no git, no IO, no runtime; `cases/` holds the scenario table both sides answer, `reasons.json` and `actions.json` the catalogs the presenters word
├── app-api/         # The adapters' crate of the app and the e2e server, only theirs: the one list of the commands (`commands.rs`, `for_each_command!`, the grammar of `registry.rs`), their bodies by family (`reads/`, `session/`, `remote/`, `app/`), the table of the page mock (`commands.json`), `AppContext` (the key of a session, the windows), the operation frame (`operation`), the call and the channel of a command (`transport`), the repository state cache; `api_error`, a core's error as the `ApiError` of a command (one downcast chain)
├── app-settings/    # The settings of the app for the app and bd: the one writer of `settings.json` and `repository-state.json` (`SettingsService`: patches, versions, a debounced atomic save), the tolerant load, the keys of each window, the order of the branch prefixes with its source (`resolve_branch_prefix`, `BranchPrefixes`); no Tauri, no git
├── cli-tools/       # The teamcity and youtrack CLIs of Settings → Tools: status, install and update through Homebrew; no git, no service
├── sync-utils/      # Small shared utilities, no git process and no service: the files (atomic, private, `fs_retry`, the secret store), the processes that are not git and the process scan, the grammars of a commit (subject, body, issue patterns), the id and the cost of an operation, the clock, the span of a thread, the single flight of a read, timestamps and sizes
├── branch-integration/ # Integration detection: rebase, merge and squash strategies
├── bd-cli/          # `bd` agent CLI: the `Cli`, the dispatch, `bd schema`, the help texts, the process budgets (docs/contributing/agent-cli.md)
├── bd-core/         # What every `bd` command builds on: the context, the errors, the output, the sync and its summaries, the summaries of a check that `bd check` and `bd safe-push` print, the remote services of a command and the host of its reads of the facts (`services`), the record of its operation in the operations registry and its entry in the journal (`operation_record`)
├── bd-lease/        # `bd lease` and `bd workspace`: the worktrees of the lease pool and whether an agent leases one; its tests run bd through the fixtures of bd-cli
├── bd-remote/       # The bd commands of the remote: status, push, safe-push, land, request-review, robot, robot-stats, operation, the landed branches
├── bd-rewrite/      # The bd commands that read and rewrite the commits by their messages: group, reword, fold, check, suggest
├── bd-update/       # `bd update`, `bd pick`, `bd unapply`, `bd reapply` and `bd undo`; its tests run bd through the fixtures of bd-cli
├── bd-host/         # `bd-host`, the binary of the remote runs (spec/cli/host.spec.md): the client and the agent; neither bd nor the app links it
├── host-link/       # The link of the remote runs: identity, TLS 1.3 with pinned certificates on the provider of the app, frames, protocol host-link; no git
├── host-client/     # The client of a remote run: the commit of the working tree, the pack, the follow, the fetch; git through git-ops
├── host-agent/      # The agent on a host: serve, the supervisors, the keeper, the upgrade; the host slots through lease-pool in process
├── commit-clustering/ # What `bd suggest` and `bd commits` compute: the clustering, the areas of a commit, the generated files; no CLI, a library of bd-rewrite and bd-remote
├── object-cache/    # LMDB cache keyed by git object ids, shared by the app and bd: the answers of the Patronus pre-check, the robots, the passed builds, the last frame of each session
├── code-symbols/    # Declarations, imports and references of a Kotlin/Java/Rust/TS/JS file on tree-sitter, no git; the highlighting of the diffs (`highlight/`)
├── text-diff/       # Line, word and char comparison of two texts, ported from IntelliJ (Apache-2.0, NOTICE)
├── file-diffs/      # The typed diff of each file of a commit, a range or the working tree, and blob texts by id
├── patronus/        # Patronus REST client, Space token of the Safe Push CLI, request policy
├── github/          # GitHub REST and GraphQL client (rate limits, SSO, conditional GETs), fake GitHub for tests
├── youtrack/        # YouTrack REST client, the sign-in through its Hub (OAuth, PKCE, refresh token), issues and commands
├── test-server/     # E2E mock server: a host of app-api (`invoke.rs`: one route `/invoke/<name>` per entry of the registry, SSE for a channel; `host.rs`: the desktop and the event streams of the pages), the fakes and the fixture routes of each test repository ([routes](../design/e2e-testing.md#the-routes-of-the-server))
├── test-utils/      # Test repositories, fixture git (`git_command`), the fixtures of the sync tests (`sync_fixtures`) and, with the feature `http`, `fake_server`: the loopback HTTP server of the fakes of patronus, github, youtrack and the test server
└── …
tests/
├── e2e/             # Playwright specs by feature, their HTML snapshots, fixtures/ and helpers/
├── perf/            # Performance tests (`pnpm perf:diff`, `pnpm perf:table`)
└── screenshots/     # The README images (`pnpm screenshots:readme`)
```

## Documents

### Design

- [Async Patterns and spawn_blocking](../design/async-patterns.md) - Explains async/sync boundaries and proper use of spawn_blocking
- [Diff View](../design/diff-view.md) - Our own renderer on the blocks of the engine (`DiffView`: the row model, the exact layout, the highlight worker by blob id), for the 2-way diffs and the 3-way merges
- [Distributed Settings Architecture](../design/distributed-settings.md) - One writer of `settings.json` and `repository-state.json` in Rust (`SettingsService`): the windows send patches, apply changes by version, start from the snapshot of their init script; `bd` reads the same file; the branch prefix resolved per command
- [Running Operations](../design/operations.md) - One registry of the actions that run: a second start joins, a rewrite waits for a rewrite, the spinner where it started, a rewrite lasts until its sync; between processes the operations registry of the repository, `busy` naming the holder
- [Rust as the Domain Authority](../design/rust-domain-authority.md) - The rework plan (Draft, in Russian): one Rust session owns the table, the facts and the running operations, `domain-policy` holds every rule in one file, every mutation is an intent through `prepare`/`execute`, bd and the app are two adapters; the stages and their progress
- [Rendering Performance](../design/rendering-performance.md) - React Compiler, narrow selectors, the patches of the Rust session (16 ms, by part) applied in one update, derived structure cached, lazy main page
- [E2E Testing Architecture](../design/e2e-testing.md) - E2E test infrastructure with mock Tauri API
- [Git Operations](../design/git-operations.md) - Working-tree safety, exact-metadata rewrites, `git replay`, batching and the Git 2.54 minimum
- [Landing Work into Master](../design/landing.md) - The product model: everything lands into master, stacks cut it into pieces, every pull request into master, a whole stack in one robot; the routes into master (a robot, a push, a merge)
- [Host Runs](../design/host-runs.md) - `bd-host`, the binary of the remote runs outside `bd` and the app: the client and the agent call the lease pool and git in process; the slot model, `serve`, the supervisors and the keeper, the upgrade
- [Integration Detection and Caching](../design/integration-detection.md) - How branch integration detection works with git notes caching
- [Remote Status](../design/remote-status.md) - How remote tracking status is computed and displayed
- [Text Diff Engine](../design/text-diff.md) - The IntelliJ comparison ported to `crates/text-diff`: lines, words, chars, and the typed diff model of the views
- [Symbol Dependencies](../design/symbol-dependencies.md) - tree-sitter summaries of the changed files, `Order` edges by evidence, the subtraction in the dangling-reference check
- [UI Theme](../design/ui-theme.md) - Islands layering, the radius scale and the rules for UI changes
- [Virtual Branches](../design/virtual-branches.md) - How commits are grouped and copied into virtual branches
- [Specs](../../spec/) - Requirement specs in the air format (spec/<area>/<name>.spec.md, test links); start with spec/commit/commit-row.spec.md

### Guides

- [Runs on Another Mac](../guide/host-runs.md) - For a person: set up a host Mac for `bd-host`, the first runs, the slots, the upgrade, the refusals

### Contributing

- [Adding a Git Operation](adding-git-operation.md) - The layers of an operation and the helpers to reuse
- [Agent CLI](agent-cli.md) - The `bd` commands, their JSON, error codes and exit codes
- [Platform Checks](platform-checks.md) - Linux in Docker and Windows in the Parallels VM before platform code lands
- [Fast Rust Tests](rust-tests.md) - A test costs its git processes: the fixtures of one process per step, no sleeps or wall-clock limits, `pnpm perf:test-processes`
- [Studio Runs](studio-runs.md) - `bd-host run studio -- <command>`: a heavy check on the Mac Studio, on the exact working tree of a lease, in a warm host slot that the Studio lends to that run
- [Real App Checks](real-app-checks.md) - The release app driven with Peekaboo: windows, input, frames, timings
- [Import Guidelines](imports.md) - No re-exports policy and proper import patterns
- [UX Writing](ux-writing.md) - Toast titles and actions, "Cannot …" errors, Title Case and sentence case, punctuation
- [Design Docs Guidelines](design-docs.md) - How to write design documents
- [Releases](releases.md) - The release pull request, the builds of the assets, macOS signing and notarization
