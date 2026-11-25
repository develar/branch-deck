/**
 * The geometry of the picture of the landing plan (openspec/specs/branch/landing-plan/spec.md, "Picture"): the waves Rust
 * decides as columns from left to right, each unit a cell of one grid, so a pair offered to land together stands on
 * one row and its frame, its price and its buttons span exactly the two; and the curve of an edge between two cards.
 * Only the places: what a wave, an edge or an offer is comes from the plan.
 */

/** Where a unit stands: its column (its wave) and its row of the grid, both from 0 */
export interface PlanCell {
  readonly column: number
  readonly row: number
}

/**
 * The frame of an offer to land two units together: the row of its cards, its price and buttons on the row below,
 * across `columnStart` to `columnEnd`; of a pair whose unit it needs stands elsewhere already (it is needed by two), the
 * column of the unit that waits alone
 */
export interface PlanFrame {
  /** The index of the offer in the pairs given */
  readonly pair: number
  readonly columnStart: number
  readonly columnEnd: number
  readonly row: number
}

export interface PlanLayout {
  readonly columns: number
  readonly rows: number
  readonly cells: ReadonlyMap<string, PlanCell>
  readonly frames: readonly PlanFrame[]
}

/** An offer to land the unit `from` together with the unit `onto` it needs */
export interface PlanPair {
  readonly from: string
  readonly onto: string
}

/**
 * Lays the units of `waves` (each the units of a column, in their order) out on one grid, a chain of needs on one row
 * where it can, as the lanes of the stack graph keep a chain on one lane: column by column each unit not laid yet takes
 * the first free row from that of the first unit it needs (`needs`), and the units that wait for it follow on its row in
 * their columns while that row is free up to them, the cells between kept free for the curve, so a curve runs straight
 * and crosses no card. An offer to land together (`pairs`) goes first among the units that wait, and takes the row below
 * across the columns of its frame for its price and buttons; a unit some pair needs takes a row where the whole frame
 * fits. A unit that waits whose row is taken stands where its column has room, its curve slanting.
 */
export function layoutPlan(waves: readonly (readonly string[])[], pairs: readonly PlanPair[], needs: (unit: string) => readonly string[]): PlanLayout {
  const columnOf = new Map<string, number>()
  waves.forEach((units, column) => units.forEach(unit => columnOf.set(unit, column)))
  const cells = new Map<string, PlanCell>()
  const taken = new Set<string>()
  const frames: PlanFrame[] = []
  let rows = 0
  const take = (column: number, row: number) => {
    taken.add(`${column}:${row}`)
    rows = Math.max(rows, row + 1)
  }
  const isFree = (column: number, row: number) => !taken.has(`${column}:${row}`)
  const range = (start: number, end: number) => Array.from({ length: Math.max(0, end - start + 1) }, (_, index) => start + index)
  // the first row from `start` where `columns` are free for `height` rows
  const firstFree = (columns: readonly number[], start: number, height: number) => {
    let row = start
    while (columns.some(column => range(row, row + height - 1).some(at => !isFree(column, at)))) {
      row++
    }
    return row
  }
  // the first unit `unit` needs that stands in a column to its left: the one whose row it follows
  const anchorOf = (unit: string) => needs(unit).find(needed => (columnOf.get(needed) ?? Number.POSITIVE_INFINITY) < columnOf.get(unit)!)
  const ordered = waves.flat()
  const waitersOf = (unit: string) => {
    const paired = pairs.flatMap(pair => (pair.onto === unit && columnOf.has(pair.from) ? [pair.from] : []))
    return [...paired, ...ordered.filter(waiter => !paired.includes(waiter) && anchorOf(waiter) === unit)]
  }
  const pairOf = (from: string, onto: string) => pairs.findIndex(pair => pair.from === from && pair.onto === onto)
  const place = (unit: string, column: number, row: number) => {
    cells.set(unit, { column, row })
    take(column, row)
  }
  const frame = (pair: number, columnStart: number, columnEnd: number, row: number) => {
    range(columnStart, columnEnd).forEach(column => take(column, row + 1))
    frames.push({ pair, columnStart, columnEnd, row })
  }
  // the units that wait for `unit` on its row, while the row is free up to each
  const follow = (unit: string) => {
    const { column, row } = cells.get(unit)!
    for (const waiter of waitersOf(unit)) {
      const end = columnOf.get(waiter)!
      if (cells.has(waiter) || end <= column) {
        continue
      }
      const pair = pairOf(waiter, unit)
      const between = range(column + 1, end)
      const fits = between.every(at => isFree(at, row)) && (pair === -1 || range(column, end).every(at => isFree(at, row + 1)))
      if (!fits) {
        continue
      }
      between.forEach(at => take(at, row))
      place(waiter, end, row)
      if (pair !== -1) {
        frame(pair, column, end, row)
      }
      follow(waiter)
    }
  }
  const preferred = (unit: string) => {
    const rowsOfNeeded = needs(unit).flatMap(needed => {
      const cell = cells.get(needed)
      return cell === undefined ? [] : [cell.row]
    })
    return rowsOfNeeded.length === 0 ? 0 : Math.min(...rowsOfNeeded)
  }
  waves.forEach((units, column) => {
    for (const unit of units) {
      if (cells.has(unit)) {
        continue
      }
      // a unit a pair needs takes a row where the frame of the pair fits; one that waits of a pair whose needed unit
      // stands elsewhere the frame of its column alone
      const paired = pairs.find(pair => pair.onto === unit && !cells.has(pair.from) && (columnOf.get(pair.from) ?? -1) > column)
      const own = pairs.findIndex(pair => pair.from === unit)
      const columns = paired === undefined ? [column] : range(column, columnOf.get(paired.from)!)
      const row = firstFree(columns, preferred(unit), paired !== undefined || own !== -1 ? 2 : 1)
      place(unit, column, row)
      if (paired === undefined && own !== -1) {
        frame(own, column, column, row)
      }
      follow(unit)
    }
  })
  return { columns: waves.length, rows, cells, frames }
}

/** A box of a card in the coordinates of the picture */
export interface PlanBox {
  readonly left: number
  readonly top: number
  readonly width: number
  readonly height: number
}

/** The curve of an edge: its SVG path and the point halfway along it, where its chip stands */
export interface PlanCurve {
  readonly path: string
  readonly middle: { readonly x: number; readonly y: number }
}

/**
 * The curve from the card of the unit an edge needs (`needed`, a wave to the left) to the card of the unit that waits
 * (`waits`): from the middle of the right side of the first to the middle of the left side of the other, a cubic whose
 * handles run level; when the unit that waits stands in the same column or to the left (a cycle of needs), the curve
 * leaves and enters on the right sides, bowing out
 */
export function edgeCurve(needed: PlanBox, waits: PlanBox): PlanCurve {
  const startX = needed.left + needed.width
  const startY = needed.top + needed.height / 2
  const forward = waits.left > startX + 8
  const endX = forward ? waits.left : waits.left + waits.width
  const endY = waits.top + waits.height / 2
  const reach = forward ? Math.max(24, (endX - startX) / 2) : 40
  const control1 = { x: startX + reach, y: startY }
  const control2 = forward ? { x: endX - reach, y: endY } : { x: Math.max(startX, endX) + reach, y: endY }
  const round = (value: number) => Math.round(value * 10) / 10
  const path = `M${round(startX)} ${round(startY)}C${round(control1.x)} ${round(control1.y)} ${round(control2.x)} ${round(control2.y)} ${round(endX)} ${round(endY)}`
  // the point of the cubic at its half: (P0 + 3·P1 + 3·P2 + P3) / 8
  const middle = { x: round((startX + 3 * control1.x + 3 * control2.x + endX) / 8), y: round((startY + 3 * control1.y + 3 * control2.y + endY) / 8) }
  return { path, middle }
}
