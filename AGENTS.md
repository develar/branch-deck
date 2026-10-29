# Branch Deck Development Context

## Project Overview
The work of one local branch (the master of a large repository) is done when it is in master; in the IntelliJ
monorepo it gets there through Patronus Safe Push. Branch Deck cuts that work into virtual branches, optionally
stacked, only so that it can land in pieces: a piece is a branch and what it is built on, landed in one robot; a whole
stack lands in one robot too; every pull request goes into master, holding what its Safe Push lands. Judge every
feature by what it lands into master and in how many robots ([Landing Work into Master](docs/design/landing.md)).

A desktop app (React 19 + Vite SPA in a Tauri v2 shell), a Rust workspace under `crates/`, and `bd`, the agent CLI
([Agent CLI](docs/guidelines/agent-cli.md)). Use `pnpm` only: the version is `packageManager` in `package.json`,
Node.js is `.node-version`; CI reads both through `voidzero-dev/setup-vp`.

**A sync** ([Virtual Branches](docs/design/virtual-branches.md)): `CommitGrouper` groups the commits ahead of the
baseline by their `(prefix)`, else by the issue group of the subject's issue key (`refs/branch-deck/issue-groups`),
else by the issue key; with stacks on, `(parent/child)` groups into the leaf. Each group becomes the branch
`{branch prefix}/virtual/<group>`: its commits copied onto the baseline as trees, one `merge-tree --write-tree --stdin`
and one `fast-import` per branch (`CommitChain`), never `git cherry-pick`, never the working tree. A conflict comes from
the same merge: grouping changed the history of commits that depend on each other. The conflict view shows the merge of
each file in three columns, read-only (`get_file_merges`), and offers the regroup a trial build verified
([Conflict Fix](spec/branch/conflict-fix.spec.md)). A note on each original commit (the default notes ref,
`core.notesRef`; `MappingNote` in `crates/git-ops/src/notes.rs`) names its copy, `v-commit-v1:<copy>`, the trees it
was copied from, `v-commit-source-v1:<parent tree> <tree>`, and for a copy resolved by words
`v-commit-resolved-v1:<version> <conflicts> <files>`; a later sync reuses the copy while the original's trees match.

## Never
Hard rules in one place; the linked section says why and what to do instead.
- Never edit, build or commit in the main checkout: every session, a background job too, works in its own lease
  ([Worktrees](#worktrees)).
- Never `git stash` in a lease, and never a file rerere resolved left unread: every worktree of the pool shares the
  stash and the rerere cache of the repository ([Agent Pitfalls](#agent-pitfalls)).
- Never `cargo test`: always `cargo nextest run` ([Verification](#verification)).
- Never a merge commit, `--no-ff`, `--no-verify`, or `commit-tree` with two `-p` ([Commits and History](#commits-and-history)).
- Never a behaviour change without its spec, `[@test]` links and docs in the same commit ([Specs and Docs](#specs-and-docs)).
- Never git outside `GitCommandExecutor` in the app's code, never a write to the user's working tree beyond the named
  exception, never a raw `Command::new("git")` in a test fixture (`test_utils::git_test_utils::git_command`)
  ([Git Operations](#git-operations)).
- Never a `gh` process for GitHub (HTTP through `crates/github`; the one `gh` is `gh auth token` of the token chain),
  never `-c credential.helper=`.
- Never a branch on `error.message` or on an error's `kind`: the page acts on `ApiError.code` (`getErrorCode`,
  `describeError` in `src/lib/errors.ts`), `pnpm lint:arch` fails on the words ([Code Style](#code-style)).
- Never a read of the main window before its subscription: its start asks `subscribe_view` alone (the frame of its
  session comes as `window.__BD_MODEL__`, the view names the prefix, the subscription answers a missing git, the session
  runs the open sync); `pnpm lint:arch` (`startup-read`) fails on a command of `src/main.tsx`, `src/router.ts` or
  `src/stores/repository.ts`, a `resolveBranchPrefix` outside Settings and a `syncBranches("open")`
  ([Warm start](docs/design/git-operations.md#warm-start)).
- Never `#[allow]` (`#[expect(lint, reason = "…")]`), never `pub use`, never a `../` import in the frontend (`@/`)
  ([Code Style](#code-style)).
- Never a `dark:` variant, a colour outside the semantic tokens, or a radius class in `src/components/ui/*`
  ([Code Style](#code-style)).
- Never a read or write of the operations registry (`operations.json`, `operations.lock`) or of its journal
  (`journal.jsonl`) beside `operations::registry_file` and `operations::journal`, never a fetch of the baseline but
  `sync_core::baseline::fetch_baseline` (`fetch.lock`), never a clean rule that deletes a lock of theirs
  ([Git Operations](#git-operations)).
- Never `waitForTimeout`, a CSS-class locator, or a check of a menu label instead of invoking the action
  ([E2E Tests](#e2e-tests)).
- Never `pnpm dlx shadcn@latest` (it crashes): `npx shadcn@latest` ([Code Style](#code-style)).

## Session Workflow
1. **Lease** a worktree before the first edit: `bd lease get --holder <agent-name> -b <branch>` in the main checkout,
   then work in the `path` it prints; its `home` names where the commits land, its `next` what to run
   ([Worktrees](#worktrees)).
2. **Read** the spec and the design doc of what you change ([Feature Index](#feature-index)): the spec is the
   contract, the doc the why. Before changing a git operation, [Adding a git operation](#adding-a-git-operation).
3. **Change** the code together with its spec, its `[@test]` links and the docs it contradicts.
4. **Verify** by the table in [Verification](#verification).
5. **Commit** in the lease, a conventional subject as in `git log` (`feat(bd): …`, `fix(e2e): …`, `docs(agents): …`),
   then land it on `master` of the main checkout with `bd -C <path> lease pick` (a bd without it: `bd pick --from
   <path> --follow -C <main checkout>`): the commit goes onto `master` as it is then, the lease follows; a conflict
   lands nothing and comes back to you (`conflict.next`). Never `git rebase` or `git merge` for it, never `bd update`
   in the lease.
6. **Release**: stop the processes you started in the lease (the e2e mock server, `cargo watch`), then
   `bd lease return --workspace <path>`; a run on the Studio keeps nothing there.

### Worktrees
Parallel work runs in a worktree of the `bd lease` pool (`~/.branch-deck/pool/<repo>-<hash>/`,
[bd lease](spec/cli/lease.spec.md)); never `git worktree add`, `EnterWorktree` or `isolation: "worktree"`.
- **Every session edits in its own lease**, the main session and a background job too, not only a spawned agent: other
  sessions work in this repository at the same time. A harness default such as "edit files directly in your working
  directory" does not override this rule. Nothing is edited, built or committed in the main checkout: the commits of a
  lease reach its `master` only by `bd lease pick` ("commit right away" means: commit in the lease, then
  `bd lease pick`).
- **Acquire**: `bd lease get --holder <agent-name> -b <branch>` creates `<branch>` at the HEAD of the checkout that runs
  it (local `master` in the main checkout, which may be ahead of `origin/master`; `-C <worktree>` cuts from another
  branch; without `-b` the worktree is detached) and prints the lease as JSON (`path`, `lease_id`, `home`, `next`).
  It does not fetch.
  A leased worktree is never handed out again.
- **Inside**: the rules of the main checkout apply (`.githooks/` via the shared `core.hooksPath`, `cargo nextest`).
  `node_modules` and `target/` survive a reuse; on macOS a new slot is an APFS clone of a free slot or of the main
  checkout, so it starts with them too. Run `pnpm install` when `pnpm-lock.yaml` changed.
- **Rust cache**: every checkout keeps its own `target/`; never a shared `CARGO_TARGET_DIR` or `build.build-dir`: the
  unit hash of a workspace crate does not depend on the checkout's path, so worktrees would overwrite each other's
  units. Cargo never deletes a stale unit, so the return of a lease applies the clean rules of `.branchdeck.toml` (the
  stale units of `target/`, the e2e artifacts); `bd lease clean --free` applies them to the free slots now
  (`--dry-run` only reports). Dependencies are built with workspace-wide features (`.cargo/config.toml`) and line
  tables only.
- **Disk**: a get refuses as `low_disk_space` when the volume of the pool has less free space than
  `branchdeck.leaseMinFree` (git config, the machine's: `200G` on the user's Mac); tell the user, never pass
  `--min-free 0` on your own.
- **E2E**: the mock server listens on 3030 in every checkout; when another worktree's server holds it, run
  `E2E_PORT=3031 pnpm e2e` (any free port). Global setup refuses a server that serves another checkout, because
  Playwright reuses whatever listens on the port.
- **Release**: `bd lease return --workspace <path>` refuses local changes, a commit that no branch holds and that
  `master` lacks by content, or a process inside. `--confirm-preserved` discards the changes and commits it reports, so
  pass it only when they are preserved elsewhere. The branch survives; the next get moves the worktree.
  `bd lease status` lists the pool. A worktree leased from the old Treehouse pool (`~/.treehouse/<repo>-<hash>/`)
  goes back with `treehouse return <path>`.

## Verification
⚠️ **NEVER `cargo test`, ALWAYS `cargo nextest run`.** `cargo test` runs the tests of a binary as threads of one
process, and a test that sets environment variables (`test_utils::env_vars::EnvVars`, sound only with a process per
test) changes them under the others. `nextest` (`.config/nextest.toml`) runs a process per test, all in parallel
(the suite starts about 70,000 git processes, 94,000 at 2e503b03; on the 20 cores of the Studio 208–220 s against
232–279 s for 2e503b03 in turns, several times that on a loaded Mac); kills a test still running after 5 minutes (a hung git) and fails one whose
git process outlives it; in CI (`NEXTEST_PROFILE=ci`) runs a failed test twice more, and one that passes on a retry is
FLAKY and still fails the run, because a flaky test is a race to fix (locally no retries). With recording on in the user
config (`[experimental] record = true` and `[record] enabled = true` in `~/.config/nextest/config.toml`),
`cargo nextest run -R latest` runs only what failed; finish with a full run.

**A test's time is its git processes** ([Fast Rust Tests](docs/guidelines/rust-tests.md)): a fixture repository is
`TestRepo::new()` (one `git init`, its config appended by `append_config`), a commit `create_commit*` or `commit_with`
(`add` and `commit`, the id from the summary, no `rev-parse HEAD`), a history of many commits `with_history` or
`commit_history` (one `fast-import`); never a `git config` process per key, a sleep, or a wall-clock limit an
assertion depends on. `pnpm perf:test-processes measure|summarize|compare` counts the git
processes of every test, which the load of the machine does not change.

**Heavy runs go to the Mac Studio**: the full `cargo nextest run --workspace`, a full `pnpm e2e` and
`pnpm lint:backend` run as `bd-host run studio -- <command>` from the lease, on its working tree, committed or not
([Studio Runs](docs/guidelines/studio-runs.md), `[host]` of `.branchdeck.toml`). This Mac runs several agents at once,
and under that load the suites take far longer and E2E fails on timeouts. What a command edits (`cargo fmt`,
`pnpm e2e-up`, `UPDATE_BINDINGS=1`) comes back with `--apply`. Exit code 75: the Studio did not run it, run it here.

If Homebrew `cargo`/`rustc` come before rustup in `PATH`, prefix cargo commands with `PATH=$HOME/.cargo/bin:$PATH`:
the nightly toolchain pinned in `rust-toolchain.toml` is required to parse the workspace manifest.

| You changed | Run |
|---|---|
| Rust | `pnpm lint:backend`, then `cargo nextest run --workspace` (`-p <crate>` or `-E 'test(/…/)'` while iterating); a new cross-crate edge fails the architecture test (`crates/bd-cli/src/architecture_test.rs`: the rules of [Crates](docs/design/rust-domain-authority.md#crates-и-направление-зависимостей) on `cargo metadata`, an allowlist that only shrinks) |
| a git operation | also its test in `process_budget_test.rs` and the scan tests (`worktree_safety_test.rs`, `git_usage_scan_test.rs`) |
| a command of app-api (`crates/app-api/src/commands.rs`) or a type it passes | `src/lib/bindings.ts`: `UPDATE_BINDINGS=1 cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'` writes it, headless, the commands in the order of the registry; `crates/app-api/commands.json`, the table of the page mock: `UPDATE_COMMANDS=1 cargo nextest run -p app-api -E 'test(/commands_json/)'`; without the variables the tests fail on a stale file; a new command raises `COMMAND_COUNT` (`registry_test.rs`) and needs no route or wrapper of its own |
| frontend | `pnpm lint:frontend`, `pnpm test:unit`, `pnpm e2e`; `pnpm e2e-up` when the HTML changes, its snapshot diff reviewed like code; an E2E spec or helper changed: also `pnpm lint:e2e-pins` (`command_registry_test.rs`: no page answer of a write, the pinned page answers only go down; `pnpm e2e` doesn't run it) |
| code anywhere | `pnpm lint:cpd`: fix a clone with a helper, constant or fixture; `// jscpd:ignore-start` / `// jscpd:ignore-end` only for an intentional duplication, with the reason in the comment |
| code anywhere | `pnpm lint:arch` (`scripts/lint-arch.mjs`): no branch on the words of an error, no `kind` of an error, a code in every `routeCommandError`, no polling timer in `src/stores` or `src/hooks` (`poll-timer`: a fact is the session's), no rule of the domain in the page (`rule-module`: a deleted rule module, an export of `src/lib` or `src/hooks` named `can…`, `needs…`, `allows…`, `…Blocker`, `…Decision`, `is…Blocked`; the rules are `domain-policy`'s, the page presents them), no read of the start (`startup-read`: a command of `src/main.tsx`, `src/router.ts`, `src/stores/repository.ts` but Browse, `resolveBranchPrefix` outside Settings, `syncBranches("open")`); an exception is an `ALLOWLIST` entry with its reason |
| a fact of the session (a family, `auth` of the sign-ins among them, its refresh rule, the scheduler, the demand) | `cargo nextest run -p domain-policy -p remote-facts -E 'test(/refresh|scheduler/)'` and `cargo nextest run -p repository-model -E 'test(/facts|session/)'`; `auth` also `cargo nextest run -p remote-facts -E 'test(/sign_in/)'` (the sign-ins its host reads); the e2e of the family moves the session's clock with `repo.advanceBackendClock(ms)` and asserts the counters of the fakes ([Session facts](docs/design/git-operations.md#session-facts)) |
| the frame of a session (what `warm_start.rs` keeps, the seed, the catch-up, a type a frame holds, the view's types of `domain-types` among them) or the start of the main window | bump `FRAME_SCHEMA` when a type a frame holds changes (no frame of the old schema is read, the first start is cold); `cargo nextest run -p repository-model -p operations` (`warm_start_test.rs`, the seeded sessions of `session_test.rs`, `rewrite_test.rs`), `pnpm e2e tests/e2e/startup/` (`warm-start.spec.ts`: B7, one `subscribe_view` and its channel before the rows) and `pnpm lint:arch`; a change of what a first sync patches: `pnpm perf:table --project=chromium` on the Studio (`warmFirstSync*`, B4) ([Warm start](docs/design/git-operations.md#warm-start)) |
| the next step, the Safe Push decision or `plan()` of `bd safe-push`; the sync, `patronus_view`, the robot list, the review scan or what the session derives (`repository_model::derive`) | `cargo nextest run -p bd-remote -E 'test(/scenario/)'`: one expectation per case that the app's session and `bd` both answer; the cases in `crates/domain-policy/cases/` updated in the same commit, the diff reviewed like code ([Scenario table](docs/design/rust-domain-authority.md#таблица-сценариев)) |
| a rule of `domain-policy` (a file of the [rule index](#rule-index)) or what the view derives of it | `cargo nextest run -p domain-policy` (the table test of each rule beside it, `<rule>_test.rs`) and `cargo nextest run -p repository-model -E 'test(/derive|action_view|state/)'` (the parts of the view, `derive_budget_test.rs` its time); then the scenario table (the row above), and the page's presenter of what changed (`src/presenters/*.test.ts`) |
| a `Reason` code (`domain_types::reason`), an action or a blocker of an action (`domain_types::actions`) | `UPDATE_REASON_CATALOG=1 cargo nextest run -p domain-policy -E 'test(/reason/)'` writes `crates/domain-policy/reasons.json`, `UPDATE_ACTION_CATALOG=1 cargo nextest run -p domain-policy -E 'test(/action_catalog/)'` `crates/domain-policy/actions.json`; `src/presenters/reasons.test.ts` and `actions.test.ts` word them code by code, so a code without words fails `pnpm test:unit` |
| a spec, a doc or a test title | `pnpm lint:specs`: every `[@test]` link a line of its own, naming tests that exist |
| `#[cfg]` or `cfg!` code | Linux and Windows too: [Platform Checks](docs/guidelines/platform-checks.md) (a check on the Mac compiles only its side) |
| what the release app shows (a window that opens late, a "Loading…" frame) | [Real App Checks](docs/guidelines/real-app-checks.md) (`pnpm install-app`, Peekaboo) |

### Quick Commands
See `package.json` for the full list of scripts.

```bash
pnpm install              # Install dependencies
pnpm run tauri dev        # The full app (frontend + backend)
UPDATE_BINDINGS=1 cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'  # Write src/lib/bindings.ts
UPDATE_COMMANDS=1 cargo nextest run -p app-api -E 'test(/commands_json/)'  # Write crates/app-api/commands.json (the page mock's table)
pnpm dev                  # Only the Vite frontend (port 1420)
pnpm build                # The frontend into dist/ (E2E global setup runs it when its inputs changed)
pnpm install-app          # Unsigned release build of the .app, copied to /Applications (macOS)

pnpm test                 # test:backend + test:unit (Rust and Vitest; not E2E)
pnpm test:backend         # cargo nextest run --workspace
bd-host run studio -- cargo nextest run --workspace  # The same on the Mac Studio, on this working tree (75: run it here)
bd-host run studio -- pnpm e2e  # The E2E suite there; --apply brings back what a command edits (pnpm e2e-up, cargo fmt)
cargo nextest run -R latest  # Rerun what failed or did not run in the last run (record on in the user config)
pnpm test:unit            # Vitest (`vp test run`; src/**/*.test.ts)

pnpm e2e                  # Playwright E2E tests (installs Chromium if needed; `pnpm setup:e2e` for every browser)
pnpm e2e-up               # E2E tests that rewrite the snapshots
DEBUG_E2E=1 pnpm e2e      # With the mock server's debug log and every page console line
E2E_PORT=3031 pnpm e2e    # On another port when another checkout's mock server holds 3030
pnpm test-server:dev      # Optional: the E2E mock server with auto-restart (needs watchexec); `pnpm e2e` reuses it
pnpm perf:diff            # The diff review performance test (playwright.perf.config.ts)
pnpm perf:table           # The branch view at 141 rows: layout, sync, IPC (ui-theme.md#branch-columns); --project=webkit
pnpm screenshots:readme   # The README images in docs/screenshots (tests/screenshots, the readme_showcase template)

pnpm lint                 # Every lint:* script
pnpm exec vp check        # Formatting, lint and types without fixing (what CI checks first)
pnpm lint:backend         # cargo fmt, clippy --fix, then clippy -D warnings
pnpm lint:frontend        # `vp check --fix` (Oxfmt, type-aware Oxlint) + lint:typecheck
pnpm lint:typecheck       # tsc of tsconfig.app.json, tsconfig.node.json and tsconfig.e2e.json
pnpm lint:cpd             # jscpd copy/paste detection (config in .jscpd.json); fails on any clone
pnpm lint:specs           # the [@test] links of spec/ and docs/ name test files and tests that exist
pnpm lint:arch            # the page acts on the code of an error, never on its words, decides no rule, reads nothing before its subscription (scripts/lint-arch.mjs)
pnpm lint:e2e-pins        # the commands E2E tests answer in the page, pinned (crates/test-server/src/command_registry_test.rs)
bd lease clean --free     # the clean rules of .branchdeck.toml on the free lease slots (`--dry-run` only reports)
pnpm perf:monorepo pin|measure|summarize|compare  # bd's git processes and times on a pinned large clone (git-operations.md)
pnpm perf:test-processes measure|summarize|compare  # the git processes of every Rust test (docs/guidelines/rust-tests.md)
cargo run -p branch-deck -- cli status -C <repo>  # agent CLI, JSON on stdout (docs/guidelines/agent-cli.md)
```

### Debugging
- **E2E**: a run ends with its timing (start, slowest tests, stalled actions; `tests/results/timing.json`), and a
  failed test carries the console errors of its pages as the `page errors` attachment. Run with
  `DEBUG_E2E=1` for the debug log of the mock server and every console line live; without it the server logs warnings
  and errors only (`TEST_SERVER_LOG`, not `RUST_LOG`, which `.cargo/config.toml` forces to debug), and a green run
  prints none. Global setup installs Chromium when the version of Playwright has none; "Executable doesn't exist" of
  another browser: `pnpm setup:e2e`.
- **Types**: `pnpm lint:typecheck` isolates TypeScript errors.

## Agent Pitfalls
Mistakes agents made here; each cost a session.
- **One `pnpm e2e` per checkout**, even on two ports: a global setup that finds the frontend changed runs `pnpm build`,
  which empties `dist/` under the other run (timeouts in page setup, not test failures).
- **`CI=1` does not speed up E2E**: it means 1 worker, 2 retries and no reuse of a running server
  (`playwright.config.ts`). A test that fails only under parallel load is a race: fix the wait, don't serialize.
- **Same-second commits are the same commit**: a `git replay` or cherry-pick of a commit onto its own parent within one
  second writes the identical object, so "the copy" is the original. A test that needs a new copy makes the target
  diverge first (one more commit) or uses `cherry-pick -x`; a test that passes on a rerun is often this.
- **React Compiler and store reads**: a function that reads stores with `getState()`, called in the render body beside
  `useStore`, is memoized on its arguments and shows a stale value. The value must come out of a selector, or be an
  argument the hook selected: `useStepInputs` beside `useRowPart(name, "landing")` (`src/hooks/useBranchNextStep.ts`);
  a menu builder likewise (`useBranchMenu`, `src/hooks/useBranchContextActions.ts`).
- **The effects of a kept view run again**: a view of the main window (React's `Activity`, `KeepMounted` in
  `IndexPage`) destroys the effects of its components as it hides and runs them again as it shows, state and refs kept.
  An effect that acts once (a replay, a focus) marks it in a ref, as `useLazyRoot` does; a popup root closes as its view
  hides (`useCloseWhenViewHides`). WebKit, the engine of the app, keeps the focus on a hidden element until a later
  layout: test the focus and the popups in `pnpm e2e:webkit` too (`{ tag: "@webkit" }`).
- **Content that moves under a still pointer**: WebKit tells the over and the enter of the element that came under it
  at once (trusted, at the same position), and moves the pointer where it is after a scroll. A hover opens a popup only
  after a move over its trigger (`hoverOpensOnMove`, `src/lib/pointer-moves.ts`); a control that renders itself anew
  as another element keeps it only by its `data-control-key`, never by its place. Its E2E test brings a trigger under
  the pointer with `bringUnderStillPointer` and runs the delay with `page.clock.fastForward`.
- **Typing into a Base UI dialog in E2E**: wait for `toBeFocused()` first; `press`/`type` on an unfocused input puts the
  caret at 0 and the text lands at the start.
- **zsh does not word-split `$args`**: `args="--no-default-features --features=x"; cargo build $args` passes one
  argument. Write the arguments out, use `${=args}`, or run the loop under `bash -c '…'`.
- **An "unresolved import" of a module that clearly exists**, after another session moved branches under the same
  `target/`, is a stale cargo fingerprint: `touch` the crate's files and rebuild.
- **One stash and one rerere cache for every lease**: `refs/stash` and `.git/rr-cache` belong to the common git dir, so
  every worktree of the lease pool shares them, and a `git stash pop` in one lease can apply the entry another lease
  pushed meanwhile. Rerere is on without any config, because the `rr-cache` directory exists (132 resolutions on
  2026-10-29): a resolution recorded at a commit in one lease is replayed into the same conflict of a `git rebase`,
  `cherry-pick`, `merge` or `stash pop` in another ("Resolved '<path>' using previous resolution.", the file left for
  `git add`); a conflict of `bd lease pick` opens its resolve session, which rerere never touches. Never `git stash` in
  a lease: keep work in progress as a WIP commit of the lease or a patch file in `notes.local/`; after a conflicted
  rebase or cherry-pick, check every file rerere resolved as if you wrote it.
- **Throwaway Playwright specs** (screenshots, debugging) go into `shots.local/` of the worktree (`*.local` is
  ignored), with a config there that spreads `../playwright.config` and sets `testDir: "."`, `globalSetup:
  "../tests/e2e/setup/global-setup.ts"` and `webServer[].cwd: ".."`; run `npx playwright test --config
  shots.local/shots.config.ts`. Delete them before a commit: `pnpm lint:frontend` lints them too. The README images
  are no throwaway: `pnpm screenshots:readme` writes them.

## Development Rules

### Specs and Docs
Specs (`spec/<area>/<name>.spec.md`) and design docs (`docs/design/`) are part of the change, not a follow-up.
- **Before** changing a feature, read its spec and the design doc it links: the spec is the contract, the doc the why.
- **Same commit**: a behaviour change updates its spec: requirements, `[@test]` links, `targets:` front matter and the
  UX tokens. A renamed or moved test or file updates every link to it; grep `spec/` and `docs/` for the old name.
- **New user-visible feature**: a new spec in the air format (copy the structure of an existing one), a row in the
  [Feature Index](#feature-index), plus at most one sentence and a spec link in the matching README "Features" group;
  the detail belongs in the spec. README is the pitch for a newcomer: no new bullet per feature, no shortcut lists,
  nothing internal to one company (Patronus, Space).
- **Git**: a new git operation or a changed flow gets its section in [Git Operations](docs/design/git-operations.md);
  a new or changed hard rule also goes into "Git Operations" below.
- **AGENTS.md** holds the rules an agent must not miss and pointers; the detail of a feature goes into its spec, of a
  git flow into the design doc, of a one-off procedure into `docs/guidelines/`.
- **Truth**: docs describe the code as it is. A doc that contradicts the code is a bug: fix the doc (or the code) in
  the same change; never leave it for later. See [Design Docs Guidelines](docs/guidelines/design-docs.md).

### E2E Tests
How to write a Playwright test; the infrastructure (mock IPC, templates, sub-windows, snapshots) is in
[E2E Testing Architecture](docs/design/e2e-testing.md).
- **Invoke every surface**: an action reachable by a key on the row, a menu item, a footer button, Enter in a tree, a
  toast action gets one step per surface that presses or clicks it and asserts the effect. A check of a menu item's
  label and key hint (`toHaveText("Review ChangesD")`) is not a test of the action: the commit "Review Changes" item
  shipped dead behind one. Labels and hints belong to the HTML snapshots.
- **Assert the effect**: what the user sees (text, focus, `aria-*`, a closed dialog) or what reached the backend (the
  command's result on the next screen, `getOpenedSubWindows`, `getEmittedEvents`, the counts of a fixture route), not
  the internal state of a store.
- **No vacuous negatives**: "nothing happened" (`toHaveCount(0)`, `toBeHidden()`) passes when the action never ran or
  the element was never there. Anchor it: assert the thing existed first, or assert on the recorded list
  (`getOpenedSubWindows` is `[]` after the press), or follow it with a positive step the same press would have blocked.
- **Wait for the state, never for time**: wait for the element or the event a state comes with (`runAndWaitForSync`,
  `data-sync-count`, the patch of a version a command answered by `data-view-version`, `waitForViewVersion`, an
  `aria-busy` that clears); an app timer runs with `page.clock.fastForward`, a timer of the session (a poll, a
  retry, the sweep) with `repo.advanceBackendClock(ms)`, a debounced
  settings change with `waitForStorePersistence` (the key as the writer answered it, as a reload reads it). No
  `waitForTimeout`: under load
  (several checkouts running e2e and cargo) any fixed wait is too short; Oxlint refuses it in `tests/`, and a
  `setTimeout` in a spec, a fixture or a helper. A box or a point measured for a press waits for the facts that move
  it: Patronus answers after the sync and moves the header row (`waitForHeaderFacts`). A chain of commands gets a wait
  per step, each of which takes seconds under load: a rewrite and its sync `runAndWaitForSync`, Update Project
  `runAndWaitForUpdate`, a start `runAndWaitForAnswer(page, "execute", …)` (the end of its stream), the facts of a next step
  `waitForStepFacts`, any other step `nextAnswer(page, command)` before the assertion that reads its answer. The press
  goes into the wait (`runAndWaitForAnswer(page, command, () => button.click())`) or comes after `nextAnswer`: a wait
  that starts after the press misses a quick answer and holds the test until its timeout.
- **Close a read toast** before acting where it lies (`closeToast` of `tests/e2e/helpers/toast-helpers.ts`): the toasts
  lie above menus and dialogs at the bottom right, and a click or hover under one waits until it times out (5 s, 10 s
  with an action); five tests spent 7-9 s each this way.
- **Locators**: role and accessible name first (`getByRole("menuitem", { name: "Review Changes" })`), `getByTestId` for
  what has no role or name; never CSS classes or DOM shape, which the theme and the React Compiler change.
- **Through the user's path**: open, select and press like the user (hover, focus, right-click, keys); call a command
  directly only to set up a state the user path cannot reach in a test. An error path fails the command with
  `routeCommandError`, not by breaking the repository.
- **Remote answers come from the fakes**: Patronus, GitHub and YouTrack are fakes of each test repository behind the
  app's real services; a test sets their fixture (`setPatronus`, `setPullRequests`, `setYouTrack`, …) and asserts what
  the fake got (`getPatronusLog`, `getGitHubLog`, `getYouTrack`). The page before an answer holds it at the fake
  (`holdRemoteAnswers`, waited for with `getArrivedAnswers`), so the request has reached the backend. A page answer of a write (`routeCommand`, a
  fulfilling `page.route`) fails `command_registry_test.rs`, and each read's page answers are pinned and only go down
  ([Remote services](docs/design/e2e-testing.md#remote-services)).
- **Linked**: every requirement of a spec names its test (`[@test]` with the exact test title, the link a line of its
  own); a renamed, split or deleted test updates its links in the same commit, which `pnpm lint:specs` checks.
- **Snapshots**: `captureHtmlSnapshot` for structure; `pnpm e2e-up` rewrites them, and the diff is reviewed like code: a
  changed snapshot can hide a regression.

### Commits and History
How agents commit to this repository; "Git Operations" below is about the git the app runs.
- **Linear history, no merge commits**: not of master, not of an integration branch, not of an agent worktree branch.
- An agent lands the commits of its lease with `bd lease pick` (Session Workflow, step 5), never with
  `git cherry-pick`, which leaves `CHERRY_PICK_HEAD`. By hand, bring a branch up to date with `git rebase <target>`
  and integrate it with `git merge --ff-only <branch>`. Rebasing your own branch in your worktree is fine: the
  "never write to the working tree" rule is about the app's code.
- `.githooks/` enforces it (`pre-merge-commit`, and `pre-commit` while `MERGE_HEAD` exists). Enable it once per clone,
  every worktree shares the setting: `git config core.hooksPath .githooks && git config merge.ff only`.
- Never `--no-ff`, never `--no-verify`, never `commit-tree` with two `-p`.

### Git Operations
Hard rules of the git the app and `bd` run; the flows and the reasons are in
[Git Operations](docs/design/git-operations.md) (2600 lines: open the section a rule links).
- **Git 2.54+** (`MIN_VERSION` in `crates/git-executor/src/git_info.rs`), checked when git is discovered, on every platform.
- All git runs through `GitCommandExecutor` ([Executor](docs/design/git-operations.md#executor)). It adds
  `-c core.quotePath=false -c color.ui=never -c log.showSignature=false -c i18n.logOutputEncoding=UTF-8`, sets
  `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0` (only the `status` of a lease slot may write its index), strips inherited `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/
  `GIT_NOTES_REF`, empties `GIT_NOTES_DISPLAY_REF` (`%N` shows only the mapping notes, or an explicit `--notes=<ref>`)
  and gives git a null stdin unless the call pipes one; callers don't add `--no-pager`. A git with an environment,
  stdin, accepted exit codes, a timeout, streamed stdout, progress or a reply protocol is `git.cmd(&args, repo)` with
  `.env`, `.stdin`, `.accept`, `.timeout` and one finisher (`.text`, `.bytes`, `.lines`, `.output`, `.status`,
  `.stream`, `.stream_progress`, `.progress`, `.interactive`), all on one spawn core; the only shorthands are `execute_command` (`.text`),
  `execute_command_lines` (`.lines`) and `execute_command_with_status` (`.status` as text). The executor knows no git
  beyond that: a rev or ref fact (`resolve_tree_id`, `ref_exists`) is a git-ops function. It carries what its operation
  read once (`with_fact`): the repository's directories (`git_ops::revs::with_dirs_of`), so a step asks `known_dirs`,
  `git_common_dir` or `current_worktree` instead of its own `rev-parse`. Test fixtures run git through
  `test_utils::git_test_utils::git_command()`, which removes the repository variables a hook or `rebase --exec`
  exports (`git_command_test.rs` fails on a raw `Command::new("git")`).
- **Scan tests** (`crates/git-executor/src/git_usage_scan_test.rs`, on the sources as `source_scan_test.rs` reads them
  for the worktree safety scan too): git starts only in `GitCommandExecutor` (and `git_info.rs`'s discovery); the bd
  crates (`crates/bd-*`) and src-tauri start no git of their own; no `--no-pager`, no `credential.helper=`; `update-ref` only in
  `ref_transaction.rs`; `cat-file --batch*` only in `cat_file_batch.rs`; `--name-only`, `--name-status` and `--raw`
  only with `-z`; paths after `--` only with `--literal-pathspecs`; `push`, `fetch` and `ls-remote` only through
  `execute_remote_command*`. An exemption names its file, its reason and its exact number of hits.
- **Never write to the working tree**: no `checkout`, `checkout-index`, `switch`, `reset`, `rebase`, `stash`, `commit`,
  `cherry-pick`, `merge`, `restore`, `clean`, `apply` without `--cached`, `read-tree -u`/`--reset`, `git history`;
  `crates/git-executor/src/worktree_safety_test.rs` enforces it. Reading worktree files into a temporary
  `GIT_INDEX_FILE` is fine. Besides the apply of `bd-host run --apply` (`crates/git-ops/src/host_client.rs`: the patch of a remote run
  the user asked to apply, all or nothing, the index left alone), the single exception is the guarded two-way `read-tree -m -u` in
  `crates/git-ops/src/worktree_update.rs` (dry run on an index copy first, then the CAS ref move, rollback on failure),
  `pub(crate)`, called only by `update_branch.rs`: for Update Project (the app, `bd update`, and the update of a lease
  onto its home branch, detached, after a conflict of `bd lease pick`), its undo, the finish
  of a resolve session (`update_resolve.rs`), `bd pick` (`pick.rs`, also the worktree of `--follow`) and its undo, and
  the unapply and the reapply of a virtual branch (`unapply.rs`, `reapply.rs`) and their undo
  ([Invariants](docs/design/git-operations.md#invariants)). The scan
  counts its `-u` argument lists and the files that call the moves (`MOVE_CALLERS`). Don't add another caller, another
  argument list, or a public way to build a move. A slot of the lease pool is not the user's working
  tree: its `checkout --force`, `reset --hard` and `clean -fd` live in `crates/git-ops/src/lease_slot.rs` only, each
  counted by the scan ([Lease](docs/design/git-operations.md#lease)).
- **The user's index** changes only where HEAD's tree changed (`IndexUpdate` in `crates/git-ops/src/rewrite_guard.rs`):
  a drop runs `read-tree -m -i <old> <new>` after a preflight on a copy of the index, an amend stages the amended files.
  Staged work on other paths survives; never `reset --mixed`, never ignore an index error.
- **Index lock**: a write of the user's index (a move the files follow, `IndexUpdate::apply`) goes through
  `git_ops::index_lock`: it waits up to 30 s for the `index.lock` of another process (an IDE's `git status`),
  removes one that no process owns (a killed git's), and answers `IndexLocked` (`locked`) after the wait; never a
  removal of any other lock file of git ([Index lock](docs/design/git-operations.md#index-lock)).
- **Ref moves**: one `RefTransaction` (`update-ref --stdin`) with old-value CAS and a `branch-deck: <op>` reflog
  message; related refs (unapply rename, temp refs) go into the same transaction. The named exceptions: a notes ref
  that `fast-import` writes (`write_notes_to_refs`, `write_commits_with_notes`, `write_notes_on_existing`: `from <ref>^0`, and a tip that does not
  contain the ref's value fails the run, so a concurrent writer is never overwritten; the cache notes of a sync and of
  the detection and the notes a squash carries to the commits it replays run once more from the new tip, then only
  warn, `write_cache_notes`, but an identity git cannot form (`NoIdentity`) fails the run at once (a sync with
  `no_identity`: no later run could write them); the mapping notes a rewrite carries
  once more, then fail it; the resolve notes move in the
  session's transaction); the temporary ref of a sync chain's commit writer (`commit_chain.rs`): one per writer
  (`unique_temp_ref`), deleted in its own `fast-import` stream after a `checkpoint` wrote it (`reset` from the null
  id), and a sync deletes the stale temporary refs its listing found (`is_stale_temp_ref`); and the refs of the review
  fetch (below). A move that the files follow (an update and its undo) updates `HEAD` itself (deref) and commits only if
  `symbolic-ref HEAD`, read while git holds the locks after `prepare`, is still the checked branch
  (`RefTransaction::commit_while`, else `BranchSwitched`); git refuses `symref-verify HEAD` next to either form of the
  update. The finish of a resolve session moves the session ref and the notes ref (`refs/notes/branch-deck/resolve`,
  written only there) in that transaction.
- **Who writes the refs of Branch Deck**:
  - `refs/branch-deck/issue-groups` and `refs/branch-deck/stack-parents`: two refs, never one tree (a store commit
    replaces its whole tree). Only a regroup writes them, under the repository lock, in the ref transaction that moves
    HEAD, with CAS on every ref it moves; a sync only reads them, except that it drops the stack parent of a branch whose
    archived branch is integrated (one CAS write, best effort, only while the repository lock is free,
    `drop_integrated_stack_parents`)
    ([Regroup](docs/design/git-operations.md#regroup-rename-restack)).
  - `refs/branch-deck/parked` (`parked.rs`, a ref of its own too): park and unpark under the repository lock with CAS,
    a rename in its regroup transaction; a sync only drops the line of a branch whose archived branch is integrated
    (`drop_integrated_parked`, as the stack parents). It decides no group: only the pushes leave a parked branch out
    ([Regroup](docs/design/git-operations.md#regroup-rename-restack)).
  - `{prefix}/unapplied/<name>` (`unapply.rs`, `reapply.rs`): only the unapply, the reapply and their undo, in the
    transaction that moves HEAD, and its delete (`delete_unapplied_branch_core`, CAS); a sync only lists it, and a push
    of it goes under its own name ([Drop and unapply](docs/design/git-operations.md#drop-and-unapply)).
  - `refs/branch-deck/conflict/<sha256 of the branch's full name>` (`conflict_chain.rs`): only a sync, in one CAS
    transaction after the branches, only when one changes; a failed move fails no sync
    ([Virtual branch sync](docs/design/git-operations.md#virtual-branch-sync)).
  - `{prefix}/stack/<root>` (`stack_branch.rs`): only a sync, for a stack that forks, with CAS on the tip it listed and
    only when every member was built; deleted once the stack stops forking, never while it has a remote branch or a
    worktree has it checked out. Only a `Built` one is pushed: by the Safe Push of the whole stack (its restart, the
    creation of its pull request, Request Review) and by Push with Children of the root once it is on the remote or its
    pull request is open. Its build is remembered by the base and the original ids (`StackBuilds`), never by
    `CommitKey`, which an amend keeps ([Stack branch](docs/design/git-operations.md#stack-branch)).
  - `refs/branch-deck/review/<n>/{head,base}`: only `review_fetch.rs` (the intent `FetchReview` of the session),
    through its own fetch refspecs; always `--refmap=` and `--no-prune`, so no remote-tracking ref moves; never under
    the repository lock; no fetch when both commits are local; killed on `cancel_operation`, the close of its window or
    120 s of silence; pruned by the session at each change of its review windows, with one `for-each-ref` and one CAS
    `update-ref --stdin`. Range diffs (`DiffSubject::Range`) never go into the object cache (`RangeFilesCache` in
    memory) ([Review fetch](docs/design/git-operations.md#review-fetch)).
  - `refs/branch-deck/resolve/<worktree id>` (a shared ref: a gc in another worktree prunes a `refs/worktree/` ref):
    the state of a resolve session. The copies an agent edits (`resolve_copies.rs`) live only in `branch-deck/resolve/`
    of the worktree's git dir or in `--dir` (absolute, not a symlink, outside every worktree, empty or marked for this
    git dir, used by no other session; the marker `.branch-deck-resolve` names the owner's git dir); flat names
    `<key>-<basename>`, never tree paths; tree paths validated (no empty, `.`, `..` or `.git` component, nor an
    NTFS/HFS+ alias of `.git`); regular files only, bytes as git stores them without filters, never read, written or
    deleted through a symlink, every write a temporary file plus a rename; delete only the files bd wrote, only in a
    directory whose marker names this git dir ([Resolve session](docs/design/git-operations.md#resolve-session)).
- **Repository lock**: every rewrite of the checked-out branch takes it in its core, only for the witness and the
  write. The regroup (`regroup.rs`: the move, rename, restack, unstack and the conflict fix, also `bd group`,
  `bd reword` and the app's reword through `reword.rs`) and its undo, park and unpark, the squash (the app's and
  `bd squash`) and its undo,
  squash fixups (also `bd fold`), amend, add issue reference, the move of a hunk (`bd move-hunk`), unapply, reapply,
  the undo of an update, of a pick, of an unapply and of a reapply read and compute before it, on the oids they read
  once (the snapshot or the history, merge-tree, replay, the dry run of the files on a copy of the index, the
  `fast-import` of the new objects on temporary refs), and hold it only for the witness of those refs and the CAS
  write (`lock_prepared`: HEAD, its branch and the refs they computed on in one `for-each-ref`; a move that the files
  follow reads the mark of an interrupted move with them and lets those files follow first, `lock_prepared_move`,
  `lock_checked_undo`; a write of the stores alone has the compare-and-swap of its transaction as its witness); a ref
  that moved meanwhile, HEAD between two reads of the computation too, computes the rewrite once more under the lock,
  where another HEAD than `expect_head` is `HeadMoved`. All through their options (`RewriteOptions` or `RegroupOptions`: `held_lock`, `lock_wait`, `expect_head`
  checked by `ensure_expected_head`, `repository_lock.rs`; a move, the conflict fix and an undo take a `LockWait`
  beside the check their request carries); then `bd push` and
  `bd safe-push` (only their sync, then again the witness of their refs and the freeze of their push in the engine,
  `operations::land_execute`; the reads of the remote, the trial, the retargets, the cancel, the network push and the
  robot start without it; a ref that moved meanwhile is `plan_changed`), the app's pushes (`execute` of the intent
  `Push`, `operations::push::execute_push`: of a branch alone, of a stack by `domain_policy::push::push_plan`, up to a
  commit, of an unapplied branch, of the stack branch of a whole stack; admitted per ref in the registry of the
  session, the user's robots before the lock, the refusals of parked, landed, held and not built branches in its plan,
  the witness and the freeze of its refs under the lock, the network push of those commits after it, side by side
  with the app's other pushes; only a push of the same remote ref waits, in the registry),
  `bd move-hunk`, `bd undo`,
  Update Project (the app and `bd update`: after the fetch and the check, for the witness, the local changes and
  the move; a conflict that starts a resolve session computes its first round before the lock too and holds it for the
  witness, the state with the session ref and the copies of the round; the landed report after the release), `bd pick` (after its check:
  the witness and the moves of both worktrees under one hold, its landing report after it), `bd unapply`, `bd reapply` and their undo, and `bd update --continue` (not its `--dry-run`) and `--abort` hold
  `$GIT_COMMON_DIR/branch-deck.lock`
  (`bd`: `LockWait::Bounded`, then `locked`; the app's rewrites, pushes and Update Project: `LockWait::WhileHeld`, as
  long as the holder record names a live process: an operation of the engine, every rewrite of `operations::rewrite`,
  Update Project of `operations::update` and a push, says whom for in its step `WaitingForLock { holder }`, stopped by
  `cancel_operation`), because
  the app and `bd` processes rewrite the same branch. Before the file lock, the operations of one process wait for
  each other in its line (a fair queue per lock file, `repository_lock.rs`, without a bound), so `locked` names only
  another process; `try_lock_repository` waits for nobody, `lock_as_another_process` is for tests and the E2E server. A sync never waits for it:
  neither the app's, nor `bd sync`, nor the `--sync` of a `bd` rewrite, which syncs after the release; only its drop of
  store lines takes it, while it is free (`drop_store_lines`), and skips the drop when it is held. A fetch never runs under it ([Regroup](docs/design/git-operations.md#regroup-rename-restack)).
- **Operations registry** (`$GIT_COMMON_DIR/branch-deck/operations.json` and its `operations.lock`): only
  `operations::registry_file` reads or writes them (`registry_scan_test.rs`); the lock is held for one
  read-modify-write, never across git, HTTP, an `await` or the actor of a session; an operation (every one of the
  app's, `bd push`, `bd safe-push`, `bd request-review`) is recorded before its session admits it, a rewrite, an
  update or a pick of `bd` before it takes any lock (`bd_core::operation_record`), and an operation of another process
  that holds what it needs makes it `busy` (exit 3), never one of its own process, HEAD never (the repository lock
  orders the rewrites of the processes); no clean rule deletes the lock. A window's subscription watches the
  directory (`operations::registry_watch`, `notify`, no git): the operations of other processes show in its session
  and decide no admission there; `bd` touches
  `$GIT_COMMON_DIR/branch-deck/stamp` after every command that succeeded and is no read, in one place
  (`bd-cli::execute`), which syncs the watching sessions (`SyncReason::Stamp`). A start records what it sends before
  its request goes out; an answer lost after it went out is `EffectStatus::Unknown`, never sent again: its record stays
  `unknown` and holds for every process until a read of Patronus settles it (`operations::reconcile`, at an admission
  and after a list of the robots) or the user forgets it (Forget This Start, `bd operation forget`)
  ([Operations registry](docs/design/git-operations.md#operations-registry)). The journal beside it
  (`journal.jsonl`) has one writer, `operations::journal`, called by the registry under `operations.lock` as a record
  leaves: one entry per operation that ended, of the app and of every `bd`, with what it cost from its admission
  (`sync_utils::operation_tally`), its id the `operation_id` of its log lines and of the JSON of `bd`; the same module
  writes the journal of the page in the app data directory (`record_page`, under `journal.lock`) and empties both for
  Clear Log (`clear`); Activity reads them, no window keeps a log ([Journal](docs/design/git-operations.md#journal)).
- **Guards** (`History::check_rewrite`, `check_rewrite_range`, `check_rewrite_on`) before rewriting the checked-out
  branch, with the rest of the computation before the repository lock, whose witness checks what they read: no
  rebase/merge/cherry-pick/revert in progress, HEAD not detached, no merges in the range (refuse, no rebase fallback).
  A sync (the app's and `bd sync`) is refused under the first two as `SyncDeferred` (`check_sync_guards`): a half-done
  history would archive the branches it lacks.
- **Metadata**: message-only rewrites (reword, move) keep author and committer byte-for-byte (`commit_writer`:
  `cat-file --batch` + `fast-import`); content rewrites (amend, drop via `git replay`) keep the author, committer =
  current user. The one named exception: `bd move-hunk` (`move_hunk.rs`) keeps author and committer byte-for-byte for
  every commit whose diff stays the same (`patch-id --stable`), so their virtual commits stay; `from`, `to` and the
  commits whose diff of the file changed get committer = current user and no mapping note. Signatures are stripped.
- **Batch**: one process per phase, not per item (`--stdin`, `cat-file --batch`, `fast-import` for commits and notes,
  `update-ref --stdin`). A chain whose next step needs the answer of the one before keeps one interactive process per
  chain: a sync merges a branch in one `merge-tree --stdin` and writes its copies with one `fast-import`
  (`commit_chain.rs`), never `commit-tree` per commit; `with_process_stats` counts the processes of an operation. A
  check of an update finds the first conflicting commit with a binary search over prefixes, each probe from the longest
  clean prefix: ⌈log₂ n⌉ `replay` runs; a resolve session opens its first round there, then one `replay` of the rest
  per round. Never parse translated text: exit codes, `--porcelain`, `-z`. The one named
  exception: a branch description costs one `git config` per changed branch (`bd describe`, `descriptions` of a
  `bd group` plan), because git has no batch config writer; read them with one `--get-regexp`.
- **Exit codes**: `merge-tree` and `replay` exit with 1 on conflicts; `merge-tree --stdin` exits with 0 even then, and
  each merge of its output starts with its status, 1 clean, 0 conflicted (`parse_batch_merge`); `merge-base` exits with
  1 for "no" (`--is-ancestor`) or "none" (no merge base, no fork point), and any other non-zero is an error, never a
  "no" (`git_ops::revs`).
- **Network commands** (push, fetch) run through `execute_remote_command` (`crates/sync-core/src/remote_command.rs`):
  the user's credential helpers stay, prompts are off, stalled HTTP transfers abort, the locale is C (untranslated
  `--progress` lines). Never pass `-c credential.helper=`.
- **The branch of the baseline on its remote** (`master` of `origin`) takes only the push of a start of a repository
  that lands by a push (`sync_core::baseline_push::push_baseline`, run by `land_execute` after the lock): the copy as
  the witness froze it, `<tip>:refs/heads/<target>`, no `+`, no lease (the server refuses what is no fast-forward), by
  the first push URL of the remote with its `url` and `pushurl` cleared (`-c remote.<r>.url=`), so git takes no
  configured remote for the URL and moves no remote-tracking ref: only `fetch_baseline` writes
  `refs/remotes/origin/master`. A pre-push hook sees the URL; a second push URL, `receivepack` and `proxy` play no
  part ([Push into the baseline](docs/design/git-operations.md#push-into-the-baseline)).
- **Fetch lock**: the baseline is fetched only by `sync_core::baseline::fetch_baseline` (the app's Update Project and
  Fetch, `bd update`), which holds `$GIT_COMMON_DIR/branch-deck/fetch.lock` for its fetch (`fetch_lock.rs`, its one
  taker: `registry_scan_test.rs`), its record naming the tracking ref: a second fetch of any process waits while the
  holder lives and takes its result when the ref moved, one transfer; no retry on `cannot lock ref`, no clean rule
  ([Update project](docs/design/git-operations.md#update-project), Fetch lock).
- **Git config writes**: only the `branchdeck.*` keys of Settings → Repository, Diffs & Merge, Landing and Agents
  (`repository_config.rs`: one `--get-regexp --show-scope` read, `--replace-all` or `--unset-all`, refused when the
  worktree config or `git -c` sets the key), `--local` (`.git/config`), or `--global` only where the request names the
  global scope (`set_repository_config` with `scope: "global"`); and the branch descriptions of `bd describe`
  ([Repository settings](docs/design/git-operations.md#repository-settings)).
- **GitHub credentials**: the only git of the GitHub token chain is `git credential fill` (`crates/github/src/token.rs`),
  a `GitCommand` with `.timeout` (10 s) and `.status`, with `credential.interactive=false`, an empty `GIT_ASKPASS` and
  `GCM_INTERACTIVE=never`; never `credential approve` or `reject`. GitHub itself is asked over HTTP (`GitHubService`),
  never by a `gh` process; the app's own sign-ins (GitHub, YouTrack) keep their tokens through
  `sync_utils::secret_store`: on macOS a login keychain item written and read by `/usr/bin/security` (the secret on
  stdin, as `gh` does; the item trusts `security`, so an update of the unsigned app asks nothing), elsewhere or
  while the keychain can't be used a 0600 file of the app data dir; `bd` reads them too
  ([GitHub credentials](docs/design/git-operations.md#github-credentials)).
- **Symbols** (`symbol_scan.rs`): source blobs by id from the commit diffs, the declarations the range adds or changes
  as the index, never the whole tree; the summaries of trial blobs stay in memory, never in the object cache
  ([Symbol summaries](docs/design/git-operations.md#symbol-summaries)).

### Adding a git operation
Read [Adding a Git Operation](docs/guidelines/adding-git-operation.md) (the layers in detail, the helper table) before
you add or change git code in `crates/git-ops/`, `crates/history-rewrite/`, `crates/sync-core/`, `crates/update-core/`,
`crates/trial-check/`, `crates/lease-pool/`, `crates/remote-facts/`, `crates/repository-model/`, `crates/operations/`,
`crates/bd-*/` or `crates/app-api/`.
The checklist:
- One name across the layers (`git_ops::move_hunk`, `trial_check::move_hunk`, `bd move-hunk`); the `//!` headers of
  `crates/git-ops/src/lib.rs`, `crates/history-rewrite/src/lib.rs`, `crates/sync-core/src/lib.rs`,
  `crates/update-core/src/lib.rs`, `crates/trial-check/src/lib.rs`, `crates/lease-pool/src/lib.rs`,
  `crates/remote-facts/src/lib.rs`, `crates/repository-model/src/lib.rs` and `crates/operations/src/lib.rs` map the
  modules by area.
- **git-ops** has the git: one process per phase, `-z` output, full ids, `--end-of-options` before a revision from
  outside, `--literal-pathspecs` for paths after `--`; no lock and no policy.
- **The core** `<op>_core` in `crates/history-rewrite/` (a rewrite that needs no sync, GitHub or Patronus),
  `crates/update-core/` (Update Project, `bd pick`, the resolve sessions), `crates/trial-check/` (the trial checks,
  `bd move-hunk`, whatever reads the diffs or the symbols of the commits), `crates/lease-pool/` (`bd lease`,
  `bd workspace`), `crates/remote-facts/` (what GitHub and Patronus answer over HTTP, and the writes to them),
  `crates/repository-model/` (the branch table Rust derives, the guards of the app's push and robot actions) or
  `crates/sync-core/` (the sync, the pushes and the rest, with no remote service but git's) is the one
  entry of every caller: a rewrite of the checked-out branch
  takes `&RewriteOptions`, reads and computes on the oids it read once (checking the expected HEAD there), then takes
  the lock for the witness of those refs and the CAS write (`RewriteOptions::lock_prepared`).
- **operations**: an action on the remote, a rewrite of the checked-out branch and Update Project are an intent of its
  family in `crates/operations/` (`prepare` and `execute` of `engine`; a rewrite and an update are `execute` alone): it
  runs in `engine::run`, admitted by `registry` over the resources it names (`domain_policy::admission`), its steps the
  `operations` part of the view, and checks its `witness` before the first effect (`plan_changed`). A write
  to Patronus, GitHub or YouTrack and the leased push are effects of a plan, called only from `crates/operations`
  (`remote_write_scan_test.rs`, exact counts per file). What
  its plan reads of the remote is `needs::needs(intent)`, read by `facts::fill` through the one reader of every host
  (`remote_facts::service_sources::ServiceFactSources`) into its session; `bd`'s is a one-shot session of its sync
  (`repository_model::one_shot::OneShot`, its services `bd_core::services::BdServices`), never a reader of its own.
- **bd** and **the app** parse, call the core and present its result; no git of their own. A `bd` command answers one
  JSON object with `schema`, errors as `CliError` with an `ErrorCode`, and records a rewrite, an update, a pick and an
  operation of the remote in the operations registry (`bd_core::operation_record`); a command of the app is one entry
  of the registry of app-api (`crates/app-api/src/commands.rs`): a read `run_blocking` of the core, a write `execute`
  of its intent or a command of the app, `app(<reason>)` (`AppReason`: what has no repository as its subject); the
  app's Tauri wrapper and the E2E server's route come from the entry and run the same body.
- **Reuse a helper** before writing a parser or a process: `ref_transaction`, `commit_writer`, `commit_chain`,
  `cat_file_batch`, `object_texts`, `history`, `rewrite_guard` and `revs` in `git_ops`; `snapshot`, `repository_lock`
  and `regroup` in `history_rewrite`; `repo_facts`, `baseline` and `remote_command` in `sync_core`.
- **Same commit**: the spec, the flow's section in [Git Operations](docs/design/git-operations.md), and a test in
  `process_budget_test.rs` that pins its processes by subcommand; the scan tests stay green.

### Code Style
- **Colors**: shadcn semantic tokens only (`text-muted-foreground`, `bg-background`, `bg-muted`, `border-border`, `text-primary`,
  `text-destructive`) plus the custom `canvas`, `success`, `warning`, `info` (`text-success`, `bg-warning/10`); the one
  exception is the `window-button-*` palette of the Linux window buttons (IntelliJ's icons, `WindowControls`). **Never** use
  `dark:` variants; the theme is handled by the tokens, which are the IntelliJ Islands values (`src/index.css`).
- **Window and cards** (Islands layering, see [UI Theme](docs/design/ui-theme.md)):
  - the page canvas is `bg-canvas`; `bg-muted` is a translucent tint (hover, chips, code), not a surface;
  - top-level blocks (tables, collapsible sections, alerts) are `Card` islands, or `CollapsibleCard`; the toolbar
    (`ConfigurationHeader`) lies on the canvas as the title bar ([Title Bar](spec/repository/title-bar.spec.md));
  - never put `bg-muted`/`bg-background` divs on the canvas, and never recolour a top-level `Card` with `bg-muted`;
  - nested panels inside a card use `rounded-lg` (`rounded-xl` at most).
- **Radii** come only from `--radius` (`0.25rem`) in `src/index.css`. Never add radius classes to `src/components/ui/*`
  or hardcode `rounded-[…]`. To change the feel, change the token. Prefer a coherent token-driven scale over copying
  IntelliJ pixel values.
- **Spacing**: `flex flex-col gap-*` on the parent, not `space-y-*` or child `mt-*`.
- **Font**: InterVariable from `inter-ui` (the full rsms build; `@fontsource-variable/inter` lacks `ss02`/`cv*`
  features), falling back to the system UI font, at 13px (`text-sm`, the IntelliJ UI size). Commit bodies enable `ss04`
  (I/l/1 disambiguation). `font-mono` is JetBrains Mono (`@fontsource-variable/jetbrains-mono`) without ligatures.
- **Icons**: lucide-react components (`<GitBranchIcon />`). Inside `Button`, add `data-icon="inline-start"` (or `inline-end`)
  and no size classes; the button sizes them. Elsewhere use `size-*` classes (e.g. `size-3`).
- **Components**: `Tooltip` for simple text tooltips, `Popover` for rich content, `Kbd` for keyboard shortcuts,
  `showToast({ title, description, action })` from `@/lib/toast-message` for notifications; a toast of an event of
  an object (a robot, a push, a merge, a review) is `showEventToast` of `src/lib/event-toast.ts`, one per event, its
  later states and their actions in place ([One toast per event](docs/guidelines/ux-writing.md#one-toast-per-event)),
  never a second toast of the same event. Every user-facing error
  goes through `notifyError` in `src/lib/errors.ts` (sticky toast, logged to the Tauri log plugin, recorded in Activity),
  which words it by its code (`describeError`: a held lock names its holder); a store decides by `error.code`
  (`canceled` toasts nothing), never by `error.message`.
- **Wording** ([UX Writing](docs/guidelines/ux-writing.md)): a toast title says what happened ("Pushed feature-ui"),
  never "Success"/"Error"; errors are "Cannot <verb> <object>" with the reason; a toast about an object opens it
  ("Open in Patronus"); Title Case for buttons, menu items and toast actions, sentence case for the rest; "…", "·",
  "canceled", no exclamation marks.
- **Actions** ([UX Writing → Actions](docs/guidelines/ux-writing.md#actions)): one builder of `MenuItem`s (stable `id`) per
  object, rendered by every surface; the same actions in every form; a key hint only where the key works; disabled
  with a reason, not hidden; one name per action ("Open Pull Request", "Restart Safe Push").
- **React Compiler**: its Rust port `oxc-transform-react` (`react({ compiler })` in `vite.config.ts`, no Babel)
  compiles every component and hook; a function it would skip fails the build (`panicThreshold: "all_errors"`): no
  disable comment for a React rule (`react/rule-suppression`; use `useEffectEvent`), no `try`/`finally` without
  `catch` (use `promise.finally`), no JSX parameter default ([Rendering Performance](docs/design/rendering-performance.md)).
  `oxc-transform-react` is pinned to the range `@vitejs/plugin-react` declares; `pnpm update-deps` leaves it alone.
- **State**: zustand vanilla stores (`createStore`) read with `useStore` selectors; select narrow slices: the compiler
  memoizes renders, not selectors, which run on every store change. A persisted setting is a `createPersistentStore`
  (`src/stores/settings/persistent-store.ts`): it starts from the snapshot the backend injects and changes only through
  the one writer in Rust (`set_setting` → `SettingsService` of `crates/app-settings/`), applying every window's changes
  by version; no window and no Rust code writes `settings.json` beside it, and a rule on a setting (a default, the
  branch prefix) is Rust's ([Distributed Settings](docs/design/distributed-settings.md)). Debounce user inputs where
  appropriate (`p-debounce`). An export of `src/stores` or `src/hooks` that no page code calls is deleted with the tests
  of it alone; the seams of the unit tests stay, which no page code calls by design: `reset…` of a store (`resetPatronus`,
  `resetPullRequests`, `resetReviewReadiness`, `resetReviewRequests`, `resetRobotDetails`: the state of its module
  between tests) and `whenRepositoryViewCurrent` (a test waits for the view it answered).
- **Base UI**: compose with the `render` prop (`<TooltipTrigger render={<Button />} />`), not `asChild`. Base UI emits
  `data-open`/`data-closed`; set `data-state` manually where E2E tests read it.
- **shadcn workflow**: components are generated into `src/components/ui/` (`components.json`, style `base-rhea`). Run
  the CLI with `npx shadcn@latest …` (`add <component>`, `docs`, `init --preset`), then adapt the files.
  `pnpm dlx shadcn@latest` crashes with `o.deepPartial is not a function`: `@shadcn/registry` uses the zod 3 API and
  pnpm dlx resolves zod 4.
- **Locally customised ui files**: `alert`, `badge`, `button`, `card`, `chart`, `checkbox`, `context-menu`,
  `dropdown-menu`, `input`, `popover`, `select`, `spinner`, `tabs`, `textarea`, `tooltip`, `toast` in
  `src/components/ui/` carry a `// local customisation: …` header naming the edits. After `add --overwrite` (e.g. a
  style switch), re-apply them; `git diff HEAD -- src/components/ui` shows what was lost.
- **Toolchain** (Vite+, `vite-plus`, the `vp` CLI): one pinned toolchain of Vite 8, Vitest, Oxlint and Oxfmt,
  configured in `vite.config.ts` (`lint`, `fmt`); Tailwind CSS v4 through `@tailwindcss/vite`, its theme tokens in
  `src/index.css`. Oxlint is type-aware through tsgolint (TypeScript 7, the Go compiler, strict, checked with `tsc` on
  `tsconfig.app.json`, `tsconfig.node.json` and `tsconfig.e2e.json`) and runs the React Compiler rules; there is no ESLint, because typescript-eslint cannot use
  TypeScript 7. Oxfmt formats code and config (not Markdown) and sorts imports and Tailwind classes. Test files import
  from `vite-plus/test`.
- **TypeScript**: Use `u32` for timestamps and `snake_case` for properties that bind to Rust structs.
- **Rust**: `snake_case` for variables, functions and modules; `tracing` for structured logging, `#[instrument]` to
  trace a function's entry, exit and arguments; proper `Result` types, not strings.
- **Operations and spans** ([Operation spans](docs/design/async-patterns.md#operation-spans)): every command of the
  app (a body of the registry of app-api; the app has no other) has `#[instrument]`, with `skip_all` and a field
  or two when an argument holds a collection; a command that can fail runs its body as an operation (`operation`,
  `operation_now`, or its one blocking step `run_blocking`; `crates/app-api/src/operation.rs`), so its error names the
  `operation_id` of its log lines; the object cache, the git directories and the GitHub repository only in that step
  (`crates/app-api/src/command_scan_test.rs`). A `session` command is the exception: its frame opens the span and the
  operation, so its body has no `#[instrument]` and no `operation`
  (`the_body_of_a_session_command_opens_no_span_and_no_operation_of_its_own`), and its entry names the fields of its
  span after its kind, beside `repository_path` (`session(reason = ?params.reason)`, `session(window)` for the window
  of the call; `the_span_of_a_session_command_records_the_fields_its_entry_names` of the e2e server). A
  closure or future handed to another thread carries the caller's span: `sync_utils::span::in_current_span(…)` or
  `.in_current_span()` (`crates/git-executor/src/thread_span_scan_test.rs`).
- **Clippy**: pedantic, nursery and cargo plus the restriction lints that catch defects (`[workspace.lints]` in
  `Cargo.toml`), on all targets; any warning fails `pnpm lint:backend` and CI (`-D warnings`), Windows included. An
  exception is `#[expect(lint, reason = "…")]` where it holds, never `#[allow]`; a lint that is wrong for this code is
  allowed in `Cargo.toml` with its reason.
- **A platform branch in a test** asserts on every platform: an `if cfg!(target_os = "macos")` has an `else` that
  checks what the other platforms do (a `worktree add` where no APFS clone is), never a branch that asserts nothing.
  A helper or import that only `#[cfg(unix)]` tests use carries `#[cfg(unix)]` too, or Windows clippy fails on it.
- **Removals and renames** of the file system go through `sync_utils::fs_retry` (`remove_dir_all`, `remove_dir`,
  `remove_file`, `rename`), never `std::fs`: on Windows a git or a shell whose current directory is in the path, or a
  scanner with a file open, fails them at once (`fs_retry_scan_test.rs`). A process that works in a test's directory is
  `test_utils::git_in_directory::GitInDirectory`.
- **A process** starts through `sync_utils::child_process::windowless_command`, git through `GitCommandExecutor`,
  never `Command::new`: the release app on Windows is a GUI program, and every console program it started got a
  console window of its own, a Windows Terminal window in front of the app for each git (`console_window_scan_test.rs`).
- **Rust imports** ([Import Guidelines](docs/guidelines/imports.md)): never `pub use` re-exports (except for internal
  module organization); make modules public with `pub mod`; every import uses the full module path, so it shows where
  a type or function comes from:
  - ✅ `use git_executor::git_command_executor::GitCommandExecutor`
  - ✅ `use domain_types::issue_navigation::IssueNavigationConfig`
  - ❌ `pub use pull_request::PullRequestAvailability` (in lib.rs)

### Frontend Imports and Naming
- Import every React API, component, hook, store, utility and type explicitly; types with `import type`.
- Project modules only through the `@/` alias (`@/stores/branch-sync`), never `../`. shadcn: `import { Button } from "@/components/ui/button"`.
- Generated Rust bindings: `import { commands } from "@/lib/bindings"` (written by Specta from `src-tauri/src/lib.rs`).
- Heavy or rarely shown components: `React.lazy` with the named export, rendered inside `Suspense`.
- **Components**: PascalCase `.tsx` files with a named export (`BranchTableCard.tsx`). **Hooks**: camelCase with the
  `use` prefix in `src/hooks` (`useRowSelection.ts`). **Stores**: `src/stores/*.ts` (`branch-sync.ts`), persisted
  settings in `src/stores/settings/`. **Rust-bound props**: `snake_case`, as in the generated bindings.

## Where Things Are

### Feature Index
Each feature's contract is its spec (paths under `spec/`, without `.spec.md`); read it before changing the feature.

| Feature | Specs | Entry code |
|---|---|---|
| Repository session: the table, the syncs, the remote tips and the facts of the remote services (robots, pull requests, readiness, review requests, read by the session's scheduler) of a worktree and branch prefix live in Rust; a window subscribes to its snapshot and patches, asks `sync` and `refresh_facts`, tells its focus and its demand (`set_demand`); `bd sync` folds its sync through the same `TableState`. The warm start: the frame of a session's last sync in the object cache, seeded before the main window is built and injected as `window.__BD_MODEL__` beside `__TAURI_STORE__`, its facts `Cached` and its remote actions `Disabled{Cached}` until the first sync | `repository/onboarding`, `branch/branch-review` ([Session facts](docs/design/git-operations.md#session-facts), [Warm start](docs/design/git-operations.md#warm-start), [Rendering Performance](docs/design/rendering-performance.md#loading)) | `crates/repository-model/src/session.rs`, `crates/repository-model/src/table.rs`, `crates/repository-model/src/facts.rs`, `crates/repository-model/src/warm_start.rs`, `crates/remote-facts/src/scheduler.rs`, `crates/app-api/src/session/view.rs`, `src-tauri/src/services.rs`, `src-tauri/src/warm_start.rs`, `src/model/store.ts` (`adoptCachedModel`), `src/model/demand.ts` |
| Integration status: archive the branches the baseline has (by default rebase/cherry-pick detection only; merge and squash by an environment variable) | `branch/branch-row` ([Integration Detection](docs/design/integration-detection.md)) | `crates/branch-integration/` |
| Context menus (the "⋯" menu on right-click, acting on the selection) | `branch/branch-row`, `commit/commit-row` | `src/components/shared/ContextMenuItems.tsx`, `src/hooks/useBranchContextActions.ts`, `src/presenters/actions.ts` (the words and looks of the actions `crates/domain-policy/src/actions.rs` decides) |
| AI branch naming (`claude -p --model haiku`, after consent) | `branch/branch-name-suggestions` | `src/components/ai/` |
| Squash and fixups (`bd squash`, `bd fold`) | `commit/squash`, `commit/fixups`, `cli/agent-cli` | `crates/history-rewrite/src/squash.rs`, `crates/history-rewrite/src/autosquash.rs`, `crates/bd-rewrite/src/squash_command.rs` |
| Message of a commit (`bd reword`; the app's edit of a message) | `cli/agent-cli`, `commit/commit-row` | `crates/history-rewrite/src/reword.rs`, `crates/history-rewrite/src/fixup_targets.rs`, `crates/bd-rewrite/src/reword_command.rs`, `crates/operations/src/rewrite.rs`, `src/hooks/useRewordCommit.ts` |
| Update Project (the intents `UpdateProject`, `FetchBaseline`, `DismissLanded`; `bd update`, resolve sessions, `bd undo update:`) | `repository/update-project`, `cli/update` | `crates/operations/src/update.rs`, `crates/update-core/src/update_project.rs`, `crates/update-core/src/resolve_session.rs`, `crates/git-ops/src/update_branch.rs`, `crates/git-ops/src/update_resolve.rs`, `src/stores/update-project.ts`, `update-progress.ts` |
| Pick between worktrees (`bd pick --from <worktree> --follow`); the commits someone else pushed to a remote virtual branch (`bd pick --remote`, Pick Remote Commits) | `cli/pick`, `repository/outgoing` | `crates/update-core/src/pick.rs`, `crates/git-ops/src/pick.rs`, `crates/sync-core/src/moved_remote.rs`, `crates/repository-model/src/moved_remote.rs`, `src/lib/git/pick-remote.ts` |
| Agent workspace (`bd workspace`): a lease and the `(branch)` subject by `branchdeck.agentLease` and `branchdeck.agentSubjectBranch`, Settings → Agents | `cli/workspace`, `repository/settings`, `repository/onboarding` | `crates/lease-pool/src/agent_workspace.rs`, `crates/bd-lease/src/workspace_command.rs`, `src/components/settings/AgentsPage.tsx` |
| Command-line tools: the `teamcity` and `youtrack` CLIs in Settings → Tools, installed and updated through Homebrew, winget or scoop, with their agent skills; the Setup dot on Settings (updates, expiring sign-ins) | `repository/cli-tools`, `repository/setup-attention`, `repository/settings` | `crates/cli-tools/`, `crates/sync-utils/src/known_tools.rs`, `crates/app-api/src/app/cli_tools.rs`, `src/components/settings/ToolsPage.tsx` |
| Landing routes: the route into master (`branchdeck.landing`: Patronus, a push into the target, a merge of the pull request; unset by the host of `origin`) and its gate of the write access and the rules of the target; Push to master, Merge #7 and Auto-Merge #7 (`bd land`), Cancel Auto-Merge | `branch/landing-routes`, `branch/next-step`, `repository/settings`, `cli/agent-cli` ([Landing](docs/design/landing.md#routes)) | `crates/domain-policy/src/landing_route.rs`, `crates/domain-policy/src/land_direct.rs`, `crates/domain-types/src/landing_route.rs`, `crates/sync-core/src/repository_config.rs`, `crates/sync-core/src/repo_facts.rs` (`landing_remote`), `crates/sync-core/src/baseline_push.rs`, `crates/operations/src/land_execute.rs` (`push_baseline`, `merge`), `crates/operations/src/auto_merge.rs`, `crates/remote-facts/src/pull_request_writes.rs` (`merge_pull_request`, `enable_auto_merge`, `disable_auto_merge`), `crates/bd-remote/src/land_command.rs`, `crates/remote-facts/src/branch_rules.rs`, `crates/remote-facts/src/pull_requests.rs` (`viewerPermission`, `mergeStateStatus`) |
| Lease pool (`bd lease`), its clean rules (`.branchdeck.toml`), the host slots that `bd-host serve` lends in process | `cli/lease` | `crates/lease-pool/src/lease.rs`, `crates/lease-pool/src/lease_clean.rs`, `crates/lease-pool/src/host_slots.rs`, `crates/bd-lease/src/lease_command.rs`, `crates/git-ops/src/lease_slot.rs` (state in `$GIT_COMMON_DIR/branch-deck/leases.json`) |
| Remote runs on another Mac (`bd-host`, Draft): a command of a checkout in a warm host slot, the agent under launchd, the upgrade | `cli/host`, `cli/lease` ([Host Runs](docs/design/host-runs.md)) | `crates/bd-host/`, `crates/host-client/`, `crates/host-agent/`, `crates/host-link/` |
| Landed commits | `repository/landed-commits` | `crates/sync-core/src/landed.rs` (report in `branch-deck/landed.json`) |
| Landing overview, PR status, Create Pull Request, Request Review, the push | `repository/landing-overview`, `repository/outgoing`, `branch/pull-requests` | `crates/github/`, `crates/operations/src/pull_request.rs` (the intents `CreatePullRequest`, `RequestReview`, `Retarget`, each with its push first; `bd request-review`), `crates/operations/src/close.rs` (`ClosePullRequest`, `CloseLandedPullRequests`), `crates/operations/src/push.rs` (the intent `Push` of a branch, a stack, up to a commit, an unapplied branch; its network part for `bd push`), `src/lib/pull-request-run.ts`, `src/lib/push-run.ts`, `remote_facts::pull_requests`, `remote_facts::pull_request_create`, `remote_facts::pull_request_text`, `repository_model::pull_request_facts` (the pull requests, rules and profiles of the session), `crates/domain-policy/src/pull_request.rs` (where a pull request goes, the rules of its base), `crates/domain-policy/src/review.rs` (the candidates of Request Review, where the approvals count), `src/stores/pull-requests.ts` |
| Patronus: robots, Safe Push and Dry Run, the next step, the failure prompt (`bd safe-push`, `bd robot`) | `branch/patronus`, `branch/next-step`, `branch/safe-push-failure-prompt`, `branch/whole-stack`, `cli/agent-cli` | `crates/patronus/`, `crates/trial-check/src/landing_gate.rs` (the landing gate of a start), `crates/repository-model/` (the Patronus view and the landing needs the session derives: `facts.rs`; the app's check of a push: `push_guard.rs`), `crates/operations/` (the plan of a start and its run, one for the app and `bd`: `land.rs`, `land_execute.rs`, `land_session.rs`, `robot_start.rs`; the restart and the cancel of a robot: `robot.rs`), `remote_facts::scheduler` (when the session reads each fact: the robots, their details, the pull requests, the readiness, the review requests), `crates/domain-policy/` (the next step, the holds, the start and what landed: the [rule index](#rule-index)), `remote_facts::review_readiness` (the code-owner pre-check), `remote_facts::patronus_command`, `src/presenters/next-step.ts`, `src/presenters/land.ts`, `src/presenters/outcome.ts` (the toast of a start's outcome), `src/presenters/actions.ts` (the items of a robot), `src/lib/robot-run.ts` (its restart and cancel), `src/hooks/useSafePush.ts`, `src/stores/review-readiness.ts` |
| Safe Push statistics, the Landing view of the main window (Branches · Reviews · Landing, ⌘1/⌘2/⌘3): the user's robots of a period (30 days by default) against everyone else's, time lost, failure causes, start hours (`bd robot-stats`) | `repository/safe-push-statistics`, `repository/main-views` | `crates/remote-facts/src/robot_stats.rs`, `crates/remote-facts/src/robot_stats_aggregate.rs`, `crates/patronus/src/stats.rs`, `crates/bd-remote/src/robot_stats_command.rs`, `src/components/landing/LandingView.tsx` |
| GitHub sign-in (OAuth device flow); the sign-ins of the app's accounts as commands of the app (one per provider, a second call joins it) and their status, the fact `auth` of every session | `repository/github-sign-in` | `crates/github/src/token.rs`, `crates/remote-facts/src/sign_in.rs`, `crates/app-api/src/app/sign_in.rs`, `src/stores/auth.ts`, `src/lib/sign-in-run.ts` |
| YouTrack sign-in (Hub OAuth in the browser, refresh token; `sign_in` of the app), the issue popover, Mark Fixed | `repository/youtrack` | `crates/youtrack/`, `crates/remote-facts/src/sign_in.rs`, `src/stores/auth.ts`, `src/stores/youtrack-issues.ts` |
| Issue groups | `branch/issue-groups` | `crates/history-rewrite/src/issue_groups.rs` |
| Conflict fix (`bd check --fix`) | `branch/conflict-fix`, `commit/commit-row` | `crates/trial-check/src/conflict_fix.rs` |
| Move to a branch (drag and drop, menus, keys) | `branch/move-commits` | `crates/history-rewrite/src/move_plan.rs` |
| Stacked branches, the stack island, the whole stack | `branch/stacked-branches`, `branch/stack-island`, `branch/whole-stack`, `branch/branch-row` | `crates/domain-policy/src/stack.rs` (parents, children, descendants, ancestors, main line), `repository_model::derive` (`BranchView.structure`, the push orders `RepositoryView.pushes`), `src/stores/stack-view.ts`, `crates/operations/src/push.rs` (the pushes of a stack), `src/lib/push-run.ts`, `crates/sync-core/src/stack_branch.rs` |
| Stack parents (`bd restack`, `bd unstack`) | `branch/stack-parents` | `crates/history-rewrite/src/stack_parents.rs` |
| Parked branches: kept local, never pushed, not Your Move (`bd park`, `bd unpark`) | `branch/parked-branches` | `crates/history-rewrite/src/parked.rs`, `crates/sync-core/src/parked.rs`, `crates/domain-policy/src/parked.rs` |
| Unapply and Reapply: a branch and its files out of HEAD, kept as `{prefix}/unapplied/<name>` in the Unapplied card, and back on top (`bd unapply`, `bd reapply`, `bd push --unapplied`) | `branch/unapply` | `crates/sync-core/src/unapply_branch.rs`, `crates/sync-core/src/reapply_branch.rs`, `crates/git-ops/src/unapply.rs`, `crates/git-ops/src/reapply.rs`, `src/components/branch/UnappliedBranchTableCard.tsx` |
| Operations across processes: the operations registry of a repository (an operation of the remote that needs what another process runs is `busy`, naming that operation; the operations of `bd` on the rows of the app; the stamp after a mutation of `bd`), a start whose answer was lost (Start Unknown, Forget This Start, `bd operation forget`), one fetch of the baseline for two processes (`fetch.lock`) | `cli/agent-cli`, `branch/patronus`, `repository/update-project` ([Operations registry](docs/design/git-operations.md#operations-registry), [Running Operations](docs/design/operations.md)) | `crates/operations/src/registry_file.rs`, `crates/operations/src/registry_watch.rs`, `crates/operations/src/reconcile.rs`, `crates/operations/src/forget.rs`, `crates/domain-policy/src/admission.rs`, `crates/sync-core/src/fetch_lock.rs`, `crates/bd-remote/src/operation_command.rs`, `crates/bd-core/src/operation_record.rs`, `src/presenters/operations.ts` |
| Activity log, its side panel and its window: read from the journals (the operations of the app and `bd` in the journal of their repository, the page's records in the journal of the app data directory; `read_activity`, `record_activity`, `clear_activity`) | `repository/activity` | `crates/operations/src/journal.rs`, `crates/app-settings/src/journals.rs`, `crates/app-api/src/app/activity.rs`, `src/stores/activity.ts`, `src/presenters/activity.ts`, `src/components/repository/ActivityPanel.tsx`, `src/pages/ActivityPage.tsx`, `src/lib/activity-window-link.ts` |
| Commit files, diffs, the commit review (`D`), the tree-sitter highlighting | `commit/commit-files`, `commit/commit-review` ([Diff View](docs/design/diff-view.md)) | `crates/file-diffs/`, `crates/code-symbols/src/highlight/`, `src/components/diff/` |
| Go to Declaration and Find Usages in a review (⌘-click, ⌘B, ⌥F7, Back), outside it through accio | `commit/symbol-navigation` | `crates/code-symbols/src/navigate.rs`, `crates/file-diffs/src/declarations.rs`, `crates/file-diffs/src/usages.rs`, `crates/file-diffs/src/accio.rs`, `src/hooks/useSymbolNavigation.ts`, `src/components/diff/DeclarationFileView.tsx` |
| Command palette (⌘K: the views, Activity, the commands of the app, Go to a branch) | `repository/command-palette` | `src/components/command-palette/CommandPalette.tsx`, `src/lib/command-palette.ts` |
| Title bar and main menu | `repository/title-bar` | `src-tauri/src/title_bar.rs`, `src/components/shared/WindowTitleBar.tsx` |
| Your Turn (review requests, the pull request review) | `repository/review-requests`, `repository/robots-indicator` | `src/components/review/`, `remote_facts::review_requests`, `repository_model::review_request_facts` (the review requests of the session, their news), `src/stores/review-requests.ts` |
| Reviews, the second view of the main window (⌘2): every open pull request the user takes part in on GitHub (the teams' requests, the reviewed, the own), each once, and a pull request added by its link; read only while it shows | `repository/reviews`, `repository/main-views` | `crates/domain-policy/src/reviews.rs`, `remote_facts::reviews`, `repository_model::review_view` (the sections and the `pull:<n>` actions), `app_settings::linked_pull_requests`, `src/components/reviews/ReviewsView.tsx`, `src/presenters/reviews.ts`, `src/stores/reviews.ts` |
| Settings window; the settings of the app (one writer, versions, the branch prefix resolved in Rust) | `repository/settings` | `src/components/settings/`, `crates/app-settings/`, `src/stores/settings/persistent-store.ts`, `crates/sync-core/src/repository_config.rs` |
| Branch review | `branch/branch-review` | `src/components/review/BranchReview.tsx`, `crates/git-ops/src/ownership.rs` |
| Symbol dependencies | `commit/symbol-dependencies` ([Symbol Dependencies](docs/design/symbol-dependencies.md)) | `crates/code-symbols/`, `crates/trial-check/src/symbol_scan.rs` |
| Zoom | `repository/zoom` | `src-tauri/src/zoom.rs` |

The rows and the menus of the main window: `branch/branch-row`, `commit/commit-row` (start there). The `bd` commands:
`cli/agent-cli` and [Agent CLI](docs/guidelines/agent-cli.md). The `branch-deck` agent skill is generated by
`skill_text()` (`crates/bd-cli/src/agent_help.rs`) and installed by `bd install`; the repository has no skill file.

### Rule Index
Every rule of the domain is one file of `crates/domain-policy/src/` (a `//!` header names the rule and its spec, a table
test `<rule>_test.rs` beside it), a pure function of `domain-types` that the app and `bd` both call: the session
derives its answer into a part of the view (`repository_model::derive`, [The landing of a
session](docs/design/git-operations.md#the-landing-of-a-session)), which the page only words (`src/presenters/`), and
`bd` decides by the same function. A rule never goes into `src/lib` or `src/hooks` (`pnpm lint:arch`, `rule-module`).

| Rule | File | Its answer in the view; in `bd` |
|---|---|---|
| The table as the rules read it, borrowed from the view or a grouping | `facts.rs` (`Repository`) | `derive::repository`; `bd`: `derive::grouped_rows` |
| The robot of a branch and of a whole stack | `robot_match.rs` | `robotId` (`facts::derive_patronus`); `bd`: the same of its one-shot session (`one_shot::OneShot`) |
| What a push does (create, push, force with the drops), needs push, one push again over a moved remote, what keeps a branch out of a push, the push orders (Push Stack, the button, Push with Children and its stack branch) and what a start pushes (`start_push`) | `push.rs` | `RemoteStatusUpdate::action`, `RepositoryView.pushes` (`derive::push_view`), the `Push` effect of `land::decide`; `push_guard`, `bd push`, `bd safe-push` |
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
landed and spans by content (`crates/sync-core/src/robot_landing.rs`, a fact of git) and the Safe Push statistics
(`crates/remote-facts/src/robot_stats_aggregate.rs`). The plan and its stages: [Rust as the Domain
Authority](docs/design/rust-domain-authority.md#индекс-правил).

### Design Documents
- [Async Patterns and spawn_blocking](docs/design/async-patterns.md) - Explains async/sync boundaries and proper use of spawn_blocking
- [Diff View](docs/design/diff-view.md) - Our own renderer on the blocks of the engine (`DiffView`: the row model, the exact layout, the highlight worker by blob id), for the 2-way diffs and the 3-way merges
- [Distributed Settings Architecture](docs/design/distributed-settings.md) - One writer of `settings.json` and `repository-state.json` in Rust (`SettingsService`): the windows send patches, apply changes by version, start from the snapshot of their init script; `bd` reads the same file; the branch prefix resolved per command
- [Running Operations](docs/design/operations.md) - One registry of the actions that run: a second start joins, a rewrite waits for a rewrite, the spinner where it started, a rewrite lasts until its sync; between processes the operations registry of the repository, `busy` naming the holder
- [Rust as the Domain Authority](docs/design/rust-domain-authority.md) - The rework plan (Draft, in Russian): one Rust session owns the table, the facts and the running operations, `domain-policy` holds every rule in one file, every mutation is an intent through `prepare`/`execute`, bd and the app are two adapters; the stages and their progress
- [Rendering Performance](docs/design/rendering-performance.md) - React Compiler, narrow selectors, the patches of the Rust session (16 ms, by part) applied in one update, derived structure cached, lazy main page
- [E2E Testing Architecture](docs/design/e2e-testing.md) - E2E test infrastructure with mock Tauri API
- [Git Operations](docs/design/git-operations.md) - Working-tree safety, exact-metadata rewrites, `git replay`, batching and the Git 2.54 minimum
- [Landing Work into Master](docs/design/landing.md) - The product model: everything lands into master, stacks cut it into pieces, every pull request into master, a whole stack in one robot; the routes into master (a robot, a push, a merge)
- [Host Runs](docs/design/host-runs.md) - `bd-host`, the binary of the remote runs outside `bd` and the app: the client and the agent call the lease pool and git in process; the slot model, `serve`, the supervisors and the keeper, the upgrade
- [Integration Detection and Caching](docs/design/integration-detection.md) - How branch integration detection works with git notes caching
- [Remote Status](docs/design/remote-status.md) - How remote tracking status is computed and displayed
- [Text Diff Engine](docs/design/text-diff.md) - The IntelliJ comparison ported to `crates/text-diff`: lines, words, chars, and the typed diff model of the views
- [Symbol Dependencies](docs/design/symbol-dependencies.md) - tree-sitter summaries of the changed files, `Order` edges by evidence, the subtraction in the dangling-reference check
- [UI Theme](docs/design/ui-theme.md) - Islands layering, the radius scale and the rules for UI changes
- [Virtual Branches](docs/design/virtual-branches.md) - How commits are grouped and copied into virtual branches
- [Specs](spec/) - Requirement specs in the air format (spec/<area>/<name>.spec.md, [@test] links); start with spec/commit/commit-row.spec.md

### Guidelines
- [Adding a Git Operation](docs/guidelines/adding-git-operation.md) - The layers of an operation and the helpers to reuse
- [Agent CLI](docs/guidelines/agent-cli.md) - The `bd` commands, their JSON, error codes and exit codes
- [Platform Checks](docs/guidelines/platform-checks.md) - Linux in Docker and Windows in the Parallels VM before platform code lands
- [Fast Rust Tests](docs/guidelines/rust-tests.md) - A test costs its git processes: the fixtures of one process per step, no sleeps or wall-clock limits, `pnpm perf:test-processes`
- [Studio Runs](docs/guidelines/studio-runs.md) - `bd-host run studio -- <command>`: a heavy check on the Mac Studio, on the exact working tree of a lease, in a warm host slot that the Studio lends to that run
- [Real App Checks](docs/guidelines/real-app-checks.md) - The release app driven with Peekaboo: windows, input, frames, timings
- [Import Guidelines](docs/guidelines/imports.md) - No re-exports policy and proper import patterns
- [UX Writing](docs/guidelines/ux-writing.md) - Toast titles and actions, "Cannot …" errors, Title Case and sentence case, punctuation
- [Design Docs Guidelines](docs/guidelines/design-docs.md) - How to write design documents

### File Structure
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
├── model/           # The window's side of its repository session: the view and the patches it applies (store.ts, patch.ts), the parts of a row (`useRowPart`)
├── presenters/      # The words, icons and menu items of what Rust decided: next-step.ts, landing-node.ts, land.ts, actions.ts, reasons.ts; no rule
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
├── git-executor/    # GitCommandExecutor: the one place git starts; the scan tests
├── git-ops/         # Git operations library
├── history-rewrite/ # The rewrites of the checked out branch (regroup, reword, squash, fold), the grouping, the stores, the repository lock
├── sync-core/       # Branch sync, the pushes, the baseline and the cores of the other operations; no HTTP crate, no `file-diffs`, no `code-symbols`
├── trial-check/     # The trial checks (`bd check`, the conflict fix, the landing gate), `bd move-hunk`, the diff scan and the symbol summaries of the commits
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
├── sync-utils/      # Small shared utilities (issue patterns, timestamps, child processes, git booleans, the process scan)
├── branch-integration/ # Integration detection: rebase, merge and squash strategies
├── bd-cli/          # `bd` agent CLI: the `Cli`, the dispatch, `bd schema`, the help texts, the process budgets (docs/guidelines/agent-cli.md)
├── bd-core/         # What every `bd` command builds on: the context, the errors, the output, the sync and its summaries, the summaries of a check that `bd check` and `bd safe-push` print, the remote services of a command and the host of its reads of the facts (`services`), the record of its operation in the operations registry and its entry in the journal (`operation_record`)
├── bd-lease/        # `bd lease`: the worktrees of the lease pool; its tests run bd through the fixtures of bd-cli
├── bd-remote/       # The bd commands of the remote: status, push, safe-push, request-review, robot, the landed branches
├── bd-rewrite/      # The bd commands that read and rewrite the commits by their messages: group, reword, fold, check, suggest
├── bd-update/       # `bd update`, `bd pick` and `bd undo`; its tests run bd through the fixtures of bd-cli
├── bd-host/         # `bd-host`, the binary of the remote runs (spec/cli/host.spec.md): the client and the agent; neither bd nor the app links it
├── host-link/       # The link of the remote runs: identity, TLS 1.3 with pinned certificates on the provider of the app, frames, protocol host-link; no git
├── host-client/     # The client of a remote run: the commit of the working tree, the pack, the follow, the fetch; git through git-ops
├── host-agent/      # The agent on a host: serve, the supervisors, the keeper, the upgrade; the host slots through lease-pool in process
├── commit-clustering/ # The clustering of `bd suggest`, the areas of a commit, the generated files; no CLI, a library of bd-rewrite and bd-remote
├── object-cache/    # LMDB cache keyed by git object ids, shared by the app and bd
├── code-symbols/    # Declarations, imports and references of a Kotlin/Java/Rust/TS/JS file on tree-sitter, no git; the highlighting of the diffs (`highlight/`)
├── text-diff/       # Line, word and char comparison of two texts, ported from IntelliJ (Apache-2.0, NOTICE)
├── file-diffs/      # The typed diff of each file of a commit, a range or the working tree, and blob texts by id
├── patronus/        # Patronus REST client, Space token of the Safe Push CLI, request policy
├── github/          # GitHub REST and GraphQL client (rate limits, SSO, conditional GETs), fake GitHub for tests
├── youtrack/        # YouTrack REST client, the sign-in through its Hub (OAuth, PKCE, refresh token), issues and commands
├── test-server/     # E2E mock server: a host of app-api (`invoke.rs`: one route `/invoke/<name>` per entry of the registry, SSE for a channel; `host.rs`: the desktop and the event streams of the pages), the fakes and the fixture routes of each test repository ([routes](docs/design/e2e-testing.md#the-routes-of-the-server))
├── test-utils/      # Test repositories, fixture git (`git_command`), the fixtures of the sync tests (`sync_fixtures`) and, with the feature `http`, `fake_server`: the loopback HTTP server of the fakes of patronus, github, youtrack and the test server
└── …
tests/
├── e2e/             # Playwright specs by feature, their HTML snapshots, fixtures/ and helpers/
├── perf/            # Performance tests (`pnpm perf:diff`, `pnpm perf:table`)
└── screenshots/     # The README images (`pnpm screenshots:readme`)
```
