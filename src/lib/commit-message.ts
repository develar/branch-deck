/** Prose: hard-wrapped lines joined back into one line. */
export interface CommitBodyParagraph {
  type: "paragraph"
  text: string
}

/** Bullet (`- `, `* `, `• `) or numbered (`1. `, `1) `) list; items are reflowed and have no marker. */
export interface CommitBodyList {
  type: "list"
  ordered: boolean
  /** Number of the first item of an ordered list. */
  start: number
  items: string[]
}

/** Code or other text whose line breaks and spacing matter, kept verbatim without the common indent. */
export interface CommitBodyPreformatted {
  type: "preformatted"
  text: string
}

/** `Key: value` lines at the end of the message (`Co-Authored-By:`, `Signed-off-by:`, `Fixes:`…). */
export interface CommitBodyTrailers {
  type: "trailers"
  lines: string[]
}

export type CommitBodyBlock = CommitBodyParagraph | CommitBodyList | CommitBodyPreformatted | CommitBodyTrailers

const FENCE = /^\s*```/
const UNORDERED_ITEM = /^\s*[-*•]\s+(.*)$/
const ORDERED_ITEM = /^\s*(\d+)[.)]\s+(.*)$/
// git's trailer syntax: a token of letters, digits and hyphens, a colon, whitespace, a value
const TRAILER = /^[A-Za-z0-9][A-Za-z0-9-]*:\s+\S/
const CHERRY_PICKED = /^\(cherry picked from commit [0-9a-f]+\)$/

interface ListItemMatch {
  ordered: boolean
  number: number
  text: string
}

/**
 * Splits a commit message body (the message without its subject line) into blocks for display.
 *
 * The heuristics follow common git conventions and stay deliberately simple:
 * - Blocks are separated by blank lines. A ```` ``` ```` fence starts a preformatted block that runs to the closing
 *   fence (or the end of the body), blank lines included; the fences are dropped.
 * - A final block whose lines are all git trailers (`Token: value`, the token without spaces, or
 *   `(cherry picked from commit …)`) becomes a `trailers` block; indented lines continue the previous trailer.
 * - Inside a block, a run of lines indented by 4+ spaces or a tab is `preformatted`; non-indented lines are a
 *   `paragraph` whose lines are joined with single spaces, undoing git's hard wrapping.
 * - A line starting with `- `, `* `, `• `, `1. ` or `1) ` starts a `list`. Every following line of the block
 *   either starts a new item or continues the previous one (joined with a space), so wrapped and indented
 *   continuation lines stay in their item. Nested items are flattened; a switch between bullets and numbers
 *   starts a new list.
 */
export function parseCommitBody(body: string): CommitBodyBlock[] {
  const rawBlocks = splitRawBlocks(body.replaceAll("\r\n", "\n").split("\n"))

  const result: CommitBodyBlock[] = []
  for (const [index, rawBlock] of rawBlocks.entries()) {
    if (rawBlock.fenced) {
      result.push({ type: "preformatted", text: dedent(rawBlock.lines).join("\n") })
      continue
    }

    const trailers = index === rawBlocks.length - 1 ? parseTrailers(rawBlock.lines) : null
    if (trailers == null) {
      parseBlock(rawBlock.lines, result)
    }
    else {
      result.push({ type: "trailers", lines: trailers })
    }
  }
  return result
}

interface RawBlock {
  fenced: boolean
  lines: string[]
}

function splitRawBlocks(lines: string[]): RawBlock[] {
  const blocks: RawBlock[] = []
  let current: string[] = []
  const flush = () => {
    if (current.length > 0) {
      blocks.push({ fenced: false, lines: current })
      current = []
    }
  }

  for (let index = 0; index < lines.length; index++) {
    const line = lines[index]!
    if (FENCE.test(line)) {
      flush()
      const fenced: string[] = []
      index++
      while (index < lines.length && !FENCE.test(lines[index]!)) {
        fenced.push(lines[index]!)
        index++
      }
      blocks.push({ fenced: true, lines: trimBlankEdges(fenced) })
    }
    else if (line.trim() === "") {
      flush()
    }
    else {
      current.push(line.trimEnd())
    }
  }
  flush()
  return blocks
}

function parseTrailers(lines: string[]): string[] | null {
  if (!isTrailer(lines[0]!)) {
    return null
  }

  const trailers: string[] = []
  for (const line of lines) {
    if (isTrailer(line)) {
      trailers.push(line)
    }
    else if (/^\s/.test(line)) {
      trailers[trailers.length - 1] += ` ${line.trim()}`
    }
    else {
      return null
    }
  }
  return trailers
}

function isTrailer(line: string): boolean {
  return TRAILER.test(line) || CHERRY_PICKED.test(line)
}

function parseBlock(lines: string[], result: CommitBodyBlock[]) {
  let paragraph: string[] = []
  let preformatted: string[] = []
  let list: CommitBodyList | null = null

  const flushParagraph = () => {
    if (paragraph.length > 0) {
      result.push({ type: "paragraph", text: paragraph.join(" ") })
      paragraph = []
    }
  }
  const flushPreformatted = () => {
    if (preformatted.length > 0) {
      result.push({ type: "preformatted", text: dedent(preformatted).join("\n") })
      preformatted = []
    }
  }

  for (const line of lines) {
    const item = matchListItem(line)
    if (item != null) {
      flushParagraph()
      flushPreformatted()
      if (list == null || list.ordered !== item.ordered) {
        list = { type: "list", ordered: item.ordered, start: item.number, items: [] }
        result.push(list)
      }
      list.items.push(item.text)
    }
    else if (list != null) {
      list.items[list.items.length - 1] += ` ${line.trim()}`
    }
    else if (isCodeIndented(line)) {
      flushParagraph()
      preformatted.push(line)
    }
    else {
      flushPreformatted()
      paragraph.push(line.trim())
    }
  }
  flushParagraph()
  flushPreformatted()
}

function matchListItem(line: string): ListItemMatch | null {
  const unordered = UNORDERED_ITEM.exec(line)
  if (unordered != null) {
    return { ordered: false, number: 1, text: unordered[1]! }
  }
  const ordered = ORDERED_ITEM.exec(line)
  if (ordered != null) {
    return { ordered: true, number: Number(ordered[1]), text: ordered[2]! }
  }
  return null
}

function isCodeIndented(line: string): boolean {
  return line.startsWith("    ") || line.startsWith("\t")
}

/** Removes the indentation shared by all non-blank lines. */
function dedent(lines: string[]): string[] {
  let common = Infinity
  for (const line of lines) {
    if (line.trim() !== "") {
      common = Math.min(common, line.length - line.trimStart().length)
    }
  }
  return common === Infinity || common === 0 ? lines : lines.map(line => line.slice(common))
}

function trimBlankEdges(lines: string[]): string[] {
  let start = 0
  let end = lines.length
  while (start < end && lines[start]!.trim() === "") {
    start++
  }
  while (end > start && lines[end - 1]!.trim() === "") {
    end--
  }
  return lines.slice(start, end)
}
