<template>
  <UCard class="overflow-hidden" :ui="{ body: 'p-0 sm:p-0' }">
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

    <!-- Inline Branch Creator -->
    <InlineBranchCreator
      :selected-commits="selectedItems"
      :repository-path="repositoryPath"
      :branch-prefix="branchPrefix"
      :is-active="isInlineCreationActive"
      @cancel="cancelInlineCreation"
      @created="handleBranchCreated"
    />

    <CommitList
      ref="commitListRef"
      :commits="commits"
      variant="compact"
      :show-author="true"
      :selectable="true"
      :highlight-selection="isInlineCreationActive"
      @selection-change="handleSelectionChange"
      @keydown="handleKeydown"
    />
  </UCard>
</template>

<script lang="ts" setup>
import type { CommitDetail } from "~/utils/bindings"
import type { SyncOptions } from "~/composables/branch/syncBranches"
// SelectionHelpPopover, InlineBranchCreator, and FloatingSelectionBar are auto-imported by Nuxt

defineProps<{
  commits: CommitDetail[]
  repositoryPath: string
  branchPrefix: string
}>()

const emit = defineEmits<{
  refresh: [options?: SyncOptions]
}>()

// Ref to the CommitList component
const commitListRef = ref()

// Toast for notifications
const toast = useToast()

// Inline creation state
const isInlineCreationActive = ref(false)

// Track first selected row element for floating bar positioning
const firstSelectedRowElement = ref<HTMLElement | null>(null)

// Computed properties to access table data
const selectedCount = computed(() => {
  if (!commitListRef.value?.table) return 0
  return commitListRef.value.table.getSelectedRowModel().rows.length
})

const selectedItems = computed((): CommitDetail[] => {
  if (!commitListRef.value?.table) return []
  return commitListRef.value.table.getSelectedRowModel().rows.map((row: { original: CommitDetail }) => row.original)
})

// Update first selected row element when selection changes
watchEffect(() => {
  if (selectedCount.value > 0 && commitListRef.value?.table) {
    // Get first selected row from TanStack Table
    const selectedRows = commitListRef.value.table.getSelectedRowModel().rows
    if (selectedRows.length > 0) {
      // Get the row ID (commit hash) of the first selected row
      const firstRowId = selectedRows[0].id
      // Find the DOM element using the data-row-id attribute
      const rowElement = commitListRef.value.$el?.querySelector(`[data-row-id="${firstRowId}"]`)
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
  // Handle Enter key to open branch creation form
  if (event.key === "Enter" && selectedCount.value > 0 && !isInlineCreationActive.value) {
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

  // Restore focus to commit list so keyboard shortcuts work
  nextTick(() => {
    const listElement = commitListRef.value?.$el?.querySelector("[tabindex=\"0\"]")
    if (listElement) {
      listElement.focus()
    }
  })
}

// Handle successful branch creation
function handleBranchCreated(branchName: string) {
  isInlineCreationActive.value = false
  clearSelection()
  emit("refresh", {
    targetBranchName: branchName,
    autoExpand: true,
    autoScroll: true,
  })

  toast.add({
    title: "Success",
    description: `Branch "${branchName}" created successfully`,
    color: "success",
  })

  // Restore focus to commit list
  nextTick(() => {
    const listElement = commitListRef.value?.$el?.querySelector("[tabindex=\"0\"]")
    if (listElement) {
      listElement.focus()
    }
  })
}

// Watch for inline creator activation to ensure selected commits remain visible
watch(isInlineCreationActive, (active) => {
  if (active) {
    // Wait for next tick to ensure the inline creator has rendered
    nextTick(() => {
      // Get the first selected row element
      const selectedRows = commitListRef.value?.$el?.querySelectorAll("[data-selected=\"true\"]")
      if (!selectedRows || selectedRows.length === 0) return

      const firstSelectedRow = selectedRows[0]
      const cardElement = firstSelectedRow.closest(".overflow-hidden")

      if (cardElement && firstSelectedRow) {
        // Get positions
        const cardRect = cardElement.getBoundingClientRect()
        const rowRect = firstSelectedRow.getBoundingClientRect()

        // Check if the selected row is below the viewport
        if (rowRect.top > window.innerHeight - 100) {
          // Scroll to bring the card header into view with some padding
          cardElement.scrollIntoView({ behavior: "smooth", block: "start" })
          // Add a small offset to show some context above
          window.scrollBy({ top: -20, behavior: "smooth" })
        }
        // Check if the row would be hidden by the inline creator
        else if (rowRect.top < cardRect.top + 200) {
          // Estimate inline creator height (approximately 200-250px)
          window.scrollBy({ top: -250, behavior: "smooth" })
        }
      }
    })
  }
})
</script>