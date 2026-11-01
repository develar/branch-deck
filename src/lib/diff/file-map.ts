import type { DiffLayout } from "@/lib/diff/layout"

/*
 * The map of the files beside the diffs of a review (docs/design/diff-view.md, "The map of files"): the document scaled
 * to the height of the map, a tick at the start of each section, across the map at the start of a step of the plan. The
 * layout is arithmetic, so the ticks are too: no section is measured.
 */

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

/** Where a section starts in the document for the map: its divider, else its card, as `DiffLayout.fileAt` counts it */
export function sectionStart(layout: DiffLayout, index: number): number {
  return layout.offsets[index]! - layout.dividerHeight(index)
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
    const y = Math.round(sectionStart(layout, index) * scale)
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
