import type { Table } from "@tanstack/vue-table"
import { computed } from "vue"
import type { ComputedRef } from "vue"

export function useTableSelection<TData>(table: ComputedRef<Table<TData>>) {
  // Selection count - using table's built-in method
  const selectedCount = computed(() => {
    return table.value.getSelectedRowModel().rows.length
  })

  // Get selected items directly from table's built-in method
  const selectedItems = computed(() => {
    return table.value.getSelectedRowModel().rows.map(row => row.original)
  })

  // Clear selection
  function clearSelection() {
    table.value.resetRowSelection()
  }

  // Handle keyboard shortcuts
  function handleKeyboardShortcuts(event: KeyboardEvent) {
    // Cmd/Ctrl+A: Select all
    if ((event.metaKey || event.ctrlKey) && event.key === "a") {
      event.preventDefault()
      table.value.toggleAllRowsSelected()
    }
    else if (event.key === "Escape" && selectedCount.value > 0) {
      // Escape: Clear selection
      event.preventDefault()
      clearSelection()
    }
  }

  // Track last selected row for shift+click range selection
  let lastSelectedRowId: string | null = null

  // Handle row click with modifiers - desktop app behavior
  function handleRowClick(event: MouseEvent, rowId: string) {
    const row = table.value.getRow(rowId)
    if (!row) {
      return
    }

    if (event.shiftKey && lastSelectedRowId) {
      // Shift+Click: Select range
      const lastRow = table.value.getRow(lastSelectedRowId)
      if (lastRow) {
        const start = Math.min(lastRow.index, row.index)
        const end = Math.max(lastRow.index, row.index)

        const newSelection: Record<string, boolean> = {}

        // If Cmd/Ctrl is also pressed, add to existing selection
        if (event.metaKey || event.ctrlKey) {
          // Start with current selection
          const currentSelection = table.value.getState().rowSelection
          Object.assign(newSelection, currentSelection)
        }

        // Select all rows in range
        const rows = table.value.getRowModel().rows
        for (let i = start; i <= end; i++) {
          if (rows[i]) {
            newSelection[rows[i]!.id] = true
          }
        }

        table.value.setRowSelection(newSelection)
      }
    }
    else if (event.metaKey || event.ctrlKey) {
      // Cmd/Ctrl+Click: Toggle single row
      row.toggleSelected()
    }
    else {
      // Regular click: Select only this row (desktop app behavior)
      table.value.resetRowSelection()
      row.toggleSelected(true)
    }

    // Remember last selected row for shift+click
    lastSelectedRowId = rowId
  }

  return {
    selectedCount,
    selectedItems,
    clearSelection,
    handleKeyboardShortcuts,
    handleRowClick,
  }
}