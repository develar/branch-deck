<template>
  <UCard v-if="branches.length > 0" :ui="{ body: 'p-0 sm:p-0' }">
    <div ref="tableRef" class="overflow-x-auto">
      <table class="w-full">
        <thead class="bg-muted/50 border-b border-default">
          <tr v-for="headerGroup in table.getHeaderGroups()" :key="headerGroup.id">
            <th
              v-for="header in headerGroup.headers"
              :key="header.id"
              class="text-left px-6 py-3 text-sm font-medium text-highlighted"
            >
              <FlexRender
                v-if="!header.isPlaceholder"
                :render="header.column.columnDef.header"
                :props="header.getContext()"
              />
            </th>
          </tr>
        </thead>
        <tbody class="divide-y divide-default">
          <template v-for="row in table.getRowModel().rows" :key="row.id">
            <!-- Branch row -->
            <UContextMenu :items="getContextMenuItems(row.original)">
              <tr
                data-testid="branch-row"
                :data-branch-name="row.original.name"
                :data-state="row.getIsExpanded() ? 'open' : 'closed'"
                :class="[
                  'branch-row hover:bg-muted transition-all cursor-pointer group',
                  'data-[state=open]:bg-elevated',
                  processingBranch === row.original.name && 'animate-pulse ring-2 ring-primary/50'
                ]"
                @click="row.toggleExpanded()"
              >
                <td
                  v-for="cell in row.getVisibleCells()"
                  :key="cell.id"
                  class="px-6 py-4"
                >
                  <FlexRender
                    :render="cell.column.columnDef.cell"
                    :props="cell.getContext()"
                  />
                </td>
              </tr>
            </UContextMenu>

            <!-- Inline issue reference input row -->
            <tr v-if="inlineInputActiveBranch === row.original.name" :key="`${row.id}-inline`">
              <td colspan="4" class="p-0">
                <!-- Portal target for dialog content -->
                <div :id="`inline-form-${row.original.name}`" />
              </td>
            </tr>

            <!-- Expanded row content -->
            <tr v-if="row.getIsExpanded()" :key="`${row.id}-expanded`" class="!border-t-0">
              <td colspan="4" class="px-0 py-4">
                <div class="ml-4 border-l-2 border-primary/50 pl-2 pr-6">
                  <!-- Error display -->
                  <div v-if="row.original.errorDetails" class="mb-3">
                    <template v-if="row.original.errorDetails && 'MergeConflict' in row.original.errorDetails">
                      <MergeConflictViewer
                        :conflict="row.original.errorDetails.MergeConflict"
                        :branch-name="row.original.name"
                      />
                    </template>
                    <UAlert
                      v-else-if="row.original.errorDetails && 'Generic' in row.original.errorDetails"
                      color="error"
                      variant="subtle"
                      size="sm"
                    >
                      <template #description>
                        {{ (row.original.errorDetails as any).Generic }}
                      </template>
                    </UAlert>
                  </div>

                  <!-- Commit list -->
                  <CommitList
                    :commits="row.original.commits"
                    :variant="'status'"
                    :show-file-count="false"
                  />
                </div>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>

    <!-- Inline issue reference input (renders via portal) -->
    <LazyInlineIssueReferenceInput
      v-if="inlineInputActiveBranch"
      :branch-name="inlineInputActiveBranch || ''"
      :commit-count="getActiveBranchCommitCount()"
      :dialog-title="inlineInputActiveBranch ? `Add Issue Reference to ${inlineInputActiveBranch}` : ''"
      :dialog-description="inlineInputActiveBranch ? `Add issue reference form for ${inlineInputActiveBranch} branch` : ''"
      :portal-target="inlineInputActiveBranch ? `inline-form-${inlineInputActiveBranch}` : undefined"
      :is-active="!!inlineInputActiveBranch"
      @submit="(issueReference) => handleInlineSubmit(issueReference, getActiveBranch()!)"
      @cancel="hideInlineInput"
    />
  </UCard>
</template>

<script lang="ts" setup>
import {
  createColumnHelper,
  getCoreRowModel,
  getExpandedRowModel,
  useVueTable,
  FlexRender,
  type ExpandedState,
  type ColumnDef,
} from "@tanstack/vue-table"
import { usePush } from "~/composables/git/push"

const { vcsRequestFactory, effectiveBranchPrefix } = useRepository()
const { isPushing, pushBranch } = usePush(vcsRequestFactory)
const { syncBranches, isSyncing, branches } = useBranchSync()

// Template ref for scrolling
const tableRef = useTemplateRef<HTMLDivElement>("tableRef")

// Context actions composable
const {
  inlineInputActiveBranch,
  processingBranch,
  getContextMenuItems,
  hideInlineInput,
  handleInlineSubmit,
} = useBranchContextActions()

// TanStack Table setup
const expanded = ref<ExpandedState>({})

// Create column helper for type safety
const columnHelper = createColumnHelper<ReactiveBranch>()

// Define columns
const columns: ColumnDef<ReactiveBranch>[] = [
  columnHelper.display({
    id: "expander",
    header: "Branch Name",
    cell: ({ row }) => {
      const branch = row.original
      return h("div", { class: "flex items-center gap-2" }, [
        row.getCanExpand() && h(resolveComponent("UButton"), {
          icon: row.getIsExpanded() ? "i-lucide-folder-open" : "i-lucide-folder-closed",
          variant: "ghost",
          size: "xs",
          onClick: (e: MouseEvent) => {
            e.stopPropagation()
            row.toggleExpanded()
          },
        }),
        h("span", { class: "text-sm font-medium" }, branch.name),
      ])
    },
  }),
  columnHelper.display({
    id: "commits",
    header: "Commits",
    cell: ({ row }) => {
      const count = row.original.commitCount
      return h("span", { class: "text-sm text-muted" }, `${count} commit${count === 1 ? "" : "s"}`)
    },
  }),
  columnHelper.display({
    id: "status",
    header: "Status",
    cell: ({ row }) => {
      const branch = row.original
      const statusText = branch.status === "Syncing"
        ? `syncing ${branch.processedCount}/${branch.commitCount}…`
        : branch.statusText

      return h("div", { class: "w-40" }, [
        h(resolveComponent("UBadge"), {
          color: getIncrementalStatusColor(branch.status),
          variant: "soft",
          class: "lowercase",
        }, () => statusText),
      ])
    },
  }),
  columnHelper.display({
    id: "actions",
    header: "Actions",
    cell: ({ row }) => {
      const branch = row.original
      const buttons = []

      // Always show copy button
      buttons.push(h(resolveComponent("CopyButton"), {
        text: getFullBranchName(branch.name),
        tooltip: "Copy full branch name",
        size: "xs",
        alwaysVisible: true,
      }))

      // Show push button when applicable
      if (!branch.hasError && branch.commitCount > 0) {
        buttons.push(h(resolveComponent("UButton"), {
          disabled: isPushing(branch.name) || isSyncing.value || branch.status === "Syncing",
          loading: isPushing(branch.name),
          icon: "i-lucide-upload",
          size: "xs",
          variant: "outline",
          onClick: (e: MouseEvent) => {
            e.stopPropagation()
            pushBranch(branch.name)
          },
        }, () => branch.status === "Updated" ? "Force Push" : "Push"))
      }

      return h("div", { class: "flex items-center gap-2" }, buttons)
    },
  }),
]

const table = useVueTable({
  data: branches,
  columns: columns,
  getCoreRowModel: getCoreRowModel(),
  getExpandedRowModel: getExpandedRowModel(),
  getRowId: row => row.name,
  getRowCanExpand: row => row.original.commitCount > 0 || row.original.hasError,
  state: {
    get expanded() {
      return expanded.value
    },
  },
  onExpandedChange: (updater) => {
    expanded.value = typeof updater === "function" ? updater(expanded.value) : updater
  },
})

// Watch for branches that need auto-expansion with debouncing
// watchDebounced(
//   branches,
//   () => {
//     // Collect all branches that need expansion
//     const branchesToExpand = branches.value.filter(branch => branch.autoExpandRequested)
//     if (branchesToExpand.length === 0) {
//       return
//     }
//
//     // Find the branch with the most recent activity that needs scrolling
//     let branchToScroll: ReactiveBranch | null = null
//     let latestCommitTime = 0
//
//     // First, expand all branches that need it
//     branchesToExpand.forEach((branch) => {
//       const row = table.getRowModel().rowsById[branch.name]
//       if (row && !row.getIsExpanded()) {
//         row.toggleExpanded(true)
//
//         // Track the branch with the most recent commit time that needs scrolling
//         if (branch.autoScrollRequested && branch.latestCommitTime > latestCommitTime) {
//           branchToScroll = branch
//           latestCommitTime = branch.latestCommitTime
//         }
//       }
//
//       // Clear the expansion flag
//       branch.autoExpandRequested = false
//     })
//
//     // Then, scroll to the most recently active branch that requested it (only once)
//     if (ENABLE_AUTO_SCROLL && branchToScroll && tableRef.value) {
//       nextTick(() => {
//         const rowElement = tableRef.value?.querySelector(`[data-branch-name="${branchToScroll!.name}"]`) as HTMLElement
//         if (rowElement) {
//           rowElement.scrollIntoView({ behavior: "smooth", block: "center" })
//         }
//
//         // Clear the scroll flag for all branches
//         branchesToExpand.forEach((branch) => {
//           branch.autoScrollRequested = false
//         })
//       })
//     }
//     else {
//       // Still clear the scroll flags even if scrolling is disabled
//       branchesToExpand.forEach((branch) => {
//         branch.autoScrollRequested = false
//       })
//     }
//   },
//   {
//     debounce: 100,
//     deep: true,
//   },
// )

// Get full branch name with prefix
const getFullBranchName = (branchName: string) => {
  return `${effectiveBranchPrefix.value}/${branchName}`
}

// Get incremental status color
const getIncrementalStatusColor = (status: string) => {
  switch (status) {
    case "Created":
      return "success"
    case "Updated":
      return "primary"
    case "Unchanged":
      return "neutral"
    case "Error":
    case "MergeConflict":
      return "error"
    case "AnalyzingConflict":
      return "warning"
    default:
      return "neutral"
  }
}

// Get active branch data
const getActiveBranch = () => {
  if (!inlineInputActiveBranch.value) {
    return null
  }
  return branches.value.find(b => b.name === inlineInputActiveBranch.value)
}

const getActiveBranchCommitCount = () => {
  const branch = getActiveBranch()
  return branch?.commitCount || 0
}

// Listen for sync-branches event from menu
scopedListen("sync-branches", () => {
  syncBranches()
})
</script>

<style scoped>
/* Commented out - keeping CopyButton always visible in table
.branch-row :deep(.copy-button) {
  transition: opacity 200ms ease-out;
}

.branch-row:hover :deep(.copy-button) {
  opacity: 1;
  transition: opacity 200ms ease-out 300ms;
}
*/
</style>