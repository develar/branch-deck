import type { DiffLayout } from "@/lib/diff/layout"

/*
 * The map of the files beside the diffs of a review (docs/design/diff-view.md, "The map of files"): the document scaled
 * to the height of the map, a tick at the start of each section, across the map at the start of a step of the plan, the
 * prologue above the first a band of its own. The ticks come from the layout: nothing is measured for them.
 */

/** The index of the band of the prologue on the map, above the first section: no section has it */
export const PROLOGUE_SECTION = -1

/** What the map shows of a section of the view */
export interface FileMapSection {
  /** It starts a step of the review plan: its tick crosses the map */
  step: boolean
  /** Of another owner than the chosen chip's: its tick paler */
  muted: boolean
}

export interface FileMapTick {
  /** The top of the tick in device pixels from the top of the map */
  y: number
  /** Its height in device pixels */
  height: number
  kind: "file" | "step"
  /** Every section it stands for is muted */
  muted: boolean
}

/** The height of the tick of a file and of a step, in CSS px */
const FILE_TICK = 1
const STEP_TICK = 2

/**
 * The pixels of the map per pixel of the document: the document as tall as the scroller's `scrollHeight`, never shorter
 * than the view, so a short review keeps its sections where the cards are; 0 while either has no height
 */
export function fileMapScale(mapHeight: number, documentHeight: number, viewHeight: number): number {
  const scrolled = Math.max(documentHeight, viewHeight)
  return mapHeight <= 0 || scrolled <= 0 ? 0 : mapHeight / scrolled
}

/**
 * The thumb of the map for the window of the view, `top` and `height` in px of the map: at least `min`, a thicker one
 * centred on the window, all of it inside the map
 */
export function fileMapThumb(windowTop: number, windowHeight: number, mapHeight: number, min: number): { top: number; height: number } {
  const height = Math.min(Math.max(windowHeight, min), mapHeight)
  const top = windowTop + (windowHeight - height) / 2
  return { top: Math.min(Math.max(top, 0), mapHeight - height), height }
}

/**
 * The ticks of the sections at `scale` device pixels per pixel of the document, `ratio` device pixels per CSS px: a tick
 * of 1 px at the start of each section, of 2 px at the start of a step. Where the pixels are too few for every section,
 * the ticks that overlap are one: a step when any of them is, muted only when all are
 */
export function fileMapTicks(layout: DiffLayout, sections: readonly FileMapSection[], scale: number, ratio: number): FileMapTick[] {
  const fileHeight = Math.max(Math.round(FILE_TICK * ratio), 1)
  const stepHeight = Math.max(Math.round(STEP_TICK * ratio), 2)
  const ticks: FileMapTick[] = []
  sections.forEach((section, index) => {
    const y = Math.round(layout.sectionStart(index) * scale)
    const height = section.step ? stepHeight : fileHeight
    const last = ticks.at(-1)
    if (last !== undefined && y < last.y + last.height) {
      last.height = Math.max(last.y + last.height, y + height) - last.y
      last.kind = last.kind === "step" || section.step ? "step" : "file"
      last.muted = last.muted && section.muted
      return
    }
    ticks.push({ y, height, kind: section.step ? "step" : "file", muted: section.muted })
  })
  return ticks
}

/**
 * The section of the map at `y` px of the document, the one its hover names and a click shows: `PROLOGUE_SECTION` in
 * the band of the prologue (down to the start of the first section), else the section `fileAt` counts; null outside
 * the document, and in a document of nothing
 */
export function fileMapSectionAt(layout: DiffLayout, y: number): number | null {
  if (y < 0 || y > layout.height || (layout.files.length === 0 && layout.prologue === 0)) {
    return null
  }
  return layout.partAt(y) === "prologue" ? PROLOGUE_SECTION : layout.fileAt(y)
}

/**
 * The band the map fills for the section at the top, `start` and `end` in px of the document: of the prologue from the
 * top of the document to the first section, of a section from its start to the end of its card; null for none
 */
export function fileMapBand(layout: DiffLayout, index: number): { start: number; end: number } | null {
  if (index === PROLOGUE_SECTION) {
    return layout.prologue > 0 ? { start: 0, end: layout.files.length === 0 ? layout.height : layout.sectionStart(0) } : null
  }
  if (index < 0 || index >= layout.files.length) {
    return null
  }
  return { start: layout.sectionStart(index), end: layout.offsets[index]! + layout.fileHeight(index) }
}
