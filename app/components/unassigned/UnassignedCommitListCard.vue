<template>
  <UCard class="overflow-hidden" :ui="{ body: 'p-0 sm:p-0' }" data-testid="unassigned-commits-section">
    <template #header>
      <CardHeader
        title="Unassigned Commits"
        :count="commits.length"
        item-singular="commit"
        item-plural="commits"
        badge-color="neutral"
      >
        <template #actions>
          <!-- Combined help for selection and unassigned commits -->
          <UnassignedCommitsHelpPopover v-if="commits.length > 0" />
        </template>
      </CardHeader>
    </template>

    <!-- Floating Selection Bar -->
    <FloatingSelectionBar
      :selected-count="selectedCount"
      :target-element="firstSelectedRowElement"
      :is-inline-creation-active="isInlineCreationActive"
      @create-branch="activateInlineCreation"
    />

    <!-- Inline Branch Creator (renders via portal) -->
    <InlineBranchCreator
      v-if="isInlineCreationActive"
      :selected-commits="selectedItems"
      :is-active="isInlineCreationActive"
      @cancel="cancelInlineCreation"
      @success="handleBranchSuccess"
    />

    <CommitList
      ref="commitListRef"
      :commits="commits"
      variant="compact"
      :show-author="true"
      :selectable="true"
      :highlight-selection="isInlineCreationActive"
      :context-menu-items="getContextMenuItemsForCommits"
      @selection-change="handleSelectionChange"
      @keydown="handleKeydown"
    >
      <template #portal-target>
        <!-- Portal target for inline branch creator -->
        <div v-if="isInlineCreationActive" id="inline-branch-creator-portal" class="mb-4 relative overflow-hidden">
          <!-- Progress bar overlaying the top border -->
          <div
            v-if="activeInline?.processing && activeInline.branchName === 'branch-creation'"
            class="absolute top-0 left-0 w-full h-px bg-accented overflow-hidden">
            <div class="absolute inset-y-0 w-1/2 bg-primary animate-[carousel_2s_ease-in-out_infinite]"/>
          </div>
        </div>
      </template>
    </CommitList>
  </UCard>
</template>

<script lang="ts" setup>
import type { CommitList } from "#components"

import type { Commit, MissingCommit } from "~/utils/bindings"
import type { SyncedCommit } from "~/composables/branchSyncProvider"

// Union type for all supported commit types
type CommitUnion = Commit | SyncedCommit | MissingCommit

const { getContextMenuItems } = useUnassignedCommitContextActions()
const { activeInline } = useInlineRowAction()

defineProps<{
  commits: Commit[]
}>()

// No longer need to emit refresh events - using store directly now

// Ref to the CommitList component
const commitListRef = useTemplateRef<InstanceType<typeof CommitList>>("commitListRef")

// Inline creation state
const isInlineCreationActive = ref(false)

// Track first selected row element for floating bar positioning
const firstSelectedRowElement = ref<HTMLElement | null>(null)

// Computed properties to access table data
const selectedCount = computed(() => {
  if (!commitListRef.value?.table) {
    return 0
  }
  return commitListRef.value.table.getSelectedRowModel().rows.length
})

const selectedItems = computed((): Commit[] => {
  if (!commitListRef.value?.table) {
    return []
  }
  return commitListRef.value.table.getSelectedRowModel().rows.map(row => row.original as Commit)
})

// Update first selected row element when selection changes
watch([selectedCount, () => commitListRef.value?.table], ([count, table]) => {
  if (count > 0 && table) {
    // Get first selected row from TanStack Table
    const selectedRows = table.getSelectedRowModel().rows
    if (selectedRows.length > 0) {
      // Get the row ID (commit hash) of the first selected row
      const firstRowId = selectedRows[0]!.id
      // Find the DOM element using the data-row-id attribute
      const rowElement = commitListRef.value?.$el?.querySelector(`[data-row-id="${firstRowId}"]`)
      firstSelectedRowElement.value = rowElement as HTMLElement || null
    }
    else {
      firstSelectedRowElement.value = null
    }
  }
  else {
    firstSelectedRowElement.value = null
  }
})

// Clear selection
function clearSelection() {
  if (commitListRef.value?.table) {
    commitListRef.value.table.resetRowSelection()
  }
}

// Handle selection changes from CommitList
function handleSelectionChange(_selectedItems: unknown[]) {
  // Close inline creation if no commits selected
  if (selectedItems.value.length === 0) {
    isInlineCreationActive.value = false
  }
}

// Handle keyboard events from CommitList
function handleKeydown(event: KeyboardEvent) {
  // Only block ESC key when form is active to prevent selection clearing
  if (isInlineCreationActive.value && event.key === "Escape") {
    event.preventDefault()
    return
  }

  // Handle Enter key to open branch creation form
  if (event.key === "Enter" && selectedCount.value > 0) {
    event.preventDefault()
    activateInlineCreation()
  }
}

// Activate inline creation mode
function activateInlineCreation() {
  isInlineCreationActive.value = true
}

// Cancel inline creation
function cancelInlineCreation() {
  isInlineCreationActive.value = false
}

// Handle successful branch creation
function handleBranchSuccess() {
  isInlineCreationActive.value = false
  clearSelection()
}

// Create context menu items for selected commits
function getContextMenuItemsForCommits(selectedCommits: CommitUnion[]) {
  // Disable context menu when inline creation is active
  if (isInlineCreationActive.value) {
    return []
  }

  return getContextMenuItems(selectedCommits, activateInlineCreation)
}

// Watch for inline creator activation
watch(isInlineCreationActive, (active) => {
  if (!active) {
    // Dialog is closing
    return
  }

  // When activating, ensure the inline form portal is visible
  nextTick(() => {
    const portal = document.getElementById("inline-branch-creator-portal")
    if (portal) {
      const rect = portal.getBoundingClientRect()
      // Check if portal is outside viewport
      if (rect.top < 0 || rect.bottom > window.innerHeight) {
        // Scroll the portal into view
        portal.scrollIntoView({ behavior: "smooth", block: "center" })
      }
    }
  })
})
</script>
