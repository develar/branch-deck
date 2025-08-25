/**
 * Generic composable for table row expansion
 * Works with any data type and doesn't depend on TanStack Table
 */
export function useGenericTableExpansion<T>(keyFn: (item: T) => string) {
  const expandedItems = ref(new Set<string>())

  const isExpanded = (item: T): boolean => {
    return expandedItems.value.has(keyFn(item))
  }

  const toggleExpanded = (item: T): void => {
    const key = keyFn(item)
    if (expandedItems.value.has(key)) {
      expandedItems.value.delete(key)
    }
    else {
      expandedItems.value.add(key)
    }
  }

  const setExpanded = (item: T, expanded: boolean): void => {
    const key = keyFn(item)
    if (expanded) {
      expandedItems.value.add(key)
    }
    else {
      expandedItems.value.delete(key)
    }
  }

  const collapseAll = (): void => {
    expandedItems.value.clear()
  }

  const expandAll = (items: T[]): void => {
    items.forEach(item => expandedItems.value.add(keyFn(item)))
  }

  const getExpandedCount = (): number => {
    return expandedItems.value.size
  }

  return {
    isExpanded,
    toggleExpanded,
    setExpanded,
    collapseAll,
    expandAll,
    getExpandedCount,
    expandedItems: readonly(expandedItems),
  }
}