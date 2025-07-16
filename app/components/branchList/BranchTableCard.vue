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
                :class="[
                  'hover:bg-muted/50 transition-all cursor-pointer',
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
            <tr v-if="row.getIsExpanded()" :key="`${row.id}-expanded`">
              <td colspan="4" class="py-2 border-t border-dotted border-default">
                <div class="relative">
                  <!-- Visual connector line -->
                  <div class="absolute left-6 top-0 bottom-0 w-0.5 bg-primary/20" />

                  <!-- Content with subtle indentation -->
                  <div class="ml-8">
                    <!-- Error alert if there are error details -->
                    <UAlert
                      v-if="row.original.errorDetails"
                      color="error"
                      variant="subtle"
                      class="mb-4 mr-6"
                    >
                      <template #title>Error Details</template>
                      <template v-if="row.original.errorDetails && 'MergeConflict' in row.original.errorDetails" #description>
                        <MergeConflictViewer
                          :conflict="row.original.errorDetails.MergeConflict"
                          :branch-name="row.original.name"
                        />
                      </template>
                      <template v-else-if="row.original.errorDetails && 'Generic' in row.original.errorDetails" #description>
                        {{ (row.original.errorDetails as any).Generic }}
                      </template>
                    </UAlert>

                    <!-- Commit list -->
                    <CommitList
                      :commits="getCommitsAsArray(row.original.commits)"
                      :variant="'status'"
                      :show-file-count="false"
                      :container-class="''"
                      :item-class="''"
                    />
                  </div>
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
import type { ReactiveBranch, SyncOptions } from "~/composables/branch/syncBranches"
import type { CommitDetail } from "~/utils/bindings"
import {
  createColumnHelper,
  getCoreRowModel,
  getExpandedRowModel,
  useVueTable,
  FlexRender,
  type ExpandedState,
  type ColumnDef,
} from "@tanstack/vue-table"
import { useRepositoryStore } from "~/stores/repository"
// useCopyToClipboard is auto-imported from shared-ui layer
import { usePush } from "~/composables/git/push"
import { useBranchContextActions } from "~/composables/branch/useBranchContextActions"

interface Props {
  branches: ReactiveBranch[]
  isSyncing: boolean
}

const props = defineProps<Props>()

const emit = defineEmits<{
  "push-branch": [branchName: string]
  "refresh": [options?: SyncOptions]
}>()
const repositoryStore = useRepositoryStore()
const { isPushing } = usePush(repositoryStore.vcsRequestFactory)

// Template ref for scrolling
const tableRef = ref<HTMLDivElement>()

// Context actions composable
const {
  inlineInputActiveBranch,
  processingBranch,
  getContextMenuItems,
  hideInlineInput,
  handleInlineSubmit,
} = useBranchContextActions({
  onRefresh: options => emit("refresh", options),
})

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
      return h("div", { class: "flex items-center gap-2 group" }, [
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
        h(resolveComponent("CopyButton"), {
          text: getFullBranchName(branch.name),
          tooltip: "Copy full branch name to clipboard",
        }),
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
      if (branch.status === "Syncing") {
        return h("div", { class: "flex items-center gap-2" }, [
          h(resolveComponent("UProgress"), {
            modelValue: branch.processedCount,
            max: branch.commitCount,
            status: true,
            size: "sm",
            class: "w-20",
          }),
        ])
      }
      else {
        return h(resolveComponent("UBadge"), {
          color: getIncrementalStatusColor(branch.status),
          variant: "soft",
          class: "lowercase",
        }, () => branch.statusText)
      }
    },
  }),
  columnHelper.display({
    id: "actions",
    header: "Actions",
    cell: ({ row }) => {
      const branch = row.original
      if (!branch.hasError && branch.commitCount > 0) {
        return h(resolveComponent("UButton"), {
          disabled: isPushing(branch.name) || props.isSyncing || branch.status === "Syncing",
          loading: isPushing(branch.name),
          icon: "i-lucide-upload",
          size: "xs",
          variant: "outline",
          onClick: (e: MouseEvent) => {
            e.stopPropagation()
            emit("push-branch", branch.name)
          },
        }, () => branch.status === "Updated" ? "Force Push" : "Push")
      }
      return null
    },
  }),
]

const table = useVueTable({
  get data() {
    return props.branches
  },
  get columns() {
    return columns
  },
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

// Expose table ref and methods for parent component
defineExpose({
  tableRef,
  table,
  expandBranch: (branchName: string, scroll: boolean) => {
    const row = table.getRow(branchName)
    if (row && !row.getIsExpanded()) {
      row.toggleExpanded(true)
    }

    // Handle scrolling if requested
    if (scroll && tableRef.value) {
      nextTick(() => {
        const rowElement = tableRef.value?.querySelector(`[data-branch-name="${branchName}"]`) as HTMLElement
        if (rowElement) {
          rowElement.scrollIntoView({ behavior: "smooth", block: "center" })
        }
      })
    }
  },
})

// Helper to get commits as array
const getCommitsAsArray = (commits: Map<string, CommitDetail>): CommitDetail[] => {
  return Array.from(commits.values())
}

// Get full branch name with prefix
const getFullBranchName = (branchName: string) => {
  return `${repositoryStore.branchPrefix}/${branchName}`
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
  return props.branches.find(b => b.name === inlineInputActiveBranch.value)
}

const getActiveBranchCommitCount = () => {
  const branch = getActiveBranch()
  return branch?.commitCount || 0
}
</script>