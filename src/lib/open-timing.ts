/** A phase of the open of a sub-window, in the order it usually comes */
export type OpenPhase = "loaded" | "shell" | "files" | "diffs" | "painted" | "ready"

interface OpenTiming {
  readonly id: number
  readonly openedAt: number
  readonly phases: Map<OpenPhase, number>
}

let current: OpenTiming | null = null

/**
 * Records when `phase` of the open `open` (`__SUB_WINDOW_OPEN__`) came, in ms after the opener asked; the first time
 * of a phase counts. A new open starts over.
 */
export function markOpenPhase(open: { id: number; openedAt: number | null } | undefined, phase: OpenPhase, now = Date.now()): void {
  if (open === undefined || open.openedAt === null) {
    return
  }
  if (current?.id !== open.id) {
    current = { id: open.id, openedAt: open.openedAt, phases: new Map() }
  }
  if (!current.phases.has(phase)) {
    current.phases.set(phase, Math.max(0, Math.round(now - current.openedAt)))
  }
}

/**
 * The timing line of the open `id`, "commit-review open 96 ms: loaded 41, files 44, diffs 47, ready 96", once: null
 * for another open or when it was told
 */
export function takeOpenTimingLine(label: string, id: number): string | null {
  if (current?.id !== id) {
    return null
  }
  const timing = current
  current = null
  const ready = timing.phases.get("ready") ?? 0
  const phases = [...timing.phases].filter(([phase]) => phase !== "ready").map(([phase, ms]) => `${phase} ${ms}`)
  return `${label} open ${ready} ms${phases.length === 0 ? "" : `: ${phases.join(", ")}`}`
}
