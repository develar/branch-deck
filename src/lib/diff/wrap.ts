/*
 * Our own soft wraps (docs/design/diff-view.md, "Soft wraps: our own breaks"): the view breaks a line itself and paints
 * each visual line as its own line box, so the height of a row is known before the paint and the DOM matches it.
 */

export interface TextMetrics {
  /** The width of one column of the monospace font, in CSS px */
  columnWidth: number
  /** The columns of a tab stop */
  tabSize: number
  /** The width of a code point outside printable ASCII, in CSS px: measured once by the font */
  widthOf: (codePoint: number) => number
}

/** A line of printable ASCII and tabs: its width is its column count */
const ASCII_LINE = /^[\t\x20-\x7e]*$/

/** Where a break may follow: after a space or a punctuation mark */
function breaksAfter(code: number): boolean {
  switch (code) {
    case 0x20:
    case 0x09:
    case 0x2c: // ,
    case 0x3b: // ;
    case 0x2e: // .
    case 0x29: // )
    case 0x5d: // ]
    case 0x7d: // }
    case 0x3e: // >
    case 0x2f: // /
    case 0x2d: // -
      return true
    default:
      return false
  }
}

function leadingWhitespace(text: string): number {
  let index = 0
  while (index < text.length && (text.charCodeAt(index) === 0x20 || text.charCodeAt(index) === 0x09)) {
    index++
  }
  return index
}

/**
 * The offsets in `text` where its visual lines after the first start, for a box of `width` CSS px. The break is greedy:
 * after the last space or punctuation mark that fits, else at the edge. A wrapped part starts at the indent of its
 * line, at most half the width, as IntelliJ's soft wraps do. Empty: the line fits.
 */
export function wrapBreaks(text: string, width: number, metrics: TextMetrics): number[] {
  const { columnWidth, tabSize, widthOf } = metrics
  const ascii = ASCII_LINE.test(text)
  if (ascii && !text.includes("\t") && text.length * columnWidth <= width) {
    return []
  }
  const breaks: number[] = []
  // the width of the indent of every wrapped part: the leading whitespace of the line, at most half the box
  const indentLength = leadingWhitespace(text)
  let indent = 0
  let x = 0
  // the columns from the start of the line, for the tab stops
  let column = 0
  let lastBreak = -1
  let xAtLastBreak = 0
  let lineStart = 0
  for (let index = 0; index < text.length;) {
    const code = text.codePointAt(index)!
    const length = code > 0xffff ? 2 : 1
    let charWidth: number
    let columns = 1
    if (code === 0x09) {
      columns = tabSize - (column % tabSize)
      charWidth = columns * columnWidth
    } else {
      charWidth = code >= 0x20 && code <= 0x7e ? columnWidth : widthOf(code)
    }
    if (x + charWidth > width && index > lineStart && x > indent) {
      if (lastBreak > lineStart) {
        breaks.push(lastBreak)
        lineStart = lastBreak
        x = indent + (x - xAtLastBreak)
      } else {
        breaks.push(index)
        lineStart = index
        x = indent
      }
      lastBreak = -1
    }
    x += charWidth
    column += columns
    index += length
    if (index === indentLength) {
      indent = x <= width / 2 ? x : 0
    }
    if (breaksAfter(code) && index > indentLength) {
      lastBreak = index
      xAtLastBreak = x
    }
  }
  return breaks
}

/** The visual lines of a line in a box of `width` CSS px */
export function visualLineCount(text: string, width: number, metrics: TextMetrics): number {
  return wrapBreaks(text, width, metrics).length + 1
}
