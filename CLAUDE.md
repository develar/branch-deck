# Branch Deck Development Context

## Project Overview
Desktop Git branch management tool using Nuxt 4 + Tauri v2. Use `pnpm` only.

## Quick Commands
```bash
pnpm install         # Install dependencies
pnpm lint            # Lint all code (frontend + backend)
pnpm lint:frontend   # Lint TypeScript/Vue only (faster)
pnpm run tauri dev   # Run Tauri app
pnpm test            # Run all tests (Rust)
pnpm test:rust       # Run Rust tests with nextest
pnpm e2e             # Run E2E tests

# TypeScript bindings regenerate automatically with `pnpm run tauri dev`
```

## Development Rules

### Git Operations
- All git operations use CLI via `GitCommandExecutor`
- Always use `--no-pager` flag
- Handle special exit codes (merge-tree exit 1 = conflicts)

### Code Style
- **UI**: Semantic colors only (`text-muted`, `bg-default`), never `dark:` classes
- **Icons**: Lucide only (`i-lucide-*`), semantic sizes (`size-3`)
- **TypeScript**: u32 for timestamps, snake_case for Rust bindings
- **Spacing**: Parent `space-y-*` instead of child `mt-*`
- **Components**: `UTooltip` for text, `UPopover` for rich content, `UKbd` for shortcuts

### Button Styling Guidelines
- **Colors**:
  - `primary`: Main actions (e.g., Sync button when active)
  - `neutral`: Secondary actions and utility buttons (default for most buttons)
  - `error`: Error states and destructive actions
  - `warning`/`info`: Contextual states
- **Variants**:
  - `ghost`: Utility buttons with minimal styling (e.g., copy, expand/collapse)
  - `outline`: Secondary actions with borders
  - `solid`: Primary actions with filled backgrounds
  - `soft`: Subtle background for suggestions and badges
- **Sizes**: Use `xs` for table/compact layouts, `sm` for standard buttons
- **Copy Buttons**: Always `color="neutral"`, `variant="ghost"`, `size="xs"`
- **Modal Buttons**: Primary action (default), Cancel (`variant="ghost"`)
- **Icon Buttons**: Always include tooltips for accessibility

### Testing
- Run `pnpm lint` before commits
- Rust tests use **nextest** runner with automatic retries
- Use `TestRepo` framework in Rust tests
- Use `pretty_assertions` for better test failure diffs
- E2E tests use Playwright with custom test fixtures

## AI Model Integration

**Default Model**: Qwen2.5-Coder-0.5B (500MB) - Fast (2-3s) branch name generation

**Important**: Always build with `--release` for ML inference (8-10x faster)

### Development Optimization
- Only ML dependencies are optimized in dev mode (candle-*, tokenizers)
- All other code remains unoptimized for fast rebuilds
- This gives ~2-3x faster ML inference in development while keeping build times fast

### Model Architecture
Three-tier separation for clean architecture:
- **model-core**: Core ML inference engine (candle, tokenizers)
- **model-ai**: Domain logic, prompt engineering, generators
- **model-tauri**: Tauri integration layer (commands, progress, downloads)  








## Architecture

### Layer Structure
- **shared-ui**: Basic UI components and utilities
- **commit-ui**: Commit display and interaction components
- **conflict-ui**: Conflict visualization components
- **app**: Application-specific features

### State Management
- Use Pinia stores for global state
- Stores use Zod schemas for validation and defaults
- Persistence handled by Pinia plugin (auto-saves to Tauri store)
- Use reactive Maps for collections
- Compute arrays from Maps for reactivity

### Nuxt Auto-imports (IMPORTANT!)
- **ALL** composables, types, and utilities are auto-imported by Nuxt 4
- **NEVER** manually import: Vue APIs, Nuxt composables, project composables, or their types
- This includes: `ref`, `computed`, `watch`, `useRepository`, `RepositoryState`, etc.
- Auto-imports work in both components AND composables
- In Vue templates, refs are automatically unwrapped (no `.value` needed)

### Key Patterns
- Use `#[instrument]` for Rust tracing
- Batch git operations with `--stdin`
- Use `GitCommandExecutor` for all git operations
- When destructuring from composables, refs keep their reactive nature
- Always destructure only what you need: `const { pathValidation } = useRepository()`



## UI Patterns

### Key Components
- **Modals**: Use `useOverlay()` for programmatic modals
- **Toasts**: Use `toast.update()` for progress notifications
- **Forms**: Use `UForm` with validation schemas



### Button Conventions
- Modal buttons: `[Cancel] [Primary Action]` right-aligned
- Use `variant="ghost"` for cancel buttons
- Copy buttons: Use `color="neutral"` (not `text-muted`)



## Conflict Handling
- Uses `git merge-tree --write-tree` for detection
- Shows 3-way merge view with base, target, and cherry-pick
- Pre-fetches commit info for conflict markers

### How Branch Deck Conflicts Work
Branch Deck groups commits by branch prefix and cherry-picks them to virtual branches. Conflicts occur when this grouping changes the order of commits from their linear history.

**Example Linear History:**
```
1. (feature-auth) Add authentication
2. (feature-cache) Add caching  
3. (feature-auth) Integrate auth with cache (depends on commit 2)
4. (feature-cache) Optimize cache with auth (depends on commit 1)
```

**When Grouped:**
- feature-auth branch gets: commits 1, 3
- feature-cache branch gets: commits 2, 4

Commit 3 expects caching from commit 2 (missing in feature-auth) → **CONFLICT**

### Missing Commits
"Missing commits" are commits that:
- Exist in the source branch's history but NOT in the target branch
- Modified the same files that are now in conflict
- Can be assigned (with prefix) or unassigned (no prefix)

The UI shows "X commits behind target" to indicate how many commits the target is missing.

### Test Repository Requirements
A good test repo needs:
1. **Interleaved commits** with different prefixes modifying the same files
2. **Dependencies between commits** with different prefixes  
3. **Mix of assigned and unassigned commits**
4. **Clear order-dependent changes** that will conflict when reordered

## Window Management
- Use `openSubWindow()` utility from shared-ui layer
- Sub-windows use handshake pattern for data transfer
- Pages use `definePageMeta({ layout: false })`


## E2E Testing
- Tests use Playwright with Tauri `mockIPC` API
- Test server on port 3030 handles Tauri commands
- Use custom `base-test` fixture for automatic mocking
- **Debug E2E tests**: Use `DEBUG_E2E=1 pnpm e2e` to see Tauri mock debug logs

## E2E Test Architecture

### Test Isolation
Each test runs with its own isolated repository:
1. Test calls `setupRepo("template_name")` which creates repository via test server
2. Test server copies pre-created template to new isolated directory
3. Test navigates to `/?repoId=xxx` - this ID is used by tauri mock for isolation
4. All commands use this repository ID to maintain test isolation

### Template System
- Templates are pre-created when test server starts (in `tests/test-repos/`)
- Each test gets a fresh copy of the template in an isolated directory
- Available templates:
  - `simple`: Basic git repo with 2 commits
  - `unassigned`: Repository with commits lacking branch prefix
  - `conflict_*`: Various conflict scenarios
  - `NO_REPO`: Special - creates entry without actual directory
  - `empty-non-git`: Directory without git initialization

### How Commands Find Their Repository
Most Tauri commands receive `repository_path` in their request parameters. The test server:
1. Uses `path_to_id` map to find repository ID from path
2. Validates the repository exists in test state
3. Executes the command on that repository

### browse_repository Special Behavior
Unlike other commands, `browse_repository`:
- Does NOT receive a repository path (it's browsing for one)

### Test Configuration
Tests can configure mock behavior using `__BRANCH_DECK_TEST_CONFIG__`:
```javascript
await page.addInitScript(() => {
  (globalThis as any).__BRANCH_DECK_TEST_CONFIG__ = {
    browseRepository: {
      returnInvalid: true  // Will use template that fails validation
    }
  }
})
```

## Test Configuration
- **nextest**: Fast parallel test runner with automatic retries
- **pretty_assertions**: Colored diffs for test failures
- Config in `.config/nextest.toml` (minimal by design)
- Run `cargo nextest run` for better test output

## File Structure
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
├── auto_update.rs   # Auto-update functionality
├── menu.rs          # Application menu
├── progress.rs      # Progress reporting
├── telemetry.rs     # OpenTelemetry integration
crates/
├── git-ops/         # Git operations library
├── model-ai/        # AI model inference
├── model-core/      # Core AI types
├── model-tauri/     # Tauri integration for AI models
├── branch-sync/     # Branch sync logic
├── test-utils/      # Testing utilities
├── test-server/     # E2E test server
```





## TanStack Table
- Use commit hashes as row IDs for stable selection
- Let TanStack handle selection state
- Convert between v-model array and TanStack object format

## Key Patterns

- **State**: Pinia stores for global, reactive Maps for collections
- **UI**: Semantic colors, Lucide icons, parent spacing
- **Git**: CLI only with special exit code handling
- **Windows**: Handshake pattern for data transfer
- **Testing**: Run `pnpm lint` before commits
- **Types**: snake_case for Rust, u32 for timestamps