/** The selection of a list of rows by their ids, with the two ends of a desktop list (Swing's `ListSelectionModel`) */
export interface SelectionState {
  ids: ReadonlySet<string>
  /** The fixed end of a Shift+click range: the row of the last plain or Cmd/Ctrl click */
  anchor: string | null
  /** The row clicked last; null after a selection made by its ids (`selectIds`) */
  lead: string | null
}

export const emptySelection: ReadonlySet<string> = new Set()

export const NO_SELECTION: SelectionState = { ids: emptySelection, anchor: null, lead: null }

export interface RowClick {
  id: string
  index: number
  /** Cmd/Ctrl: toggles the row, or adds the range to the selection */
  toggle: boolean
  /** Shift: the range from the anchor */
  range: boolean
}

/**
 * The selection after a click of a row of `rowIds`: plain selects the row alone, Cmd/Ctrl toggles it, Shift selects the
 * range from the anchor (Cmd/Ctrl+Shift adds it), the anchor staying where it was; every click makes the row the lead
 */
export function clickRow(previous: SelectionState, click: RowClick, rowIds: readonly string[]): SelectionState {
  const { id, index } = click
  const anchorIndex = previous.anchor === null ? -1 : rowIds.indexOf(previous.anchor)
  if (click.range && anchorIndex !== -1) {
    const next = new Set(click.toggle ? previous.ids : emptySelection)
    for (const rowId of rowIds.slice(Math.min(anchorIndex, index), Math.max(anchorIndex, index) + 1)) {
      next.add(rowId)
    }
    return { ids: next, anchor: previous.anchor, lead: id }
  }
  if (click.toggle) {
    const next = new Set(previous.ids)
    if (!next.delete(id)) {
      next.add(id)
    }
    return { ids: next.size === 0 ? emptySelection : next, anchor: id, lead: id }
  }
  return { ids: new Set([id]), anchor: id, lead: id }
}

/**
 * The selected row the selection's actions sit on: the lead if it is selected, else the selected row nearest to it (the
 * earlier of two as near: a Cmd/Ctrl click took the lead out), else the first selected (a selection by ids); null when no
 * row of `rowIds` is selected
 */
export function leadOf(state: SelectionState, rowIds: readonly string[]): string | null {
  const leadIndex = state.lead === null ? -1 : rowIds.indexOf(state.lead)
  if (leadIndex !== -1) {
    for (let distance = 0; distance < rowIds.length; distance++) {
      const before = rowIds[leadIndex - distance]
      if (before !== undefined && state.ids.has(before)) {
        return before
      }
      const after = rowIds[leadIndex + distance]
      if (after !== undefined && state.ids.has(after)) {
        return after
      }
    }
    return null
  }
  return rowIds.find(id => state.ids.has(id)) ?? null
}
