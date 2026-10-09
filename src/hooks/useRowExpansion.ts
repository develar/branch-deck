import { useCallback, useMemo, useState } from "react"

export interface RowExpansion {
  expandedKeys: ReadonlySet<string>
  isExpanded: (key: string) => boolean
  toggleExpanded: (key: string) => void
  setExpanded: (key: string, expanded: boolean) => void
  collapseAll: () => void
  expandAll: (keys: Iterable<string>) => void
}

/** Expanded state of table rows, keyed by row id. All callbacks are stable. */
export function useRowExpansion(): RowExpansion {
  const [expandedKeys, setExpandedKeys] = useState<ReadonlySet<string>>(() => new Set())

  const setExpanded = useCallback((key: string, expanded: boolean) => {
    setExpandedKeys((current) => {
      if (current.has(key) === expanded) {
        return current
      }
      const next = new Set(current)
      if (expanded) {
        next.add(key)
      }
      else {
        next.delete(key)
      }
      return next
    })
  }, [])

  const toggleExpanded = useCallback((key: string) => {
    setExpandedKeys((current) => {
      const next = new Set(current)
      if (!next.delete(key)) {
        next.add(key)
      }
      return next
    })
  }, [])

  const collapseAll = useCallback(() => {
    setExpandedKeys(current => current.size === 0 ? current : new Set())
  }, [])

  const expandAll = useCallback((keys: Iterable<string>) => {
    setExpandedKeys(current => new Set([...current, ...keys]))
  }, [])

  return useMemo(() => ({
    expandedKeys,
    isExpanded: (key: string) => expandedKeys.has(key),
    toggleExpanded,
    setExpanded,
    collapseAll,
    expandAll,
  }), [expandedKeys, toggleExpanded, setExpanded, collapseAll, expandAll])
}
