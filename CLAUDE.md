# Branch Deck Development Context

## Project Overview
Desktop Git branch management tool:
- **Frontend**: Nuxt 4 (Vue 3) + Nuxt UI v3 + UI Pro
- **Backend**: Rust + Tauri v2  
- **Purpose**: Sync virtual branches from commits, display status, push changes
- **Package Manager**: pnpm only (never npm/yarn)

## Quick Commands
```bash
pnpm install         # Install dependencies
pnpm run dev         # Start dev server
pnpm lint            # Lint Rust + JS/TS + TypeScript
pnpm run tauri dev   # Run Tauri app
cargo check          # Check Rust code (from src-tauri/)
pnpm test            # Run Rust tests
pnpm test:unit       # Run Vue component unit tests
```

## Development Rules

### Git Operations
- **CLI Only**: All git operations use git CLI via `GitCommandExecutor` (git2 dependency completely removed)
- Always use `--no-pager` flag to prevent hanging
- Handle special exit codes (e.g., merge-tree exit 1 = conflicts, not error)
- Batch operations: Use `--stdin` for bulk updates (notes, refs)
- Performance: Git CLI is more efficient than libgit2 for most operations

### Code Style
- **Colors**: Use semantic colors (`text-muted`, `bg-default`, `border-default`)
- **Icons**: Use Lucide icons (`i-lucide-upload`) never heroicons, use semantic sizes (`size-3`, `size-4`, `size-5`) not `w-* h-*`
- **Rust Bindings**: Use snake_case (`commit.original_hash`)
- **Maps**: Use `.size` not `.length`, iterate with `.values()`
- **Badges**: Use `size="sm"` or default (never `size="xs"`)
- **TypeScript**: Use u32 for timestamps (JS compatible), avoid i64
- **Tracing**: Use `#[instrument]` instead of manual debug!/info! calls
- **ESLint**: Enforces double quotes, no semicolons, 2-space indentation, no forced newline at EOF

### Testing & Linting
- Run `pnpm lint` before commits
- Run tests after changes
- Use `cargo check` from src-tauri/ for Rust type checking
- **Always use TestRepo** framework in Rust tests (uses git CLI internally)
- **Vue Component Tests**: Use `pnpm test:unit` with Nuxt vitest setup
- **Test Setup**: Tests use `@nuxt/test-utils` with `mountSuspended` for component testing

## Architecture Patterns

### State Management
```typescript
// ✅ Maps for reactivity
const branches = reactive(new Map<string, Branch>())
const branchArray = computed(() => Array.from(branches.values()))

// ❌ Don't break reactivity
const branchArray = Array.from(branches.values())
```

### Rust Patterns
```rust
// Git CLI executor pattern
#[derive(Clone)]
pub struct GitCommandExecutor {
  info: Arc<Mutex<Option<GitInfo>>>,
}

// Logging with #[instrument] - ALWAYS prefer this
#[instrument(skip(git_executor, large_data), fields(branch = %branch_name))]
fn process_data(git_executor: &GitCommandExecutor, branch_name: &str, large_data: &[u8]) -> Result<()> {
    // Tracing context automatically included
    Ok(())
}

// Batch git operations
let args = vec!["update-ref", "--stdin"];
git_executor.execute_command_with_input(&args, repo_path, &batch_input)?;
```

### AppStore Architecture
```typescript
// Centralized settings store
import { appStore, appStoreKey } from "~/utils/app-store"

// Provide in app.vue
provide(appStoreKey, appStore)

// Use in components
const appStore = inject(appStoreKey)
if (!appStore) throw new Error("AppStore not provided")

// Auto-persisted settings
const settings = await appStore.getConflictViewerSettings()
await appStore.updateAppSetting("primaryColor", "blue")
```

## UI Component Patterns

### Common Components
```vue
<!-- Error Display -->
<UAlert color="error" variant="soft" :title="error.title">
  <template #description>{{ error.message }}</template>
</UAlert>

<!-- Loading States -->
<UProgress size="xs" indeterminate />

<!-- Text Metadata -->
<div class="text-xs text-muted">
  <span>{{ commit.hash.substring(0, 7) }}</span>
  <span>•</span>
  <span>{{ formatTime(commit.timestamp) }}</span>
</div>
```

### Tabs for View Switching
```vue
<UTabs 
  v-model="selectedTab" 
  :items="[
    { value: "diff", slot: "diff", label: "Diff", icon: "i-lucide-align-left" },
    { value: "3way", slot: "3way", label: "3-way", icon: "i-lucide-columns-3" }
  ]"
  variant="link"
  color="neutral"
>
  <template #diff><!-- Content --></template>
  <template #3way><!-- Content --></template>
</UTabs>
```

### Responsive Grids
```vue
<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
```

## Conflict Handling

### Data Structure
```rust
pub struct MergeConflictInfo {
  pub conflicting_files: Vec<ConflictDetail>,
  pub conflict_marker_commits: HashMap<String, ConflictMarkerCommitInfo>, // Shared
}

pub struct ConflictDetail {
  pub file: String,
  pub file_diff: FileDiff,  // Contains conflict markers
  pub base_file: Option<FileInfo>,      // Common ancestor
  pub target_file: Option<FileInfo>,    // Target branch
  pub cherry_file: Option<FileInfo>,    // Cherry-pick
  pub base_to_target_diff: FileDiff,    // Base → Target diff
  pub base_to_cherry_diff: FileDiff,    // Base → Cherry diff
}
```

### Merge Base Calculation
```rust
// ✅ Find actual divergence point
let merge_base_id = find_merge_base(
  git_executor, 
  repo_path, 
  parent_commit_id,    // Parent of cherry-pick  
  target_commit_id     // Current HEAD of target
);
```

### Git Integration
- Uses `git merge-tree --write-tree` for conflict detection
- Parses NUL-separated output for conflict files
- Pre-fetches commit info for all conflict markers

## Tauri Window Management

### Creating Sub-Windows
```typescript
import { WebviewWindow } from "@tauri-apps/api/webviewWindow"
import { emit } from "@tauri-apps/api/event"

async function openSubWindow() {
  const data = { /* prepare data */ }
  
  const subWindow = new WebviewWindow("window-id", {
    url: "/page-name",
    title: `Title - ${contextInfo}`,
    width: 1400,
    height: 900,
    center: true,
    skipTaskbar: true,
  })
  
  // Handshake pattern
  subWindow.once("window-ready", async () => {
    await emit("init-window-data", data)
  })
}
```

### Sub-Window Page Setup
```vue
<script setup>
definePageMeta({ layout: false })

onMounted(async () => {
  // Check sessionStorage for hot reload
  const stored = sessionStorage.getItem("window-data")
  if (stored) {
    windowData.value = JSON.parse(stored)
    return
  }

  // Listen for data
  const unlisten = await listen("init-window-data", (event) => {
    windowData.value = event.payload
    sessionStorage.setItem("window-data", JSON.stringify(event.payload))
    unlisten()
  })
  
  await emit("window-ready", {})
})
</script>
```

### Required Permissions
```json
// src-tauri/capabilities/default.json
"permissions": ["core:webview:allow-create-webview-window"]

// src-tauri/capabilities/window-name.json
{
  "identifier": "window-name",
  "windows": ["window-name"],
  "permissions": ["core:event:default"]
}
```

## Application Menus

### Menu Events (src-tauri/src/lib.rs)
```rust
fn handle_menu_event(app: &tauri::AppHandle, event: MenuEvent) {
  match event.id().as_ref() {
    "github_link" => {
      tauri_plugin_opener::open_url(url, None::<String>).ok();
    },
    "color_selector" => {
      app.emit("open_color_selector", ()).ok();
    },
    _ => {}
  }
}
```

### Frontend Menu Handlers
```typescript
onMounted(async () => {
  unlisten = await listen("open_color_selector", () => {
    isOpen.value = true
  })
})
```

## File Structure
```
app/
├── components/         # Vue components
├── composables/        # Reactive logic  
├── utils/bindings.ts   # Auto-generated Rust types
src-tauri/src/
├── commands/           # Tauri commands
├── git/               # Git operations
```

## Conflict Viewer UI Architecture

### Main Components
```
MergeConflictViewer.vue          # Primary conflict display component
├── ConflictExplanationAlert.vue # Context-aware card description
├── MissingCommitsDetails.vue    # Missing commits analysis
└── ConflictingFilesSection.vue  # Tab-based file diff viewer
    ├── FileDiffList.vue         # Diff view with conflict markers
    └── GitDiffMergeView.vue     # 3-way merge view
```

### Key UI Components

#### **CommitList.vue**
- Unified commit list component replacing multiple implementations
- Supports three variants: `compact`, `detailed`, `status`
- Accepts `CommitDetail[]`, `Map<string, CommitDetail>`, `GenericCommit[]`, or `MissingCommit[]`
- Features configurable via props: `showAuthor`, `showFileCount`, `showDividers`, `showHover`
- Provides `after-commit` slot for custom content after each commit
- Type-safe with runtime type guards for union types
- Example usage:
```vue
<CommitList
  :commits="missingCommits"
  variant="detailed"
  :show-file-count="true"
  container-class="space-y-4"
  item-class="pb-4 border-b border-default last:border-0 last:pb-0"
>
  <template #after-commit="{ commit }">
    <!-- Custom content -->
  </template>
</CommitList>
```

#### **MergeConflictViewer.vue**
- Main conflict display with commit overview
- Missing commits insight card with divergence analysis
- Auto-expansion for branches with conflicts
- Sub-window integration for detailed views

#### **ConflictingFilesSection.vue**
- Tab interface: "Diff View" vs "3-way Merge"
- Controls: "Conflicts only" toggle + Unified/Split modes
- Settings persistence via `useConflictViewerSettings`
- Proper spacing (`mt-6`) between tabs and content

#### **ConflictExplanationAlert.vue**
- Context-aware explanation focusing on missing commits
- Branch Deck-specific messaging about prefix-based copying
- Card-style design instead of generic alert

### Conflict Data Flow
```
Rust Backend (MergeConflictInfo) → TypeScript Bindings → Vue Components
```

### Component Communication Patterns
```typescript
// Shared conflict data
const conflictMarkerCommits = computed(() => {
  // Convert Partial to Record, filter undefined values
  const commits = props.conflict.conflictMarkerCommits
  const result: Record<string, CommitInfo> = {}
  for (const [key, value] of Object.entries(commits)) {
    if (value) result[key] = value
  }
  return result
})

// Settings composable pattern
const { showConflictsOnly, viewMode, conflictDiffViewMode } = useConflictViewerSettings()
```

### Sub-Window Integration
```typescript
// Open conflict details in separate window
async function openConflictingFilesWindow() {
  const data = {
    conflict: props.conflict,
    branchName: props.branchName,
    viewSettings: { /* current settings */ }
  }
  
  await openSubWindow({
    windowId: "conflicting-files",
    url: "/conflicting-files",
    title: `Files Modified by Missing Commits - ${branchName}`,
    data
  })
}
```

### UI Messaging Strategy
- **Focus on Root Cause**: "Missing commits cause conflicts" not "files have conflicts"
- **Context-Aware**: Explain Branch Deck's prefix-based commit copying workflow
- **Actionable**: Point users to apply missing commits to resolve conflicts
- **Educational**: Include tooltips about 3-way merge concepts

### Visual Design Patterns
- **Semantic Icons**: `i-lucide-git-branch` for Branch Deck context, `i-lucide-file-x` for modified files
- **Color Coding**: Warning colors for conflicts, primary for Branch Deck branding
- **Spacing**: Consistent `space-y-4` between cards, `mt-6` between tabs and content
- **Typography**: `text-highlighted` for headers, `text-toned` for descriptions, `text-muted` for metadata

### Pages for Sub-Windows
- **conflicting-files.vue**: Full-page conflict file viewer
- **missing-commits.vue**: Detailed missing commits analysis
- Both use `definePageMeta({ layout: false })` for clean sub-window experience

### Real-Time Updates
- Components reactively update as sync operations progress
- Auto-expansion of branches with conflicts via `expanded[branchName] = true`
- Persistent settings across sessions and window reloads

## Key Development Patterns

1. **Reactivity**: Keep Maps reactive, use computed for derived state
2. **State Persistence**: Use AppStore for settings that need to persist
3. **Error Handling**: Auto-expand UI elements on errors/conflicts
4. **Performance**: Return shared data separately, not duplicated per item
5. **Type Safety**: Use snake_case for Rust bindings, u32 for timestamps
6. **Testing**: Always run `pnpm lint` before commits
7. **Git Operations**: Handle special exit codes, use --no-pager flag
8. **UI Consistency**: Use semantic colors, Lucide icons, proper badge sizing
9. **Conflict UX**: Focus on missing commits as root cause, not file conflicts
10. **Sub-Windows**: Use handshake pattern for reliable data transfer