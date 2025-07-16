# Branch Deck Development Context

## Project Overview
Desktop Git branch management tool using Nuxt 4 + Tauri v2. Use `pnpm` only.

## Quick Commands
```bash
pnpm install         # Install dependencies
pnpm lint            # Lint all code
pnpm run tauri dev   # Run Tauri app
pnpm test:unit       # Run Vue tests
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

### Testing
- Run `pnpm lint` before commits
- Use `TestRepo` framework in Rust tests
- Vue tests use `@nuxt/test-utils` with `mountSuspended`

## AI Model Integration

**Default Model**: Qwen2.5-Coder-0.5B (500MB) - Fast (2-3s) branch name generation

**Important**: Always build with `--release` for ML inference (8-10x faster)  








## Architecture

### Layer Structure
- **shared-ui**: Basic UI components and utilities
- **commit-ui**: Commit display and interaction components
- **conflict-ui**: Conflict visualization components
- **app**: Application-specific features

### State Management
- Use Pinia stores for global state
- Use reactive Maps for collections
- Compute arrays from Maps for reactivity

### Key Patterns
- Use `#[instrument]` for Rust tracing
- Batch git operations with `--stdin`
- Use `GitCommandExecutor` for all git operations



## UI Patterns

### Key Components
- **Modals**: Use `useOverlay()` for programmatic modals
- **Toasts**: Use `toast.update()` for progress notifications
- **Forms**: Use `UForm` with validation schemas



### Button Conventions
- Modal buttons: `[Cancel] [Primary Action]` right-aligned
- Use `variant="ghost"` for cancel buttons



## Conflict Handling
- Uses `git merge-tree --write-tree` for detection
- Shows 3-way merge view with base, target, and cherry-pick
- Pre-fetches commit info for conflict markers

## Window Management
- Use `openSubWindow()` utility from shared-ui layer
- Sub-windows use handshake pattern for data transfer
- Pages use `definePageMeta({ layout: false })`


## E2E Testing
- Tests use Playwright with Tauri `mockIPC` API
- Test server on port 3030 handles Tauri commands
- Use custom `base-test` fixture for automatic mocking

## File Structure
```
layers/
├── shared-ui/       # Basic UI components and utilities
├── commit-ui/       # Commit display components
├── conflict-ui/     # Conflict visualization
app/
├── components/      # App-specific components
├── stores/          # Pinia stores
├── pages/           # Nuxt pages
src-tauri/
├── commands/        # Tauri commands
├── git/             # Git operations
├── model/           # AI integration
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