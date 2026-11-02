import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, MouseEvent } from "react"
import { clickRow, emptySelection, leadOf, NO_SELECTION } from "@/lib/row-selection"
import type { SelectionState } from "@/lib/row-selection"

export interface RowSelectionOptions<T> {
  items: readonly T[]
  /** A pure function of the item, stable across renders (a module-level function): a new one recomputes the selected items */
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
  /**
   * The selected row the selection's actions sit on (the lead of a desktop list): the row clicked last if it is selected,
   * else the selected row nearest to it, else the first selected one (a selection made by `selectIds`); null without a
   * selection
   */
  leadId: string | null
  isSelected: (id: string) => boolean
  clearSelection: () => void
  /** Replaces the selection (e.g. with the commits just moved into the list); its first row leads it */
  selectIds: (ids: Iterable<string>) => void
  /**
   * Desktop-style click: plain = select only this row, Cmd/Ctrl = toggle, Shift = range from the anchor (the row of the
   * last plain or Cmd/Ctrl click), Cmd/Ctrl+Shift adds the range. Every click makes the row the lead.
   */
  handleRowClick: (event: MouseEvent, id: string, index: number) => void
  /** Cmd/Ctrl+A toggles select all, Escape clears a non-empty selection */
  handleKeyDown: (event: KeyboardEvent) => void
  /** Id of the row clicked last, selected or not, or null before the first click; for event handlers */
  getLastClickedId: () => string | null
}

/** Multi-row selection state for a list (replaces the TanStack Table row selection feature). */
export function useRowSelection<T>({ items, getId, onSelectionChange }: RowSelectionOptions<T>): RowSelection<T> {
  const [state, setState] = useState<SelectionState>(NO_SELECTION)
  const selectedIds = state.ids
  // the row clicked last, for the keys of a list that act on it while nothing is selected (Space previews it)
  const lastClickedIdRef = useRef<string | null>(null)

  const itemsRef = useRef(items)
  const getIdRef = useRef(getId)
  const onSelectionChangeRef = useRef(onSelectionChange)
  useEffect(() => {
    itemsRef.current = items
    getIdRef.current = getId
    onSelectionChangeRef.current = onSelectionChange
  })

  const selectedItems = useMemo(() => (selectedIds.size === 0 ? [] : items.filter(item => selectedIds.has(getId(item)))), [items, selectedIds, getId])
  const rowIds = useMemo(() => items.map(getId), [items, getId])
  const leadId = useMemo(() => leadOf(state, rowIds), [state, rowIds])

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
    setState(previous => (previous.ids === emptySelection ? previous : { ...previous, ids: emptySelection }))
  }, [])

  const selectIds = useCallback((ids: Iterable<string>) => {
    const next = new Set(ids)
    setState(previous => ({ ...previous, ids: next.size === 0 ? emptySelection : next, lead: null }))
  }, [])

  const handleRowClick = useCallback((event: MouseEvent, id: string, index: number) => {
    const isToggle = event.metaKey || event.ctrlKey
    const isRange = event.shiftKey
    lastClickedIdRef.current = id
    setState(previous => clickRow(previous, { id, index, toggle: isToggle, range: isRange }, itemsRef.current.map(getIdRef.current)))
  }, [])

  const selectedCount = selectedItems.length
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "a") {
        event.preventDefault()
        const currentItems = itemsRef.current
        const allSelected = currentItems.length > 0 && selectedCount === currentItems.length
        setState(previous => ({ ...previous, ids: allSelected ? emptySelection : new Set(currentItems.map(item => getIdRef.current(item))) }))
      } else if (event.key === "Escape" && selectedCount > 0) {
        event.preventDefault()
        clearSelection()
      }
    },
    [selectedCount, clearSelection],
  )

  const isSelected = useCallback((id: string) => selectedIds.has(id), [selectedIds])
  const getLastClickedId = useCallback(() => lastClickedIdRef.current, [])

  return {
    selectedIds,
    selectedItems,
    selectedCount,
    leadId,
    isSelected,
    clearSelection,
    selectIds,
    handleRowClick,
    handleKeyDown,
    getLastClickedId,
  }
}
