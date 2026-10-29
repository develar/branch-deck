# Branch Deck Development Context

## Project Overview
The work of one local branch (the master of a large repository) is done when it is in master; in the IntelliJ
monorepo it gets there through Patronus Safe Push. Branch Deck cuts that work into virtual branches, optionally
stacked, only so that it can land in pieces: a piece is a branch and what it is built on, landed in one robot; a whole
stack lands in one robot too; every pull request goes into master, holding what its Safe Push lands. Judge every
feature by what it lands into master and in how many robots ([Landing Work into Master](docs/design/landing.md)).

A desktop app (React 19 + Vite SPA in a Tauri v2 shell), a Rust workspace under `crates/`, and `bd`, the agent CLI
([Agent CLI](docs/contributing/agent-cli.md)). Use `pnpm` only: the version is `packageManager` in `package.json`,
Node.js is `.node-version`; CI reads both through `voidzero-dev/setup-vp`.

**A sync** ([Virtual Branches](docs/design/virtual-branches.md)): `CommitGrouper` groups the commits ahead of the
baseline by their `(prefix)`, else by the issue group of the subject's issue key, else by the issue key. Each group
becomes `{branch prefix}/virtual/<group>`: its commits copied onto the baseline as trees, one `merge-tree --stdin` and
one `fast-import` per branch (`CommitChain`), never `git cherry-pick`, never the working tree. A conflict means the
grouping changed the history of commits that depend on each other ([Conflict Fix](spec/branch/conflict-fix.spec.md)).
A note on each original commit (`MappingNote`, `crates/git-ops/src/notes.rs`) names its copy; a later sync reuses it
while the original's trees match.

## Never
Hard rules in one place; the linked section says why and what to do instead.
- Never edit, build or commit in the main checkout: every session, a background job too, works in its own lease
  ([Worktrees](#worktrees)).
- Never `git stash` in a lease, never rerere: the stash and the rerere cache are shared by every worktree of the pool
  ([Agent Pitfalls](#agent-pitfalls)).
- Never `cargo test`: always `cargo nextest run` ([Verification](#verification)).
- Never a merge commit, `--no-ff`, `--no-verify`, or `commit-tree` with two `-p` ([Commits and History](#commits-and-history)).
- Never a behaviour change without its spec, `[@test]` links and docs in the same commit ([Specs and Docs](#specs-and-docs)).
- Never git outside `GitCommandExecutor` in the app's code, never a write to the user's working tree beyond the named
  exception, never a raw `Command::new("git")` in a test fixture ([Git Operations](#git-operations)).
- Never a `gh` process for GitHub (HTTP through `crates/github`; the one `gh` is `gh auth token` of the token chain),
  never `-c credential.helper=`.
- Never a branch on `error.message` or an error's `kind`: the page acts on `ApiError.code` (`src/lib/errors.ts`);
  `pnpm lint:arch` fails on it ([Code Style](#code-style)).
- Never a read of the main window before its subscription: its start asks `subscribe_view` alone
  ([Warm start](docs/design/git-operations.md#warm-start)); `pnpm lint:arch` (`startup-read`) fails on it.
- Never a rule of the domain in TS: the page words the view, sends intents (`src/model/intents.ts`) and its demand,
  reads the selected project before the first `await`; `pnpm lint:arch` fails on the rest ([Code Style](#code-style)).
- Never `#[allow]` (`#[expect(lint, reason = "…")]`), never `pub use`, never a `../` import in the frontend (`@/`).
- Never a `dark:` variant, a colour outside the semantic tokens, or a radius class in `src/components/ui/*`.
- Never a read or write of the operations registry or its journal beside `operations::registry_file` and
  `operations::journal`, never a fetch of the baseline but `sync_core::baseline::fetch_baseline`, never a clean rule
  that deletes their locks ([Git Operations](#git-operations)).
- Never `waitForTimeout`, a CSS-class locator, or a check of a menu label instead of invoking the action
  ([E2E Tests](#e2e-tests)).
- Never `pnpm dlx shadcn@latest` (it crashes): `npx shadcn@latest`.

## Session Workflow
1. **Lease** a worktree before the first edit: `bd lease get --holder <agent-name> -b <branch>` in the main checkout,
   then work in the `path` it prints; its `home` names where the commits land, its `next` what to run
   ([Worktrees](#worktrees)).
2. **Read** the spec and the design doc of what you change ([Feature Index](#feature-index)): the spec is the
   contract, the doc the why. Before changing a git operation, [Adding a git operation](#adding-a-git-operation).
3. **Change** the code together with its spec, its `[@test]` links and the docs it contradicts.
4. **Verify** by the table in [Verification](#verification).
5. **Commit** in the lease, a conventional subject as in `git log` (`feat(bd): …`, `fix(e2e): …`, `docs(agents): …`),
   then land it on `master` of the main checkout with `bd -C <path> lease pick`: the commit goes onto `master` as it
   is then, the lease follows; a conflict lands nothing and comes back to you (`conflict.next`). Never `git rebase` or
   `git merge` for it, never `bd update` in the lease.
6. **Release**: stop the processes you started in the lease (the e2e mock server, `cargo watch`), then
   `bd lease return --workspace <path>`.

### Worktrees
Parallel work runs in a worktree of the `bd lease` pool (`~/.branch-deck/pool/<repo>-<hash>/`,
[bd lease](spec/cli/lease.spec.md)); never `git worktree add`, `EnterWorktree` or `isolation: "worktree"`. In full:
[Working in a Lease](docs/contributing/leases.md).
- **Every session edits in its own lease**, the main session and a background job too: other sessions work in this
  repository at the same time, and a harness default such as "edit files directly in your working directory" does not
  override this. "Commit right away" means: commit in the lease, then `bd lease pick`.
- **Acquire**: `bd lease get` creates `<branch>` at the HEAD of the checkout that runs it (local `master`, maybe ahead
  of `origin/master`) and does not fetch. A leased worktree is never handed out again.
- **Inside**: the rules of the main checkout apply. `node_modules` and `target/` survive a reuse (on macOS a new slot is
  an APFS clone); run `pnpm install` when `pnpm-lock.yaml` changed. Never a shared `CARGO_TARGET_DIR` or
  `build.build-dir`: worktrees would overwrite each other's units. The return of a lease applies the clean rules of
  `.branchdeck.toml`.
- **Disk**: `low_disk_space` (less free than `branchdeck.leaseMinFree`): tell the user, never pass `--min-free 0`.
- **E2E**: the mock server listens on 3030 in every checkout; when another worktree's server holds it, run
  `E2E_PORT=3031 pnpm e2e`.
- **Release**: `bd lease return` refuses local changes, a commit that `master` lacks, or a process inside;
  `--confirm-preserved` discards them, so pass it only when they are preserved elsewhere. `bd lease status` lists the
  pool; a worktree of the old Treehouse pool goes back with `treehouse return <path>`.

## Verification
⚠️ **NEVER `cargo test`, ALWAYS `cargo nextest run`**: `cargo test` runs a binary's tests as threads of one process,
and a test that sets environment variables (`test_utils::env_vars::EnvVars`) changes them under the others. In CI a
test that passes only on a retry is FLAKY and fails the run: a race to fix. `cargo nextest run -R latest` reruns what
failed (record on in the user config); finish with a full run. Why and how: [Checks](docs/contributing/checks.md).

**A test's time is its git processes** ([Fast Rust Tests](docs/contributing/rust-tests.md)): `TestRepo::new()`,
`create_commit*` or `commit_with`, `with_history` for many commits; never a `git config` process per key, a sleep, or
a wall-clock limit an assertion depends on.

**Heavy runs go to the Mac Studio**: the full `cargo nextest run --workspace`, a full `pnpm e2e` and
`pnpm lint:backend` run as `bd-host run studio -- <command>` from the lease ([Studio Runs](docs/contributing/studio-runs.md)):
this Mac runs several agents at once, and E2E fails on timeouts under that load. `--apply` brings back what a command
edits; exit code 75: the Studio did not run it, run it here.

If Homebrew `cargo` comes before rustup in `PATH`, prefix cargo commands with `PATH=$HOME/.cargo/bin:$PATH`.

| You changed | Run (every filter: [Checks → By Change](docs/contributing/checks.md#by-change)) |
|---|---|
| Rust | `pnpm lint:backend`, then `cargo nextest run --workspace` (`-p`, `-E 'test(/…/)'` while iterating); a new cross-crate edge fails `crates/bd-cli/src/architecture_test.rs` |
| a git operation | also its test in `process_budget_test.rs`, the scan tests (`cargo nextest run -p git-executor`), a core under the repository lock its row of [Lock spans](docs/design/git-operations.md#lock-spans) and its test |
| a command of app-api or a type it passes | `UPDATE_BINDINGS=1 cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'` (`src/lib/bindings.ts`), `UPDATE_COMMANDS=1 cargo nextest run -p app-api -E 'test(/commands_json/)'`; a new command raises `COMMAND_COUNT` |
| frontend | `pnpm lint:frontend`, `pnpm test:unit`, `pnpm e2e`; `pnpm e2e-up` when the HTML changes, its snapshot diff reviewed like code; an E2E spec or helper: also `pnpm lint:e2e-pins` |
| code anywhere | `pnpm lint:cpd` (fix a clone; `jscpd:ignore` only with the reason) and `pnpm lint:arch` (an exception is an `ALLOWLIST` entry with its reason) |
| a fact of the session, its refresh, the scheduler | `cargo nextest run -p domain-policy -p remote-facts -p repository-model`; its e2e moves the clock with `repo.advanceBackendClock(ms)` |
| the frame of a session or the start of the main window | bump `FRAME_SCHEMA` when a type a frame holds changes; `cargo nextest run -p repository-model -p operations`, `pnpm e2e tests/e2e/startup/`, `pnpm lint:arch` |
| the next step, the Safe Push decision, `plan()`, `repository_model::derive` | `cargo nextest run -p bd-remote -E 'test(/scenario/)'`, the cases in `crates/domain-policy/cases/` in the same commit |
| a rule of `domain-policy` | `cargo nextest run -p domain-policy -p repository-model`, the scenario table, the presenter (`src/presenters/*.test.ts`) |
| a `Reason` code, an action or its blocker | `UPDATE_REASON_CATALOG=1` / `UPDATE_ACTION_CATALOG=1 cargo nextest run -p domain-policy -E 'test(/reason\|action_catalog/)'`, then `pnpm test:unit` |
| a spec, a doc, a test title, a rule or a presenter | `pnpm lint:specs` (the `targets:` of the specs too) |
| `#[cfg]` or `cfg!` code | [Platform Checks](docs/contributing/platform-checks.md): Linux and Windows too |
| what the release app shows | [Real App Checks](docs/contributing/real-app-checks.md) |

```bash
pnpm run tauri dev        # The full app
pnpm test:unit            # Vitest
pnpm e2e                  # Playwright; DEBUG_E2E=1 for the mock server's log; E2E_PORT=3031 on another port
pnpm lint                 # Every lint:* script; `pnpm exec vp check` is what CI checks first
```
The other scripts and debugging: [Checks → Commands](docs/contributing/checks.md#commands).

## Agent Pitfalls
Mistakes agents made here; each cost a session.
- **One `pnpm e2e` per checkout**, even on two ports: a global setup that runs `pnpm build` empties `dist/` under the
  other run. **`CI=1` does not speed up E2E** (1 worker, 2 retries); a test that fails only under load is a race.
- **Same-second commits are the same commit**: a replay of a commit onto its own parent within one second writes the
  identical object. A test that needs a new copy makes the target diverge first or uses `cherry-pick -x`.
- **React Compiler and store reads**: a `getState()` read in the render body is memoized and goes stale; take the
  value from a selector ([Frontend Rules → Pitfalls](docs/contributing/frontend.md#pitfalls)).
- **A kept view runs its effects again** as it shows (`Activity`, `KeepMounted`); WebKit keeps the focus on a hidden
  element: test the focus and the popups in `pnpm e2e:webkit` too.
- **Content that moves under a still pointer**: WebKit fires over and enter at once; a hover opens only after a move
  (`hoverOpensOnMove`), and another element keeps it only as the same control.
- **Typing into a Base UI dialog in E2E**: wait for `toBeFocused()` first, or the text lands at the start.
- **zsh does not word-split `$args`**: write the arguments out, use `${=args}`, or `bash -c '…'`.
- **An "unresolved import" of a module that exists**, after another session moved branches under the same `target/`,
  is a stale cargo fingerprint: `touch` the crate's files and rebuild.
- **One stash and one rerere cache for every lease** (`refs/stash`, `.git/rr-cache` of the common git dir): a
  `git stash pop` applies another lease's entry, rerere replays another lease's resolution. Keep work in progress as a
  WIP commit or a patch in `notes.local/`; the clone sets `rerere.enabled false`
  ([Working in a Lease → Pitfalls](docs/contributing/leases.md#pitfalls)).
- **Throwaway Playwright specs** go into `shots.local/` of the worktree
  ([Writing E2E Tests → Pitfalls](docs/contributing/e2e-tests.md#pitfalls)); delete them before a commit.

## Development Rules

### Specs and Docs
Specs (`spec/<area>/<name>.spec.md`) and design docs (`docs/design/`) are part of the change, not a follow-up.
- **Before** changing a feature, read its spec and the design doc it links: the spec is the contract, the doc the why.
- **Same commit**: a behaviour change updates its spec: requirements, `[@test]` links, `targets:` front matter (the
  files where the feature lives: its rule, intent, `bd` command and presenter) and the UX tokens. A renamed or moved
  test or file updates every link to it in `spec/` and `docs/`.
- **New user-visible feature**: a new spec in the air format (copy an existing one), a row in the
  [Code Map](docs/contributing/code-map.md#feature-index), at most one sentence and a spec link in README "Features".
  README is the pitch for a newcomer: no bullet per feature, no shortcut lists, nothing internal to one
  company (Patronus, Space).
- **Git**: a new git operation or a changed flow gets its section in [Git Operations](docs/design/git-operations.md);
  a new or changed hard rule goes into [Git Rules](docs/contributing/git-rules.md) and one line below.
- **AGENTS.md** holds the rules an agent must not miss, one line each, and pointers; at most 32 KB
  (`pnpm lint:specs`), because every session of every agent reads all of it. The detail goes to its owner: a feature
  into its spec, a git flow into the design doc, a rule into `docs/contributing/`, a guide for a person who uses
  Branch Deck into `docs/guide/`.
- **Truth**: docs describe the code as it is; a doc that contradicts the code is a bug, fixed in the same change
  ([Design Docs Guidelines](docs/contributing/design-docs.md)).

### E2E Tests
In full: [Writing E2E Tests](docs/contributing/e2e-tests.md); the infrastructure: [E2E Testing Architecture](docs/design/e2e-testing.md).
- **Invoke every surface** (key, menu item, button, toast action) and assert the effect; a check of a label is not a
  test of the action.
- **Assert the effect** the user sees or what reached the backend, never the state of a store; **no vacuous
  negatives**: anchor a "nothing happened" on something that existed first.
- **Wait for the state, never for time**: `runAndWaitForSync`, `waitForViewVersion`, `runAndWaitForAnswer(page,
  command, () => button.click())`; app timers with `page.clock.fastForward`, session timers with
  `repo.advanceBackendClock(ms)`.
- **Close a read toast** (`closeToast`) before acting where it lies.
- **Locators**: role and accessible name, then `getByTestId`; never CSS classes or DOM shape.
- **Through the user's path**; an error path fails the command with `routeCommandError`.
- **Remote answers come from the fakes** (`setPatronus`, `setPullRequests`, `setYouTrack`); never a page answer of a write.
- **Linked**: every requirement names its test (`[@test]`, the exact title); **snapshots**: `captureHtmlSnapshot`, the
  diff of `pnpm e2e-up` reviewed like code.

### Commits and History
How agents commit to this repository; "Git Operations" below is about the git the app runs.
- **Linear history, no merge commits**, on any branch. Land a lease with `bd lease pick`, never `git cherry-pick`; by
  hand `git rebase <target>` and `git merge --ff-only <branch>`.
- Once per clone: `git config core.hooksPath .githooks && git config merge.ff only && git config rerere.enabled false`.
- Never `--no-ff`, never `--no-verify`, never `commit-tree` with two `-p`.

### Git Operations
Hard rules of the git the app and `bd` run. Each rule in full, with its exceptions and its test, is the section of the
same name in [Git Rules](docs/contributing/git-rules.md); the flows and the reasons are
[Git Operations](docs/design/git-operations.md).
- **Git 2.54+** (`MIN_VERSION`, `crates/git-executor/src/git_info.rs`).
- **Executor**: all git runs through `GitCommandExecutor` (`git.cmd(&args, repo)` and one finisher); a rev or ref fact
  is a git-ops function; test fixtures run `test_utils::git_test_utils::git_command()`.
- **Scan tests** (`git_usage_scan_test.rs`): no `--no-pager`, no `credential.helper=`; `update-ref` only in
  `ref_transaction.rs`, `cat-file --batch*` only in `cat_file_batch.rs`; `-z` with `--name-only`/`--raw`;
  `--literal-pathspecs` after `--`; `concurrency_scan_test.rs`: no async guard across another `.await`, no repository
  lock across a network call; an exemption names its file, reason and exact count.
- **Never write to the working tree** (`worktree_safety_test.rs`): no `checkout`, `reset`, `rebase`, `stash`, `commit`,
  `merge`, `apply` without `--cached` and the like. The single exception is the guarded `read-tree -m -u` of
  `worktree_update.rs`, called only by `update_branch.rs`; a lease slot's resets live in `lease_slot.rs` only. Never
  another caller.
- **The user's index** changes only where HEAD's tree changed (`IndexUpdate`); never `reset --mixed`.
- **Index lock**: a write of the user's index goes through `git_ops::index_lock`; never a removal of another lock of git.
- **Ref moves**: one `RefTransaction` with old-value CAS and a `branch-deck: <op>` reflog message; the named exceptions
  are the notes `fast-import` writes, the temporary refs of `commit_chain.rs` and the refs of the review fetch.
- **Who writes the refs of Branch Deck**: every ref of `refs/branch-deck/`, `{prefix}/unapplied/` and `{prefix}/stack/`
  has its named writers, with CAS; a sync mostly only reads them.
- **Repository lock** (`$GIT_COMMON_DIR/branch-deck.lock`, `repository_lock.rs`): a rewrite of the checked-out branch,
  a push, Update Project, `bd pick`, unapply and reapply read and compute before it and hold it only for the witness of
  their refs and the CAS write (`lock_prepared`; [Lock spans](docs/design/git-operations.md#lock-spans)); a sync and a
  fetch never wait for it.
- **Operations registry**: only `operations::registry_file` touches `operations.json` and `operations.lock`, only
  `operations::journal` the journal; a start whose answer was lost is `Unknown`, never sent again.
- **Guards** (`History::check_rewrite`): no rebase/merge/cherry-pick in progress, HEAD not detached, no merges in the
  range; a sync under the first two is `SyncDeferred`.
- **Metadata**: message-only rewrites keep author and committer byte for byte, content rewrites keep the author;
  signatures are stripped.
- **Batch**: one process per phase, never per item; never parse translated text (exit codes, `--porcelain`, `-z`).
- **Exit codes**: `merge-tree` and `replay` exit 1 on conflicts, `merge-tree --stdin` 0 with a status per merge;
  `merge-base` 1 is "no", any other non-zero an error.
- **Network commands** run through `execute_remote_command`.
- **The branch of the baseline on its remote** takes only `push_baseline`; only `fetch_baseline` writes
  `refs/remotes/origin/master`.
- **Fetch lock**: the baseline is fetched only by `fetch_baseline` (`fetch.lock`); no retry on `cannot lock ref`.
- **Git config writes**: only the `branchdeck.*` keys of Settings (`repository_config.rs`) and the branch descriptions
  of `bd describe`.
- **GitHub credentials**: the only git is `git credential fill` (`crates/github/src/token.rs`), never `approve` or
  `reject`; the app's tokens go through `sync_utils::secret_store`.
- **Symbols** (`symbol_scan.rs`): the blobs of the commit diffs by id, never the whole tree.

### Adding a git operation
Read [Adding a Git Operation](docs/contributing/adding-git-operation.md) (its checklist, the layers, the helpers)
before you add or change git code in `crates/git-ops/`, the cores (`history-rewrite`, `sync-core`, `update-core`,
`trial-check`, `lease-pool`, `remote-facts`, `repository-model`), `crates/operations/`, `crates/bd-*/` or
`crates/app-api/`. In short: one name across the layers; the git in git-ops, no lock and no policy; one `<op>_core`
entry of every caller; an intent of `crates/operations` for an action on the remote, a rewrite or Update Project; bd
and the app parse, call the core and present; reuse a helper before writing a parser; the spec, the flow's section and
a `process_budget_test.rs` test in the same commit.

### Code Style
Frontend, in full: [Frontend Rules](docs/contributing/frontend.md).
- **Colors**: shadcn semantic tokens plus `canvas`, `success`, `warning`, `info`; never `dark:`. **Radii** only from
  `--radius`. The canvas is `bg-canvas`, top-level blocks are `Card` islands, `bg-muted` is a tint, not a surface
  ([UI Theme](docs/design/ui-theme.md)). **Spacing**: `gap-*` on the parent.
- **Icons** in `Button` take `data-icon="inline-start"` and no size classes.
- **Errors** go through `notifyError` (`src/lib/errors.ts`) and are decided by `error.code`; a toast of an event is
  `showEventToast`, one per event. **Wording** by [UX Writing](docs/contributing/ux-writing.md): "Cannot <verb>
  <object>", Title Case for buttons and menu items. **Actions**: one builder of `MenuItem`s per object for every surface.
- **Rules and intents**: a presenter reads no store (a store module for its types only); `execute` and
  `cancel_operation` only in `src/model/intents.ts`, each `app(…)` command from its one module (`APP_COMMAND_CALLERS`).
- **React Compiler** compiles everything (`panicThreshold: "all_errors"`): no disable comment of a React rule, no
  `try`/`finally` without `catch`, no JSX parameter default.
- **State**: zustand vanilla stores, narrow `useStore` selectors; a persisted setting is a `createPersistentStore`
  whose one writer is Rust's `SettingsService`; an export no page code calls is deleted.
- **Base UI**: the `render` prop, not `asChild`. **shadcn**: `npx shadcn@latest`; the locally customised files of
  `src/components/ui/` carry a `// local customisation: …` header, re-applied after `add --overwrite`.
- **Imports**: every API explicitly, types with `import type`, only through `@/`; components are PascalCase `.tsx`
  files with a named export; Rust-bound props and timestamps (`u32`) as in the bindings.
- **Toolchain**: Vite+ (`vp`: Vite, Vitest, Oxlint, Oxfmt), no ESLint; test files import from `vite-plus/test`.

Rust, in full: [Rust Rules](docs/contributing/rust-rules.md).
- `tracing`; every command of the app has `#[instrument]` and a fallible one runs as an operation
  ([Operation spans](docs/design/async-patterns.md#operation-spans)); a closure for another thread carries the span.
- **Clippy** pedantic with `-D warnings`, Windows included; `#[expect(lint, reason = "…")]`, never `#[allow]`.
- **A platform branch in a test** asserts on every platform; a `#[cfg(unix)]` test's helper is `#[cfg(unix)]` too.
- **Single flights**: a read others wait for is a `sync_utils::single_flight`, never an async mutex guard across an
  await ([Single flights](docs/design/async-patterns.md#single-flights-no-async-mutex)).
- **File system removals and renames** through `sync_utils::fs_retry`, **processes** through
  `sync_utils::child_process::windowless_command`, never `std::fs` or `Command::new`.
- **Imports** by the full module path, never `pub use` ([Import Guidelines](docs/contributing/imports.md)).

## Where Things Are

### Feature Index
A feature's contract is its spec (under `spec/`, without `.spec.md`); every entry file of each feature:
[Code Map](docs/contributing/code-map.md#feature-index).

| Feature | Specs | Entry |
|---|---|---|
| Repository session, its facts, the warm start | `repository/onboarding`, `branch/branch-review` | `crates/repository-model/`, `crates/remote-facts/src/scheduler.rs`, `src/model/` |
| Integration status | `branch/branch-row` | `crates/branch-integration/` |
| Context menus | `branch/branch-row`, `commit/commit-row` | `src/hooks/useBranchContextActions.ts`, `crates/domain-policy/src/actions.rs` |
| AI branch naming | `branch/branch-name-suggestions` | `src/components/ai/` |
| Squash and fixups, reword | `commit/squash`, `commit/fixups`, `cli/agent-cli` | `crates/history-rewrite/`, `crates/bd-rewrite/` |
| Update Project, resolve sessions | `repository/update-project`, `cli/update` | `crates/update-core/`, `crates/operations/src/update.rs` |
| Pick between worktrees, remote commits | `cli/pick`, `repository/outgoing` | `crates/update-core/src/pick.rs` |
| Agent workspace (`bd workspace`) | `cli/workspace` | `crates/lease-pool/src/agent_workspace.rs` |
| Command-line tools | `repository/cli-tools`, `repository/setup-attention` | `crates/cli-tools/` |
| Landing routes, `bd land` | `branch/landing-routes`, `branch/next-step` | `crates/domain-policy/src/landing_route.rs`, `crates/operations/src/land_execute.rs` |
| Lease pool (`bd lease`) | `cli/lease` | `crates/lease-pool/` |
| Remote runs (`bd-host`) | `cli/host` | `crates/bd-host/`, `crates/host-*/` |
| Landed commits | `repository/landed-commits` | `crates/sync-core/src/landed.rs` |
| Pull requests, the push, Request Review | `repository/landing-overview`, `branch/pull-requests` | `crates/github/`, `crates/operations/src/pull_request.rs`, `crates/operations/src/push.rs` |
| Patronus, Safe Push, the next step | `branch/patronus`, `branch/next-step`, `branch/whole-stack` | `crates/patronus/`, `crates/operations/`, `crates/domain-policy/` |
| Safe Push statistics, the Landing view | `repository/safe-push-statistics` | `crates/remote-facts/src/robot_stats.rs` |
| GitHub and YouTrack sign-in | `repository/github-sign-in`, `repository/youtrack` | `crates/remote-facts/src/sign_in.rs` |
| Issue groups, conflict fix, move to a branch | `branch/issue-groups`, `branch/conflict-fix`, `branch/move-commits` | `crates/history-rewrite/`, `crates/trial-check/` |
| Stacks, stack parents, parked branches | `branch/stacked-branches`, `branch/stack-parents`, `branch/parked-branches` | `crates/domain-policy/src/stack.rs`, `crates/history-rewrite/` |
| Unapply and Reapply | `branch/unapply` | `crates/sync-core/src/unapply_branch.rs` |
| Operations across processes | `cli/agent-cli`, `branch/patronus` | `crates/operations/src/registry_file.rs` |
| Activity log | `repository/activity` | `crates/operations/src/journal.rs`, `src/stores/activity.ts` |
| Commit files, diffs, the review | `commit/commit-files`, `commit/commit-review` | `crates/file-diffs/`, `src/components/diff/` |
| Go to Declaration, Find Usages | `commit/symbol-navigation` | `crates/code-symbols/src/navigate.rs` |
| Command palette, title bar, zoom | `repository/command-palette`, `repository/title-bar`, `repository/zoom` | `src/components/command-palette/`, `src-tauri/src/` |
| Your Turn, Reviews | `repository/review-requests`, `repository/reviews` | `src/components/review/`, `src/components/reviews/` |
| Settings | `repository/settings` | `src/components/settings/`, `crates/app-settings/` |
| Symbol dependencies | `commit/symbol-dependencies` | `crates/code-symbols/`, `crates/trial-check/src/symbol_scan.rs` |

The rows and the menus of the main window: `branch/branch-row`, `commit/commit-row` (start there). The `bd` commands:
`cli/agent-cli`. The `branch-deck` agent skill is generated by `skill_text()` (`crates/bd-cli/src/agent_help.rs`).

### Rule Index
Every rule of the domain is one file of `crates/domain-policy/src/` (its `//!` header names the rule and its spec, whose
`targets:` list it; a table test `<rule>_test.rs` beside it), a pure function the app and `bd` both call; the session
derives its answer into the view (`repository_model::derive`), which the page only words (`src/presenters/`). A rule
never goes into `src/lib` or `src/hooks` (`pnpm lint:arch`). The table of the rules: [Code Map](docs/contributing/code-map.md#rule-index).

### Documents
Design (`docs/design/`): [Landing](docs/design/landing.md) (the product model),
[Virtual Branches](docs/design/virtual-branches.md), [Git Operations](docs/design/git-operations.md),
[Running Operations](docs/design/operations.md), [Rust as the Domain Authority](docs/design/rust-domain-authority.md)
(the rework plan, in Russian), [Distributed Settings](docs/design/distributed-settings.md),
[Async Patterns](docs/design/async-patterns.md), [Rendering Performance](docs/design/rendering-performance.md),
[UI Theme](docs/design/ui-theme.md), [Diff View](docs/design/diff-view.md), [Text Diff](docs/design/text-diff.md),
[Symbol Dependencies](docs/design/symbol-dependencies.md), [Integration Detection](docs/design/integration-detection.md),
[Remote Status](docs/design/remote-status.md), [E2E Testing](docs/design/e2e-testing.md),
[Host Runs](docs/design/host-runs.md); [Specs](spec/) (start with `spec/commit/commit-row.spec.md`).

Contributing (`docs/contributing/`): [Code Map](docs/contributing/code-map.md), [Checks](docs/contributing/checks.md),
[Working in a Lease](docs/contributing/leases.md),
[Git Rules](docs/contributing/git-rules.md), [Adding a Git Operation](docs/contributing/adding-git-operation.md),
[Rust Rules](docs/contributing/rust-rules.md), [Frontend Rules](docs/contributing/frontend.md),
[Writing E2E Tests](docs/contributing/e2e-tests.md), [Fast Rust Tests](docs/contributing/rust-tests.md),
[Agent CLI](docs/contributing/agent-cli.md), [Platform Checks](docs/contributing/platform-checks.md),
[Studio Runs](docs/contributing/studio-runs.md), [Real App Checks](docs/contributing/real-app-checks.md),
[Import Guidelines](docs/contributing/imports.md), [UX Writing](docs/contributing/ux-writing.md),
[Design Docs Guidelines](docs/contributing/design-docs.md), [Releases](docs/contributing/releases.md). Guide for a person: [Runs on Another Mac](docs/guide/host-runs.md).

### File Structure
`src/` is the page (`components/`, `hooks/`, `model/` the window's side of its session, `presenters/` the words of
what Rust decided, `stores/`, `lib/` with the generated `bindings.ts`, `pages/`); `src-tauri/src/` the shell, whose
commands come from the registry of app-api. The crates, each mapped by the `//!` header of its `lib.rs` (in full:
[Code Map → File Structure](docs/contributing/code-map.md#file-structure)):
- git: `git-executor` (the one place git starts, the scan tests), `git-ops` (the git), `object-cache`;
- cores: `history-rewrite`, `sync-core`, `update-core`, `trial-check`, `lease-pool`, `branch-integration`,
  `commit-clustering`, `code-symbols`, `text-diff`, `file-diffs`;
- domain: `domain-types` (the types across boundaries), `domain-policy` (the rules), `repository-model` (the session),
  `operations` (the engine of the intents, the registry, the journal), `remote-facts` (GitHub and Patronus over HTTP);
- clients: `github`, `patronus`, `youtrack`;
- adapters: `app-api` (the one list of the app's commands), `app-settings`, `cli-tools`, the `bd-*` crates (`bd`),
  `bd-host`, `host-link`, `host-client`, `host-agent`;
- shared and tests: `sync-utils`, `test-utils`, `test-server` (the E2E mock server).

`tests/` holds `e2e/` (Playwright specs, snapshots, fixtures, helpers), `perf/` and `screenshots/`.
