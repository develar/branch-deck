# Proposal

## Why

The menu of a branch changed its items while the first answers of a session came in. On a GitHub mirror that
`origin` doesn't push to, "Merge" was offered until GitHub listed the pull requests. Then it left, and "Create Pull
Request…" was put in above the items below it. An item under a resting pointer moved, and a click meant for it landed
on another. The places were decided by the answers of the network. Git tells most of them before any request: whether
Patronus knows the repository (`origin` on Space), and which GitHub repository the branches end up in
(`branchdeck.githubRepository`, the one `origin` pushes to, the mirror of the committed Space settings).

## What Changes

- The sync reads the facts of the slots with the remotes (`LandingRemote::patronus`, `LandingRemote::github`,
  `LocalGitHub`), so they stand at the first view. The rules decide the presence of a slot on them, and only its
  enablement on the answers: `PatronusStanding::of_remote`, `Listing::of_remote`, `GitHubFacts::of_remote`, the merge
  gate refusing a mirror and a repository off GitHub at once, `patronus_lands` and `github_repository_known` (moved into
  `domain-policy`).
- A shown slot never leaves. Safe Push and "Patronus ›" stand for an `origin` on Space, disabled until Patronus answers.
  "Merge" stands for the GitHub repository `origin` pushes to, disabled until GitHub answers. A mirror never shows it.
  "Create Pull Request…" stands wherever git names a GitHub repository, disabled until the listing. Its place is the
  place of "Open Pull Request #N" once GitHub lists a live one. A local remote has none of them from the first view.
- Create Stack Pull Request… waits for the listing as the branch's does: the repository is known before GitHub answers
  now.
- The menu, the preview and the review window put "Create Pull Request…" before "Open Pull Request #N"
  (`pullRequestItems`), so one replaces the other in the same place.
- The mirror of the committed Space settings is read only for an `origin` on Space: one `cat-file` of each sync there,
  none elsewhere. A command of `brd` takes the GitHub repository from the report of its sync (`SyncReport::github`)
  instead of reading it again, so a command off Space runs one `cat-file` fewer and one on Space as many as before.
- The E2E server tells the sync which repositories its fake Patronus knows (`SyncOptions::space`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/next-step`: Facts without an answer: the places of the menu stand from the first view, by what git tells.
- `branch/pull-requests`: Reading pull requests: the Space mirror only of an `origin` on Space.
- `branch/landing-routes`: Gates: a mirror and a repository off GitHub are refused before GitHub answers; Merge: before
  Patronus answers, the repository is its where `origin` is on Space.
- `branch/branch-row`: Branch menu: the order of the review group, and the starts by `origin` before Patronus answers.

## Impact

- `crates/domain-types/src/landing_route.rs`, `crates/sync-core/src/{repo_facts,sync,github_repository}.rs`,
  `crates/domain-policy/src/{landing_route,next_step,land,pull_request,actions}.rs`,
  `crates/repository-model/src/{derive,action_view,warm_start}.rs` (`FRAME_SCHEMA` 13), `crates/operations/src/land.rs`,
  `crates/bd-remote/`, `crates/test-server/src/session_routes.rs`.
- `src/hooks/useBranchContextActions.ts`, `src/components/branch/BranchPreviewActions.tsx`, `src/stores/patronus.ts`,
  `src/lib/bindings.ts`.
