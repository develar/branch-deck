<template>
  <div>
    <!-- Portal slot for inline forms -->
    <slot name="portal-target" />

    <table
      ref="containerRef"
      class="w-full rounded-lg focus:outline-none"
      :tabindex="selectable ? 0 : -1"
      @keydown="handleKeydown"
    >
      <tbody class="divide-y divide-default">
        <tr
          v-for="row in table.getRowModel().rows"
          :key="row.id"
          :data-row-id="row.id"
          :data-selected="selectable && row.getIsSelected()"
          :class="[
            !selectable && 'hover:bg-muted transition-colors',
            selectable && 'cursor-pointer relative select-none hover:bg-muted',
            selectable && row.getIsSelected() && 'bg-primary/10 hover:bg-primary/15',
          ]"
          @click="handleRowClick($event, row)"
        >
          <td class="bd-padding-list-item">
            <!-- Selection indicator bar -->
            <div
              v-if="selectable && row.getIsSelected() && highlightSelection"
              class="absolute left-0 top-1 bottom-1 w-1 bg-primary rounded-r-sm"
            />

            <!-- Row content -->
            <div v-for="cell in row.getVisibleCells()" :key="cell.id">
              <component
                :is="cell.column.columnDef.cell"
                v-bind="cell.getContext()"
              />
            </div>

            <!-- Slot for additional content after commit -->
            <slot name="after-commit" :commit="row.original" :index="row.index" />
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script lang="ts" setup>
import type { Commit, CommitSyncStatus, BranchError, MissingCommit } from "~/utils/bindings"
import type { SyncedCommit } from "~/composables/branchSyncProvider"
import {
  createColumnHelper,
  getCoreRowModel,
  getExpandedRowModel,
  useVueTable,
} from "@tanstack/vue-table"
// useTableSelection is auto-imported from shared-ui layer

// Union type for all supported commit types
type CommitUnion = Commit | SyncedCommit | MissingCommit

interface Props {
  // Commits are always arrays now
  commits: Commit[] | SyncedCommit[] | MissingCommit[]

  // Display variants
  variant?: "compact" | "detailed" | "status"

  // Status-specific
  branchName?: string

  // Feature flags
  showFileCount?: boolean
  showAuthor?: boolean

  // Selection support
  selectable?: boolean
  highlightSelection?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  variant: "compact",
  showFileCount: false,
  showAuthor: false,
  branchName: undefined,
  selectable: false,
  highlightSelection: false,
})

const emit = defineEmits<{
  "selection-change": [selectedItems: CommitUnion[]]
  "keydown": [event: KeyboardEvent]
}>()

// Helper to get commit hash
function getCommitHash(commit: CommitUnion): string {
  // For Commit type, use originalHash
  if ("originalHash" in commit && commit.originalHash) {
    return commit.originalHash
  }
  // For SyncedCommit and MissingCommit types, use hash
  if ("hash" in commit && commit.hash) {
    return commit.hash
  }
  return ""
}

// Component refs
const containerRef = useTemplateRef<HTMLTableElement>("containerRef")

// Column definitions
const columnHelper = createColumnHelper<CommitUnion>()

const columns = computed(() => [
  columnHelper.display({
    id: "commit",
    cell: ({ row }) => {
      const commit = row.original
      return h("div", [
        // Commit message with optional badge
        h("div", { class: "flex items-center gap-2" }, [
          // Use strippedSubject if available, otherwise subject, otherwise fallback to message
          h(resolveComponent("CommitMessageWithPopover"), {
            subject: ("strippedSubject" in commit && commit.strippedSubject)
              ? commit.strippedSubject
              : ("subject" in commit && commit.subject)
                  ? commit.subject
                  : commit.message,
            message: commit.message,
            messageClass: "text-sm text-highlighted line-clamp-2",
          }),

          // Status badge for exceptional states (only in status variant)
          props.variant === "status" && "status" in commit && isExceptionalStatus(commit.status) && [
            commit.status === "Pending" && h(resolveComponent("UBadge"), {
              size: "sm",
              variant: "subtle",
            }, () => [
              h(resolveComponent("UIcon"), {
                name: "i-lucide-loader-circle",
                class: "animate-spin mr-1 size-3",
              }),
              "Pending",
            ]),
            commit.status === "Error" && h(resolveComponent("UBadge"), {
              color: "error",
              size: "sm",
              variant: "subtle",
            }, () => "Error"),
            commit.status === "Blocked" && h(resolveComponent("UBadge"), {
              color: "warning",
              size: "sm",
              variant: "subtle",
            }, () => "Blocked"),
          ],
        ]),

        // Metadata line
        h("div", { class: "mt-1 flex items-center gap-2 text-xs text-muted" }, [
          // Hash(es)
          h("span", { class: "font-mono" }, formatShortHash(getCommitHash(commit))),

          props.variant === "status" && "hash" in commit && commit.hash && [
            h("span", "→"),
            h("span", { class: "font-mono" }, formatShortHash(commit.hash)),
          ],

          // Author (if enabled)
          props.showAuthor && "author" in commit && commit.author && [
            h("span", "•"),
            h("span", commit.author),
          ],

          // Timestamp
          commit.authorTime && [
            h("span", "•"),
            commit.committerTime
              ? h(resolveComponent("TimeWithPopover"), {
                  authorTime: commit.authorTime,
                  committerTime: commit.committerTime,
                })
              : h("span", formatTimestamp(commit.authorTime)),
          ],

          // File count (if enabled)
          props.showFileCount && getFileCount(commit) && [
            h("span", "•"),
            h("span", `${getFileCount(commit)} ${getFileCount(commit) === 1 ? "file" : "files"}`),
          ],

          // Status text (only show for non-common statuses)
          props.variant === "status" && "status" in commit && commit.status && shouldShowStatusText(commit.status) && [
            h("span", "•"),
            h("span", {
              class: getCommitStatusClass(commit.status),
            }, getCommitStatusText(commit.status, "error" in commit ? commit.error : undefined)),
          ],
        ]),

        // Error details (status variant only)
        props.variant === "status" && "error" in commit && commit.error && h("div", { class: "mt-2" }, [
          "error" in commit && commit.error && "MergeConflict" in commit.error
            ? h(resolveComponent("LazyMergeConflictViewer"), {
                conflict: commit.error.MergeConflict,
                branchName: props.branchName,
              })
            : "error" in commit && commit.error && "Generic" in commit.error && h(resolveComponent("UAlert"), {
              color: "error",
              variant: "soft",
              size: "xs",
            }, {
              description: () => h("p", { class: "text-xs" }, (commit.error as { Generic: string }).Generic),
            }),
        ]),
      ])
    },
  }),
])

// Create table instance
const table = useVueTable<CommitUnion>({
  data: toRef(props, "commits"),
  get columns() {
    return columns.value
  },
  getCoreRowModel: getCoreRowModel(),
  getExpandedRowModel: getExpandedRowModel(),
  enableRowSelection: props.selectable,
  enableMultiRowSelection: props.selectable,
  getRowId: row => getCommitHash(row),
})

// Use selection composable
const tableRef = computed(() => table)
const {
  selectedItems,
  handleKeyboardShortcuts,
  handleRowClick: handleSelectionClick,
} = useTableSelection(tableRef)

// Watch for selection changes and emit event
watch(selectedItems, (items) => {
  emit("selection-change", items)
})

// Handle row clicks
function handleRowClick(event: MouseEvent, row: { index: number, id: string }) {
  if (!props.selectable) {
    return
  }
  handleSelectionClick(event, row.id)
}

// Handle keydown events
function handleKeydown(event: KeyboardEvent) {
  // First emit the event for parent component to handle
  emit("keydown", event)

  // Then handle table-specific shortcuts if event wasn't prevented
  if (!event.defaultPrevented && props.selectable) {
    handleKeyboardShortcuts(event)
  }
}

// Helper to get file count for a commit
function getFileCount(commit: CommitUnion): number | undefined {
  if ("fileDiffs" in commit && commit.fileDiffs?.length) {
    return commit.fileDiffs.length
  }
  if ("fileCount" in commit && typeof commit.fileCount === "number") {
    return commit.fileCount
  }
  return undefined
}

// Helper functions for status variant
function isExceptionalStatus(status?: CommitSyncStatus): boolean {
  return status === "Pending" || status === "Error" || status === "Blocked"
}

function shouldShowStatusText(status: CommitSyncStatus): boolean {
  // Only show status text for non-common statuses
  return status !== "Unchanged"
}

function getCommitStatusClass(status: CommitSyncStatus): string {
  switch (status) {
    case "Pending":
      return "text-dimmed"
    case "Error":
      return "text-error"
    case "Blocked":
      return "text-warning"
    case "Created":
      return "text-success"
    case "Unchanged":
      return "text-muted"
    default:
      return ""
  }
}

function getCommitStatusText(status: CommitSyncStatus, error?: BranchError | null): string {
  if (status === "Error" && error && "MergeConflict" in error) {
    return "Merge Conflict"
  }
  return status
}

// Expose table instance for parent components
defineExpose({
  table,
})
</script>
