import type { Table } from "@tanstack/vue-table"
import { nextTick, ref, watch } from "vue"
import type { Ref, ComputedRef } from "vue"

/**
 * Composable for handling table row expansion with pending expansion queue
 * Useful when rows need to be expanded before they exist in the table data
 */
export function useTableExpansion<T>(
  table: Table<T>,
  tableRef: Ref<HTMLDivElement | undefined>,
  data: Ref<T[]> | ComputedRef<T[]>,
) {
  // Queue for pending expansions (rows that need to be expanded once they're added to the table)
  const pendingExpansions = ref(new Map<string, boolean>())

  /**
   * Expand a row in the table
   * If the row doesn't exist yet, it will be queued for expansion when it's added
   */
  const expandRow = (rowId: string, scroll: boolean = false) => {
    // Use internal API to avoid errors when row doesn't exist
    const row = table.getRowModel().rowsById[rowId]

    if (row) {
      if (!row.getIsExpanded()) {
        row.toggleExpanded(true)
      }

      // Handle scrolling if requested
      if (scroll && tableRef.value) {
        nextTick(() => {
          const rowElement = tableRef.value?.querySelector(`[data-branch-name="${rowId}"]`) as HTMLElement
          if (rowElement) {
            rowElement.scrollIntoView({ behavior: "smooth", block: "center" })
          }
        })
      }
    }
    else {
      // Queue for later expansion when the row is added to the table
      console.warn(`[useTableExpansion] Row "${rowId}" not found in table, queuing for later expansion`)
      pendingExpansions.value.set(rowId, scroll)
    }
  }

  // Watch for data changes to process pending expansions
  watch(data, () => {
    if (pendingExpansions.value.size > 0) {
      nextTick(() => {
        // Process all pending expansions
        for (const [rowId, shouldScroll] of pendingExpansions.value) {
          // Use internal API to avoid errors when row doesn't exist
          const row = table.getRowModel().rowsById[rowId]
          if (row) {
            if (!row.getIsExpanded()) {
              row.toggleExpanded(true)
            }

            // Handle scrolling if requested
            if (shouldScroll && tableRef.value) {
              const rowElement = tableRef.value?.querySelector(`[data-branch-name="${rowId}"]`) as HTMLElement
              if (rowElement) {
                rowElement.scrollIntoView({ behavior: "smooth", block: "center" })
              }
            }

            // Remove from pending queue
            pendingExpansions.value.delete(rowId)
          }
        }
      })
    }
  }, { deep: true })

  return {
    expandRow,
    pendingExpansions: readonly(pendingExpansions),
  }
}
