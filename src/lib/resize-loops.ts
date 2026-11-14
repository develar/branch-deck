/*
 * The policy of the ResizeObserver loops of the page, in one place (docs/contributing/frontend.md, "A flushSync in the
 * delivery of a ResizeObserver"): what src/lib/page-errors.ts logs of a loop the engine reports, and what
 * src/lib/observe-size.ts logs of a cycle no loop tells any more, as its observers let go an element resized in their
 * delivery. A module of constants and of the one attribution, with no effect of its own: page-errors.ts imports it before
 * every module of the page
 */

/**
 * A loop of the observers of a library (Base UI, the panels) is a warning, as its notifications come in the next frame;
 * this many within `LOOP_REPEAT_MS` are an error: a loop that repeats, however slowly, renders the window again and again
 */
export const LOOP_REPEATS = 3
export const LOOP_REPEAT_MS = 5000

/**
 * An element of ours resized in the delivery of its own observers, by a change in it, in this many frames in a row is a
 * cycle (its callback resizes it again): frames, not time, so a window that renders slowly (a loaded machine, a window in
 * the background) tells it as well
 */
export const CYCLE_FRAMES = 10

/** The elements of the observers of the page left resized since their report, at a loop the engine reports */
let pendingOfOurs: () => readonly string[] = () => []

/** The observers of the page say which of their elements a loop left pending (src/lib/observe-size.ts) */
export function attributeLoopsWith(pending: () => readonly string[]): void {
  pendingOfOurs = pending
}

/** At a loop the engine reports: the elements of ours it left pending, none when it is a library's alone */
export function loopOfOurs(): readonly string[] {
  return pendingOfOurs()
}
