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

# Testing
pnpm test                 # Run all tests (Rust and E2E)
pnpm test:backend         # Run backend tests with nextest (fast)
pnpm test:unit            # Run frontend unit tests (vitest, src/**/*.test.ts)
pnpm test-model-ai        # Run AI model tests (requires --release)

# E2E Testing (Playwright)
pnpm setup:e2e            # First-time setup: Install Playwright browsers
pnpm e2e                  # Run Playwright E2E tests (auto-installs browsers if needed)
pnpm e2e-up               # Run E2E tests and update snapshots
DEBUG_E2E=1 pnpm e2e      # Run E2E tests with verbose debug output
pnpm test-server:dev      # Run the E2E mock server with auto-restart (requires watchexec: `brew install watchexec`)

# Linting & Formatting
pnpm lint                 # Run all linting and formatting
pnpm lint:backend         # Format and lint Rust code (clippy --fix)
pnpm lint:frontend        # ESLint + tsc type-check of the React/TypeScript code
pnpm lint:cpd             # jscpd copy/paste detection (config in .jscpd.json); fails on any clone
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
- **zustand**: vanilla stores (`createStore`) read with `useStore` selectors; persisted settings sync across windows.
- **shadcn/ui on Base UI**: components generated into `src/components/ui/` (`components.json`, style `base-rhea`).
- **Font**: InterVariable from `inter-ui` (the full rsms build; `@fontsource-variable/inter` lacks `ss02`/`cv*` features),
  falling back to the system UI font. Commit bodies enable `ss04` (I/l/1 disambiguation).
- **Tailwind CSS v4**: via `@tailwindcss/vite`; theme tokens in `src/index.css`.
- **lucide-react**: icon components.
- **TypeScript 6**: strict mode, checked with `tsc`.
- **Tauri v2**: desktop application shell.
- **Nextest**: test runner for all Rust tests.
- **Playwright**: End-to-End (E2E) tests.

## Recent Features
- **Integration Status**: Auto-detection and archiving of merged, rebased, or squashed branches.
- **AI Branch Naming**: On-device ML model to generate branch names from unassigned commits.
- **Context Menus**: Right-click operations (shadcn `ContextMenu`) for common actions on branches and commits.

## Development Rules

### Git Operations
Hard rules; rationale and per-operation flows in [Git Operations](docs/design/git-operations.md).
- **Git 2.54+** (`MIN_VERSION` in `crates/git-executor/src/git_info.rs`), checked when git is discovered, on every platform.
- All git runs through `GitCommandExecutor`. It adds `-c core.quotePath=false -c color.ui=never -c log.showSignature=false`
  and strips inherited `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/`GIT_NOTES_REF`; callers don't add `--no-pager`.
- **Never write to the working tree**: no `checkout`, `switch`, `reset`, `rebase`, `stash`, `commit`, `cherry-pick`,
  `merge`, `restore`, `clean`, `apply` without `--cached`, `git history`; `crates/git-executor/src/worktree_safety_test.rs`
  enforces it. Reading worktree files into a temporary `GIT_INDEX_FILE` is fine.
- **The user's index** changes only where HEAD's tree changed (`IndexUpdate` in `crates/git-ops/src/rewrite_guard.rs`):
  a drop runs `read-tree -m -i <old> <new>` after a preflight on a copy of the index, an amend stages the amended files.
  Staged work on other paths survives; never `reset --mixed`, never ignore an index error.
- **Ref moves**: one `RefTransaction` (`update-ref --stdin`) with old-value CAS and a `branch-deck: <op>` reflog
  message; related refs (unapply rename, temp refs) go into the same transaction.
- **Guards** (`check_rewrite`) before rewriting the checked-out branch: no rebase/merge/cherry-pick/revert in progress,
  HEAD not detached, no merges in the range (refuse, no rebase fallback).
- **Metadata**: message-only rewrites (reword, move) keep author and committer byte-for-byte (`commit_writer`:
  `cat-file --batch` + `fast-import`); content rewrites (amend, drop via `git replay`) keep the author, committer = current
  user. Signatures are stripped.
- **Batch**: one process per phase, not per item (`--stdin`, `cat-file --batch`, `fast-import` for commits and notes,
  `update-ref --stdin`). Never parse translated text: exit codes, `--porcelain`, `-z`.
- Handle special exit codes: `merge-tree` and `replay` exit with 1 on conflicts.

### Rust Import Rules
- **NO RE-EXPORTS**: Never use `pub use` statements except for internal module organization.
- **Explicit Imports**: All imports must use full module paths (e.g., `use git_executor::git_command_executor::GitCommandExecutor`).
- **Module Structure**: Make modules public with `pub mod` instead of re-exporting their contents.
- **Clarity**: Every import should clearly show where types and functions originate from.

### Code Style
- **Colors**: shadcn semantic tokens only (`text-muted-foreground`, `bg-background`, `bg-muted`, `border-border`, `text-primary`,
  `text-destructive`) plus the custom `success`, `warning`, `info` (`text-success`, `bg-warning/10`). **Never** use `dark:`
  variants; the theme is handled by the tokens.
- **Window and cards** (Islands layering, see [UI Theme](docs/design/ui-theme.md)):
  - the page canvas is `bg-muted`;
  - top-level blocks (header, tables, collapsible sections, alerts) are stock `Card` islands, or `CollapsibleCard`;
  - never put `bg-muted`/`bg-background` divs on the canvas, and never recolour a top-level `Card` with `bg-muted`;
  - nested panels inside a card use `rounded-lg` (`rounded-xl` at most).
- **Radii** come only from `--radius` (`0.25rem`) in `src/index.css`. Never add radius classes to `src/components/ui/*`
  or hardcode `rounded-[…]`. To change the feel, change the token. Prefer a coherent token-driven scale over copying
  IntelliJ pixel values.
- **Spacing**: `flex flex-col gap-*` on the parent, not `space-y-*` or child `mt-*`.
- **Icons**: lucide-react components (`<GitBranchIcon />`). Inside `Button`, add `data-icon="inline-start"` (or `inline-end`)
  and no size classes; the button sizes them. Elsewhere use `size-*` classes (e.g. `size-3`).
- **Components**: `Tooltip` for simple text tooltips, `Popover` for rich content, `Kbd` for keyboard shortcuts,
  `toast.add({ type, title, description })` from `@/components/ui/toast` for notifications.
- **Base UI**: compose with the `render` prop (`<TooltipTrigger render={<Button />} />`), not `asChild`. Base UI emits
  `data-open`/`data-closed`; set `data-state` manually where E2E tests read it.
- **shadcn workflow**: run the CLI with `npx shadcn@latest …` (`add <component>`, `docs`, `init --preset`), then adapt
  the files. `pnpm dlx shadcn@latest` crashes with `o.deepPartial is not a function`: `@shadcn/registry` uses the zod 3
  API and pnpm dlx resolves zod 4.
- **Locally customised ui files**: `alert`, `badge`, `button`, `popover`, `tooltip`, `toast` in `src/components/ui/` carry
  a `// local customisation: …` header naming the edits. After `add --overwrite` (e.g. a style switch), re-apply them;
  `git diff HEAD -- src/components/ui` shows what was lost.
- **TypeScript**: Use `u32` for timestamps and `snake_case` for properties that bind to Rust structs.
- **Rust**: Use `snake_case` for all variables and functions.
- **Imports**: Always use explicit, full module paths. Never use `pub use` re-exports. Examples:
  - ✅ `use git_executor::git_command_executor::GitCommandExecutor`
  - ✅ `use sync_types::issue_navigation::IssueNavigationConfig`
  - ❌ `pub use config::ModelConfig` (in lib.rs)

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
- **Frontend**: Select narrow slices from zustand stores and `memo` list rows. Debounce user inputs where appropriate (e.g., using `p-debounce`).
- **Backend**: One git process per operation phase, never one per commit, note or file (see [Git Operations](docs/design/git-operations.md)).
- **AI/ML**: Always build with `--release` for ML inference (it's 8-10x faster). In dev mode, only ML dependencies are optimized to keep rebuilds fast.

### Debugging & Troubleshooting
- **Slow ML inference**: Ensure you are running in `--release` mode.
- **E2E Test Issues**: Run with `DEBUG_E2E=1` for verbose logging from the mock server.
- **Playwright browser errors**: If tests fail with "Executable doesn't exist", run `pnpm setup:e2e` to install browsers. This happens automatically when running `pnpm e2e`, but may be needed after updating `@playwright/test`.
- **Type Errors**: Run `pnpm lint:typecheck` to isolate and verify TypeScript issues.

## Error Handling Patterns
- **Frontend**: Use toast notifications for user-facing errors (`notifyError` in `src/lib/errors.ts`). Log detailed errors to the Tauri log plugin.
- **Backend (Rust)**: Use the `tracing` crate for structured logging. Use `#[instrument]` to automatically trace function entry/exit and arguments. Return proper `Result` types, not strings.

## Architecture

### Design Documents
- [Async Patterns and spawn_blocking](docs/design/async-patterns.md) - Explains async/sync boundaries and proper use of spawn_blocking
- [Distributed Settings Architecture](docs/design/distributed-settings.md) - Settings synchronization across windows
- [E2E Testing Architecture](docs/design/e2e-testing.md) - E2E test infrastructure with mock Tauri API
- [Git Operations](docs/design/git-operations.md) - Working-tree safety, exact-metadata rewrites, `git replay`, batching and the Git 2.54 minimum
- [Integration Detection and Caching](docs/design/integration-detection.md) - How branch integration detection works with git notes caching
- [Remote Status](docs/design/remote-status.md) - How remote tracking status is computed and displayed
- [UI Theme](docs/design/ui-theme.md) - Islands layering, the radius scale and the rules for UI changes
- [Virtual Branches](docs/design/virtual-branches.md) - How commits are grouped and cherry-picked into virtual branches
- [Import Guidelines](docs/guidelines/imports.md) - No re-exports policy and proper import patterns
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
│   ├── ai/          # AI suggestions and model download
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
├── model-ai/        # AI model inference & domain logic
├── model-core/      # Core AI types (e.g. candle, tokenizers)
├── model-tauri/     # Tauri integration for AI models
├── model-types/     # Serializable AI command types (no ML deps; used by test-server)
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

