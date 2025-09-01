# Branch Deck Development Context

## Project Overview
Desktop Git branch management tool using Nuxt 4 + Tauri v2. Use `pnpm` only.

## Quick Commands
See `package.json` for the full list of scripts.

```bash
# Core Development
pnpm install              # Install dependencies
pnpm run tauri dev        # Run the full Tauri app (frontend + backend)
pnpm dev                  # Run only the Nuxt frontend

# Testing
pnpm test                 # Run all tests (Rust and E2E)
pnpm test:backend         # Run backend tests with nextest (fast)
pnpm test-model-ai        # Run AI model tests (requires --release)
pnpm e2e                  # Run Playwright E2E tests
DEBUG_E2E=1 pnpm e2e      # Run E2E tests with verbose debug output

# Linting & Formatting
pnpm lint                 # Run all linting and formatting
pnpm lint:backend         # Format and lint Rust code (clippy --fix)
pnpm lint:frontend        # Lint and type-check TypeScript/Vue
```

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
- **Nuxt 4**: With auto-imports and layers architecture.
- **Tauri v2**: For the desktop application shell.
- **Vue 3**: Using the Composition API and `<script setup>`.
- **TypeScript 5**: With strict mode enabled.
- **Tailwind CSS v4**: Using the new, faster Rust-based engine.
- **Vite**: As the build tool for the frontend.
- **Nextest**: As the test runner for all Rust tests.
- **Playwright**: For End-to-End (E2E) testing.

## Recent Features
- **Integration Status**: Auto-detection and archiving of merged, rebased, or squashed branches.
- **AI Branch Naming**: On-device ML model to generate branch names from unassigned commits.
- **Context Menus**: Right-click operations (`UContextMenu`) for common actions on branches and commits.

## Development Rules

### Git Operations
- All git operations use the CLI via the `GitCommandExecutor` in Rust.
- Always use the `--no-pager` flag for non-interactive commands.
- Handle special exit codes (e.g., `git merge-tree` exiting with 1 indicates conflicts).

### Rust Import Rules
- **NO RE-EXPORTS**: Never use `pub use` statements except for internal module organization.
- **Explicit Imports**: All imports must use full module paths (e.g., `use git_executor::git_command_executor::GitCommandExecutor`).
- **Module Structure**: Make modules public with `pub mod` instead of re-exporting their contents.
- **Clarity**: Every import should clearly show where types and functions originate from.

### Code Style
- **UI**: Use semantic colors only (e.g., `text-muted`, `bg-default`). **Never** use `dark:` variants; the theme is handled automatically.
- **Icons**: Use Lucide icons only (`i-lucide-*`) with semantic sizes (e.g., `size-3`).
- **Spacing**: Prefer parent `space-y-*` for vertical spacing over individual child `mt-*`.
- **Components**: Use `UTooltip` for simple text tooltips and `UPopover` for rich content. Use `UKbd` for keyboard shortcuts.
- **TypeScript**: Use `u32` for timestamps and `snake_case` for properties that bind to Rust structs.
- **Rust**: Use `snake_case` for all variables and functions.
- **Imports**: Always use explicit, full module paths. Never use `pub use` re-exports. Examples:
  - ✅ `use git_executor::git_command_executor::GitCommandExecutor`
  - ✅ `use sync_types::issue_navigation::IssueNavigationConfig`
  - ❌ `pub use config::ModelConfig` (in lib.rs)

### Naming Conventions
- **Components**: PascalCase (e.g., `BranchTableCard.vue`).
- **Composables**: camelCase with `use` prefix (e.g., `useRepository.ts`).
- **Stores**: camelCase (e.g., `repositoryStore.ts`).
- **Rust modules**: snake_case (e.g., `git_ops.rs`).

### Nuxt Auto-imports (IMPORTANT!)
- **ALL** composables, types, and utilities are auto-imported by Nuxt 4.
- **NEVER** manually import Vue APIs (`ref`, `computed`), Nuxt composables, or project utilities/types.
- In Vue templates, refs are automatically unwrapped (no `.value` needed).

## Performance & Debugging

### Performance Guidelines
- **Frontend**: Use virtual scrolling for large lists. Debounce user inputs where appropriate (e.g., using `p-debounce`).
- **Backend**: Batch Git operations using `--stdin` where possible. Always use `--no-pager`.
- **AI/ML**: Always build with `--release` for ML inference (it's 8-10x faster). In dev mode, only ML dependencies are optimized to keep rebuilds fast.

### Debugging & Troubleshooting
- **Slow ML inference**: Ensure you are running in `--release` mode.
- **E2E Test Issues**: Run with `DEBUG_E2E=1` for verbose logging from the mock server.
- **Type Errors**: Run `pnpm lint:typecheck` to isolate and verify TypeScript issues.

## Error Handling Patterns
- **Frontend**: Use toast notifications for user-facing errors. Log detailed errors to the Tauri log plugin.
- **Backend (Rust)**: Use the `tracing` crate for structured logging. Use `#[instrument]` to automatically trace function entry/exit and arguments. Return proper `Result` types, not strings.

## Architecture

### Design Documents
- [Async Patterns and spawn_blocking](docs/architecture/async-patterns.md) - Explains async/sync boundaries and proper use of spawn_blocking
- [Distributed Settings Architecture](docs/architecture/distributed-settings.md) - Settings synchronization across windows
- [E2E Testing Architecture](docs/architecture/e2e-testing.md) - E2E test infrastructure with mock Tauri API
- [Integration Detection and Caching](docs/architecture/integration-detection-caching.md) - How branch integration detection works with git notes caching
- [Import Guidelines](docs/architecture/import-guidelines.md) - No re-exports policy and proper import patterns

### Layer Structure
- **shared-ui**: Basic UI components and utilities
- **commit-ui**: Commit display and interaction components
- **conflict-ui**: Conflict visualization components
- **app**: Application-specific features

### File Structure
```
layers/
├── shared-ui/       # Basic UI components and utilities
├── commit-ui/       # Commit display components
├── conflict-ui/     # Conflict visualization
├── ai/              # AI features and model integration
app/
├── components/      # App-specific components
├── stores/          # Pinia stores with Zod schemas
├── pages/           # Nuxt pages
├── plugins/         # Nuxt plugins (includes Pinia persistence)
├── utils/           # App utilities (appStore, etc.)
src-tauri/
├── commands/        # Tauri commands
├── ...
crates/
├── git-ops/         # Git operations library
├── model-ai/        # AI model inference & domain logic
├── model-core/      # Core AI types (e.g. candle, tokenizers)
├── model-tauri/     # Tauri integration for AI models
├── branch-sync/     # Branch sync logic
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
- Uses `git merge-tree --write-tree` for conflict detection.
- Shows a 3-way merge view (base, target, cherry-pick).
- Conflicts occur when grouping by prefix changes the linear history of commits that depend on each other.

