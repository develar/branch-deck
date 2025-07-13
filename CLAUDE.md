# Branch Deck Development Context

## Bash commands
- `pnpm install`: Install dependencies
- `pnpm run dev`: Start dev server
- `pnpm lint`: Run linting (Rust + JS/TS)
- `pnpm run tauri dev`: Run Tauri app in development
- `cargo check`: Check Rust code (run from src-tauri/)
- `git --no-pager log`: Use --no-pager to prevent hanging

## Project overview
Desktop Git branch management tool
- Frontend: Nuxt 4 (Vue 3) + Nuxt UI v3 + UI Pro
- Backend: Rust + Tauri
- Purpose: Sync virtual branches from commits, display status, push changes
- Package manager: pnpm only (never npm/yarn)

## Code style
- Use semantic colors: `text-muted`, `bg-default`, `border-default` (auto dark mode)
- Use Lucide icons: `i-lucide-upload`, `i-lucide-x-circle` (never heroicons)
- Use snake_case for Rust bindings: `commit.original_hash` (not camelCase)
- Use Maps: `.size` not `.length`, iterate with `.values()`
- Badge sizing: use `size="sm"` or default (never `size="xs"`)

## Testing
- Run `pnpm lint` before commits (lints Rust + JS/TS)
- Use `cargo check` from src-tauri/ for Rust type checking
- Typecheck after code changes

## Repository etiquette
- Package manager: pnpm only (never npm/yarn)
- Git commands: use `--no-pager` flag to prevent hanging
- Handle special git exit codes (merge-tree exit 1 = conflicts)
- Auto-update feature: Commands always available in bindings.ts regardless of feature flags

## Reactivity & State Management

### Maps vs Arrays
```typescript
// ✅ Keep as Map for reactivity
const branches = reactive(new Map<string, Branch>())

// ❌ Breaks reactivity  
const branchArray = Array.from(branches.values())

// ✅ Use computed if needed
const branchArray = computed(() => Array.from(branches.values()))
```

### Template Usage
```vue
<!-- ✅ Iterate Map values -->
<div v-for="branch in branches.values()" :key="branch.name">

<!-- ✅ Check Map size -->
<div v-if="branches.size > 0">

<!-- ❌ Wrong - Maps don't have length -->
<div v-if="branches.length > 0">
```

### UTable with Expansion
```vue
<UTable
  v-model:expanded="expanded"
  :data="branches"
  :row-key="'name'"
>
  <template #expanded="{ row }">
    <!-- Content -->
  </template>
</UTable>

<script setup>
// ✅ Match row key type
const expanded = ref<Record<string, boolean>>({})

// ✅ Auto-expand on errors
branchData.expanded[branchName] = true
</script>
```

## Common Patterns

### Error Display
```vue
<UAlert color="error" variant="soft" :title="error.title">
  <template #description>{{ error.message }}</template>
</UAlert>
```

### Loading States  
```vue
<UProgress size="xs" indeterminate />
```

### Text Metadata
```vue
<!-- Use text-xs for secondary info -->
<div class="text-xs text-muted">
  <span>{{ commit.hash.substring(0, 7) }}</span>
  <span>•</span>
  <span>{{ formatTime(commit.timestamp) }}</span>
</div>
```

### UI Pro Page Layout
```vue
<!-- Use UPage components for professional layout -->
<UPage>
  <UPageHeader>
    <template #headline>
      <UBadge variant="subtle" size="sm">Category</UBadge>
    </template>
    <template #title>Page Title</template>
    <template #description>Page description text</template>
    <template #links>
      <UButton icon="i-lucide-external-link" size="sm">Action</UButton>
    </template>
  </UPageHeader>
  
  <UPageBody>
    <!-- Main content -->
  </UPageBody>
</UPage>
```

## Rust Integration

### GitCommandExecutor
```rust
// ✅ Shared state pattern
#[derive(Clone)]
pub struct GitCommandExecutor {
  info: Arc<Mutex<Option<GitInfo>>>,
}

// Usage in commands
git_executor: State<'_, GitCommandExecutor>
let clone = (*git_executor).clone();
```

### Logging with #[instrument]
```rust
// ✅ Skip non-Debug params, remove redundant logs
#[instrument(skip(repo, large_data))]
fn process_data(repo: &Repository, id: &str, large_data: &[u8]) -> Result<()> {
    // Remove: debug!("Starting process") - instrument handles this
    debug!("Processing {} items", items.len()); // ✅ Meaningful context
    Ok(())
}
```

### Git Commands
```bash
# ✅ Use --no-pager to prevent hanging
git --no-pager log

# ✅ Handle special exit codes
git merge-tree # exit 1 = conflicts, not error
```

### Git Notes Optimization
```rust
// ✅ Read operations don't need mutex
find_existing_commit(repo, commit_id) // No mutex needed

// ✅ Write operations need mutex for thread safety
write_commit_notes(repo, notes, &git_notes_mutex) // Mutex required
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

## Key Data Types
```typescript
interface ReactiveBranch {
  name: string                       // Unique identifier  
  commits: Map<string, CommitDetail> // Fast lookup
  status: string                     // "pending", "syncing", "error"
  hasError: boolean                  // Auto-expand trigger
}

// Field access (snake_case from Rust)
commit.original_hash  // ✅
commit.originalHash   // ❌
```

## Quick Commands
```bash
pnpm install      # Install deps
pnpm run dev      # Start dev server  
pnpm lint         # Lint Rust + JS/TS
pnpm run tauri dev # Run Tauri app
```

## Application Menus

### Menu Structure (src-tauri/src/lib.rs)
- **Help**: GitHub Repository (opens https://github.com/develar/branch-deck)
- **View**: Primary Color... (opens color selector modal)
- Menu events handled in Rust, UI opened via events

### Menu Event Pattern
```rust
// Handle menu events in Rust
fn handle_menu_event(app: &tauri::AppHandle, event: MenuEvent) {
  match event.id().as_ref() {
    "github_link" => {
      // Open URL directly in Rust using tauri-plugin-opener
      if let Err(e) = tauri_plugin_opener::open_url(url, None::<String>) {
        tracing::error!("Failed to open GitHub link: {:?}", e);
      }
    },
    "color_selector" => {
      // Emit event for frontend to handle
      app.emit("open_color_selector", ()).ok();
    },
    _ => {}
  }
}
```

### Color Selector Implementation
```typescript
// Listen for menu events
onMounted(async () => {
  unlisten = await listen('open_color_selector', () => {
    isOpen.value = true
  })
})

// Update app colors via CSS custom properties
const updateAppColor = (color: string) => {
  document.documentElement.style.setProperty('--color-primary-500', `var(--color-${color}-500)`)
  // ... other color shades
}
```

### Menu Dependencies
```rust
// Add tauri-plugin-opener to Cargo.toml
tauri-plugin-opener = "2"

// Register plugin in lib.rs
.plugin(tauri_plugin_opener::init())
```

## AppStore Architecture

### Centralized Settings Store
```typescript
// ✅ Centralized store with provide/inject
import { appStore, appStoreKey } from '~/utils/app-store'

// In app.vue
provide(appStoreKey, appStore)

// In components/composables
const appStore = inject(appStoreKey)
if (!appStore) throw new Error('AppStore not provided')

// Settings are persisted automatically
const settings = await appStore.getConflictViewerSettings()
await appStore.updateAppSetting('primaryColor', 'blue')
```

### Composables for Shared State
```typescript
// ✅ Extract shared logic to composables
export function useConflictViewerSettings() {
  const appStore = inject(appStoreKey)
  if (!appStore) throw new Error('AppStore not provided')

  const showConflictsOnly = ref(true)
  const viewMode = ref('diff')
  const conflictDiffViewMode = ref<'unified' | 'split'>('unified')

  // Load from store on mount
  onMounted(async () => {
    const settings = await appStore.getConflictViewerSettings()
    // Apply settings...
  })

  // Auto-persist changes
  watch([showConflictsOnly, viewMode, conflictDiffViewMode], async () => {
    await appStore.setConflictViewerSettings({
      showConflictsOnly: showConflictsOnly.value,
      viewMode: viewMode.value,
      conflictDiffViewMode: conflictDiffViewMode.value
    })
  })

  return { showConflictsOnly, viewMode, conflictDiffViewMode }
}
```

### Props vs Store
```typescript
// ❌ Don't pass settings as props
const props = defineProps<{
  initialShowConflictsOnly?: boolean
  initialViewMode?: string
}>()

// ✅ Get settings from store-backed composable
const { showConflictsOnly, viewMode } = useConflictViewerSettings()
```

## Development Workflow
1. Use semantic colors for theming
2. Keep state reactive with Maps/computed
3. Run `pnpm lint` before commits
4. Use `#[instrument]` for Rust logging
5. Handle git special cases (exit codes, paging)
6. Auto-expand UI on errors/conflicts
7. Persist settings via AppStore
8. Extract common UI patterns to reusable components

## Conflict Handling Architecture

### Data Flow
```
Rust Backend → TypeScript Bindings → Vue Components
```

### Key Components
- **MergeConflictViewer.vue**: Main conflict UI, receives MergeConflictInfo
- **FileDiffList.vue**: Displays file diffs with conflict markers
- **ConflictMarkerExtension.vue**: Renders conflict marker annotations
- **CommitHashPopover.vue**: Shows commit details on hover

### Conflict Data Structure
```rust
// Conflict info stored at merge level (not per-file)
pub struct MergeConflictInfo {
  pub conflicting_files: Vec<ConflictDetail>,
  pub conflict_marker_commits: HashMap<String, ConflictMarkerCommitInfo>, // Shared
}

// Per-file conflict details
pub struct ConflictDetail {
  pub file: String,
  pub file_diff: FileDiff,  // Contains hunks with conflict markers
  // No duplicate commit info here
}
```

### Optimization Patterns
```rust
// ✅ Return shared data separately
fn extract_conflict_details() -> Result<(Vec<ConflictDetail>, HashMap<String, CommitInfo>)>

// ❌ Don't duplicate shared data in each item
fn extract_conflict_details() -> Result<Vec<ConflictDetailWithCommitInfo>>
```

### Conditional Compilation
```rust
// ✅ Handle feature flags properly
#[cfg(feature = "auto-update")]
use tracing::info;

// ❌ Avoid cycles fixing warnings
use tracing::info; // Unused when feature disabled
```

### TypeScript Type Compatibility
```rust
// ✅ Use u32 for timestamps (JS compatible)
pub timestamp: u32,

// ❌ Avoid i64 (not JS native)
pub timestamp: i64,
```

### Git Merge-Tree Integration
- Uses `git merge-tree --write-tree` for conflict detection
- Parses NUL-separated output for conflict files
- Extracts conflict markers from merge tree objects
- Pre-fetches commit info for all conflict markers

### Component Props Pattern
```vue
<!-- Parent passes shared data down -->
<FileDiffList 
  :file-diffs="conflictingFileDiffs"
  :conflict-marker-commits="conflictMarkerCommits"
/>

<!-- Child receives and uses shared data -->
const props = defineProps<{
  conflictMarkerCommits?: Record<string, CommitInfo>
}>()
```

## 3-Way Merge Conflict Resolution

### Correct Merge Base Calculation
```rust
// ✅ CORRECT: Find actual divergence point between branches
let merge_base_id = find_merge_base(
  git_executor, 
  repo_path, 
  parent_commit_id,    // Parent of cherry-pick commit  
  target_commit_id     // Current HEAD of target branch
);

// ❌ WRONG: Would typically just return parent (no divergence)
let merge_base_id = find_merge_base(
  git_executor,
  repo_path, 
  parent_commit_id,    // Parent of cherry-pick
  cherry_commit_id     // Cherry-pick commit (descendant of parent)
);
```

### 3-Way View Data Structure
```rust
pub struct ConflictDetail {
  pub file: String,
  pub status: String,
  pub file_diff: FileDiff,              // Conflict markers (inline view)
  pub base_file: Option<FileInfo>,      // Common ancestor content
  pub target_file: Option<FileInfo>,    // Target branch content  
  pub cherry_file: Option<FileInfo>,    // Cherry-pick content
  pub base_to_target_diff: FileDiff,    // Base → Target diff (3-way view)
  pub base_to_cherry_diff: FileDiff,    // Base → Cherry diff (3-way view)
}
```

### Frontend 3-Way Display
```vue
<div class="grid grid-cols-1 xl:grid-cols-3 gap-4">
  <!-- Base (Common Ancestor) -->
  <div>
    <h4>Base (Common Ancestor)</h4>
    <p class="text-xs text-muted">
      File state at <CommitHashPopover :hash="mergeBaseInfo.hash" />
    </p>
    <!-- Show base content as context-only diff -->
    <DiffView :data="getBaseDiffData(conflict)" />
  </div>
  
  <!-- Target Branch -->
  <div>
    <h4>Target Branch (Current)</h4>
    <p class="text-xs text-muted">
      Changes from base to <CommitHashPopover :hash="targetInfo.hash" />
    </p>
    <!-- Show base → target changes -->
    <DiffView :data="conflict.baseToTargetDiff" />
  </div>
  
  <!-- Cherry-pick -->
  <div>
    <h4>Cherry-pick (Incoming)</h4>
    <p class="text-xs text-muted">
      Changes from base to <CommitHashPopover :hash="cherryInfo.hash" />
    </p>
    <!-- Show base → cherry changes -->
    <DiffView :data="conflict.baseToCherryDiff" />
  </div>
</div>
```

### Base Content Display Fix
```typescript
// ✅ Create context-only diff for base content display
function getBaseDiffData(conflict: ConflictDetail) {
  const fileData = getFileContent(conflict)
  
  return {
    oldFile: { fileName: fileData.fileName, fileLang: fileData.fileExt, content: fileData.base },
    newFile: { fileName: fileData.fileName, fileLang: fileData.fileExt, content: fileData.base },
    hunks: fileData.base ? [
      `--- a/${fileData.fileName}\n+++ b/${fileData.fileName}\n@@ -1,${lines.length} +1,${lines.length} @@\n${lines.map(line => ` ${line}`).join('\n')}`
    ] : [] as string[]
  }
}
```

### Desktop UI Considerations
- Users need maximum screen space for 3-way merge resolution
- "Missing commits" analysis is crucial context that should remain visible
- Consider Tauri sub-windows for complex conflict resolution while preserving main window context
- Provide clear explanations of what each column in 3-way view represents
- Use CommitHashPopover consistently for commit information display

## Tauri v2 Window Management

### Creating Sub-windows
```typescript
// Import from @tauri-apps/api/webviewWindow
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { emit } from '@tauri-apps/api/event'

// Create window
const window = new WebviewWindow('window-label', {
  url: 'page-route',  // e.g., 'missing-commits' for /pages/missing-commits.vue
  title: 'Window Title',
  width: 1000,
  height: 800,
  center: true,
  skipTaskbar: true,  // For sub-windows
})

// Pass data via events after creation
window.once('tauri://created', async () => {
  await emit('init-event-name', data)
})

// In the sub-window page, listen for data
onMounted(async () => {
  const unlisten = await listen<DataType>('init-event-name', (event) => {
    // Use event.payload
  })
  
  onUnmounted(() => unlisten())
})
```

### Required Permissions
In `src-tauri/capabilities/default.json`, add only what's needed:
```json
"permissions": [
  "core:webview:allow-create-webview-window"  // Only this for WebviewWindow API
]
```

### Window Labels
Main window needs a label in `tauri.conf.json`:
```json
"windows": [{
  "label": "main",
  "title": "BranchDeck"
}]
```

### UX Considerations for Sub-windows
- Place "Open in Window" buttons on the component that will be shown in the window
- Use ghost/subtle button variants for secondary actions
- Sub-windows are useful for detailed views that need more space
- Always pass necessary data via events, not URL parameters

## Window Management Patterns

### Opening Sub-Windows with Handshake
```typescript
// ✅ Correct pattern with handshake for reliable data transfer
async function openSubWindow() {
  const data = {
    // Prepare all necessary data
    conflict: props.conflict,
    branchName: props.branchName,
    viewSettings: { showConflictsOnly, viewMode, diffMode }
  }
  
  // Create window
  const subWindow = new WebviewWindow('window-id', {
    url: '/page-name',  // Route to page in /pages/
    title: `Title - ${contextInfo}`,
    width: 1400,
    height: 900,
    center: true,
    resizable: true,
    skipTaskbar: true,
  })
  
  // Wait for ready signal before sending data
  subWindow.once('window-ready', async () => {
    await emit('init-window-data', data)
  })
}
```

### Sub-Window Page Setup
```vue
<script setup>
// Disable layout for sub-windows
definePageMeta({ layout: false })

const windowData = ref<DataType | null>(null)

onMounted(async () => {
  // Restore from sessionStorage for hot reload
  const stored = sessionStorage.getItem('window-data')
  if (stored) {
    try {
      windowData.value = JSON.parse(stored)
      return  // Don't request new data
    } catch {
      // Invalid data, continue
    }
  }

  // Listen for data
  const unlisten = await listen<DataType>('init-window-data', (event) => {
    windowData.value = event.payload
    // Store for hot reload
    sessionStorage.setItem('window-data', JSON.stringify(event.payload))
    unlisten()
  })
  
  // Signal ready to receive data
  await emit('window-ready', {})
})
</script>
```

### Capability Files for Sub-Windows
```json
// src-tauri/capabilities/window-name.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "window-name",
  "description": "Capability for the window",
  "windows": ["window-name"],  // Must match WebviewWindow label
  "permissions": ["core:event:default"]
}
```

## UI Component Patterns

### Tabs vs Button Groups
```vue
<!-- ✅ Use UTabs for view mode switching with custom content -->
<UTabs 
  v-model="selectedTab" 
  :items="[
    { value: 'diff', slot: 'diff', label: 'Diff', icon: 'i-lucide-align-left' },
    { value: '3way', slot: '3way', label: '3-way', icon: 'i-lucide-columns-3' }
  ]"
  variant="link"
  color="neutral"
>
  <template #diff>
    <!-- Diff view content -->
  </template>
  <template #3way>
    <!-- 3-way view content -->
  </template>
</UTabs>

<!-- ❌ Avoid button groups for view modes -->
<UButtonGroup>...</UButtonGroup>
```

### Collapsible Sections
```vue
<!-- ✅ Collapsible sections with proper state management -->
<script setup>
const collapsedViews = ref<Record<string, boolean>>({})

// Initialize collapsed state
onMounted(() => {
  items.forEach((_, index) => {
    collapsedViews.value[`${index}-base`] = true  // Collapse by default
  })
})

function toggleView(viewKey: string) {
  collapsedViews.value[viewKey] = !collapsedViews.value[viewKey]
}
</script>

<template>
  <div class="flex items-center justify-between">
    <h4>Section Title</h4>
    <UButton
      size="xs"
      variant="ghost"
      :icon="collapsedViews[key] ? 'i-lucide-chevron-down' : 'i-lucide-chevron-up'"
      @click="toggleView(key)"
    />
  </div>
  <div v-if="!collapsedViews[key]">
    <!-- Content -->
  </div>
</template>
```

### Global Controls Pattern
```vue
<!-- ✅ Global toggle controlling multiple child instances -->
<template>
  <div class="flex items-center gap-3">
    <!-- Global control only shows when relevant -->
    <UButtonGroup v-if="hasMultipleItems" size="xs">
      <UButton
        :color="globalMode === 'unified' ? 'primary' : 'neutral'"
        @click="globalMode = 'unified'"
      >
        Unified
      </UButton>
      <UButton
        :color="globalMode === 'split' ? 'primary' : 'neutral'"
        @click="globalMode = 'split'"
      >
        Split
      </UButton>
    </UButtonGroup>
  </div>
  
  <!-- Pass control down to children -->
  <ChildComponent 
    v-for="item in items"
    :hide-controls="true"
    :view-mode="globalMode"
  />
</template>
```

### Responsive Grid Layouts
```vue
<!-- ✅ Responsive grid with proper breakpoints -->
<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
  <!-- Stacks on mobile, 2 cols on large, 3 cols on XL -->
</div>
```

## Component Communication

### Unified UI Components
```vue
<!-- ✅ Create reusable components for consistent UI -->
// CommitInfo.vue - Unified commit display
<template>
  <div class="flex items-center gap-2 text-xs text-muted">
    <span class="font-mono">{{ shortHash }}</span>
    <span v-if="author">by {{ author }}</span>
    <span v-if="timestamp">{{ formatTimestamp(timestamp) }}</span>
    <span v-if="fileCount !== undefined">
      <span>•</span>
      <UIcon name="i-lucide-file-code" class="w-3 h-3"/>
      <span>{{ fileCount }} {{ fileCount === 1 ? 'file' : 'files' }}</span>
    </span>
  </div>
</template>
```

### Controlled vs Uncontrolled Components
```typescript
// Component supports both patterns
const props = defineProps<{
  diffViewMode?: 'unified' | 'split'  // Optional for controlled mode
}>()

// Internal state for uncontrolled mode
const internalMode = ref<'unified' | 'split'>('unified')

// Use prop if provided, otherwise use internal state
const currentMode = computed(() => props.diffViewMode || internalMode.value)
```