# Branch Deck Development Context

## Project Overview
Desktop Git branch management tool using React 19 + Vite + Tauri v2. Use `pnpm` only.

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
pnpm test:unit            # Run frontend unit tests (vitest, src/**/*.test.ts)

# E2E Testing (Playwright)
pnpm setup:e2e            # First-time setup: Install Playwright browsers
pnpm e2e                  # Run Playwright E2E tests (auto-installs browsers if needed)
pnpm e2e-up               # Run E2E tests and update snapshots
DEBUG_E2E=1 pnpm e2e      # Run E2E tests with the mock server's debug log and every page console line
E2E_PORT=3031 pnpm e2e    # Run them on another port when another checkout's mock server holds 3030
pnpm test-server:dev      # Optional: the E2E mock server with auto-restart (needs watchexec); `pnpm e2e` reuses it

# Linting & Formatting
pnpm lint                 # Run all linting and formatting
pnpm lint:backend         # Format and lint Rust code (clippy --fix)
pnpm lint:frontend        # ESLint + tsc type-check of the React/TypeScript code
pnpm lint:cpd             # jscpd copy/paste detection (config in .jscpd.json); fails on any clone
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
- **React Compiler**: Babel `babel-plugin-react-compiler` (`vite.config.ts`) compiles every component and hook; a
  function it would skip fails the build (`panicThreshold: "all_errors"`): no `eslint-disable` of a `react-hooks` rule
  (use `useEffectEvent`), no `try`/`finally` without `catch` (use `promise.finally`), no JSX parameter default
  ([Rendering Performance](docs/design/rendering-performance.md)).
- **zustand**: vanilla stores (`createStore`) read with `useStore` selectors; persisted settings sync across windows.
- **shadcn/ui on Base UI**: components generated into `src/components/ui/` (`components.json`, style `base-rhea`).
- **Font**: InterVariable from `inter-ui` (the full rsms build; `@fontsource-variable/inter` lacks `ss02`/`cv*` features),
  falling back to the system UI font, at 13px (`text-sm`, the IntelliJ UI size). Commit bodies enable `ss04` (I/l/1
  disambiguation). `font-mono` is JetBrains Mono (`@fontsource-variable/jetbrains-mono`) without ligatures.
- **Tailwind CSS v4**: via `@tailwindcss/vite`; theme tokens in `src/index.css`.
- **lucide-react**: icon components.
- **TypeScript 6**: strict mode, checked with `tsc`.
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
  only working-tree write (`spec/repository/update-project.spec.md`). A conflict in `bd update` starts a resolve
  session: one round per conflicting commit, resolved in copies outside the working tree, state in git
  (`spec/cli/update.spec.md`).
- **Landed commits**: what of the user's work the baseline has, before and after an update (also one made elsewhere),
  matched by author, author time and subject; report in `branch-deck/landed.json` (`spec/repository/landed-commits.spec.md`).
- **Outgoing and PR status**: header push overview, GitHub PR state through the GitHub API (`crates/github`; the token of
  `GH_TOKEN`, `gh auth token`, the app's sign-in or `git credential fill`, no `gh` process otherwise), also for a checkout mirrored to GitHub
  (the mirror of `.space/repository-settings.json`, or `branchdeck.githubRepository`); a PR Patronus closed with the label `merged` is merged (`spec/repository/outgoing.spec.md`,
  `spec/branch/pull-requests.spec.md`).
- **Patronus**: robot status per branch over the Patronus REST API (`crates/patronus`), polled only while a robot runs
  (every minute focused, every 5 minutes in the background while a robot pushes); a merge is a toast with Fetch and,
  in the background, an OS notification; a running robot whose required check failed is "Failing" (Patronus' warning
  `robot_is_failing`), told once with Cancel Safe Push; Safe Push / Dry Run start with the code-owner review pre-check, restart, cancel, and a
  guard before a push into a branch whose Safe Push runs; a branch with a GitHub PR starts from it with a `/patronus merge`
  comment, so the PR's approvals count (`spec/branch/patronus.spec.md`); a stacked branch's Safe Push leads
  root first and closes the PRs of the ancestors it landed. `bd safe-push` starts one from the agent
  CLI with a merge profile; missing code-owner approvals refuse it (`approvals_missing`) unless `--without-approvals`; `--create-pull-request`
  opens the GitHub pull request the approvals count in (no Space merge request: deprecated).
- **GitHub sign-in**: the OAuth device flow of the Branch Deck OAuth app in Settings (client id at build time,
  `BRANCH_DECK_GITHUB_CLIENT_ID`, set by `build-tauri.yml`), the token in a 0600 `github-token.json` of the app data dir that `bd` reads too, not
  the keychain; "Sign In…" after the hint of a repository without a token (`spec/repository/github-sign-in.spec.md`).
- **Issue groups**: an issue key -> branch name map in `refs/branch-deck/issue-groups` puts the commits of several issues
  into one branch without a rewrite (`spec/branch/issue-groups.spec.md`).
- **Move to a branch**: one operation behind drag and drop of commits and branch rows, the branch selection, the menus
  and the keyboard; the smallest regroup (`move_plan.rs`): a whole issue by the issue groups, a part by the prefix; a
  dry run previews it, `undo_regroup` undoes it in one transaction (`spec/branch/move-commits.spec.md`).
- **Stacked branches** (opt-in, `branchdeck.stacks`): `(parent/child)` builds the child on its parent, dependency-driven
  sync, blocked children; the push button (and `bd push`) pushes the parents with work first, a push retargets the PR
  onto its base (`PATCH pulls/<n>`); the root row heads its stack ("22 branches", collapses with ⌥←, per repository), the
  hovered branch lights its path to the root (`stack-view.ts`) (`spec/branch/stacked-branches.spec.md`); a stack of 5+
  branches or 40+ commits is an island below the table: a header that says what it holds and pushes it all, the
  commits on the lanes, oldest first (`spec/branch/stack-island.spec.md`).
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
  (`spec/commit/commit-files.spec.md`, `docs/design/diff-view.md`). `D` reviews the commit: the tree beside all diffs in
  a virtualized `CodeView`, J/K files, N/P changes, W in the `commit-review` window; D and W work on a row too, as on a
  branch and a pull request (`spec/commit/commit-review.spec.md`).
- **Title bar**: the toolbar is the title bar, as in the IntelliJ new UI: beside the macOS traffic lights (overlay),
  with its own window buttons on Windows (no decorations), below the native title bar on Linux; the page learns the kind
  from `window.__BD_TITLE_BAR__` (`spec/repository/title-bar.spec.md`).
- **Your Turn** (review requests): the island of the open pull requests that ask for the user's review by name
  (one GraphQL search, `user-review-requested:@me`), polled every 5 min with backoff, "Returned to you" for the own pull
  requests with changes requested, a toast or an OS notification per answer with news, the Dock badge
  (`setBadgeSource`); the review (a decision header: Overview / Changes, Approve the primary) fetches the pull request
  into `refs/branch-deck/review/<n>/…` and reviews `base..head` in the range review (`RangeReview`: the commit column
  beside the commit review, as the branch review), in the dialog
  or in its own `pull-request-review` window (W, ⇧-click; the main window runs its writes, the fetch counts per
  window); Settings… (⌘,) holds the notification and badge switches (`spec/repository/review-requests.spec.md`).
- **Branch Review**: Review Changes (D) reviews a branch in its preview dialog, W in the `branch-review` sub-window (one
  `BranchReview`, the place carried over): the stack as segments (own part,
  Shift+click to extend, Whole Stack = what Patronus checks; `get_branch_review`, one `for-each-ref` + one `log`), its
  commits, the commit review of the range, owners by the `OWNERSHIP` files of the baseline (`ownership.rs`, one
  `cat-file --batch`) as labels and filter chips; the Safe Push form lists the files of each reviewer group (`paths` from
  Patronus when it sends them, else by owner name) (`spec/branch/branch-review.spec.md`).
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
- **Acquire**: `treehouse get --lease --lease-holder <agent-name> -b <branch>` fetches origin, creates `<branch>` and
  prints only the worktree path (`--json` adds the lease id, `--no-fetch` skips the fetch). A leased worktree is never
  handed out again or pruned.
- **Base**: the branch starts at local `master`, which may be ahead of `origin/master` (a local branch wins over
  `origin/<base>`); `--base <integration-branch>` cuts it from another branch. Without `-b` the worktree is detached.
- **Inside**: the rules of the main checkout apply (`.githooks/` via the shared `core.hooksPath`, `cargo nextest`).
  `node_modules` and `target/` survive a reuse; run `pnpm install` when `pnpm-lock.yaml` changed.
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
- All git runs through `GitCommandExecutor`. It adds `-c core.quotePath=false -c color.ui=never -c log.showSignature=false`,
  strips inherited `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/`GIT_NOTES_REF` and empties `GIT_NOTES_DISPLAY_REF` (`%N` shows
  only the mapping notes, or an explicit `--notes=<ref>`); callers don't add `--no-pager`.
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
  message; related refs (unapply rename, temp refs) go into the same transaction. A move that the files follow (an
  update and its undo) updates `HEAD` itself (deref) and commits only if `symbolic-ref HEAD`, read while git holds the
  locks after `prepare`, is still the checked branch (`RefTransaction::commit_while`, else `BranchSwitched`); git
  refuses `symref-verify HEAD` next to either form of the update. The finish of a resolve session moves the session
  ref and the notes ref (`refs/notes/branch-deck/resolve`, written only there) in that transaction.
- **Issue groups** (`refs/branch-deck/issue-groups`, `issue_groups.rs`) and **stack parents**
  (`refs/branch-deck/stack-parents`, `stack_parents.rs`): two refs, never one tree (a store commit replaces its whole
  tree). Only a regroup writes them, under the repository lock, in the ref transaction that moves HEAD, with CAS on
  every ref it moves; a sync only reads them, except that it drops the stack parent of a branch whose archived branch
  is integrated (one CAS write, best effort, `drop_integrated_stack_parents`).
- **Repository lock**: rewrites that regroup commits (`regroup.rs`), `bd sync`, `bd push`, `bd safe-push` (the sync,
  the checks and the push; the robot start after it), `bd fold`, `bd undo`,
  Update Project (the app and `bd update`, after the fetch, also when it starts a resolve session), and
  `bd update --continue` and `--abort` hold `$GIT_COMMON_DIR/branch-deck.lock`
  (`repository_lock.rs`, bounded wait, then `locked`), because the app and `bd` processes rewrite the same branch.
  A fetch never runs under it.
- **Guards** (`check_rewrite`) before rewriting the checked-out branch: no rebase/merge/cherry-pick/revert in progress,
  HEAD not detached, no merges in the range (refuse, no rebase fallback). A sync (the app's and `bd sync`) is refused
  under the first two as `SyncDeferred` (`check_sync_guards`): a half-done history would archive the branches it lacks.
- **Metadata**: message-only rewrites (reword, move) keep author and committer byte-for-byte (`commit_writer`:
  `cat-file --batch` + `fast-import`); content rewrites (amend, drop via `git replay`) keep the author, committer = current
  user. Signatures are stripped.
- **Batch**: one process per phase, not per item (`--stdin`, `cat-file --batch`, `fast-import` for commits and notes,
  `update-ref --stdin`). A resolve session finds the first conflicting commit with a binary search over prefixes:
  ⌈log₂ n⌉ `replay` runs, then one `replay` of the rest per round. Never parse translated text: exit codes,
  `--porcelain`, `-z`. The one named exception: a branch description costs one `git config` per changed branch
  (`bd describe`, `descriptions` of a `bd group` plan), because git has no batch config writer; read them with one
  `--get-regexp`.
- Handle special exit codes: `merge-tree` and `replay` exit with 1 on conflicts.
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
- **GitHub credentials**: the only git of the GitHub token chain is `git credential fill` (`crates/github/src/token.rs`),
  through `execute_command_with_input_until` (10 s), with `credential.interactive=false`, an empty `GIT_ASKPASS` and
  `GCM_INTERACTIVE=never`; never `credential approve` or `reject`. GitHub itself is asked over HTTP (`GitHubService`),
  never by a `gh` process.
- **Review fetch** (`review_fetch.rs`): the only writer of `refs/branch-deck/review/<n>/{head,base}`, through its own
  fetch refspecs (the named exception to the ref transaction rule); always `--refmap=` and `--no-prune`, so no
  remote-tracking ref moves; never under the repository lock; no fetch at all when both commits are local; killed on
  cancel or 120 s of silence (`execute_remote_command_with_progress_until`). Pruning is one `for-each-ref` plus one
  CAS `update-ref --stdin`. Range diffs (`DiffSubject::Range`) are never cached.

### Rust Import Rules
- **NO RE-EXPORTS**: Never use `pub use` statements except for internal module organization.
- **Explicit Imports**: All imports must use full module paths (e.g., `use git_executor::git_command_executor::GitCommandExecutor`).
- **Module Structure**: Make modules public with `pub mod` instead of re-exporting their contents.
- **Clarity**: Every import should clearly show where types and functions originate from.

### Code Style
- **Colors**: shadcn semantic tokens only (`text-muted-foreground`, `bg-background`, `bg-muted`, `border-border`, `text-primary`,
  `text-destructive`) plus the custom `canvas`, `success`, `warning`, `info` (`text-success`, `bg-warning/10`). **Never** use
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
- **Locally customised ui files**: `alert`, `badge`, `button`, `card`, `input`, `popover`, `spinner`, `textarea`,
  `tooltip`, `toast` in `src/components/ui/` carry a `// local customisation: …` header naming the edits. After
  `add --overwrite` (e.g. a style switch), re-apply them; `git diff HEAD -- src/components/ui` shows what was lost.
- **TypeScript**: Use `u32` for timestamps and `snake_case` for properties that bind to Rust structs.
- **Rust**: Use `snake_case` for all variables and functions.
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
│   └── settings/    # Settings dialogs
├── hooks/           # React hooks (useX.ts)
├── stores/          # zustand stores; settings/ holds persisted, cross-window settings
├── lib/             # Utilities and generated bindings.ts
├── pages/           # Route components (index, conflicting-files, missing-commits)
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
├── patronus/        # Patronus REST client, Space token of the Safe Push CLI, request policy
├── github/          # GitHub REST and GraphQL client (rate limits, SSO, conditional GETs), fake GitHub for tests
├── test-server/     # E2E mock server
└── ...
```

## How Branch Deck Virtual Branches Work
### Core Concept
Branch Deck automatically groups commits by prefix (e.g., `(feature-auth)`) and creates virtual branches by cherry-picking these grouped commits onto a new base.

### The Sync Process
1.  **Commit Grouping**: `CommitGrouper` reads commits and groups them by prefix.
2.  **Virtual Branch Creation**: For each group, it creates a branch `{user}/virtual/{prefix}` by cherry-picking commits.
3.  **Note Structure**: Git notes are created on the NEW virtual commits to track the ORIGINAL commit OID, for reverse tracking.

## Conflict Handling
- Uses `git merge-tree --write-tree` for conflict detection, never the working tree.
- Shows a 3-way merge view (base, target, cherry-pick).
- Conflicts occur when grouping by prefix changes the linear history of commits that depend on each other.

