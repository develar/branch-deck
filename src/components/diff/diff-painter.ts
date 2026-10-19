import { copyFile, copyText } from "@/lib/diff/copy-text"
import type { CopyFile, CopyPoint } from "@/lib/diff/copy-text"
import { lineKey } from "@/lib/diff/find"
import type { HighlightClient, BlobWant } from "@/lib/diff/highlight/client"
import { FONT_STYLE_SHIFT, PAGE_LINES, ROLE_MASK } from "@/lib/diff/highlight/protocol"
import type { Priority } from "@/lib/diff/highlight/protocol"
import type { DiffLayout } from "@/lib/diff/layout"
import { inlineParts, lineParts, runsOf } from "@/lib/diff/line-parts"
import type { InlinePart, LineHits, LinePart } from "@/lib/diff/line-parts"
import { getLineTable, lineText } from "@/lib/diff/line-table"
import type { LineTable } from "@/lib/diff/line-table"
import type { RowCells } from "@/lib/diff/row-cells"
import { NEW, OLD, rowAt } from "@/lib/diff/rows"
import type { FileRows, FoldRow, LineRow, LineTint } from "@/lib/diff/rows"
import { blobText } from "@/lib/diff/view-files"
import { wrapBreaks } from "@/lib/diff/wrap"
import type { TextMetrics } from "@/lib/diff/wrap"
import type { BlobTextEntry } from "@/stores/blob-texts"

/*
 * The painter of `DiffView` (docs/design/diff-view.md, "React for the sections, a painter for the rows"): a plain class
 * that fills the rows element of each file section with the rows in view and one viewport around them. It works in
 * the scroll event, never through React, and paints plain text until the tokens of a page arrive.
 */

export interface PainterFile {
  key: string
  /** The blob id of the text of each side; null for a side without a text (it is empty) */
  ids: readonly (string | null)[]
  /** How the cells of its rows draw; null for a file without lines */
  cells: RowCells | null
  /** The rows shown; null when the file is collapsed or shows none */
  rows: FileRows | null
  language: string
  /** The width of the code of each side, in CSS px, for the soft wraps */
  codeWidths: readonly number[]
}

export interface PainterModel {
  files: readonly PainterFile[]
  /** The hits of the find by file and line (`hitsByLine` of find.ts); none while the find is closed */
  hits?: ReadonlyMap<number, ReadonlyMap<string, LineHits>>
  layout: DiffLayout
  softWraps: boolean
  textMetrics: TextMetrics
}

export interface PainterCallbacks {
  /**
   * The files to mount, three viewports around the view: "flush" in a scroll event whose frame needs a section, "now"
   * when the paint needs one outside an event, "idle" for the sections ahead
   */
  onWindow: (first: number, last: number, when: "flush" | "now" | "idle") => void
  /** The file whose header is at the top, after a scroll of the user */
  onTopFile: (index: number) => void
  /** A click on a fold; with ⌥ (Alt) it asks for all the lines of the file */
  onFold: (file: number, fold: FoldRow, all: boolean) => void
  /** The rows of the first screen are painted with their texts; the tokens of their pages are there or overdue */
  onFirstScreen: () => void
  /** A wrapped line came out wider in the DOM than measured: the widths are corrected, the heights need a layout */
  onWidthsCorrected: () => void
}

interface PaintedRow {
  element: HTMLElement
  /** The blob and line of each code cell, to paint again when a page of tokens arrives */
  cells: { blobId: string; line: number }[]
  /** Every text of the file was read when the row was painted */
  textsRead: boolean
}

/** How long the first screen waits for its tokens before the window shows it plain */
const FIRST_SCREEN_TOKENS_MS = 80

let lastViewId = 0

/** The id of a view in the requests of the highlight worker: a new request of a view replaces its old work */
function newViewId(): string {
  lastViewId += 1
  return `diff-view-${lastViewId}`
}

function element(tag: string, className: string): HTMLElement {
  const created = document.createElement(tag)
  created.className = className
  return created
}

function partClass(part: LinePart): string {
  const role = part.style & ROLE_MASK
  const fontStyle = part.style >> FONT_STYLE_SHIFT
  let className = role === 0 ? "" : `t${role}`
  if ((fontStyle & 1) !== 0) {
    className += " ti"
  }
  if ((fontStyle & 2) !== 0) {
    className += " tb"
  }
  if (part.marker === true) {
    className += ` dv-marker dv-change-${part.change}`
  } else if (part.change >= 0 && !part.deleted) {
    className += ` dv-changed dv-change-${part.change}`
  }
  if (part.hit !== undefined) {
    className += part.hit === 2 ? " dv-hit dv-hit-current" : " dv-hit"
  }
  return className.trim()
}

/** The nodes of the parts of a line: a text node where nothing styles a part */
function partNodes(parts: readonly (LinePart | InlinePart)[], textOf: (part: LinePart | InlinePart) => string): Node[] {
  const nodes: Node[] = []
  for (const part of parts) {
    const text = textOf(part)
    const className = partClass(part)
    if (className === "" && !part.deleted) {
      nodes.push(document.createTextNode(text))
      continue
    }
    const span = element("span", className)
    if (part.deleted) {
      span.dataset.inlineDeleted = ""
    }
    span.textContent = text
    nodes.push(span)
  }
  return nodes
}

export class DiffPainter {
  /** The element of the view: its own scroller, or a block in the scroller of its parent */
  private root: HTMLElement | null = null
  /** The scroller of the parent the view lies in; null when the view scrolls itself */
  private scrollParent: HTMLElement | null = null
  private model: PainterModel | null = null
  private readonly hosts = new Map<number, HTMLElement>()
  private readonly painted = new Map<number, Map<number, PaintedRow>>()
  private window: [number, number] = [-1, -1]
  private anchor: ReturnType<DiffLayout["anchorAt"]> = null
  private updateQueued = false
  private repaintQueued = false
  /** ⌘A selected the side of a file: a copy takes all its lines, painted or not */
  private selectAll: { file: number; side: number } | null = null
  /** The pages of tokens that arrived since the last repaint, as `blobId:page` */
  private readonly arrivedPages = new Set<string>()
  private textsChanged = false
  private wantsKey = ""
  private readonly viewId = newViewId()
  /** After a scroll the view made, the file at the top is not reported until the user scrolls */
  followScroll = true
  private firstScreen: "waiting" | "tokens" | "done" = "waiting"
  private readonly checkedWraps = new Set<string>()
  private readonly cleanups: (() => void)[] = []

  constructor(
    private callbacks: PainterCallbacks,
    private readonly client: () => HighlightClient | null,
    private readonly texts: () => ReadonlyMap<string, BlobTextEntry>,
  ) {}

  setCallbacks(callbacks: PainterCallbacks): void {
    this.callbacks = callbacks
  }

  /**
   * Paints into `root` and listens on it. With `scrollParent` the view is a block of that scroller (the diff under a
   * tree row, the files of a card or a dialog): the rows in view follow its scroll.
   */
  attach(root: HTMLElement, subscribeTexts: (listener: () => void) => () => void, scrollParent: HTMLElement | null = null): void {
    this.root = root
    this.scrollParent = scrollParent
    const scroller = root
    const onScroll = (event: Event) => {
      if (event.target === scroller && scrollParent === null) {
        this.onVerticalScroll()
      } else if (event.target instanceof HTMLElement && event.target.classList.contains("dv-hscroll")) {
        this.onHorizontalScroll(event.target)
      }
    }
    const onParentScroll = () => this.onVerticalScroll()
    const follow = () => {
      this.followScroll = true
    }
    const onWheel = (event: WheelEvent) => this.onWheel(event)
    const onClick = (event: MouseEvent) => this.onClick(event)
    const onPointerDown = (event: PointerEvent) => this.onPointerDown(event)
    const onKeyDown = (event: KeyboardEvent) => this.onKeyDown(event)
    const onCopy = (event: ClipboardEvent) => this.onCopy(event)
    scroller.addEventListener("scroll", onScroll, { capture: true, passive: true })
    scroller.addEventListener("wheel", onWheel, { passive: true })
    scroller.addEventListener("click", onClick)
    scroller.addEventListener("pointerdown", onPointerDown)
    scroller.addEventListener("keydown", onKeyDown)
    // a copy goes to the focused element, the body when the view is a block of its parent: the view reads the selection
    document.addEventListener("copy", onCopy)
    scrollParent?.addEventListener("scroll", onParentScroll, { passive: true })
    // what the user scrolls with: the wheel, a touch, the scroll bar, the keys
    for (const type of ["wheel", "touchstart", "pointerdown", "keydown"] as const) {
      scroller.addEventListener(type, follow, { passive: true })
    }
    const resize = new ResizeObserver(() => this.scheduleUpdate())
    resize.observe(scroller)
    if (scrollParent !== null) {
      resize.observe(scrollParent)
    }
    const unsubscribeTokens = this.client()?.cache.subscribe((blobId, page) => this.onTokens(blobId, page))
    const unsubscribeTexts = subscribeTexts(() => this.onTexts())
    this.cleanups.push(
      () => scroller.removeEventListener("scroll", onScroll, { capture: true }),
      () => scroller.removeEventListener("wheel", onWheel),
      () => scroller.removeEventListener("click", onClick),
      () => scroller.removeEventListener("pointerdown", onPointerDown),
      () => scroller.removeEventListener("keydown", onKeyDown),
      () => document.removeEventListener("copy", onCopy),
      () => scrollParent?.removeEventListener("scroll", onParentScroll),
      () => {
        for (const type of ["wheel", "touchstart", "pointerdown", "keydown"] as const) {
          scroller.removeEventListener(type, follow)
        }
      },
      () => resize.disconnect(),
      () => unsubscribeTokens?.(),
      unsubscribeTexts,
    )
    this.update()
  }

  detach(): void {
    for (const cleanup of this.cleanups.splice(0)) {
      cleanup()
    }
    this.client()?.release(this.viewId)
    this.root = null
    this.scrollParent = null
  }

  /** The top of the viewport in the coordinates of the view: below 0 while the view starts below it */
  private viewTop(): number {
    if (this.root === null) {
      return 0
    }
    return this.scrollParent === null ? this.root.scrollTop : this.scrollParent.getBoundingClientRect().top - this.root.getBoundingClientRect().top
  }

  private viewHeight(): number {
    return (this.scrollParent ?? this.root)?.clientHeight ?? 0
  }

  private setViewTop(top: number): void {
    if (this.scrollParent !== null) {
      this.scrollParent.scrollTop += top - this.viewTop()
    } else if (this.root !== null) {
      this.root.scrollTop = top
    }
  }

  /** A new model: the anchor of the old layout keeps its place in the new one, then every row is painted again */
  setModel(model: PainterModel): void {
    const previous = this.model
    this.model = model
    // a view below the top of the viewport grows downwards: only one scrolled into keeps its anchor
    if (this.root !== null && previous !== null && previous.layout !== model.layout && this.anchor !== null && this.viewTop() > 0) {
      const top = model.layout.scrollTopOf(this.anchor)
      if (top !== null) {
        this.setViewTop(top)
      }
    }
    this.clearRows()
    this.update()
  }

  attachRows(file: number, host: HTMLElement | null): void {
    if (host === null) {
      return
    }
    if (this.hosts.get(file) !== host) {
      this.dropRows(file)
      this.hosts.set(file, host)
      host.dataset.file = String(file)
      this.scheduleUpdate()
    }
  }

  detachRows(file: number, host: HTMLElement): void {
    if (this.hosts.get(file) === host) {
      this.dropRows(file)
      this.hosts.delete(file)
    }
  }

  scrollTo(top: number): void {
    if (this.root !== null) {
      this.followScroll = false
      this.setViewTop(top)
      this.update()
    }
  }

  private scheduleUpdate(): void {
    if (!this.updateQueued) {
      this.updateQueued = true
      queueMicrotask(() => {
        this.updateQueued = false
        this.update()
      })
    }
  }

  private dropRows(file: number): void {
    for (const row of this.painted.get(file)?.values() ?? []) {
      row.element.remove()
    }
    this.painted.delete(file)
  }

  private clearRows(): void {
    for (const rows of this.painted.values()) {
      for (const row of rows.values()) {
        row.element.remove()
      }
    }
    this.painted.clear()
  }

  private onVerticalScroll(): void {
    const model = this.model
    if (model === null || this.root === null) {
      return
    }
    const start = performance.now()
    this.anchor = model.layout.anchorAt(Math.max(this.viewTop(), 0))
    this.update(true)
    // the read after the paint lays the new rows out: the cost of the event is its work and that layout
    const top = this.viewTop()
    if (this.followScroll && model.files.length > 0) {
      this.callbacks.onTopFile(model.layout.fileAt(top + model.layout.metrics.headerHeight / 2))
    }
    // the benchmark (tests/perf/diff-review.perf.ts) reads the cost of each scroll event
    ;(globalThis as { __diffPaintTimes?: number[] }).__diffPaintTimes?.push(performance.now() - start)
  }

  /** Paints the rows in view and one viewport around them, and asks for their tokens */
  update(sync = false): void {
    const model = this.model
    if (model === null || this.root === null || model.files.length === 0) {
      return
    }
    const { layout } = model
    const top = this.viewTop()
    const height = this.viewHeight()
    // a view below the viewport, or above it, paints no rows
    if (top + height < 0 || top > layout.height) {
      return
    }
    this.anchor = layout.anchorAt(Math.max(top, 0))
    const from = Math.max(top - height, 0)
    const to = top + 2 * height
    const first = layout.fileAt(from)
    const last = layout.fileAt(to)
    // React mounts the sections three viewports around the view: a scroll finds the next ones mounted, and the mount
    // of the ones ahead waits for an idle moment; only a section the paint needs now is mounted in the event
    const aheadFirst = layout.fileAt(Math.max(top - 3 * height, 0))
    const aheadLast = layout.fileAt(top + 4 * height)
    if (first < this.window[0] || last > this.window[1]) {
      this.window = [aheadFirst, aheadLast]
      this.callbacks.onWindow(aheadFirst, aheadLast, sync ? "flush" : "now")
    } else if (aheadFirst !== this.window[0] || aheadLast !== this.window[1]) {
      this.window = [aheadFirst, aheadLast]
      this.callbacks.onWindow(aheadFirst, aheadLast, "idle")
    }
    for (const file of this.painted.keys()) {
      if (file < first || file > last) {
        this.dropRows(file)
      }
    }
    const wants = new Map<string, BlobWant & { pages: number[] }>()
    let firstScreenReady = true
    for (let file = first; file <= last; file++) {
      const host = this.hosts.get(file)
      const painterFile = model.files[file]!
      if (host === undefined) {
        if (layout.offsets[file]! < top + height && layout.offsets[file + 1]! > top) {
          firstScreenReady = false
        }
        continue
      }
      if (!this.paintFile(file, painterFile, host, from, to, top, top + height, wants)) {
        firstScreenReady = false
      }
    }
    this.requestTokens(wants)
    if (this.firstScreen === "waiting" && firstScreenReady) {
      this.firstScreen = "tokens"
      setTimeout(() => this.finishFirstScreen(), FIRST_SCREEN_TOKENS_MS)
    }
    // the first screen shows once the pages in view have their tokens, or 80 ms late
    const cache = this.client()?.cache
    if (this.firstScreen === "tokens" && [...wants.values()].every(want => want.priority !== 0 || want.pages.every(page => cache?.get(want.blobId, page) !== undefined))) {
      this.finishFirstScreen()
    }
  }

  private finishFirstScreen(): void {
    if (this.firstScreen === "tokens") {
      this.firstScreen = "done"
      this.callbacks.onFirstScreen()
    }
  }

  /** The line table of the text of each side of a file; null for a side whose text is not read */
  private tablesOf(painterFile: PainterFile): (LineTable | null)[] {
    const texts = this.texts()
    return painterFile.ids.map(id => {
      const text = painterFile.cells === null ? null : blobText(texts, id)
      return text === null || id === null ? null : getLineTable(id, text)
    })
  }

  /** Paints the rows of a file in `[from, to)`; false when a row in view waits for its text */
  private paintFile(
    file: number,
    painterFile: PainterFile,
    host: HTMLElement,
    from: number,
    to: number,
    viewTop: number,
    viewBottom: number,
    wants: Map<string, BlobWant & { pages: number[] }>,
  ): boolean {
    const { layout } = this.model!
    const rows = painterFile.rows
    if (rows === null || rows.rowCount === 0) {
      return true
    }
    const first = layout.rowAt(file, from)
    const last = layout.rowAt(file, to)
    let painted = this.painted.get(file)
    if (painted === undefined) {
      painted = new Map()
      this.painted.set(file, painted)
    }
    for (const [row, entry] of painted) {
      // the row of an end of the selection stays, so the browser keeps the selection while it scrolls away
      if ((row < first || row > last) && !holdsSelection(entry.element)) {
        entry.element.remove()
        painted.delete(row)
      }
    }
    const tables = this.tablesOf(painterFile)
    const { ids } = painterFile
    const rowsTop = layout.offsets[file]! + layout.rowsOffset(file)
    let ready = true
    for (let row = first; row <= last; row++) {
      const rowTop = layout.rowTop(file, row)
      const inView = rowTop < viewBottom && rowTop + layout.rowHeight(file, row) > viewTop
      const value = rowAt(rows, row)
      if (inView && value.kind !== "fold" && value.lines.some((line, side) => line !== null && ids[side] != null && tables[side] == null)) {
        ready = false
      }
      // the pages this row shows, in view first
      if (value.kind !== "fold") {
        value.lines.forEach((line, side) => {
          const id = ids[side]
          const table = tables[side]
          if (line === null || id == null || table == null || painterFile.language === "text") {
            return
          }
          let want = wants.get(id)
          if (want === undefined) {
            want = { blobId: id, language: painterFile.language, text: table.text, pages: [], priority: 1 }
            wants.set(id, want)
          }
          const page = Math.floor(line / PAGE_LINES)
          if (!want.pages.includes(page)) {
            want.pages.push(page)
          }
          if (inView) {
            want.priority = 0 as Priority
          }
        })
      }
      if (painted.has(row)) {
        continue
      }
      const entry = this.paintRow(file, painterFile, row, value, tables, rowTop - rowsTop, layout.rowHeight(file, row))
      host.append(entry.element)
      painted.set(row, entry)
    }
    return ready
  }

  private paintRow(file: number, painterFile: PainterFile, row: number, value: LineRow | FoldRow, tables: readonly (LineTable | null)[], top: number, height: number): PaintedRow {
    const rowElement = element("div", "dv-row")
    rowElement.style.top = `${top}px`
    if (height !== this.model!.layout.metrics.rowHeight) {
      rowElement.style.height = `${height}px`
    }
    rowElement.dataset.row = String(row)
    if (value.kind === "fold") {
      rowElement.classList.add("dv-fold")
      rowElement.dataset.kind = "fold"
      rowElement.append(this.foldButton(file, value))
      return { element: rowElement, cells: [], textsRead: true }
    }
    rowElement.dataset.kind = value.kind
    const cells: PaintedRow["cells"] = []
    const { ids } = painterFile
    const rowCells = painterFile.cells!
    if (rowCells.columns.length > 1) {
      // a column per side: Split, a merge
      for (const side of rowCells.columns) {
        const line = value.lines[side] ?? null
        const half = this.half(side, rowCells.cell(value, side), value.tint, rowCells.resolved(value, side))
        half.append(this.number(line))
        half.append(this.code(file, painterFile, value, side, line, tables, height))
        rowElement.append(half)
        const id = ids[side]
        if (line !== null && id != null) {
          cells.push({ blobId: id, line })
        }
      }
    } else {
      // Unified and Inline: one side, both line numbers; a deleted row shows the old line
      const side = value.kind === "deleted" ? OLD : NEW
      const half = this.half(NEW, rowCells.cell(value, side), value.tint, false)
      half.append(this.number(value.lines[OLD] ?? null), this.number(value.lines[NEW] ?? null))
      half.append(this.code(file, painterFile, value, side, value.lines[side] ?? null, tables, height))
      rowElement.append(half)
      value.lines.forEach((line, lineSide) => {
        const id = ids[lineSide]
        if (line !== null && line !== undefined && id != null) {
          cells.push({ blobId: id, line })
        }
      })
    }
    const textsRead = ids.every((id, side) => id === null || tables[side] != null)
    return { element: rowElement, cells, textsRead }
  }

  private half(side: number, cell: string, tint: LineTint, resolved: boolean): HTMLElement {
    const half = element("div", "dv-half")
    half.dataset.side = String(side)
    half.dataset.cell = cell
    if (tint !== "full" && cell !== "context" && cell !== "filler") {
      half.dataset.tint = tint
    }
    if (resolved) {
      half.dataset.resolved = ""
    }
    return half
  }

  private number(line: number | null): HTMLElement {
    const cell = element("span", "dv-num")
    cell.setAttribute("aria-hidden", "true")
    if (line !== null) {
      cell.textContent = String(line + 1)
    }
    return cell
  }

  private foldButton(file: number, fold: FoldRow): HTMLElement {
    const button = element("button", "dv-fold-button")
    button.setAttribute("type", "button")
    button.tabIndex = -1
    button.dataset.file = String(file)
    button.dataset.gap = String(fold.gap)
    const count = Math.max(...fold.lines.map(([start, end]) => end - start))
    const lines = `${count} unchanged ${count === 1 ? "line" : "lines"}`
    button.setAttribute("aria-label", fold.label === null ? `Expand ${lines}` : `Expand ${lines} in ${fold.label}`)
    const text = element("span", "")
    text.textContent = `⋯ ${lines}`
    button.append(text)
    if (fold.label !== null) {
      const label = element("span", "dv-fold-label")
      label.textContent = fold.label
      button.append(label)
    }
    return button
  }

  /** The code of one side of a row: plain text, or the parts of its tokens and ranges; wrapped in visual lines */
  private code(file: number, painterFile: PainterFile, value: LineRow, side: number, line: number | null, tables: readonly (LineTable | null)[], height: number): HTMLElement {
    const cell = element("span", "dv-code")
    const text = element("span", "dv-text")
    cell.append(text)
    const table = tables[side]
    if (line === null || table == null) {
      return cell
    }
    // where a copy reads the line from (copy-text.ts)
    cell.dataset.side = String(side)
    const content = lineText(table, line)
    const parts = this.partsOf(file, painterFile, value, side, line, content, tables)
    const oldTable = tables[OLD]
    const textOf = (part: LinePart | InlinePart) =>
      ("side" in part && part.side === OLD && oldTable != null ? lineText(oldTable, value.lines[OLD]!) : content).slice(part.start, part.end)
    if (this.model!.softWraps && height > this.model!.layout.metrics.rowHeight) {
      this.paintWrapped(text, parts, textOf, painterFile.codeWidths[side] ?? 0)
    } else {
      text.append(...partNodes(parts, textOf))
    }
    if (line === table.lineCount - 1 && !table.endsWithLineEnd) {
      const marker = element("span", "dv-eof")
      marker.title = "No newline at end of file"
      marker.textContent = "⌀"
      text.append(marker)
    }
    return cell
  }

  private partsOf(
    file: number,
    painterFile: PainterFile,
    value: LineRow,
    side: number,
    line: number,
    content: string,
    tables: readonly (LineTable | null)[],
  ): (LinePart | InlinePart)[] {
    const hits = this.model?.hits?.get(file)?.get(lineKey(side, line)) ?? null
    const cache = this.client()?.cache
    const runsAt = (lineSide: number, lineIndex: number) => {
      const id = painterFile.ids[lineSide]
      if (id == null || cache === undefined) {
        return null
      }
      return runsOf(cache.get(id, Math.floor(lineIndex / PAGE_LINES)), lineIndex % PAGE_LINES)
    }
    const cells = painterFile.cells!
    const oldTable = tables[OLD]
    if (value.kind === "inline" && oldTable != null) {
      const oldLine = value.lines[OLD]!
      return inlineParts(
        lineText(oldTable, oldLine).length,
        runsAt(OLD, oldLine),
        cells.ranges(value, OLD) ?? [],
        content.length,
        runsAt(NEW, line),
        cells.ranges(value, NEW) ?? [],
        hits,
      )
    }
    return lineParts(content.length, runsAt(side, line), cells.ranges(value, side), hits)
  }

  private paintWrapped(text: HTMLElement, parts: readonly (LinePart | InlinePart)[], textOf: (part: LinePart | InlinePart) => string, width: number): void {
    const full = parts.map(textOf).join("")
    const breaks = [...wrapBreaks(full, width, this.model!.textMetrics), full.length]
    let offset = 0
    let start = 0
    let partIndex = 0
    for (const end of breaks) {
      const line = element("span", "dv-vl")
      const lineParts: (LinePart | InlinePart)[] = []
      // the parts are in the order of the composed text: cut them at the break
      while (partIndex < parts.length) {
        const part = parts[partIndex]!
        const length = part.end - part.start
        const partStart = offset
        const partEnd = offset + length
        const cutStart = Math.max(partStart, start)
        const cutEnd = Math.min(partEnd, end)
        if (cutStart < cutEnd) {
          lineParts.push({ ...part, start: part.start + cutStart - partStart, end: part.start + cutEnd - partStart })
        }
        if (partEnd > end) {
          break
        }
        offset = partEnd
        partIndex++
      }
      line.append(...partNodes(lineParts, textOf))
      text.append(line)
      start = end
    }
    if (!/^[\t\x20-\x7e]*$/.test(full)) {
      this.checkWrap(text, full, width)
    }
  }

  /** A line outside ASCII: the DOM width of each visual line against the box; a wider one corrects the widths */
  private checkWrap(text: HTMLElement, full: string, width: number): void {
    const key = `${width}:${full}`
    if (this.checkedWraps.has(key)) {
      return
    }
    this.checkedWraps.add(key)
    queueMicrotask(() => {
      if (!text.isConnected) {
        return
      }
      const overflows = [...text.children].some(line => line.scrollWidth > line.clientWidth + 1)
      if (!overflows) {
        return
      }
      const probe = element("span", "")
      probe.style.position = "absolute"
      probe.style.visibility = "hidden"
      probe.style.whiteSpace = "pre"
      text.append(probe)
      const metrics = this.model!.textMetrics as TextMetrics & { correct?: (codePoint: number, width: number) => void }
      for (const char of new Set(full)) {
        const codePoint = char.codePointAt(0)!
        if (codePoint > 0x7e) {
          probe.textContent = char
          metrics.correct?.(codePoint, probe.getBoundingClientRect().width)
        }
      }
      probe.remove()
      this.callbacks.onWidthsCorrected()
    })
  }

  private requestTokens(wants: Map<string, BlobWant>): void {
    const client = this.client()
    if (client === null) {
      return
    }
    const list = [...wants.values()].map(want => ({ ...want, pages: want.pages.toSorted((a, b) => a - b) }))
    const key = list.map(want => `${want.blobId}:${want.priority}:${want.pages.join(",")}`).join(";")
    for (const want of list) {
      client.cache.touch(want.blobId)
    }
    if (key !== this.wantsKey) {
      this.wantsKey = key
      client.want(this.viewId, list)
    }
  }

  /** A page of tokens arrived: the rows that show it are painted again, once per task */
  private onTokens(blobId: string, page: number): void {
    this.arrivedPages.add(`${blobId}:${page}`)
    this.scheduleRepaint()
  }

  /** New texts: the rows painted without their text are painted again */
  private onTexts(): void {
    this.textsChanged = true
    this.scheduleRepaint()
  }

  private scheduleRepaint(): void {
    if (!this.repaintQueued) {
      this.repaintQueued = true
      queueMicrotask(() => {
        this.repaintQueued = false
        this.repaint()
      })
    }
  }

  /** Paints again only the rows whose tokens or texts arrived, each in place */
  private repaint(): void {
    const model = this.model
    if (model === null) {
      return
    }
    const textsChanged = this.textsChanged
    this.textsChanged = false
    const arrived = new Set(this.arrivedPages)
    this.arrivedPages.clear()
    for (const [file, rows] of this.painted) {
      const painterFile = model.files[file]
      if (painterFile?.rows === null || painterFile?.rows === undefined) {
        continue
      }
      let tables: (LineTable | null)[] | undefined
      for (const [row, entry] of rows) {
        const stale = (textsChanged && !entry.textsRead) || entry.cells.some(cell => arrived.has(`${cell.blobId}:${Math.floor(cell.line / PAGE_LINES)}`))
        if (!stale || holdsSelection(entry.element)) {
          continue
        }
        tables ??= this.tablesOf(painterFile)
        if (textsChanged && !entry.textsRead && tables.some((table, side) => table === null && painterFile.ids[side] !== null)) {
          continue
        }
        const { layout } = model
        const top = layout.rowTop(file, row) - layout.offsets[file]! - layout.rowsOffset(file)
        const next = this.paintRow(file, painterFile, row, rowAt(painterFile.rows, row), tables, top, layout.rowHeight(file, row))
        entry.element.replaceWith(next.element)
        rows.set(row, next)
      }
    }
    this.update()
  }

  private onHorizontalScroll(bar: HTMLElement): void {
    const section = bar.closest<HTMLElement>(".dv-section")
    if (section === null) {
      return
    }
    const side = bar.dataset.side ?? "1"
    const left = bar.scrollLeft
    section.style.setProperty(`--dv-scroll-${side}`, String(left))
    // the sides scroll together, as IntelliJ's SyncScrollSupport
    for (const other of section.querySelectorAll<HTMLElement>(".dv-hscroll")) {
      if (other !== bar && Math.abs(other.scrollLeft - left) > 0.5) {
        other.scrollLeft = left
      }
    }
  }

  private onWheel(event: WheelEvent): void {
    const horizontal = event.shiftKey ? event.deltaY : event.deltaX
    if (horizontal === 0 || (!event.shiftKey && Math.abs(event.deltaX) < Math.abs(event.deltaY))) {
      return
    }
    const target = event.target instanceof Element ? event.target : null
    const section = target?.closest<HTMLElement>(".dv-section")
    const side = target?.closest<HTMLElement>("[data-side]")?.dataset.side ?? "1"
    const bar = section?.querySelector<HTMLElement>(`.dv-hscroll[data-side="${side}"]`) ?? section?.querySelector<HTMLElement>(".dv-hscroll")
    if (bar !== null && bar !== undefined) {
      bar.scrollLeft += horizontal
    }
  }

  /** With a column per side, a press on a side makes the others not selectable, so a selection stays on one side */
  private onPointerDown(event: PointerEvent): void {
    this.selectAll = null
    const scroller = this.root
    if (scroller === null) {
      return
    }
    const half = event.target instanceof Element ? event.target.closest<HTMLElement>(".dv-half") : null
    const side = half?.dataset.side
    if (side !== undefined && (half?.parentElement?.childElementCount ?? 0) > 1) {
      scroller.dataset.selectSide = side
    } else {
      delete scroller.dataset.selectSide
    }
  }

  /** ⌘A (Ctrl+A off macOS) selects the side of the file at the top; a copy then takes all its lines */
  private onKeyDown(event: KeyboardEvent): void {
    const model = this.model
    const scroller = this.root
    if (model === null || scroller === null || event.altKey || event.shiftKey || !(event.metaKey || event.ctrlKey) || event.code !== "KeyA") {
      if (!(event.metaKey || event.ctrlKey)) {
        this.selectAll = null
      }
      return
    }
    const file = model.layout.fileAt(Math.max(this.viewTop(), 0) + model.layout.metrics.headerHeight / 2)
    const host = this.hosts.get(file)
    if (host === undefined) {
      return
    }
    event.preventDefault()
    document.getSelection()?.selectAllChildren(host)
    this.selectAll = { file, side: sideBySide(model.files[file]) ? Number(scroller.dataset.selectSide ?? NEW) : NEW }
  }

  /** A copy gives the lines of the model between the ends of the selection, also of rows no longer painted */
  private onCopy(event: ClipboardEvent): void {
    const model = this.model
    const selection = document.getSelection()
    if (model === null || this.root === null || selection === null || selection.isCollapsed || event.clipboardData === null || !this.root.contains(selection.anchorNode)) {
      return
    }
    const fileOf = (index: number): CopyFile => {
      const file = model.files[index]
      return { rows: file?.rows ?? null, tables: file === undefined ? [] : this.tablesOf(file) }
    }
    let text: string
    if (this.selectAll !== null) {
      text = copyFile(fileOf(this.selectAll.file), sideBySide(model.files[this.selectAll.file]), this.selectAll.side)
    } else {
      const anchor = pointOf(selection.anchorNode, selection.anchorOffset)
      const focus = pointOf(selection.focusNode, selection.focusOffset)
      if (anchor === null || focus === null) {
        return
      }
      const files: CopyFile[] = []
      for (let index = Math.min(anchor.file, focus.file); index <= Math.max(anchor.file, focus.file); index++) {
        files[index] = fileOf(index)
      }
      text = copyText(files, anchor, focus, sideBySide(model.files[anchor.file]), anchor.side)
    }
    event.clipboardData.setData("text/plain", text)
    event.preventDefault()
  }

  private onClick(event: MouseEvent): void {
    const button = event.target instanceof Element ? event.target.closest<HTMLElement>(".dv-fold-button") : null
    const model = this.model
    if (button === null || model === null) {
      return
    }
    const file = Number(button.dataset.file)
    const rows = model.files[file]?.rows
    const row = button.closest<HTMLElement>(".dv-row")?.dataset.row
    if (rows === null || rows === undefined || row === undefined) {
      return
    }
    const value = rowAt(rows, Number(row))
    if (value.kind === "fold") {
      // the button goes with the rows it stood for: the diffs keep the focus, so their keys still work
      if (this.scrollParent === null) {
        this.root?.focus({ preventScroll: true })
      }
      this.callbacks.onFold(file, value, event.altKey)
    }
  }
}

/** An end of a non-empty selection is in `element` */
function holdsSelection(element: HTMLElement): boolean {
  const selection = document.getSelection()
  return selection !== null && !selection.isCollapsed && (element.contains(selection.anchorNode) || element.contains(selection.focusNode))
}

/** The text of a code cell before a point of a selection, without the deleted parts of an Inline row */
function textOffset(cell: HTMLElement, node: Node, offset: number): number {
  const boundary = document.createRange()
  boundary.setStart(node, offset)
  boundary.collapse(true)
  let length = 0
  const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT)
  for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
    const excluded = text.parentElement?.closest("[data-inline-deleted], .dv-eof") != null
    if (text === node) {
      return length + (excluded ? 0 : offset)
    }
    const size = text.textContent?.length ?? 0
    // a text that ends after the point is not before it
    if (boundary.comparePoint(text, size) > 0) {
      break
    }
    if (!excluded) {
      length += size
    }
  }
  return length
}

/** A point of a selection in the model: the file and the row of its row element, the offset in its code cell */
function pointOf(node: Node | null, offset: number): (CopyPoint & { side: number }) | null {
  const element = node === null ? null : node instanceof Element ? node : node.parentElement
  const row = element?.closest<HTMLElement>(".dv-row")
  const file = row?.parentElement?.dataset.file
  if (row === null || row === undefined || file === undefined) {
    return null
  }
  const cell = element?.closest<HTMLElement>(".dv-code") ?? row.querySelector<HTMLElement>(".dv-code[data-side]")
  return {
    file: Number(file),
    row: Number(row.dataset.row),
    offset: cell === null || !cell.contains(node) ? 0 : textOffset(cell, node!, offset),
    side: Number(cell?.dataset.side ?? NEW),
  }
}

/** A file shows a column per side: Split, a merge */
function sideBySide(file: PainterFile | undefined): boolean {
  return (file?.cells?.columns.length ?? 1) > 1
}
