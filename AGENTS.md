# Branch Deck Development Context

## Project Overview
Desktop Git branch management tool using React 19 + Vite + Tauri v2. Use `pnpm` only: the version is `packageManager`
in `package.json`, Node.js is `.node-version`; CI reads both through `voidzero-dev/setup-vp`.

## Quick Commands
See `package.json` for the full list of scripts.

```bash
# Core Development
pnpm install              # Install dependencies
pnpm run tauri dev        # Run the full Tauri app (frontend + backend)
pnpm dev                  # Run only the Vite frontend (port 1420)
pnpm install-app          # Unsigned release build of the .app, copied to /Applications (macOS)

# Testing
pnpm test                 # Run all tests (Rust and E2E)
pnpm test:backend         # Run backend tests with nextest (fast)
pnpm test:unit            # Run frontend unit tests (`vp test`, Vitest; src/**/*.test.ts)

# E2E Testing (Playwright)
pnpm setup:e2e            # First-time setup: Install Playwright browsers
pnpm e2e                  # Run Playwright E2E tests (auto-installs browsers if needed)
pnpm e2e-up               # Run E2E tests and update snapshots
DEBUG_E2E=1 pnpm e2e      # Run E2E tests with the mock server's debug log and every page console line
E2E_PORT=3031 pnpm e2e    # Run them on another port when another checkout's mock server holds 3030
pnpm test-server:dev      # Optional: the E2E mock server with auto-restart (needs watchexec); `pnpm e2e` reuses it

# Linting & Formatting
pnpm lint                 # Run all linting and formatting
pnpm exec vp check        # Formatting, lint and types without fixing (what CI checks first)
pnpm lint:backend         # Format and lint Rust code (clippy --fix)
pnpm lint:frontend        # `vp check --fix` (Oxfmt, type-aware Oxlint) + tsc of the React/TypeScript code and the E2E tests
pnpm lint:cpd             # jscpd copy/paste detection (config in .jscpd.json); fails on any clone
pnpm prune:target --pool  # Dry run: stale cargo units of this checkout and the free treehouse slots (`--yes` deletes)
cargo run -p branch-deck -- cli status -C <repo>  # agent CLI, JSON on stdout (docs/guidelines/agent-cli.md)
```

Fix a clone reported by `lint:cpd` with a helper, constant or fixture. Wrap code in `// jscpd:ignore-start` /
`// jscpd:ignore-end` only when the duplication is intentional, and say why in the comment.

If Homebrew `cargo`/`rustc` come before rustup in `PATH`, prefix cargo commands with `PATH=$HOME/.cargo/bin:$PATH`:
the nightly toolchain pinned in `rust-toolchain.toml` is required to parse the workspace manifest.

## ⚠️ Critical Testing Rule
**NEVER use `cargo test` - ALWAYS use `cargo nextest run`**

This is not optional. `nextest` is mandatory for all Rust testing because it provides:
- **Automatic retries** for flaky tests
- **Parallel execution** for speed
- **Better output formatting** and diffs
- **Proper test isolation**

Using `cargo test` may lead to incorrect results or miss failures.

## Technology Stack
We use modern tools. For specific versions, see `package.json` and `Cargo.toml`.
- **React 19 + Vite 8 SPA**: function components and hooks, `react-router` (`createBrowserRouter` in `src/router.ts`).
- **React Compiler**: its Rust port `oxc-transform-react` (`react({ compiler })` in `vite.config.ts`, no Babel)
  compiles every component and hook; a function it would skip fails the build (`panicThreshold: "all_errors"`): no
  disable comment for a React rule (`react/rule-suppression`; use `useEffectEvent`), no `try`/`finally` without
  `catch` (use `promise.finally`), no JSX parameter default ([Rendering Performance](docs/design/rendering-performance.md)).
  `oxc-transform-react` is pinned to the range `@vitejs/plugin-react` declares; `pnpm update-deps` leaves it alone.
- **zustand**: vanilla stores (`createStore`) read with `useStore` selectors; persisted settings sync across windows.
- **shadcn/ui on Base UI**: components generated into `src/components/ui/` (`components.json`, style `base-rhea`).
- **Font**: InterVariable from `inter-ui` (the full rsms build; `@fontsource-variable/inter` lacks `ss02`/`cv*` features),
  falling back to the system UI font, at 13px (`text-sm`, the IntelliJ UI size). Commit bodies enable `ss04` (I/l/1
  disambiguation). `font-mono` is JetBrains Mono (`@fontsource-variable/jetbrains-mono`) without ligatures.
- **Tailwind CSS v4**: via `@tailwindcss/vite`; theme tokens in `src/index.css`.
- **lucide-react**: icon components.
- **TypeScript 7**: the Go compiler, strict mode, checked with `tsc` (`tsconfig.app.json`, `tsconfig.node.json`,
  `tsconfig.e2e.json`).
- **Vite+** (`vite-plus`, the `vp` CLI): one pinned toolchain of Vite, Vitest, Oxlint and Oxfmt, configured in
  `vite.config.ts` (`lint`, `fmt`). Oxlint is type-aware through tsgolint (TypeScript 7) and runs the React Compiler
  rules; there is no ESLint, because typescript-eslint cannot use TypeScript 7. Oxfmt formats code and config (not
  Markdown) and sorts imports and Tailwind classes. Test files import from `vite-plus/test`.
- **Tauri v2**: desktop application shell.
- **Nextest**: test runner for all Rust tests.
- **Playwright**: End-to-End (E2E) tests.

## Recent Features
- **Integration Status**: Auto-detection and archiving of merged, rebased, or squashed branches.
- **AI Branch Naming**: two names for selected commits from the user's Claude Code (`claude -p --model haiku`, commit
  messages and file names only, after a consent prompt) (`spec/branch/branch-name-suggestions.spec.md`).
- **Context Menus**: Right-click operations (shadcn `ContextMenu`) for common actions on branches and commits.
- **Squash and fixups**: squash selected commits, fold `fixup!`/`squash!`/`amend!` commits (also `bd fold`), with undo
  (`spec/commit/squash.spec.md`, `spec/commit/fixups.spec.md`).
- **Update Project**: fetch and replay onto the baseline, in the app and as `bd update`, with `bd undo update:`; the
  only working-tree write; in the app one progress over its steps (fetch, lock wait, check, files, the sync of an
  applied update), shared by their usual times (`update-progress.ts`) (`spec/repository/update-project.spec.md`). A conflict in `bd update` starts a resolve
  session: one round per conflicting commit, resolved in copies outside the working tree, state in git
  (`spec/cli/update.spec.md`).
- **Landed commits**: what of the user's work the baseline has, before and after an update (also one made elsewhere),
  matched by author, author time and subject; report in `branch-deck/landed.json` (`spec/repository/landed-commits.spec.md`).
- **Outgoing and PR status**: header push overview, GitHub PR state through the GitHub API (`crates/github`; the token of
  `GH_TOKEN`, `gh auth token`, the app's sign-in or `git credential fill`, no `gh` process otherwise), also for a checkout mirrored to GitHub
  (the mirror of `.space/repository-settings.json`, or `branchdeck.githubRepository`); a PR Patronus closed with the label `merged` is merged (`spec/repository/outgoing.spec.md`,
  `spec/branch/pull-requests.spec.md`). "Create Pull Request…" is a dialog that creates the PR through the API
  (`sync_core::pull_request_create`, shared with `bd`): its text from `bd describe` or the pushed commits, the code
  owners Patronus suggests (`githubUsername` of the pre-check) and a search of the repository's users as reviewers,
  the user as assignee; "Continue on GitHub" opens GitHub's form with the text. "Request Review…" (`bd request-review`)
  asks the suggested code owners of the groups whose approval is missing on an open pull request
  (`request_pull_request_reviewers`, the per-reviewer 422 fallback shared with the creation).
- **Patronus**: robot status per branch over the Patronus REST API (`crates/patronus`), polled only while a robot runs
  (every minute focused, every 5 minutes in the background while a robot pushes); a merge is a toast with Fetch and,
  in the background, an OS notification; a running robot whose required check failed is "Failing" (Patronus' warning
  `robot_is_failing`), told once with Cancel Safe Push; Safe Push / Dry Run start with the code-owner review pre-check, restart, cancel, and a
  no push of a branch a running Safe Push lands (`PushHold`; the row's push place stays empty, Push and Restart
  Safe Push cancels, pushes and restarts; `bd push` refuses it as `robot_running`, `--restart-running`); a branch with a GitHub PR starts from it with a `/patronus merge`
  comment, so the PR's approvals count (`spec/branch/patronus.spec.md`); a stacked branch's Safe Push leads
  root first and closes the PRs of the ancestors it landed. One model decides the next step of a branch
  (`getBranchNextStep`, `src/lib/branch-next-step.ts`, the table of `spec/branch/next-step.spec.md`), rendered by the
  slot, "⋯", S, the preview, the review window, the stack header and the form's Start: approvals are counted only by
  Patronus, the app asks it in the background (`src/stores/review-readiness.ts`, the one reader) and never offers a
  Safe Push it would fail ("Awaiting Ilya Korennoy" opens the PR). `bd safe-push` starts one from the agent
  CLI with a merge profile; missing code-owner approvals refuse it (`approvals_missing`) unless `--without-approvals`
  (Patronus judges the approvals of the pull request the robot starts from: the pre-check sends it, the readiness
  `sync_core::review_readiness` is shared with the app, its answer kept in the object cache by paths and reviews); `--create-pull-request`
  opens the GitHub pull request the approvals count in, assigned to the user, `--reviewer <login>` asked (no Space merge request: deprecated). A failed robot's toast copies
  a prompt for an agent (Restart first only when its builds never started): the failed checks with their TeamCity
  builds, the commits it tested by their local hashes (`robot_commits.rs`), with CI Details the failed builds, tests
  and log tails (`teamcity` CLI if logged in); `bd robot <branch> --full` says the same (`spec/branch/safe-push-failure-prompt.spec.md`).
- **GitHub sign-in**: the OAuth device flow of the Branch Deck OAuth app in Settings (client id at build time,
  `BRANCH_DECK_GITHUB_CLIENT_ID`, set by `build-tauri.yml`), the token in a 0600 `github-token.json` of the app data dir that `bd` reads too, not
  the keychain; "Sign In…" after the hint of a repository without a token (`spec/repository/github-sign-in.spec.md`).
- **Issue groups**: an issue key -> branch name map in `refs/branch-deck/issue-groups` puts the commits of several issues
  into one branch without a rewrite (`spec/branch/issue-groups.spec.md`).
- **Conflict fix**: a sync conflict names the branches of the commits it misses (`MissingCommit.branch`, without the
  copies of the branch and its ancestors) and the fix: the regroup trial builds verified (`conflict_fix.rs`: move the
  commit, pull what it needs in, restack; extended by a new dangling reference or conflict; never breaks a clean
  branch), offered, never decided, applied as one regroup with Undo (`apply_conflict_fix`, a fingerprint check under
  the lock), `bd check --fix` for agents (`spec/branch/conflict-fix.spec.md`).
- **Move to a branch**: one operation behind drag and drop of commits and branch rows, the branch selection, the menus
  and the keyboard; the smallest regroup (`move_plan.rs`): a whole issue by the issue groups, a part by the prefix; a
  dry run previews it, `undo_regroup` undoes it in one transaction (`spec/branch/move-commits.spec.md`).
- **Stacked branches** (opt-in, `branchdeck.stacks`): `(parent/child)` builds the child on its parent, dependency-driven
  sync, blocked children; the push button (and `bd push`) pushes the parents with work first, a push retargets the PR
  onto its base (`PATCH pulls/<n>`); the root row heads its stack ("22 branches", collapses with ⌥←, per repository), the
  hovered branch lights its path to the root (`stack-view.ts`) (`spec/branch/stacked-branches.spec.md`); a stack of 5+
  branches or 40+ commits is an island below the table: a header that says what it holds, with the Actions cell of a row for the stack (↑ Push with Children of
  the root, the Safe Push of the main line's last branch, "⋯"; nothing while a Safe Push runs in it), the
  commits on the lanes, oldest first (`spec/branch/stack-island.spec.md`); a side stack collapses into its root (a
  quiet one starts collapsed); Push Up to Here pushes the ancestors whole and the branch up to a commit
  (`PushBranchParams.tip`, one `rev-list` checks the cut).
- **Stack parents**: a child -> parent map in `refs/branch-deck/stack-parents` stacks a branch, an issue branch and an
  issue group included, without a rewrite; Stack On, Unstack, `bd restack`/`unstack` and a plan path `parent/leaf`
  write it, a prefix with a parent level still declares one; a sync drops the line of an integrated branch; a trial
  build of a stacked branch alone (`stack_independence`) lets the Safe Push form, the move toast and `bd safe-push`
  offer the unstack or the move of the commits it needs, never decide it (`spec/branch/stack-parents.spec.md`).
- **Activity**: the header log of what the app did per repository (push, update, landed, Patronus, rewrite, error) in
  the words of its toast, a main-only persistent store (`src/stores/activity.ts`), the rewrite's Undo for its 10 s
  (`spec/repository/activity.spec.md`).
- **Commit files**: "3 files · +42 −7" on the commit meta line, the Files section of the commit preview (`F`) as a
  directory tree (compacted chains, roving focus) with diffs on @pierre/diffs (word diff, IntelliJ syntax colours,
  expandable context), Copy Changed Paths; one `diff-tree --stdin --raw --numstat` per list, cached by commit id in the
  object cache; one `diff-tree -p --full-index` per commit for its diffs, one `cat-file --batch` by blob id to expand
  (`spec/commit/commit-files.spec.md`, `docs/design/diff-view.md`). `D` (on a row, its menu, the preview) reviews the
  commit in the `commit-review` window, the only review of a commit: the tree beside all diffs in a virtualized
  `CodeView`, J/K files, N/P changes, ⌥↑/⌥↓ commits (`spec/commit/commit-review.spec.md`).
- **Title bar**: the top row of every window is its title bar, as in the IntelliJ new UI: beside the macOS traffic
  lights (overlay); on Windows (no decorations) and Linux (GTK client-side decorations with an empty titlebar) with the
  window buttons where the desktop's layout puts them, in IntelliJ's looks, and ☰, the main menu that replaces the
  native menu bar (its keys handled by the page, its commands one function each, `runAppCommand`); the system title bar
  where a custom one doesn't fit (WSL, a tiling WM, no `XDG_CURRENT_DESKTOP`) or Settings → Appearance asks for it. Decided
  once per process (`init_title_bar`), the page reads `window.__BD_TITLE_BAR__` and draws `WindowTitleBar` (the main
  toolbar; a sub-window's `WindowPage`) (`spec/repository/title-bar.spec.md`).
- **Your Turn** (review requests): the island of the open pull requests that ask for the user's review by name
  (one GraphQL search, `user-review-requested:@me`), polled every 5 min with backoff, "Returned to you" for the own pull
  requests with changes requested, a toast or an OS notification per answer with news, the Dock badge
  (`setBadgeSource`); the review (a decision header: Overview / Changes, Approve the primary) fetches the pull request
  into `refs/branch-deck/review/<n>/…` and reviews `base..head` in the range review (`RangeReview`: the commit column
  beside the commit review, as the branch review), only in its own `pull-request-review` window (a row, Enter, D,
  the toast; the main window runs its writes, the fetch counts per window, the draft of Request Changes is kept per
  pull request); an approval of the user's own pull request that ends a wait ("Awaiting Ilya Korennoy") is news
  with Safe Push… (`approval-news.ts`, the poll refreshes the pull requests while a row waits); Settings →
  Notifications holds the notification and badge switches (`spec/repository/review-requests.spec.md`).
- **Settings**: a sub-window of its own (`settings`, `SettingsPage`), a sidebar of pages (General, Appearance,
  Notifications, GitHub, Repository; vertical `Tabs`, ↑/↓) beside one island; only the main window opens it
  (`open_settings`, from ⌘,, ☰, Ctrl+, and the buttons that name a page, `SettingsTarget` `{page, focus}`), the page
  shown last remembered (`appSettings.settingsPage`); its writes go through the main window, and it closes with the
  main window (`close_sub_windows`). Repository reads and writes the `branchdeck.*` git config of the selected
  repository (`.git/config` only; `repository_config_changed` makes the main window sync), refuses stacks off while
  flat names would clash; the branch prefix order is the specific one first, the repository's own, the app's, the
  global git one, in the app as in `bd` (`spec/repository/settings.spec.md`).
- **Branch Review**: Review Changes (D; on a row, its menu, the preview, the code-owner block) reviews a branch in the
  `branch-review` sub-window, the only review of a branch (`BranchReview`): the stack as segments (own part,
  Shift+click to extend, Whole Stack = what Patronus checks; `get_branch_review`, one `for-each-ref` + one `log`), its
  commits, the commit review of the range, owners by the `OWNERSHIP` files of the baseline (`ownership.rs`, one
  `cat-file --batch`) as labels and filter chips; the Safe Push form lists the files of each reviewer group (`paths` from
  Patronus when it sends them, else by owner name) (`spec/branch/branch-review.spec.md`).
- **Symbol dependencies**: tree-sitter summaries of the files the commits change (`crates/code-symbols`: Kotlin, Java,
  Rust, TypeScript and JavaScript), cached by blob id; `bd commits` names the edges by symbols (`via`, the class
  `Order`: never joins `units`), and the dangling-reference check subtracts comments, strings, locals and still-bound
  names; with `branchdeck.symbols on` (the default; `report`, `off`) they also order and stack branches in `bd suggest`,
  the conflict fix, `missing_declarations` of `bd check` and the Safe Push lead (`spec/commit/symbol-dependencies.spec.md`).
- **Zoom**: View → Zoom In / Zoom Out / Actual Size, our own (WKWebView has no zoom keys), 67–150%, one saved level
  for all windows (`appSettings.zoom`), the toolbar not zoomed, the minimum window wider above 125%
  (`spec/repository/zoom.spec.md`).

## Development Rules

### Specs and Docs
Specs (`spec/<area>/<name>.spec.md`) and design docs (`docs/design/`) are part of the change, not a follow-up.
- **Before** changing a feature, read its spec and the design doc it links: the spec is the contract, the doc the why.
- **Same commit**: a behaviour change updates its spec: requirements, `[@test]` links, `targets:` front matter and the
  UX tokens. A renamed or moved test or file updates every link to it; grep `spec/` and `docs/` for the old name.
- **New user-visible feature**: a new spec in the air format (copy the structure of an existing one), plus at most one
  sentence and a spec link in the matching README "Features" group; the detail belongs in the spec. README is the
  pitch for a newcomer: no new bullet per feature, no shortcut lists, nothing internal to one company (Patronus, Space).
- **Git**: a new git operation or a changed flow gets its section in [Git Operations](docs/design/git-operations.md);
  a new or changed hard rule also goes into "Git Operations" below.
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

### Worktrees
Parallel work runs in a worktree from the [treehouse](https://github.com/kunchenguid/treehouse) pool
(`~/.treehouse/<repo>-<hash>/`); never `git worktree add`, `EnterWorktree` or `isolation: "worktree"`.
- **Every session edits in its own lease**, the main session and a background job too, not only a spawned agent: other
  sessions work in this repository at the same time. A harness default such as "edit files directly in your working
  directory" does not override this rule. The main checkout is only for `git merge --ff-only <branch>` into `master`
  ("commit right away" means: commit in the lease, then fast-forward `master`).
- **Acquire**: `treehouse get --lease --lease-holder <agent-name> -b <branch>` fetches origin, creates `<branch>` and
  prints only the worktree path (`--json` adds the lease id, `--no-fetch` skips the fetch). A leased worktree is never
  handed out again or pruned.
- **Base**: the branch starts at local `master`, which may be ahead of `origin/master` (a local branch wins over
  `origin/<base>`); `--base <integration-branch>` cuts it from another branch. Without `-b` the worktree is detached.
- **Inside**: the rules of the main checkout apply (`.githooks/` via the shared `core.hooksPath`, `cargo nextest`).
  `node_modules` and `target/` survive a reuse; run `pnpm install` when `pnpm-lock.yaml` changed.
- **Rust cache**: every checkout keeps its own `target/`; never a shared `CARGO_TARGET_DIR` or `build.build-dir`: the
  unit hash of a workspace crate does not depend on the checkout's path, so worktrees would overwrite each other's
  units. Cargo never deletes a stale unit, so `pnpm prune:target --pool --yes` trims the free slots (dry run without
  `--yes`); dependencies are built with workspace-wide features (`.cargo/config.toml`) and line tables only.
- **E2E**: the mock server listens on 3030 in every checkout; when another worktree's server holds it, run
  `E2E_PORT=3031 pnpm e2e` (any free port). Global setup refuses a server that serves another checkout, because
  Playwright reuses whatever listens on the port.
- **Integrate** as in "Commits and History": `git rebase master` in the worktree, `git merge --ff-only <branch>` in
  the main checkout.
- **Release**: commit, then `treehouse return` inside the worktree (or `treehouse return <path>`); it cleans and
  resets the worktree, the branch survives.
  `treehouse status` lists the pool; `treehouse prune` and `treehouse destroy` are dry runs without `--yes`.

### Git Operations
Hard rules; rationale and per-operation flows in [Git Operations](docs/design/git-operations.md).
- **Git 2.54+** (`MIN_VERSION` in `crates/git-executor/src/git_info.rs`), checked when git is discovered, on every platform.
- All git runs through `GitCommandExecutor`. It adds `-c core.quotePath=false -c color.ui=never -c log.showSignature=false
  -c i18n.logOutputEncoding=UTF-8`, sets `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0`, strips inherited
  `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/`GIT_NOTES_REF`, empties `GIT_NOTES_DISPLAY_REF` (`%N` shows only the mapping
  notes, or an explicit `--notes=<ref>`) and gives git a null stdin unless the call pipes one; callers don't add
  `--no-pager`. A git with an environment, stdin, accepted exit codes, a timeout, streamed stdout, progress or a reply
  protocol is `git.cmd(&args, repo)` with `.env`, `.stdin`, `.accept`, `.timeout` and one finisher (`.text`, `.bytes`,
  `.lines`, `.output`, `.status`, `.stream`, `.progress`, `.interactive`), all on one spawn core; the only shorthands are
  `execute_command` (`.text`), `execute_command_lines` (`.lines`) and `execute_command_with_status` (`.status` as text).
- **Never write to the working tree**: no `checkout`, `checkout-index`, `switch`, `reset`, `rebase`, `stash`, `commit`,
  `cherry-pick`, `merge`, `restore`, `clean`, `apply` without `--cached`, `read-tree -u`/`--reset`, `git history`;
  `crates/git-executor/src/worktree_safety_test.rs` enforces it. Reading worktree files into a temporary
  `GIT_INDEX_FILE` is fine. The single exception is the guarded two-way `read-tree -m -u` in
  `crates/git-ops/src/worktree_update.rs` (dry run on an index copy first, then the CAS ref move, rollback on failure).
  Its functions are `pub(crate)` and only `update_branch.rs` calls them, for Update Project (`apply_update`, the app
  and `bd update`), its undo (`apply_update_undo` of an `UpdateUndo` that only `check_update_undo` builds, the
  trees swapped), and the finish of a resolve session (`update_resolve.rs` through the `pub(crate)`
  `move_branch_and_files_with` and `files_follow`, to the tip it reads from the session ref). The scan counts its
  `-u` argument lists. Don't add another caller, another argument list, or a public way to build a move; see "Update
  project" in the design doc.
- **The user's index** changes only where HEAD's tree changed (`IndexUpdate` in `crates/git-ops/src/rewrite_guard.rs`):
  a drop runs `read-tree -m -i <old> <new>` after a preflight on a copy of the index, an amend stages the amended files.
  Staged work on other paths survives; never `reset --mixed`, never ignore an index error.
- **Ref moves**: one `RefTransaction` (`update-ref --stdin`) with old-value CAS and a `branch-deck: <op>` reflog
  message; related refs (unapply rename, temp refs) go into the same transaction. The named exception is a notes ref
  that `fast-import` writes (`write_notes_to_refs`, `write_commits_with_notes`): it moves the ref itself
  (`from <ref>^0`) and refuses a tip that does not contain the ref's value at its end, so a concurrent writer fails the
  run instead of being overwritten; the resolve notes move in the session's transaction. A move that the files follow (an
  update and its undo) updates `HEAD` itself (deref) and commits only if `symbolic-ref HEAD`, read while git holds the
  locks after `prepare`, is still the checked branch (`RefTransaction::commit_while`, else `BranchSwitched`); git
  refuses `symref-verify HEAD` next to either form of the update. The finish of a resolve session moves the session
  ref and the notes ref (`refs/notes/branch-deck/resolve`, written only there) in that transaction.
- **Issue groups** (`refs/branch-deck/issue-groups`, `issue_groups.rs`) and **stack parents**
  (`refs/branch-deck/stack-parents`, `stack_parents.rs`): two refs, never one tree (a store commit replaces its whole
  tree). Only a regroup writes them, under the repository lock, in the ref transaction that moves HEAD, with CAS on
  every ref it moves; a sync only reads them, except that it drops the stack parent of a branch whose archived branch
  is integrated (one CAS write, best effort, `drop_integrated_stack_parents`).
- **Repository lock**: every rewrite of the checked-out branch takes it in its core, before it reads anything: the
  regroup (`regroup.rs`) and the move of a hunk through their options, and the app's squash and its undo, squash
  fixups (also `bd fold`), amend, unapply, reword and add issue reference through `RewriteOptions` (`held_lock`,
  `lock_timeout`, `expect_head` checked by `ensure_expected_head`, `repository_lock.rs`); then `bd sync`, `bd push`,
  `bd safe-push` (the sync, the checks and the push; the robot start after it), `bd move-hunk`, `bd undo`,
  Update Project (the app and `bd update`, after the fetch, also when it starts a resolve session), and
  `bd update --continue` and `--abort` hold `$GIT_COMMON_DIR/branch-deck.lock`
  (`repository_lock.rs`, bounded wait, then `locked`), because the app and `bd` processes rewrite the same branch.
  A fetch never runs under it.
- **Guards** (`check_rewrite`) before rewriting the checked-out branch: no rebase/merge/cherry-pick/revert in progress,
  HEAD not detached, no merges in the range (refuse, no rebase fallback). A sync (the app's and `bd sync`) is refused
  under the first two as `SyncDeferred` (`check_sync_guards`): a half-done history would archive the branches it lacks.
- **Metadata**: message-only rewrites (reword, move) keep author and committer byte-for-byte (`commit_writer`:
  `cat-file --batch` + `fast-import`); content rewrites (amend, drop via `git replay`) keep the author, committer = current
  user. The one named exception: `bd move-hunk` (`move_hunk.rs`) keeps author and committer byte-for-byte for every
  commit whose diff stays the same (`patch-id --stable`; the commits between the two and above), so their virtual
  commits stay; `from`, `to` and the commits whose diff of the file changed get committer = current user and no mapping
  note. Signatures are stripped.
- **Batch**: one process per phase, not per item (`--stdin`, `cat-file --batch`, `fast-import` for commits and notes,
  `update-ref --stdin`); the patches of a list are warmed by one `diff-tree --stdin -p`, cut at its commit id lines
  (`warm_patches`). A chain whose next step needs the answer of the one before keeps one interactive process per
  chain: a sync merges a branch in one `merge-tree --stdin` and writes its copies with one `fast-import`
  (`commit_chain.rs`), never `commit-tree` per commit; `with_process_stats` counts the processes of an operation. A
  resolve session finds the first conflicting commit with a binary search over prefixes:
  ⌈log₂ n⌉ `replay` runs, then one `replay` of the rest per round. Never parse translated text: exit codes,
  `--porcelain`, `-z`. The one named exception: a branch description costs one `git config` per changed branch
  (`bd describe`, `descriptions` of a `bd group` plan), because git has no batch config writer; read them with one
  `--get-regexp`.
- Handle special exit codes: `merge-tree` and `replay` exit with 1 on conflicts; `merge-tree --stdin` exits with 0
  even then, and each merge of its output starts with its status, 1 clean, 0 conflicted (`parse_batch_merge`);
  `merge-base` exits with 1 for "no" (`--is-ancestor`) or "none" (no merge base, no fork point), and any other non-zero
  is an error, never a "no" (`git_ops::revs`).
- **Resolve session store** (`resolve_copies.rs`): the copies an agent edits live in `branch-deck/resolve/` of the
  worktree's git dir, or in `--dir` (absolute, not a symlink, outside every worktree, empty or marked for this git dir,
  used by no other session); the marker `.branch-deck-resolve` names the owner's git dir; flat names
  `<key>-<basename>` (the basename cut to 100 bytes), never tree paths; tree paths validated (no empty, `.`, `..` or
  `.git` component, nor an NTFS/HFS+ alias of `.git`, `gix-validate`); regular files only, bytes as git stores them without filters, never read, written or deleted
  through a symlink (of the file or of any directory below the git dir or `--dir`), every write a temporary file plus
  a rename; delete only the files bd wrote, only in a directory whose marker names this git dir. All other session
  state is in git (`refs/branch-deck/resolve/<worktree id>`, a shared ref: a gc in another worktree prunes a
  `refs/worktree/` ref).
- **Network commands** (push, fetch) run through `execute_remote_command` (`crates/sync-core/src/remote_command.rs`):
  the user's credential helpers stay, prompts are off, stalled HTTP transfers abort, the locale is C (untranslated
  `--progress` lines). Never pass `-c credential.helper=`.
- **Git config writes**: only `--local` (`.git/config`), only the `branchdeck.*` keys of Settings → Repository
  (`repository_config.rs`: one `--get-regexp --show-scope` read, `--replace-all` or `--unset-all`, refused when the
  worktree config or `git -c` sets the key) and the branch descriptions of `bd describe`; never `--global`.
- **GitHub credentials**: the only git of the GitHub token chain is `git credential fill` (`crates/github/src/token.rs`),
  a `GitCommand` with `.timeout` (10 s) and `.status`, with `credential.interactive=false`, an empty `GIT_ASKPASS` and
  `GCM_INTERACTIVE=never`; never `credential approve` or `reject`. GitHub itself is asked over HTTP (`GitHubService`),
  never by a `gh` process.
- **Review fetch** (`review_fetch.rs`): the only writer of `refs/branch-deck/review/<n>/{head,base}`, through its own
  fetch refspecs (the named exception to the ref transaction rule); always `--refmap=` and `--no-prune`, so no
  remote-tracking ref moves; never under the repository lock; no fetch at all when both commits are local; killed on
  cancel or 120 s of silence (`execute_remote_command_with_progress_until`). Pruning is one `for-each-ref` plus one
  CAS `update-ref --stdin`. Range diffs (`DiffSubject::Range`) are never written to the object cache: the app keeps
  them in memory only (`PatchCache`), the files and the patch of a range from one `diff-tree`.
- **Symbols** (`symbol_scan.rs`): source blobs by id from the `--raw --no-abbrev` of the commit diffs; one
  `cat-file --batch` of the directories above them at HEAD, one for the blobs the `blob_summary` cache misses and the
  manifests; the declarations the range adds are the index, never the whole tree. The summaries of trial blobs (the
  dangling-reference check) stay in memory, never in the object cache.

### Rust Import Rules
- **NO RE-EXPORTS**: Never use `pub use` statements except for internal module organization.
- **Explicit Imports**: All imports must use full module paths (e.g., `use git_executor::git_command_executor::GitCommandExecutor`).
- **Module Structure**: Make modules public with `pub mod` instead of re-exporting their contents.
- **Clarity**: Every import should clearly show where types and functions originate from.

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
- **Icons**: lucide-react components (`<GitBranchIcon />`). Inside `Button`, add `data-icon="inline-start"` (or `inline-end`)
  and no size classes; the button sizes them. Elsewhere use `size-*` classes (e.g. `size-3`).
- **Components**: `Tooltip` for simple text tooltips, `Popover` for rich content, `Kbd` for keyboard shortcuts,
  `showToast({ title, description, action })` from `@/lib/toast-message` for notifications, `notifyError` for errors.
- **Wording** ([UX Writing](docs/guidelines/ux-writing.md)): a toast title says what happened ("Pushed feature-ui"),
  never "Success"/"Error"; errors are "Cannot <verb> <object>" with the reason; a toast about an object opens it
  ("Open in Patronus"); Title Case for buttons, menu items and toast actions, sentence case for the rest; "…", "·",
  "canceled", no exclamation marks.
- **Actions** ([UX Writing → Actions](docs/guidelines/ux-writing.md#actions)): one builder of `MenuItem`s (stable `id`) per
  object, rendered by every surface; the same actions in every form; a key hint only where the key works; disabled
  with a reason, not hidden; one name per action ("Open Pull Request", "Restart Safe Push").
- **Base UI**: compose with the `render` prop (`<TooltipTrigger render={<Button />} />`), not `asChild`. Base UI emits
  `data-open`/`data-closed`; set `data-state` manually where E2E tests read it.
- **shadcn workflow**: run the CLI with `npx shadcn@latest …` (`add <component>`, `docs`, `init --preset`), then adapt
  the files. `pnpm dlx shadcn@latest` crashes with `o.deepPartial is not a function`: `@shadcn/registry` uses the zod 3
  API and pnpm dlx resolves zod 4.
- **Locally customised ui files**: `alert`, `badge`, `button`, `card`, `checkbox`, `input`, `popover`, `spinner`, `tabs`,
  `textarea`, `tooltip`, `toast` in `src/components/ui/` carry a `// local customisation: …` header naming the edits. After
  `add --overwrite` (e.g. a style switch), re-apply them; `git diff HEAD -- src/components/ui` shows what was lost.
- **TypeScript**: Use `u32` for timestamps and `snake_case` for properties that bind to Rust structs.
- **Rust**: Use `snake_case` for all variables and functions.
- **Clippy**: pedantic, nursery and cargo plus the restriction lints that catch defects (`[workspace.lints]` in
  `Cargo.toml`), on all targets; any warning fails `pnpm lint:backend` and CI (`-D warnings`), Windows included. An
  exception is `#[expect(lint, reason = "…")]` where it holds, never `#[allow]`; a lint that is wrong for this code is
  allowed in `Cargo.toml` with its reason.
- **Imports**: Always use explicit, full module paths. Never use `pub use` re-exports. Examples:
  - ✅ `use git_executor::git_command_executor::GitCommandExecutor`
  - ✅ `use sync_types::issue_navigation::IssueNavigationConfig`
  - ❌ `pub use pull_request::PullRequestAvailability` (in lib.rs)

### Naming Conventions
- **Components**: PascalCase `.tsx` files with a named export (e.g., `BranchTableCard.tsx`).
- **Hooks**: camelCase with `use` prefix in `src/hooks` (e.g., `useRowSelection.ts`).
- **Stores**: zustand stores in `src/stores/*.ts` (e.g., `branch-sync.ts`), persisted settings in `src/stores/settings/`.
- **Rust-bound props**: `snake_case`, as in the generated bindings.
- **Rust modules**: snake_case (e.g., `git_ops.rs`).

### Frontend Imports (IMPORTANT!)
- Import every React API, component, hook, store, utility and type explicitly; types with `import type`.
- Project modules only through the `@/` alias (`@/stores/branch-sync`), never `../`. shadcn: `import { Button } from "@/components/ui/button"`.
- Generated Rust bindings: `import { commands } from "@/lib/bindings"` (written by Specta from `src-tauri/src/lib.rs`).
- Heavy or rarely shown components: `React.lazy` with the named export, rendered inside `Suspense`.
- See [Import Guidelines](docs/guidelines/imports.md).

## Performance & Debugging

### Performance Guidelines
- **Frontend**: Select narrow slices from zustand stores: the compiler memoizes renders, not selectors, which run on every
  store change. Debounce user inputs where appropriate (e.g., using `p-debounce`). See [Rendering Performance](docs/design/rendering-performance.md).
- **Backend**: One git process per operation phase, never one per commit, note or file (see [Git Operations](docs/design/git-operations.md)).

### Debugging & Troubleshooting
- **E2E Test Issues**: a failed test carries the console errors of its pages as the `page errors` attachment. Run
  with `DEBUG_E2E=1` for the debug log of the mock server and every console line live; without it the server logs
  warnings and errors only (`TEST_SERVER_LOG`, not `RUST_LOG`, which `.cargo/config.toml` forces to debug), and a
  green run prints none.
- **Playwright browser errors**: If tests fail with "Executable doesn't exist", run `pnpm setup:e2e` to install browsers. This happens automatically when running `pnpm e2e`, but may be needed after updating `@playwright/test`.
- **Type Errors**: Run `pnpm lint:typecheck` to isolate and verify TypeScript issues.

### The Real App (macOS, Peekaboo)
What a release build shows on screen (a window that opens late, a frame of "Loading…") is checked in the real app,
not only in E2E: `pnpm install-app` (5–7 min, quits and replaces /Applications/BranchDeck.app), then drive it with
the `peekaboo` CLI.
- **Through Peekaboo.app**: its Bridge holds Screen Recording, Accessibility and Event Synthesizing. Keep Peekaboo.app
  open, never pass `--no-remote`, never delete or replace the CLI (other agents call it), kill nothing. Peekaboo may
  pick its on-demand daemon (`daemon-*.sock`), which has no permissions ("Operation hotkey is not allowed"): pass
  `--bridge-socket "$HOME/Library/Application Support/Peekaboo/bridge.sock"` on every call; if the socket is missing,
  ask the user to open Peekaboo.app. From an agent shell `screencapture` and `osascript` keystrokes are refused.
- **Windows**: `peekaboo window list --pid <pid> --json` gives the window ids (a webview window has several rows);
  `peekaboo window focus --window-id <id>`, a screenshot with `peekaboo see --window-id <id> --no-elements --path x.png`.
- **Input**: `peekaboo move --at x,y --global --foreground` (a move needs `--foreground` as consent), `peekaboo click
  --at x,y --global --foreground`, `peekaboo press d --foreground`.
- **Frames of an open**: start `peekaboo capture live --mode area --region x,y,w,h --duration 2500 --active-fps 15
  --threshold 0 --path <dir>` in the background, then press; `<dir>/metadata.json` lists the frames with their time
  and change, so a blank or "Loading" frame before the content shows.
- **Timings**: every sub-window open logs its phases in `~/Library/Logs/org.develar.branch-deck/BranchDeck.log`
  ("commit-review open 258 ms: loaded 91, files 187, patch 187, engine 195, primed 235"), the backend the source of
  each patch (memory, disk, git) at debug level.

## Error Handling Patterns
- **Frontend**: Every user-facing error goes through `notifyError` in `src/lib/errors.ts` (sticky toast, logged to the Tauri log plugin, recorded in Activity), titled "Cannot <verb> <object>" ([UX Writing](docs/guidelines/ux-writing.md)).
- **Backend (Rust)**: Use the `tracing` crate for structured logging. Use `#[instrument]` to automatically trace function entry/exit and arguments. Return proper `Result` types, not strings.

## Architecture

### Design Documents
- [Async Patterns and spawn_blocking](docs/design/async-patterns.md) - Explains async/sync boundaries and proper use of spawn_blocking
- [Diff View](docs/design/diff-view.md) - The @pierre/diffs engine: Shiki in a worker pool, IntelliJ syntax themes, word diff, the lazy chunk
- [Distributed Settings Architecture](docs/design/distributed-settings.md) - Settings synchronization across windows
- [Rendering Performance](docs/design/rendering-performance.md) - React Compiler, narrow selectors, sync events applied in batches, derived structure cached, lazy main page
- [E2E Testing Architecture](docs/design/e2e-testing.md) - E2E test infrastructure with mock Tauri API
- [Git Operations](docs/design/git-operations.md) - Working-tree safety, exact-metadata rewrites, `git replay`, batching and the Git 2.54 minimum
- [Integration Detection and Caching](docs/design/integration-detection.md) - How branch integration detection works with git notes caching
- [Remote Status](docs/design/remote-status.md) - How remote tracking status is computed and displayed
- [Symbol Dependencies](docs/design/symbol-dependencies.md) - tree-sitter summaries of the changed files, `Order` edges by evidence, the subtraction in the dangling-reference check
- [UI Theme](docs/design/ui-theme.md) - Islands layering, the radius scale and the rules for UI changes
- [Virtual Branches](docs/design/virtual-branches.md) - How commits are grouped and cherry-picked into virtual branches
- [Import Guidelines](docs/guidelines/imports.md) - No re-exports policy and proper import patterns
- [UX Writing](docs/guidelines/ux-writing.md) - Toast titles and actions, "Cannot …" errors, Title Case and sentence case, punctuation
- [Design Docs Guidelines](docs/guidelines/design-docs.md) - How to write design documents
- [Specs](spec/) - Requirement specs in the air format (spec/<area>/<name>.spec.md, [@test] links); start with spec/commit/commit-row.spec.md

### File Structure
```
src/
├── components/
│   ├── ui/          # shadcn/ui components (generated)
│   ├── layout/      # Layout primitives (Container, FieldGroup)
│   ├── shared/      # Shared app components (cards, inline forms, copy button)
│   ├── repository/  # Header, repository selector, welcome card
│   ├── branch/      # Branch tables, rows, inline actions
│   ├── commit/      # Commit list, commit info, floating selection bar
│   ├── unassigned/  # Unassigned commits card and inline branch creator
│   ├── conflict/    # Conflict and missing-commits views
│   ├── diff/        # Diff views on @pierre/diffs (lazy chunk)
│   ├── files/       # The directory tree of changed files
│   ├── ai/          # AI branch name suggestions (Claude Code)
│   └── settings/    # The pages of the Settings window and their rows
├── hooks/           # React hooks (useX.ts)
├── stores/          # zustand stores; settings/ holds persisted, cross-window settings
├── lib/             # Utilities and generated bindings.ts
├── pages/           # Route components (index, the sub-windows: reviews, conflict windows, settings)
├── router.ts        # react-router routes
└── main.tsx         # App entry (store handlers, router)
src-tauri/
├── commands/        # Tauri commands
├── ...
crates/
├── git-executor/    # Git CLI executor
├── git-ops/         # Git operations library
├── sync-core/       # Branch sync logic
├── bd-cli/          # `bd` agent CLI (docs/guidelines/agent-cli.md)
├── object-cache/    # LMDB cache keyed by git object ids, shared by the app and bd
├── code-symbols/    # Declarations, imports and references of a Kotlin/Java/Rust/TS file on tree-sitter, no git
├── patronus/        # Patronus REST client, Space token of the Safe Push CLI, request policy
├── github/          # GitHub REST and GraphQL client (rate limits, SSO, conditional GETs), fake GitHub for tests
├── test-server/     # E2E mock server
└── ...
tests/
└── e2e/             # Playwright specs by feature, their HTML snapshots, fixtures/ and helpers/
```

## How Branch Deck Virtual Branches Work
### Core Concept
Branch Deck automatically groups commits by prefix (e.g., `(feature-auth)`) and creates virtual branches by cherry-picking these grouped commits onto a new base.

### The Sync Process
1.  **Commit Grouping**: `CommitGrouper` reads commits and groups them by prefix.
2.  **Virtual Branch Creation**: For each group, it creates a branch `{user}/virtual/{prefix}` by cherry-picking commits.
3.  **Note Structure**: A git note on each ORIGINAL commit (the default notes ref, `core.notesRef`) names its NEW virtual
    copy, `v-commit-v1:<copy>`, and the trees it was copied from, `v-commit-source-v1:<parent tree> <tree>`
    (`MappingNote` in `crates/git-ops/src/notes.rs`); a later sync reuses that copy while the original's trees match.

## Conflict Handling
- Uses `git merge-tree --write-tree` for conflict detection, never the working tree.
- Shows each conflict of a file as the change of the base against the change of the commit (the diff engine's
  `UnresolvedFile`), and offers the regroup a trial build verified (`spec/branch/conflict-fix.spec.md`).
- Conflicts occur when grouping by prefix changes the linear history of commits that depend on each other.

