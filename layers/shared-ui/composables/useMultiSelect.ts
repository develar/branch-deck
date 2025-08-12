import type { Ref } from "vue"
import { useKeyModifier } from "@vueuse/core"

export interface UseMultiSelectOptions {
  onSelectionChange?: (selected: string[]) => void
}

export function useMultiSelect<T>(
  items: Ref<T[]>,
  getKey: (item: T) => string,
  options: UseMultiSelectOptions = {},
) {
  const selected = ref<Set<string>>(new Set())
  const lastSelected = ref<string | null>(null)
  const hoveredItem = ref<string | null>(null)

  // Track keyboard modifiers
  const shiftKey = useKeyModifier("Shift")
  const ctrlKey = useKeyModifier("Control")
  const metaKey = useKeyModifier("Meta")

  // Computed for easier access
  const selectedArray = computed(() => Array.from(selected.value))
  const selectedCount = computed(() => selected.value.size)
  const hasSelection = computed(() => selected.value.size > 0)

  // Helper to find item index
  function findItemIndex(key: string): number {
    return items.value.findIndex(item => getKey(item) === key)
  }

  // Select range between two items
  function selectRange(fromKey: string, toKey: string) {
    const fromIndex = findItemIndex(fromKey)
    const toIndex = findItemIndex(toKey)

    if (fromIndex === -1 || toIndex === -1) {
      return
    }

    const start = Math.min(fromIndex, toIndex)
    const end = Math.max(fromIndex, toIndex)

    // Add all items in range to selection
    for (let i = start; i <= end; i++) {
      const item = items.value[i]
      if (item) {
        const key = getKey(item)
        selected.value.add(key)
      }
    }

    notifySelectionChange()
  }

  // Handle click on item
  function handleClick(event: MouseEvent, item: T) {
    const key = getKey(item)
    const isMultiSelect = event.metaKey || event.ctrlKey

    if (event.shiftKey && lastSelected.value) {
      // Shift+click: range selection
      if (!isMultiSelect) {
        // Clear selection if not holding Cmd/Ctrl
        selected.value.clear()
      }
      selectRange(lastSelected.value, key)
    }
    else if (isMultiSelect) {
      // Cmd/Ctrl+click: toggle selection
      if (selected.value.has(key)) {
        selected.value.delete(key)
      }
      else {
        selected.value.add(key)
      }
      notifySelectionChange()
    }
    else {
      // Regular click: single selection
      selected.value.clear()
      selected.value.add(key)
      notifySelectionChange()
    }

    lastSelected.value = key
  }

  // Toggle selection for an item
  function toggleSelection(key: string) {
    if (selected.value.has(key)) {
      selected.value.delete(key)
    }
    else {
      selected.value.add(key)
    }
    lastSelected.value = key
    notifySelectionChange()
  }

  // Select all items
  function selectAll() {
    selected.value.clear()
    items.value.forEach((item) => {
      selected.value.add(getKey(item))
    })
    notifySelectionChange()
  }

  // Clear selection
  function clearSelection() {
    selected.value.clear()
    lastSelected.value = null
    notifySelectionChange()
  }

  // Check if item is selected
  function isSelected(key: string): boolean {
    return selected.value.has(key)
  }

  // Set hover state
  function setHoveredItem(key: string | null) {
    hoveredItem.value = key
  }

  // Notify selection change
  function notifySelectionChange() {
    options.onSelectionChange?.(selectedArray.value)
  }

  // Handle keyboard shortcuts
  function handleKeyboard(event: KeyboardEvent) {
    // Cmd/Ctrl+A: Select all
    if ((event.metaKey || event.ctrlKey) && event.key === "a") {
      event.preventDefault()
      selectAll()
    }
    else if (event.key === "Escape" && hasSelection.value) {
      // Escape: Clear selection
      event.preventDefault()
      clearSelection()
    }
  }

  // Watch for items changes to clean up invalid selections
  watch(items, () => {
    const validKeys = new Set(items.value.map(item => getKey(item)))
    const toRemove: string[] = []

    selected.value.forEach((key) => {
      if (!validKeys.has(key)) {
        toRemove.push(key)
      }
    })

    if (toRemove.length > 0) {
      toRemove.forEach(key => selected.value.delete(key))
      notifySelectionChange()
    }
  })

  return {
    // State
    selected: selectedArray,
    selectedCount,
    hasSelection,
    hoveredItem: readonly(hoveredItem),

    // Modifiers
    shiftKey: readonly(shiftKey),
    ctrlKey: readonly(ctrlKey),
    metaKey: readonly(metaKey),

    // Actions
    handleClick,
    toggleSelection,
    selectAll,
    clearSelection,
    isSelected,
    setHoveredItem,
    handleKeyboard,
  }
}
