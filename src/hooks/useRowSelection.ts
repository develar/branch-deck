import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, MouseEvent } from "react"

export interface RowSelectionOptions<T> {
  items: readonly T[]
  /** Must be a pure function of the item (it is not a dependency of the memoized values) */
  getId: (item: T) => string
  /** Called with the selected items (in item order) whenever the selection or the items change; not called on mount */
  onSelectionChange?: (selectedItems: T[]) => void
}

export interface RowSelection<T> {
  /** Raw selected ids; may contain ids of items that no longer exist (they are ignored by the other members) */
  selectedIds: ReadonlySet<string>
  /** Selected items in item order */
  selectedItems: T[]
  selectedCount: number
  isSelected: (id: string) => boolean
  clearSelection: () => void
  /** Desktop-style click: plain = select only this row, Cmd/Ctrl = toggle, Shift = range from the last clicked row (Cmd/Ctrl+Shift adds the range) */
  handleRowClick: (event: MouseEvent, id: string, index: number) => void
  /** Cmd/Ctrl+A toggles select all, Escape clears a non-empty selection */
  handleKeyDown: (event: KeyboardEvent) => void
  /** Id of the last clicked row (the Shift+click anchor), or null before the first click */
  getAnchorId: () => string | null
}

const emptySelection: ReadonlySet<string> = new Set()

/** Multi-row selection state for a list (replaces the TanStack Table row selection feature). */
export function useRowSelection<T>({ items, getId, onSelectionChange }: RowSelectionOptions<T>): RowSelection<T> {
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(emptySelection)
  // anchor for Shift+click range selection
  const lastClickedIdRef = useRef<string | null>(null)

  const itemsRef = useRef(items)
  const getIdRef = useRef(getId)
  const onSelectionChangeRef = useRef(onSelectionChange)
  useEffect(() => {
    itemsRef.current = items
    getIdRef.current = getId
    onSelectionChangeRef.current = onSelectionChange
  })

  const selectedItems = useMemo(
    () => selectedIds.size === 0 ? [] : items.filter(item => selectedIds.has(getId(item))),
    // `getId` is a pure function of the item
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [items, selectedIds],
  )

  // change callback: not called for the initial value
  const isFirstRef = useRef(true)
  useEffect(() => {
    if (isFirstRef.current) {
      isFirstRef.current = false
      return
    }
    onSelectionChangeRef.current?.(selectedItems)
  }, [selectedItems])

  const clearSelection = useCallback(() => {
    setSelectedIds(emptySelection)
  }, [])

  const handleRowClick = useCallback((event: MouseEvent, id: string, index: number) => {
    const isToggle = event.metaKey || event.ctrlKey
    const anchorId = lastClickedIdRef.current
    lastClickedIdRef.current = id

    if (event.shiftKey && anchorId !== null) {
      const currentItems = itemsRef.current
      const anchorIndex = currentItems.findIndex(item => getIdRef.current(item) === anchorId)
      if (anchorIndex === -1) {
        return
      }
      const start = Math.min(anchorIndex, index)
      const end = Math.max(anchorIndex, index)
      setSelectedIds((previous) => {
        const next = new Set(isToggle ? previous : emptySelection)
        for (let i = start; i <= end; i++) {
          const item = currentItems[i]
          if (item !== undefined) {
            next.add(getIdRef.current(item))
          }
        }
        return next
      })
    }
    else if (isToggle) {
      setSelectedIds((previous) => {
        const next = new Set(previous)
        if (!next.delete(id)) {
          next.add(id)
        }
        return next
      })
    }
    else {
      setSelectedIds(new Set([id]))
    }
  }, [])

  const selectedCount = selectedItems.length
  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "a") {
      event.preventDefault()
      const currentItems = itemsRef.current
      const allSelected = currentItems.length > 0 && selectedCount === currentItems.length
      setSelectedIds(allSelected ? emptySelection : new Set(currentItems.map(item => getIdRef.current(item))))
    }
    else if (event.key === "Escape" && selectedCount > 0) {
      event.preventDefault()
      setSelectedIds(emptySelection)
    }
  }, [selectedCount])

  const isSelected = useCallback((id: string) => selectedIds.has(id), [selectedIds])
  const getAnchorId = useCallback(() => lastClickedIdRef.current, [])

  return {
    selectedIds,
    selectedItems,
    selectedCount,
    isSelected,
    clearSelection,
    handleRowClick,
    handleKeyDown,
    getAnchorId,
  }
}
