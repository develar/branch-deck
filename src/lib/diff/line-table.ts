/*
 * The lines of a text without an array of line strings (docs/design/diff-view.md, "Input"): the offsets where each line
 * starts, kept once per blob id. A line is sliced from the text only when it is painted.
 */

export interface LineTable {
  readonly text: string
  /** Where line i starts; `starts[lineCount]` is the length of the text */
  readonly starts: Uint32Array
  /** One more than the line ends, as the engine counts: the empty line after a final line end is a line */
  readonly lineCount: number
  /** The text ends with a line end, or is empty: no marker after its last line */
  readonly endsWithLineEnd: boolean
}

/**
 * The lines as IntelliJ splits them: `\n`, `\r\n` and a lone `\r` each end a line. The line indices are the ones of
 * the engine, so a text that ends with a line end, and an empty text, end with an empty line.
 */
export function buildLineTable(text: string): LineTable {
  const starts: number[] = [0]
  for (let index = 0; index < text.length; index++) {
    const char = text.charCodeAt(index)
    if (char === 10 || char === 13) {
      if (char === 13 && text.charCodeAt(index + 1) === 10) {
        index++
      }
      starts.push(index + 1)
    }
  }
  const endsWithLineEnd = starts.at(-1) === text.length
  starts.push(text.length)
  return { text, starts: Uint32Array.from(starts), lineCount: starts.length - 1, endsWithLineEnd }
}

/** The text of line `index` without its line end */
export function lineText(table: LineTable, index: number): string {
  const start = table.starts[index]!
  let end = table.starts[index + 1]!
  // a line holds no \r or \n, so the ones before the next start are its line end
  if (end > start && table.text.charCodeAt(end - 1) === 10) {
    end--
  }
  if (end > start && table.text.charCodeAt(end - 1) === 13) {
    end--
  }
  return table.text.slice(start, end)
}

const widest = new WeakMap<LineTable, number>()

/** The columns of the widest line, a tab to its next stop: the width of the horizontal scroll */
export function maxColumns(table: LineTable, tabSize: number): number {
  let result = widest.get(table)
  if (result === undefined) {
    result = 0
    for (let line = 0; line < table.lineCount; line++) {
      let columns = 0
      const end = table.starts[line + 1]!
      for (let index = table.starts[line]!; index < end; index++) {
        const code = table.text.charCodeAt(index)
        if (code === 9) {
          columns += tabSize - (columns % tabSize)
        } else if (code !== 10 && code !== 13) {
          columns++
        }
      }
      result = Math.max(result, columns)
    }
    widest.set(table, result)
  }
  return result
}

const tables = new Map<string, LineTable>()
/** The tables kept by blob id; a table costs 4 bytes per line beside its text, which the blob texts store holds */
const MAX_TABLES = 512

/** The line table of a blob's text, built once per blob id while it is among the last used */
export function getLineTable(blobId: string, text: string): LineTable {
  let table = tables.get(blobId)
  if (table === undefined || table.text !== text) {
    table = buildLineTable(text)
    if (tables.size >= MAX_TABLES) {
      tables.delete(tables.keys().next().value!)
    }
  } else {
    tables.delete(blobId)
  }
  tables.set(blobId, table)
  return table
}
