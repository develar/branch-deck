import type { TextMetrics } from "@/lib/diff/wrap"

/*
 * The metrics of the editor font in the diffs (docs/design/diff-view.md, "The row height"): the row height from the
 * ascent and the descent of the font, on the pixel grid of the zoom; the column width for the soft wraps and the
 * horizontal scroll.
 */

/** IntelliJ's default line spacing of the editor */
export const LINE_SPACING = 1.2
/** The font size of the code of a diff, in CSS px */
export const DIFF_FONT_SIZE = 13
export const DIFF_TAB_SIZE = 4

export interface FontMetrics {
  /** The ascent and the descent of the font, in CSS px */
  ascent: number
  descent: number
  /** The width of one column, in CSS px */
  columnWidth: number
}

/**
 * The height of a row in CSS px: the ascent and the descent of the font times the line spacing, rounded to whole
 * device pixels at `scale` (`devicePixelRatio`, which holds the zoom), so every row starts on the pixel grid.
 */
export function rowHeightOf({ ascent, descent }: FontMetrics, scale: number): number {
  const devicePixels = Math.max(1, Math.round((ascent + descent) * LINE_SPACING * scale))
  return devicePixels / scale
}

/** Measures the font of the diffs with a canvas: `font` is a CSS font shorthand, such as `13px "JetBrains Mono"` */
export function measureFont(font: string): FontMetrics & { context: CanvasRenderingContext2D } {
  const context = document.createElement("canvas").getContext("2d")!
  context.font = font
  const sample = context.measureText("0".repeat(100))
  return {
    ascent: sample.fontBoundingBoxAscent,
    descent: sample.fontBoundingBoxDescent,
    columnWidth: sample.width / 100,
    context,
  }
}

/** The metrics of the soft wraps for a measured font: each code point outside ASCII measured once */
export function textMetricsOf(
  font: FontMetrics & { context: CanvasRenderingContext2D },
  tabSize = DIFF_TAB_SIZE,
): TextMetrics & { correct: (codePoint: number, width: number) => void } {
  const widths = new Map<number, number>()
  return {
    columnWidth: font.columnWidth,
    tabSize,
    widthOf: codePoint => {
      let width = widths.get(codePoint)
      if (width === undefined) {
        width = font.context.measureText(String.fromCodePoint(codePoint)).width
        widths.set(codePoint, width)
      }
      return width
    },
    // the width the DOM gave a code point, when it differs from the canvas (docs/design/diff-view.md, "When the
    // estimate is wrong")
    correct: (codePoint, width) => {
      widths.set(codePoint, width)
    },
  }
}
