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
- Never `cargo test`: always `cargo nextest run` ([Verification](#verification)).
- Never a merge commit, `--no-ff`, `--no-verify`, or `commit-tree` with two `-p` ([Commits and History](#commits-and-history)).
- Never a behaviour change without its spec, `[@test]` links and docs in the same commit ([Specs and Docs](#specs-and-docs)).
- Never git outside `GitCommandExecutor` in the app's code, never a write to the user's working tree beyond the named
  exception, never a raw `Command::new("git")` in a test fixture (`test_utils::git_test_utils::git_command`)
  ([Git Operations](#git-operations)).
- Never a `gh` process for GitHub (HTTP through `crates/github`; the one `gh` is `gh auth token` of the token chain),
  never `-c credential.helper=`.
- Never `#[allow]` (`#[expect(lint, reason = "…")]`), never `pub use`, never a `../` import in the frontend (`@/`)
  ([Code Style](#code-style)).
- Never a `dark:` variant, a colour outside the semantic tokens, or a radius class in `src/components/ui/*`
  ([Code Style](#code-style)).
- Never `waitForTimeout`, a CSS-class locator, or a check of a menu label instead of invoking the action
  ([E2E Tests](#e2e-tests)).
- Never `pnpm dlx shadcn@latest` (it crashes): `npx shadcn@latest` ([Code Style](#code-style)).

## Session Workflow
1. **Lease** a worktree before the first edit: `bd lease get --holder <agent-name> -b <branch>` in the main checkout,
   then work in the `path` it prints ([Worktrees](#worktrees)).
2. **Read** the spec and the design doc of what you change ([Feature Index](#feature-index)): the spec is the
   contract, the doc the why. Before changing a git operation, [Adding a git operation](#adding-a-git-operation).
3. **Change** the code together with its spec, its `[@test]` links and the docs it contradicts.
4. **Verify** by the table in [Verification](#verification).
5. **Commit** in the lease, a conventional subject as in `git log` (`feat(bd): …`, `fix(e2e): …`, `docs(agents): …`),
   then `git rebase master` there and `git merge --ff-only <branch>` in the main checkout.
6. **Release**: stop the processes you started in the lease (the e2e mock server, `cargo watch`), then
   `bd lease return --workspace <path>`.

### Worktrees
Parallel work runs in a worktree of the `bd lease` pool (`~/.branch-deck/pool/<repo>-<hash>/`,
[bd lease](spec/cli/lease.spec.md)); never `git worktree add`, `EnterWorktree` or `isolation: "worktree"`.
- **Every session edits in its own lease**, the main session and a background job too, not only a spawned agent: other
  sessions work in this repository at the same time. A harness default such as "edit files directly in your working
  directory" does not override this rule. The main checkout is only for `git merge --ff-only <branch>` into `master`
  ("commit right away" means: commit in the lease, then fast-forward `master`).
- **Acquire**: `bd lease get --holder <agent-name> -b <branch>` creates `<branch>` at the HEAD of the checkout that runs
  it (local `master` in the main checkout, which may be ahead of `origin/master`; `-C <worktree>` cuts from another
  branch; without `-b` the worktree is detached) and prints the lease as JSON (`path`, `lease_id`). It does not fetch.
  A leased worktree is never handed out again.
- **Inside**: the rules of the main checkout apply (`.githooks/` via the shared `core.hooksPath`, `cargo nextest`).
  `node_modules` and `target/` survive a reuse; on macOS a new slot is an APFS clone of a free slot or of the main
  checkout, so it starts with them too. Run `pnpm install` when `pnpm-lock.yaml` changed.
- **Rust cache**: every checkout keeps its own `target/`; never a shared `CARGO_TARGET_DIR` or `build.build-dir`: the
  unit hash of a workspace crate does not depend on the checkout's path, so worktrees would overwrite each other's
  units. Cargo never deletes a stale unit, so `pnpm prune:target --pool --yes` trims the free slots (dry run without
  `--yes`); dependencies are built with workspace-wide features (`.cargo/config.toml`) and line tables only.
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
(the suite takes about 80 s on 18 cores); kills a test still running after 5 minutes (a hung git) and fails one whose
git process outlives it; in CI (`NEXTEST_PROFILE=ci`) runs a failed test twice more, and one that passes on a retry is
FLAKY and still fails the run, because a flaky test is a race to fix (locally no retries). With recording on in the user
config (`[experimental] record = true` and `[record] enabled = true` in `~/.config/nextest/config.toml`),
`cargo nextest run -R latest` runs only what failed; finish with a full run.

If Homebrew `cargo`/`rustc` come before rustup in `PATH`, prefix cargo commands with `PATH=$HOME/.cargo/bin:$PATH`:
the nightly toolchain pinned in `rust-toolchain.toml` is required to parse the workspace manifest.

| You changed | Run |
|---|---|
| Rust | `pnpm lint:backend`, then `cargo nextest run --workspace` (`-p <crate>` or `-E 'test(/…/)'` while iterating) |
| a git operation | also its test in `process_budget_test.rs` and the scan tests (`worktree_safety_test.rs`, `git_usage_scan_test.rs`) |
| a Tauri command or a type it passes | `src/lib/bindings.ts`: a debug run of the app writes it (`pnpm tauri dev`, or `target/debug/branch-deck` started in `src-tauri/` for a few seconds); `the_bindings_are_what_the_export_writes` fails on a stale one |
| frontend | `pnpm lint:frontend`, `pnpm test:unit`, `pnpm e2e`; `pnpm e2e-up` when the HTML changes, its snapshot diff reviewed like code |
| code anywhere | `pnpm lint:cpd`: fix a clone with a helper, constant or fixture; `// jscpd:ignore-start` / `// jscpd:ignore-end` only for an intentional duplication, with the reason in the comment |
| `#[cfg]` or `cfg!` code | Linux and Windows too: [Platform Checks](docs/guidelines/platform-checks.md) (a check on the Mac compiles only its side) |
| what the release app shows (a window that opens late, a "Loading…" frame) | [Real App Checks](docs/guidelines/real-app-checks.md) (`pnpm install-app`, Peekaboo) |

### Quick Commands
See `package.json` for the full list of scripts.

```bash
pnpm install              # Install dependencies
pnpm run tauri dev        # The full app (frontend + backend); a debug run writes src/lib/bindings.ts
pnpm dev                  # Only the Vite frontend (port 1420)
pnpm build                # The frontend into dist/ (E2E global setup runs it)
pnpm install-app          # Unsigned release build of the .app, copied to /Applications (macOS)

pnpm test                 # test:backend + test:unit (Rust and Vitest; not E2E)
pnpm test:backend         # cargo nextest run --workspace
cargo nextest run -R latest  # Rerun what failed or did not run in the last run (record on in the user config)
pnpm test:unit            # Vitest (`vp test run`; src/**/*.test.ts)

pnpm e2e                  # Playwright E2E tests (installs the browsers if needed; `pnpm setup:e2e` by hand)
pnpm e2e-up               # E2E tests that rewrite the snapshots
DEBUG_E2E=1 pnpm e2e      # With the mock server's debug log and every page console line
E2E_PORT=3031 pnpm e2e    # On another port when another checkout's mock server holds 3030
pnpm test-server:dev      # Optional: the E2E mock server with auto-restart (needs watchexec); `pnpm e2e` reuses it
pnpm perf:diff            # The diff review performance test (playwright.perf.config.ts)

pnpm lint                 # Every lint:* script
pnpm exec vp check        # Formatting, lint and types without fixing (what CI checks first)
pnpm lint:backend         # cargo fmt, clippy --fix, then clippy -D warnings
pnpm lint:frontend        # `vp check --fix` (Oxfmt, type-aware Oxlint) + lint:typecheck
pnpm lint:typecheck       # tsc of tsconfig.app.json, tsconfig.node.json and tsconfig.e2e.json
pnpm lint:cpd             # jscpd copy/paste detection (config in .jscpd.json); fails on any clone
pnpm prune:target --pool  # Dry run: stale cargo units of this checkout and the free lease slots (`--yes` deletes)
pnpm perf:monorepo pin|measure|summarize|compare  # bd's git processes and times on a pinned large clone (git-operations.md)
cargo run -p branch-deck -- cli status -C <repo>  # agent CLI, JSON on stdout (docs/guidelines/agent-cli.md)
```

### Debugging
- **E2E**: a failed test carries the console errors of its pages as the `page errors` attachment. Run with
  `DEBUG_E2E=1` for the debug log of the mock server and every console line live; without it the server logs warnings
  and errors only (`TEST_SERVER_LOG`, not `RUST_LOG`, which `.cargo/config.toml` forces to debug), and a green run
  prints none. "Executable doesn't exist": `pnpm setup:e2e` (needed after an update of `@playwright/test`).
- **Types**: `pnpm lint:typecheck` isolates TypeScript errors.

## Agent Pitfalls
Mistakes agents made here; each cost a session.
- **One `pnpm e2e` per checkout**, even on two ports: each global setup runs `pnpm build`, which empties `dist/` under
  the other run (timeouts in page setup, not test failures).
- **`CI=1` does not speed up E2E**: it means 1 worker, 2 retries and no reuse of a running server
  (`playwright.config.ts`). A test that fails only under parallel load is a race: fix the wait, don't serialize.
- **`bd check` without `--plan` reads the plan from stdin** when stdin is not a terminal (`read_plan` in
  `crates/bd-cli/src/check_command.rs`): from a background or piped shell it waits forever. Pass `</dev/null`.
- **Same-second commits are the same commit**: a `git replay` or cherry-pick of a commit onto its own parent within one
  second writes the identical object, so "the copy" is the original. A test that needs a new copy makes the target
  diverge first (one more commit) or uses `cherry-pick -x`; a test that passes on a rerun is often this.
- **React Compiler and store reads**: a function that reads stores with `getState()`, called in the render body beside
  `useStore`, is memoized on its arguments and shows a stale value. The value must come out of the selector:
  `useNextStepStores(() => getBranchNextStepNow(name))` (`src/hooks/useBranchNextStep.ts`); a menu builder likewise
  (`useBranchMenu`, `src/hooks/useBranchContextActions.ts`).
- **Typing into a Base UI dialog in E2E**: wait for `toBeFocused()` first; `press`/`type` on an unfocused input puts the
  caret at 0 and the text lands at the start.
- **zsh does not word-split `$args`**: `args="--no-default-features --features=x"; cargo build $args` passes one
  argument. Write the arguments out, use `${=args}`, or run the loop under `bash -c '…'`.
- **An "unresolved import" of a module that clearly exists**, after another session moved branches under the same
  `target/`, is a stale cargo fingerprint: `touch` the crate's files and rebuild.
- **Throwaway Playwright specs** (screenshots, debugging) go into `shots.local/` of the worktree (`*.local` is
  ignored), with a config there that spreads `../playwright.config` and sets `testDir: "."`, `globalSetup:
  "../tests/e2e/setup/global-setup.ts"` and `webServer[].cwd: ".."`; run `npx playwright test --config
  shots.local/shots.config.ts`. Delete them before a commit: `pnpm lint:frontend` lints them too.

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
  `data-sync-count`, an `aria-busy` that clears); an app timer runs with `page.clock.fastForward`, a debounced
  settings write with `waitForStorePersistence` (the key as a reload reads it). No `waitForTimeout`: under load
  (several checkouts running e2e and cargo) any fixed wait is too short.
- **Locators**: role and accessible name first (`getByRole("menuitem", { name: "Review Changes" })`), `getByTestId` for
  what has no role or name; never CSS classes or DOM shape, which the theme and the React Compiler change.
- **Through the user's path**: open, select and press like the user (hover, focus, right-click, keys); call a command
  directly only to set up a state the user path cannot reach in a test. An error path fails the command with
  `routeCommandError`, not by breaking the repository.
- **Linked**: every requirement of a spec names its test (`[@test]` with the exact test title); a renamed, split or
  deleted test updates its links in the same commit.
- **Snapshots**: `captureHtmlSnapshot` for structure; `pnpm e2e-up` rewrites them, and the diff is reviewed like code: a
  changed snapshot can hide a regression.

### Commits and History
How agents commit to this repository; "Git Operations" below is about the git the app runs.
- **Linear history, no merge commits**: not of master, not of an integration branch, not of an agent worktree branch.
- Bring a branch up to date with `git rebase <target>` and resolve conflicts there; integrate it with
  `git merge --ff-only <branch>` or `git cherry-pick`. Rebasing your own branch in your worktree is fine: the
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
  `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0`, strips inherited `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/
  `GIT_NOTES_REF`, empties `GIT_NOTES_DISPLAY_REF` (`%N` shows only the mapping notes, or an explicit `--notes=<ref>`)
  and gives git a null stdin unless the call pipes one; callers don't add `--no-pager`. A git with an environment,
  stdin, accepted exit codes, a timeout, streamed stdout, progress or a reply protocol is `git.cmd(&args, repo)` with
  `.env`, `.stdin`, `.accept`, `.timeout` and one finisher (`.text`, `.bytes`, `.lines`, `.output`, `.status`,
  `.stream`, `.progress`, `.interactive`), all on one spawn core; the only shorthands are `execute_command` (`.text`),
  `execute_command_lines` (`.lines`) and `execute_command_with_status` (`.status` as text). The executor knows no git
  beyond that: a rev or ref fact (`resolve_tree_id`, `ref_exists`) is a git-ops function. Test fixtures run git through
  `test_utils::git_test_utils::git_command()`, which removes the repository variables a hook or `rebase --exec`
  exports (`git_command_test.rs` fails on a raw `Command::new("git")`).
- **Scan tests** (`crates/git-executor/src/git_usage_scan_test.rs`, on the sources as `source_scan_test.rs` reads them
  for the worktree safety scan too): git starts only in `GitCommandExecutor` (and `git_info.rs`'s discovery); bd-cli
  and src-tauri start no git of their own; no `--no-pager`, no `credential.helper=`; `update-ref` only in
  `ref_transaction.rs`; `cat-file --batch*` only in `cat_file_batch.rs`; `--name-only`, `--name-status` and `--raw`
  only with `-z`; paths after `--` only with `--literal-pathspecs`; `push`, `fetch` and `ls-remote` only through
  `execute_remote_command*`. An exemption names its file, its reason and its exact number of hits.
- **Never write to the working tree**: no `checkout`, `checkout-index`, `switch`, `reset`, `rebase`, `stash`, `commit`,
  `cherry-pick`, `merge`, `restore`, `clean`, `apply` without `--cached`, `read-tree -u`/`--reset`, `git history`;
  `crates/git-executor/src/worktree_safety_test.rs` enforces it. Reading worktree files into a temporary
  `GIT_INDEX_FILE` is fine. The single exception is the guarded two-way `read-tree -m -u` in
  `crates/git-ops/src/worktree_update.rs` (dry run on an index copy first, then the CAS ref move, rollback on failure),
  `pub(crate)`, called only by `update_branch.rs`: for Update Project (the app and `bd update`), its undo, the finish
  of a resolve session (`update_resolve.rs`) and `bd pick` (`pick.rs`, also the worktree of `--follow`) and its undo
  ([Invariants](docs/design/git-operations.md#invariants)). The scan counts its `-u` argument lists. Don't add another
  caller, another argument list, or a public way to build a move. A slot of the lease pool is not the user's working
  tree: its `checkout --force`, `reset --hard` and `clean -fd` live in `crates/git-ops/src/lease_slot.rs` only, each
  counted by the scan ([Lease](docs/design/git-operations.md#lease)).
- **The user's index** changes only where HEAD's tree changed (`IndexUpdate` in `crates/git-ops/src/rewrite_guard.rs`):
  a drop runs `read-tree -m -i <old> <new>` after a preflight on a copy of the index, an amend stages the amended files.
  Staged work on other paths survives; never `reset --mixed`, never ignore an index error.
- **Ref moves**: one `RefTransaction` (`update-ref --stdin`) with old-value CAS and a `branch-deck: <op>` reflog
  message; related refs (unapply rename, temp refs) go into the same transaction. The named exceptions: a notes ref
  that `fast-import` writes (`write_notes_to_refs`, `write_commits_with_notes`: `from <ref>^0`, and a tip that does not
  contain the ref's value fails the run, so a concurrent writer is never overwritten; the resolve notes move in the
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
    archived branch is integrated (one CAS write, best effort, `drop_integrated_stack_parents`)
    ([Regroup](docs/design/git-operations.md#regroup-rename-restack)).
  - `refs/branch-deck/conflict/<sha256 of the branch's full name>` (`conflict_chain.rs`): only a sync, in one CAS
    transaction after the branches, only when one changes; a failed move fails no sync
    ([Virtual branch sync](docs/design/git-operations.md#virtual-branch-sync)).
  - `{prefix}/stack/<root>` (`stack_branch.rs`): only a sync, for a stack that forks, with CAS on the tip it listed and
    only when every member was built; deleted once the stack stops forking, never while it has a remote branch or a
    worktree has it checked out. Only a `Built` one is pushed: by the Safe Push of the whole stack (its restart, the
    creation of its pull request, Request Review) and by Push with Children of the root once it is on the remote or its
    pull request is open. Its build is remembered by the base and the original ids (`StackBuilds`), never by
    `CommitKey`, which an amend keeps ([Stack branch](docs/design/git-operations.md#stack-branch)).
  - `refs/branch-deck/review/<n>/{head,base}`: only `review_fetch.rs`, through its own fetch refspecs; always
    `--refmap=` and `--no-prune`, so no remote-tracking ref moves; never under the repository lock; no fetch when both
    commits are local; killed on cancel or 120 s of silence; pruned with one `for-each-ref` and one CAS
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
- **Repository lock**: every rewrite of the checked-out branch takes it in its core, before it reads anything: the
  regroup (`regroup.rs`) and the move of a hunk through their options, and the app's squash and its undo, squash
  fixups (also `bd fold`), amend, unapply, reword and add issue reference through `RewriteOptions` (`held_lock`,
  `lock_timeout`, `expect_head` checked by `ensure_expected_head`, `repository_lock.rs`); then `bd sync`, `bd push`,
  `bd safe-push` (the sync, the checks and the push; the robot start after it), `bd move-hunk`, `bd undo`,
  Update Project (the app and `bd update`, after the fetch, also when it starts a resolve session), `bd pick` (both
  worktrees under one hold), and `bd update --continue` and `--abort` hold `$GIT_COMMON_DIR/branch-deck.lock`
  (bounded wait, then `locked`), because the app and `bd` processes rewrite the same branch. The app's sync never
  takes it. A fetch never runs under it ([Regroup](docs/design/git-operations.md#regroup-rename-restack)).
- **Guards** (`History::check_rewrite`, `check_rewrite_range`) before rewriting the checked-out branch: no
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
  resolve session finds the first conflicting commit with a binary search over prefixes: ⌈log₂ n⌉ `replay` runs, then
  one `replay` of the rest per round. Never parse translated text: exit codes, `--porcelain`, `-z`. The one named
  exception: a branch description costs one `git config` per changed branch (`bd describe`, `descriptions` of a
  `bd group` plan), because git has no batch config writer; read them with one `--get-regexp`.
- **Exit codes**: `merge-tree` and `replay` exit with 1 on conflicts; `merge-tree --stdin` exits with 0 even then, and
  each merge of its output starts with its status, 1 clean, 0 conflicted (`parse_batch_merge`); `merge-base` exits with
  1 for "no" (`--is-ancestor`) or "none" (no merge base, no fork point), and any other non-zero is an error, never a
  "no" (`git_ops::revs`).
- **Network commands** (push, fetch) run through `execute_remote_command` (`crates/sync-core/src/remote_command.rs`):
  the user's credential helpers stay, prompts are off, stalled HTTP transfers abort, the locale is C (untranslated
  `--progress` lines). Never pass `-c credential.helper=`.
- **Git config writes**: only the `branchdeck.*` keys of Settings → Repository and Settings → Diffs & Merge
  (`repository_config.rs`: one `--get-regexp --show-scope` read, `--replace-all` or `--unset-all`, refused when the
  worktree config or `git -c` sets the key), `--local` (`.git/config`), or `--global` only where the request names the
  global scope (`set_repository_config` with `scope: "global"`); and the branch descriptions of `bd describe`
  ([Repository settings](docs/design/git-operations.md#repository-settings)).
- **GitHub credentials**: the only git of the GitHub token chain is `git credential fill` (`crates/github/src/token.rs`),
  a `GitCommand` with `.timeout` (10 s) and `.status`, with `credential.interactive=false`, an empty `GIT_ASKPASS` and
  `GCM_INTERACTIVE=never`; never `credential approve` or `reject`. GitHub itself is asked over HTTP (`GitHubService`),
  never by a `gh` process; the app's own sign-in keeps its token in a 0600 `github-token.json` of the app data dir
  (which `bd` reads too), not the keychain ([GitHub credentials](docs/design/git-operations.md#github-credentials)).
- **Symbols** (`symbol_scan.rs`): source blobs by id from the commit diffs, the declarations the range adds or changes
  as the index, never the whole tree; the summaries of trial blobs stay in memory, never in the object cache
  ([Symbol summaries](docs/design/git-operations.md#symbol-summaries)).

### Adding a git operation
Read [Adding a Git Operation](docs/guidelines/adding-git-operation.md) (the layers in detail, the helper table) before
you add or change git code in `crates/git-ops/`, `crates/sync-core/`, `crates/bd-cli/` or `src-tauri/src/commands/`.
The checklist:
- One name across the layers (`git_ops::move_hunk`, `sync_core::move_hunk`, `bd move-hunk`); the `//!` headers of
  `crates/git-ops/src/lib.rs` and `crates/sync-core/src/lib.rs` map the modules by area.
- **git-ops** has the git: one process per phase, `-z` output, full ids, `--end-of-options` before a revision from
  outside, `--literal-pathspecs` for paths after `--`; no lock and no policy.
- **The core** `<op>_core` in `crates/sync-core/` is the one entry of every caller: a rewrite of the checked-out branch
  takes `&RewriteOptions` and the lock before it reads anything, then checks the expected HEAD.
- **bd** and **the app** parse, call the core and present its result; no git of their own. A `bd` command answers one
  JSON object with `schema`, errors as `CliError` with an `ErrorCode`; a Tauri command is `run_blocking` of the core,
  and the E2E server routes it to the same core.
- **Reuse a helper** before writing a parser or a process: `ref_transaction`, `commit_writer`, `commit_chain`,
  `cat_file_batch`, `object_texts`, `history`, `rewrite_guard` and `revs` in `git_ops`; `repo_facts`, `snapshot`,
  `repository_lock` and `remote_command` in `sync_core`.
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
  `showToast({ title, description, action })` from `@/lib/toast-message` for notifications. Every user-facing error
  goes through `notifyError` in `src/lib/errors.ts` (sticky toast, logged to the Tauri log plugin, recorded in Activity).
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
  memoizes renders, not selectors, which run on every store change. Persisted settings sync across windows
  ([Distributed Settings](docs/design/distributed-settings.md)). Debounce user inputs where appropriate (`p-debounce`).
- **Base UI**: compose with the `render` prop (`<TooltipTrigger render={<Button />} />`), not `asChild`. Base UI emits
  `data-open`/`data-closed`; set `data-state` manually where E2E tests read it.
- **shadcn workflow**: components are generated into `src/components/ui/` (`components.json`, style `base-rhea`). Run
  the CLI with `npx shadcn@latest …` (`add <component>`, `docs`, `init --preset`), then adapt the files.
  `pnpm dlx shadcn@latest` crashes with `o.deepPartial is not a function`: `@shadcn/registry` uses the zod 3 API and
  pnpm dlx resolves zod 4.
- **Locally customised ui files**: `alert`, `badge`, `button`, `card`, `checkbox`, `input`, `popover`, `spinner`, `tabs`,
  `textarea`, `tooltip`, `toast` in `src/components/ui/` carry a `// local customisation: …` header naming the edits. After
  `add --overwrite` (e.g. a style switch), re-apply them; `git diff HEAD -- src/components/ui` shows what was lost.
- **Toolchain** (Vite+, `vite-plus`, the `vp` CLI): one pinned toolchain of Vite 8, Vitest, Oxlint and Oxfmt,
  configured in `vite.config.ts` (`lint`, `fmt`); Tailwind CSS v4 through `@tailwindcss/vite`, its theme tokens in
  `src/index.css`. Oxlint is type-aware through tsgolint (TypeScript 7, the Go compiler, strict, checked with `tsc` on
  `tsconfig.app.json`, `tsconfig.node.json` and `tsconfig.e2e.json`) and runs the React Compiler rules; there is no ESLint, because typescript-eslint cannot use
  TypeScript 7. Oxfmt formats code and config (not Markdown) and sorts imports and Tailwind classes. Test files import
  from `vite-plus/test`.
- **TypeScript**: Use `u32` for timestamps and `snake_case` for properties that bind to Rust structs.
- **Rust**: `snake_case` for variables, functions and modules; `tracing` for structured logging, `#[instrument]` to
  trace a function's entry, exit and arguments; proper `Result` types, not strings.
- **Clippy**: pedantic, nursery and cargo plus the restriction lints that catch defects (`[workspace.lints]` in
  `Cargo.toml`), on all targets; any warning fails `pnpm lint:backend` and CI (`-D warnings`), Windows included. An
  exception is `#[expect(lint, reason = "…")]` where it holds, never `#[allow]`; a lint that is wrong for this code is
  allowed in `Cargo.toml` with its reason.
- **A platform branch in a test** asserts on every platform: an `if cfg!(target_os = "macos")` has an `else` that
  checks what the other platforms do (a `worktree add` where no APFS clone is), never a branch that asserts nothing.
  A helper or import that only `#[cfg(unix)]` tests use carries `#[cfg(unix)]` too, or Windows clippy fails on it.
- **Rust imports** ([Import Guidelines](docs/guidelines/imports.md)): never `pub use` re-exports (except for internal
  module organization); make modules public with `pub mod`; every import uses the full module path, so it shows where
  a type or function comes from:
  - ✅ `use git_executor::git_command_executor::GitCommandExecutor`
  - ✅ `use sync_types::issue_navigation::IssueNavigationConfig`
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
| Integration status: archive the branches the baseline has (by default rebase/cherry-pick detection only; merge and squash by an environment variable) | `branch/branch-row` ([Integration Detection](docs/design/integration-detection.md)) | `crates/branch-integration/` |
| Context menus (the "⋯" menu on right-click, acting on the selection) | `branch/branch-row`, `commit/commit-row` | `src/components/shared/ContextMenuItems.tsx`, `src/hooks/useBranchContextActions.ts` |
| AI branch naming (`claude -p --model haiku`, after consent) | `branch/branch-name-suggestions` | `src/components/ai/` |
| Squash and fixups (`bd fold`) | `commit/squash`, `commit/fixups` | `crates/sync-core/src/squash.rs`, `crates/sync-core/src/autosquash.rs` |
| Update Project (`bd update`, resolve sessions, `bd undo update:`) | `repository/update-project`, `cli/update` | `crates/git-ops/src/update_branch.rs`, `crates/git-ops/src/update_resolve.rs`, `update-progress.ts` |
| Pick between worktrees (`bd pick --from <worktree> --follow`) | `cli/pick` | `crates/sync-core/src/pick.rs`, `crates/git-ops/src/pick.rs` |
| Lease pool (`bd lease`) | `cli/lease` | `crates/sync-core/src/lease.rs`, `crates/bd-cli/src/lease_command.rs`, `crates/git-ops/src/lease_slot.rs` (state in `$GIT_COMMON_DIR/branch-deck/leases.json`) |
| Landed commits | `repository/landed-commits` | `crates/sync-core/src/landed.rs` (report in `branch-deck/landed.json`) |
| Landing overview, PR status, Create Pull Request, Request Review | `repository/landing-overview`, `repository/outgoing`, `branch/pull-requests` | `crates/github/`, `sync_core::pull_requests`, `sync_core::pull_request_create` |
| Patronus: robots, Safe Push and Dry Run, the next step, the failure prompt (`bd safe-push`, `bd robot`) | `branch/patronus`, `branch/next-step`, `branch/safe-push-failure-prompt`, `branch/whole-stack`, `cli/agent-cli` | `crates/patronus/`, `src/lib/branch-next-step.ts`, `src/stores/review-readiness.ts` |
| GitHub sign-in (OAuth device flow) | `repository/github-sign-in` | `crates/github/src/token.rs` |
| Issue groups | `branch/issue-groups` | `crates/sync-core/src/issue_groups.rs` |
| Conflict fix (`bd check --fix`) | `branch/conflict-fix`, `commit/commit-row` | `crates/sync-core/src/conflict_fix.rs` |
| Move to a branch (drag and drop, menus, keys) | `branch/move-commits` | `crates/sync-core/src/move_plan.rs` |
| Stacked branches, the stack island, the whole stack | `branch/stacked-branches`, `branch/stack-island`, `branch/whole-stack`, `branch/branch-row` | `src/stores/stack-view.ts`, `src/lib/git/stack-push.ts`, `crates/sync-core/src/stack_branch.rs` |
| Stack parents (`bd restack`, `bd unstack`) | `branch/stack-parents` | `crates/sync-core/src/stack_parents.rs` |
| Activity log | `repository/activity` | `src/stores/activity.ts` |
| Commit files, diffs, the commit review (`D`) | `commit/commit-files`, `commit/commit-review` ([Diff View](docs/design/diff-view.md)) | `crates/file-diffs/`, `src/components/diff/` |
| Title bar and main menu | `repository/title-bar` | `src-tauri/src/title_bar.rs`, `src/components/shared/WindowTitleBar.tsx` |
| Your Turn (review requests, the pull request review) | `repository/review-requests`, `repository/robots-indicator` | `src/components/review/` |
| Settings window | `repository/settings` | `src/components/settings/`, `crates/sync-core/src/repository_config.rs` |
| Branch review | `branch/branch-review` | `src/components/review/BranchReview.tsx`, `crates/git-ops/src/ownership.rs` |
| Symbol dependencies | `commit/symbol-dependencies` ([Symbol Dependencies](docs/design/symbol-dependencies.md)) | `crates/code-symbols/`, `crates/sync-core/src/symbol_scan.rs` |
| Zoom | `repository/zoom` | `src-tauri/src/zoom.rs` |

The rows and the menus of the main window: `branch/branch-row`, `commit/commit-row` (start there). The `bd` commands:
`cli/agent-cli` and [Agent CLI](docs/guidelines/agent-cli.md). The `branch-deck` agent skill is generated by
`skill_text()` (`crates/bd-cli/src/agent_help.rs`) and installed by `bd install`; the repository has no skill file.

### Design Documents
- [Async Patterns and spawn_blocking](docs/design/async-patterns.md) - Explains async/sync boundaries and proper use of spawn_blocking
- [Diff View](docs/design/diff-view.md) - Our own renderer on the blocks of the engine (`DiffView`: the row model, the exact layout, the highlight worker by blob id), for the 2-way diffs and the 3-way merges
- [Distributed Settings Architecture](docs/design/distributed-settings.md) - Settings synchronization across windows
- [Running Operations](docs/design/operations.md) - One registry of the actions that run: a second start joins, a rewrite waits for a rewrite, the spinner where it started, a rewrite lasts until its sync
- [Rendering Performance](docs/design/rendering-performance.md) - React Compiler, narrow selectors, sync events applied in batches, derived structure cached, lazy main page
- [E2E Testing Architecture](docs/design/e2e-testing.md) - E2E test infrastructure with mock Tauri API
- [Git Operations](docs/design/git-operations.md) - Working-tree safety, exact-metadata rewrites, `git replay`, batching and the Git 2.54 minimum
- [Landing Work into Master](docs/design/landing.md) - The product model: everything lands into master, stacks cut it into pieces, every pull request into master, a whole stack in one robot
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
│   ├── files/       # The directory tree of changed files
│   ├── review/      # Branch, range and pull request reviews
│   ├── ai/          # AI branch name suggestions (Claude Code)
│   └── settings/    # The pages of the Settings window and their rows
├── hooks/           # React hooks (useX.ts)
├── stores/          # zustand stores; settings/ holds persisted, cross-window settings
├── lib/             # Utilities and the generated bindings.ts
├── pages/           # Route components (index, the sub-windows: reviews, conflict windows, settings)
├── types/           # Shared TypeScript types
├── test-support/    # Helpers of the unit tests
├── router.ts        # react-router routes (createBrowserRouter)
├── App.tsx          # The root route
├── index.css        # Tailwind and the theme tokens
└── main.tsx         # App entry (store handlers, router)
src-tauri/src/
├── commands/        # Tauri commands
├── lib.rs           # Command registration, the bindings export of a debug run
└── …                # title_bar.rs, menu.rs, zoom.rs, …
crates/
├── git-executor/    # GitCommandExecutor: the one place git starts; the scan tests
├── git-ops/         # Git operations library
├── sync-core/       # Branch sync and the cores of every operation
├── sync-types/      # Types shared by the cores, bd and the app
├── sync-utils/      # Small shared utilities (issue patterns, timestamps, child processes)
├── branch-integration/ # Integration detection: rebase, merge and squash strategies
├── bd-cli/          # `bd` agent CLI (docs/guidelines/agent-cli.md)
├── object-cache/    # LMDB cache keyed by git object ids, shared by the app and bd
├── code-symbols/    # Declarations, imports and references of a Kotlin/Java/Rust/TS/JS file on tree-sitter, no git
├── text-diff/       # Line, word and char comparison of two texts, ported from IntelliJ (Apache-2.0, NOTICE)
├── file-diffs/      # The typed diff of each file of a commit, a range or the working tree, and blob texts by id
├── patronus/        # Patronus REST client, Space token of the Safe Push CLI, request policy
├── github/          # GitHub REST and GraphQL client (rate limits, SSO, conditional GETs), fake GitHub for tests
├── test-server/     # E2E mock server
├── test-utils/      # Test repositories and fixture git (`git_command`)
├── sync-test-utils/ # Test helpers of the sync and the integration detection
├── integration-detection-tests/ # Tests of the integration detection
└── …
tests/
├── e2e/             # Playwright specs by feature, their HTML snapshots, fixtures/ and helpers/
└── perf/            # Performance tests (`pnpm perf:diff`)
```
