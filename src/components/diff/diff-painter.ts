import type { MoveEnd, Remark, RenameNote } from "@/lib/diff/block-parts"
import { copyFile, copyText } from "@/lib/diff/copy-text"
import type { CopyFile, CopyPoint } from "@/lib/diff/copy-text"
import { lineKey } from "@/lib/diff/find"
import type { HighlightClient, BlobWant } from "@/lib/diff/highlight/client"
import { FONT_STYLE_SHIFT, PAGE_LINES, ROLE_MASK } from "@/lib/diff/highlight/protocol"
import type { Priority } from "@/lib/diff/highlight/protocol"
import { moveNoteOfKey } from "@/lib/diff/layout"
import type { DiffLayout, LayoutPart, MoveHand } from "@/lib/diff/layout"
import { inlineOffset, inlinePair, inlineParts, lineParts, ownOldHits, RENAMED_RANGE, runsOf, sameTextOffset } from "@/lib/diff/line-parts"
import type { InlinePair, InlinePart, LineHits, LinePart } from "@/lib/diff/line-parts"
import { getLineTable, lineText } from "@/lib/diff/line-table"
import type { LineTable } from "@/lib/diff/line-table"
import type { RenameSite } from "@/lib/diff/rename-sites"
import type { RowCells } from "@/lib/diff/row-cells"
import { foldWords, isLineRow, NEW, OLD, placeOfRow, rowAt, rowOfLine, rowOfPlace, rowOfRowPlace } from "@/lib/diff/rows"
import type { FileRows, FoldRow, LineRow, NoteRow, Row, RowPlace } from "@/lib/diff/rows"
import { blobText } from "@/lib/diff/view-files"
import { wrapBreaks } from "@/lib/diff/wrap"
import type { TextMetrics } from "@/lib/diff/wrap"
import { describeRenamedName, formatHiddenMatches, presentRemark } from "@/presenters/diff-marks"
import type { OtherFile, RemarkFacts } from "@/presenters/diff-marks"
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
  /** The Shiki language of each side: the old side of a rename by its old path */
  languages: readonly string[]
  /** The path of each side, whose extension tells the grammar of the backend */
  paths: readonly string[]
  /** The path in the working tree of each side that is a file of the working tree */
  worktreePaths: readonly (string | null)[]
  /** The width of the code of each side, in CSS px, for the soft wraps */
  codeWidths: readonly number[]
  /**
   * Another file of the subject of the file, by its index among the files of the subject (`FileDiff.subjectPaths`), as
   * a note names it: its path, its name among them, whether the view can show a place of it. Null for an index the
   * subject has no file at; absent for a file of no subject
   */
  otherFile?: (index: number) => OtherFile | null
}

/** A word of code in the view: where it is in the model, and its text */
export interface CodeWord {
  file: number
  side: number
  /** The line of its side, from 0 */
  line: number
  /** Its UTF-16 columns in the line */
  start: number
  end: number
  text: string
  /** Where it is drawn now, in viewport coordinates */
  rects: () => DOMRect[]
}

export interface PainterModel {
  files: readonly PainterFile[]
  /** The hits of the find by file and line (`hitsByLine` of find.ts); none while the find is closed */
  hits?: ReadonlyMap<number, ReadonlyMap<string, LineHits>>
  layout: DiffLayout
  softWraps: boolean
  textMetrics: TextMetrics
  /**
   * The key that shows and hides the old lines of the notes in view (`DiffViewProps.notesKey`): a note that opens
   * shows it after its words and names it to a screen reader; absent where no key does
   */
  notesKey?: string
  /**
   * The key that shows the other end of a move from its note in view (`DiffViewProps.moveKey`): the button of the one
   * note the key takes now (`DiffPainter.keyedMove`) shows it after its words and names it to a screen reader; absent
   * where no key does
   */
  moveKey?: string
}

export interface PainterCallbacks {
  /**
   * The files to mount, three viewports around the view: "flush" in a scroll event whose frame needs a section, "now"
   * when the paint needs one outside an event, "idle" for the sections ahead
   */
  onWindow: (first: number, last: number, when: "flush" | "now" | "idle") => void
  /** The file whose header is at the top and its part there (`DiffLayout.partShownAt`), after a scroll of the user */
  onTopFile: (index: number, part: LayoutPart) => void
  /** Any scroll of the view, the user's or one the view made: the top of the viewport in the document and its height */
  onScroll: (top: number, height: number) => void
  /** A click on a fold; with ⌥ (Alt) it asks for all the lines of the file */
  onFold: (file: number, fold: FoldRow, all: boolean) => void
  /** A click on the row of a block another section shows (`NoteRow`, `elsewhere`) */
  onNote: (file: number, note: NoteRow) => void
  /**
   * A click on a note that opens (`Remark.open` not null): its old lines show, or go again; with ⌥ (Alt) every such
   * note of the file follows it
   */
  onNoteToggle: (file: number, note: NoteRow, all: boolean) => void
  /**
   * A click on the place a note of a move names ("to line 210", "to Report.kt:12"): the other end of the move shows,
   * from this end of it
   */
  onMoveJump: (file: number, end: MoveEnd) => void
  /**
   * A click on the file a note of renames names as the one that declares a name ("declared in Store.kt"): the
   * declaration shows, from the site the note stands at; `rename` is the number of the rename among those of the note
   */
  onDeclarationJump: (file: number, note: RenameNote, rename: number) => void
  /** The rows of the first screen are painted with their texts; the tokens of their pages are there or overdue */
  onFirstScreen: () => void
  /** A wrapped line came out wider in the DOM than measured: the widths are corrected, the heights need a layout */
  onWidthsCorrected: () => void
}

/**
 * An end of the selection of the view as a new model finds it again (`DiffPainter.heldSelection`): the file by its
 * key, the row by a line of the file (`RowPlace`) and by the text it shows, and the place in that text, of the code
 * cell of a side (`side`) or of the whole row
 */
interface HeldEnd {
  key: string
  place: RowPlace
  text: string
  side: number | null
  offset: number
}

/** The selection of the view over a new model: its two ends, or the file whose side ⌘A selected */
type HeldSelection = { kind: "ends"; anchor: HeldEnd; focus: HeldEnd } | { kind: "all"; key: string }

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

/**
 * The caret of a field that takes typed text, focused or not: the selection of the page is at the field, or, as
 * Chromium reports a field it does not focus, in the parent of the field right before it
 */
function isFieldCaret(anchor: Node, offset: number): boolean {
  return (
    isTextField(anchor instanceof Element ? anchor : anchor.parentElement) ||
    (anchor instanceof Element && isTextField(anchor.childNodes[offset] ?? null)) ||
    isTextField(document.activeElement)
  )
}

/** An element that takes typed text */
function isTextField(element: Node | null): boolean {
  return (
    element instanceof HTMLTextAreaElement ||
    (element instanceof HTMLInputElement && element.type !== "checkbox" && element.type !== "radio") ||
    (element instanceof HTMLElement && element.isContentEditable)
  )
}

/** An element of the view; `testId`: how a test finds it (docs/contributing/e2e-tests.md, "The rows of a diff"), never by its class */
function element(tag: string, className: string, testId?: string): HTMLElement {
  const created = document.createElement(tag)
  created.className = className
  if (testId !== undefined) {
    created.dataset.testid = testId
  }
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
  if ((fontStyle & 4) !== 0) {
    className += " tu"
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

/** What a part of a renamed name is: the words of its hover ("Renamed from total"), and whether the name starts with the part */
interface NamePart {
  readonly hover: string
  readonly starts: boolean
}

/**
 * The nodes of the parts of a line: a text node where nothing styles a part. `nameOf`: what a part of a renamed
 * name is (`NamePart`). A name is several parts where a hit of the find, a syntax run or a visual line cuts it: the
 * part it starts with says so (`data-name-start`), so a screen reader hears "renamed" once a name (diff-view.css)
 */
function partNodes(parts: readonly (LinePart | InlinePart)[], textOf: (part: LinePart | InlinePart) => string, nameOf?: (part: LinePart | InlinePart) => NamePart | null): Node[] {
  const nodes: Node[] = []
  for (const part of parts) {
    const text = textOf(part)
    const className = partClass(part)
    if (className === "" && !part.deleted) {
      nodes.push(document.createTextNode(text))
      continue
    }
    const span = element("span", className)
    // what a part is, for a test: a changed range by its kind (a renamed name by its own), a hit of the find, the current one named
    if (part.marker !== true && part.change >= 0 && !part.deleted) {
      span.dataset.change = String(part.change)
    }
    if (part.change === RENAMED_RANGE && part.marker !== true) {
      // a renamed name says what it was, or what it became; the old one that an Inline row strikes through is one too
      const name = nameOf?.(part) ?? null
      if (name !== null) {
        span.title = name.hover
        if (name.starts) {
          span.dataset.nameStart = ""
        }
      }
      if (part.deleted) {
        span.dataset.renamed = ""
      }
    }
    if (part.hit !== undefined) {
      span.dataset.hit = part.hit === 2 ? "current" : ""
    }
    if (part.deleted) {
      // where the part starts in the old line: a point in it is a point of that line (`pointOf`)
      span.dataset.inlineDeleted = String(part.start)
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
  /**
   * The name a reveal selected: selected once its row is painted with its text, however long that takes under load, and
   * again when a paint drops it (a new layout paints every row again), until the user presses or types in the view or
   * selects elsewhere
   */
  private revealedName: { file: number; side: number; line: number; start: number; end: number; word: string | null; selected: boolean } | null = null
  /**
   * The name of the old line that a reveal selected in the row of Inline that shows it: the row, whose deleted parts
   * are selectable (`data-revealed-old`) for as long as the selection is that name (`[from, to)` of the text of the
   * row in its cell), and the name, which a copy and the keys of the code act on (the selection of the page alone
   * cannot say it: it may start in the text the row shows from the new line)
   */
  private revealedOld: { row: HTMLElement; cell: HTMLElement; from: number; to: number; word: CodeWord } | null = null
  /**
   * The ends of the selection of the view, for the return of the focus: WebKit clears the selection of the page as the
   * focus goes to an element that does not hold it (the menu of the code, a list of declarations or usages), so the
   * name the popup was opened on would be selected no more once it closes, and the keys of the code would read none
   */
  private keptSelection: { anchor: Node; anchorOffset: number; focus: Node; focusOffset: number } | null = null
  /**
   * The line a reveal landed on, itself and no name in it (the other end of a move): its row, or the note right above
   * it that says what it is, is marked once it is painted (`data-landed`, diff-view.css), and again after a paint that
   * dropped it, until the user scrolls, presses or types in the view. The file by its key: a new model may show other
   * files before it
   */
  private landed: { key: string; side: number; line: number; element: HTMLElement | null } | null = null
  /** The end of a move the view stands at (`setMoveHand`): the note the key of the moves takes first while it is in view */
  private moveHand: MoveHand | null = null
  /** The button of the note the key of the moves takes now, which shows the hint of the key (`markMoveKey`) */
  private keyedMoveButton: HTMLElement | null = null
  /** The pages of tokens that arrived since the last repaint, as `blobId:page` */
  private readonly arrivedPages = new Set<string>()
  private textsChanged = false
  private wantsKey = ""
  private readonly viewId = newViewId()
  /** The card whose header is stuck with rows under it (`data-stuck`, the shadow of diff-view.css) */
  private stuckSection: HTMLElement | null = null
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
    if (scrollParent !== null) {
      // the views keep what a reader has on screen in place themselves (`setModel`), in every engine alike: the
      // scroller must not move it a second time (diff-view.css), for as long as it holds a view
      const views = viewsOf.get(scrollParent) ?? 0
      viewsOf.set(scrollParent, views + 1)
      if (views === 0) {
        scrollParent.dataset.dvScrollParent = ""
      }
    }
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
      // the user moves on: the row a reveal landed on is marked no more
      this.letLandedGo()
    }
    // a press or a key may lay the rows out again (a note that opens, a fold) before the event of a scroll that came
    // right before it: the anchor is read where the view stands now, never where the last scroll event left it
    const stand = () => this.readAnchor()
    const onWheel = (event: WheelEvent) => this.onWheel(event)
    const onClick = (event: MouseEvent) => this.onClick(event)
    const onPointerDown = (event: PointerEvent) => this.onPointerDown(event)
    const onKeyDown = (event: KeyboardEvent) => this.onKeyDown(event)
    const onCopy = (event: ClipboardEvent) => this.onCopy(event)
    const onSelectionChange = () => this.onSelectionChange()
    const onFocusIn = () => this.restoreSelection()
    scroller.addEventListener("scroll", onScroll, { capture: true, passive: true })
    scroller.addEventListener("pointerdown", stand, { capture: true, passive: true })
    scroller.addEventListener("keydown", stand, { capture: true, passive: true })
    scroller.addEventListener("wheel", onWheel, { passive: true })
    scroller.addEventListener("click", onClick)
    scroller.addEventListener("pointerdown", onPointerDown)
    scroller.addEventListener("keydown", onKeyDown)
    scroller.addEventListener("focusin", onFocusIn)
    // a copy goes to the focused element, the body when the view is a block of its parent: the view reads the selection
    document.addEventListener("copy", onCopy)
    document.addEventListener("selectionchange", onSelectionChange)
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
      () => scroller.removeEventListener("pointerdown", stand, { capture: true }),
      () => scroller.removeEventListener("keydown", stand, { capture: true }),
      () => scroller.removeEventListener("wheel", onWheel),
      () => scroller.removeEventListener("click", onClick),
      () => scroller.removeEventListener("pointerdown", onPointerDown),
      () => scroller.removeEventListener("keydown", onKeyDown),
      () => scroller.removeEventListener("focusin", onFocusIn),
      () => document.removeEventListener("copy", onCopy),
      () => document.removeEventListener("selectionchange", onSelectionChange),
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
    const { scrollParent } = this
    if (scrollParent !== null) {
      // the last view of the scroller left: what scrolls in it is the engine's to keep in place again
      const views = (viewsOf.get(scrollParent) ?? 1) - 1
      if (views > 0) {
        viewsOf.set(scrollParent, views)
      } else {
        viewsOf.delete(scrollParent)
        delete scrollParent.dataset.dvScrollParent
      }
    }
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

  /** The height of what scrolls the view: the view itself, or the scroller of the parent it lies in */
  viewportHeight(): number {
    return this.viewHeight()
  }

  private setViewTop(top: number): void {
    if (this.scrollParent !== null) {
      this.scrollParent.scrollTop += top - this.viewTop()
    } else if (this.root !== null) {
      this.root.scrollTop = top
    }
  }

  /**
   * A new model: the anchor of the old layout keeps its place in the new one, then every row is painted again, and
   * the selection of the view is put on the new rows of its ends (`heldSelection`): a model comes for an answer a
   * reader did not ask for (the scopes of a file, what the index proved of a name), and what the reader selected to
   * copy or to act on must not go with it. A view that scrolls itself goes to where its anchor is now. A view that
   * is a block of the scroller of its parent moves that scroller by as much as its anchor moved in it: by the rows that came or went above the row at the top of the
   * viewport, or by all of them when the view lies above the viewport (its anchor is the end of its rows). Each view
   * of a scroller adds its own part, in whatever order they get their models, so what a reader has on screen stays
   * where it is though several views above it change at once
   */
  setModel(model: PainterModel): void {
    const previous = this.model
    this.model = model
    // a view below the top of the viewport grows downwards: only one scrolled into, or past, keeps its anchor
    if (this.root !== null && previous !== null && previous.layout !== model.layout && this.anchor !== null && this.viewTop() > 0) {
      const top = model.layout.scrollTopOf(this.anchor)
      if (top !== null && this.scrollParent === null) {
        this.setViewTop(top)
      } else if (top !== null) {
        const was = previous.layout.scrollTopOf(this.anchor)
        if (was !== null && was !== top) {
          this.scrollParent!.scrollTop += top - was
        }
      }
    }
    const held = previous === null ? null : this.heldSelection(previous)
    if (held !== null) {
      // the rows of its ends go: what the engine would leave of it (a caret in a host, or a range from one host to
      // another) is no selection of the reader
      document.getSelection()?.removeAllRanges()
    }
    this.clearRows()
    this.update()
    if (held !== null) {
      this.restoreHeld(held)
    }
  }

  /**
   * The selection of the view as the rows of `model` show it, for a model that takes their place: the side of a file
   * that ⌘A selected, else its two ends, each in a painted row. Null without a selection in the view, and for one an
   * end of which is in no row
   */
  private heldSelection(model: PainterModel): HeldSelection | null {
    const selection = document.getSelection()
    const root = this.root
    if (selection === null || root === null || selection.rangeCount === 0) {
      return null
    }
    const { anchorNode, focusNode } = selection
    if (anchorNode === null || focusNode === null || !root.contains(anchorNode) || !root.contains(focusNode)) {
      return null
    }
    const all = this.selectAll === null ? undefined : model.files[this.selectAll.file]
    if (all !== undefined && !selection.isCollapsed && anchorNode === this.hosts.get(this.selectAll!.file)) {
      return { kind: "all", key: all.key }
    }
    const end = (node: Node, offset: number): HeldEnd | null => {
      const element = node instanceof Element ? node : node.parentElement
      const row = element?.closest<HTMLElement>(".dv-row") ?? null
      const file = row?.parentElement?.dataset.file
      const painterFile = file === undefined ? undefined : model.files[Number(file)]
      if (row === null || painterFile === undefined || painterFile.rows === null) {
        return null
      }
      const place = placeOfRow(painterFile.rows, Number(row.dataset.row))
      if (place === null) {
        return null
      }
      const cell = element?.closest<HTMLElement>(".dv-code[data-side]") ?? null
      return {
        key: painterFile.key,
        place,
        text: row.textContent ?? "",
        side: cell === null ? null : Number(cell.dataset.side),
        offset: textOffset(cell ?? row, node, offset, NOT_COUNTED_NOWHERE),
      }
    }
    const anchor = end(anchorNode, selection.anchorOffset)
    const focus = selection.isCollapsed ? anchor : end(focusNode, selection.focusOffset)
    return anchor === null || focus === null ? null : { kind: "ends", anchor, focus }
  }

  /**
   * Puts the selection a model before held (`heldSelection`) on the rows painted now: each end in the row at its
   * place that shows the text its row showed, painted for it where it is outside the rows in view (as a row that
   * scrolled away is kept for an end). An end whose row is gone or shows another text has no place: the selection
   * has ended. A name a reveal selected in this paint stays selected
   */
  private restoreHeld(held: HeldSelection): void {
    const selection = document.getSelection()
    const model = this.model
    if (selection === null || model === null || selection.rangeCount > 0) {
      return
    }
    if (held.kind === "all") {
      const file = model.layout.indexOf(held.key)
      const host = this.hosts.get(file)
      if (host === undefined || this.selectAll === null) {
        this.selectAll = null
        return
      }
      this.selectAll = { ...this.selectAll, file }
      selection.selectAllChildren(host)
      return
    }
    const pointOfEnd = (end: HeldEnd, paints: boolean): { node: Node; offset: number } | null => {
      const file = model.layout.indexOf(end.key)
      const painterFile = model.files[file]
      const host = this.hosts.get(file)
      const rows = painterFile?.rows
      const row = rows == null ? null : rowOfRowPlace(rows, end.place)
      if (painterFile === undefined || rows == null || row === null || host === undefined) {
        return null
      }
      const painted = this.painted.get(file)
      let entry = painted?.get(row)
      if (entry === undefined && paints && painted !== undefined) {
        const { layout } = model
        const top = layout.rowTop(file, row) - layout.offsets[file]! - layout.rowsOffset(file)
        entry = this.paintRow(file, painterFile, row, rowAt(rows, row), this.tablesOf(painterFile), top, layout.rowHeight(file, row))
      }
      if (entry === undefined || (entry.element.textContent ?? "") !== end.text) {
        return null
      }
      const within = end.side === null ? entry.element : entry.element.querySelector<HTMLElement>(`.dv-code[data-side="${end.side}"]`)
      if (within === null) {
        return null
      }
      if (!entry.element.isConnected) {
        host.append(entry.element)
        painted!.set(row, entry)
      }
      return textPoint(within, end.offset)
    }
    // a caret holds no row (`holdsSelection`): it is put back only into a row in view
    const paints = held.anchor !== held.focus
    const anchor = pointOfEnd(held.anchor, paints)
    const focus = held.anchor === held.focus ? anchor : pointOfEnd(held.focus, paints)
    if (anchor !== null && focus !== null) {
      selection.setBaseAndExtent(anchor.node, anchor.offset, focus.node, focus.offset)
    }
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

  /** The user scrolls with something beside the view (the map of the files): the file at the top is reported again */
  followUserScroll(): void {
    this.followScroll = true
  }

  /**
   * The view scrolled to `top`, up to a whole pixel: the engines keep a whole `scrollTop` (WebKit drops a fraction,
   * Chromium rounds it), and a stop below its place would show the edge of what ends right above the view (an
   * opening, the card before)
   */
  scrollTo(top: number): void {
    if (this.root !== null) {
      this.followScroll = false
      this.setViewTop(Math.ceil(top))
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

  /**
   * The anchor of the view, the row at its top, read where the view stands: a new layout keeps that row in place
   * (`setModel`). Read on every scroll event and paint, and on a press or a key in the view, which may come before the
   * event of the scroll before it
   */
  private readAnchor(): void {
    if (this.model !== null && this.root !== null) {
      this.anchor = this.model.layout.anchorAt(Math.max(this.viewTop(), 0))
    }
  }

  private onVerticalScroll(): void {
    const model = this.model
    if (model === null || this.root === null) {
      return
    }
    const start = performance.now()
    this.readAnchor()
    this.update(true)
    // the read after the paint lays the new rows out: the cost of the event is its work and that layout
    const top = this.viewTop()
    const height = this.viewHeight()
    if (this.followScroll && model.files.length > 0) {
      this.callbacks.onTopFile(model.layout.fileShownAt(top, height), model.layout.partShownAt(top, height))
    }
    this.callbacks.onScroll(top, height)
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
    // a view below the viewport, or above it, paints no rows; one above it is held by the end of its rows
    if (top + height < 0 || top > layout.height) {
      this.anchor = top > layout.height ? layout.anchorAt(top) : this.anchor
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
    this.markStuckHeader(top)
    this.requestTokens(wants)
    this.selectRevealedName()
    this.markLanded()
    this.markMoveKey(top, height)
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

  /**
   * Marks the card whose header is stuck below the top of the view while its rows scroll under it, at least a row of
   * them (a card that just reached the top does not flicker): the file under the sticky top, not `fileAtTop`, which
   * names the next file while that one's card pushes the header out. Written only when it changes, on a scroll and on a
   * new layout alike
   */
  private markStuckHeader(top: number): void {
    const { layout } = this.model!
    let section: HTMLElement | null = null
    if (layout.metrics.cards !== undefined) {
      const y = top + layout.stickyTop
      const file = layout.fileAt(y)
      if (y > layout.offsets[file]! + layout.metrics.rowHeight) {
        section = this.hosts.get(file)?.parentElement ?? null
      }
    }
    if (section !== this.stuckSection) {
      if (this.stuckSection !== null) {
        delete this.stuckSection.dataset.stuck
      }
      if (section !== null) {
        section.dataset.stuck = ""
      }
      this.stuckSection = section
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
      if (inView && isLineRow(value) && value.lines.some((line, side) => line !== null && ids[side] != null && tables[side] == null)) {
        ready = false
      }
      // the pages this row shows, in view first
      if (isLineRow(value)) {
        value.lines.forEach((line, side) => {
          const id = ids[side]
          const table = tables[side]
          const language = painterFile.languages[side] ?? "text"
          if (line === null || id == null || table == null || language === "text") {
            return
          }
          let want = wants.get(id)
          if (want === undefined) {
            want = {
              blobId: id,
              language,
              text: table.text,
              pages: [],
              priority: 1,
              filePath: painterFile.paths[side],
              worktreePath: painterFile.worktreePaths[side] ?? null,
              group: painterFile.key,
            }
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

  private paintRow(file: number, painterFile: PainterFile, row: number, value: Row, tables: readonly (LineTable | null)[], top: number, height: number): PaintedRow {
    const rowElement = element("div", "dv-row", "diff-row")
    rowElement.style.top = `${top}px`
    if (height !== this.model!.layout.metrics.rowHeight) {
      rowElement.style.height = `${height}px`
    }
    rowElement.dataset.row = String(row)
    if (value.kind === "fold") {
      rowElement.classList.add("dv-fold")
      rowElement.dataset.kind = "fold"
      rowElement.append(this.foldButton(file, value), ...this.foldTargets(file, value))
      return { element: rowElement, cells: [], textsRead: true }
    }
    if (value.kind === "note") {
      rowElement.classList.add("dv-note")
      rowElement.dataset.kind = "note"
      rowElement.dataset.tone = value.tone
      if (value.tone === "remark") {
        // its words start where the code of the first column does: after the line numbers of that column (diff-view.css)
        const cells = painterFile.cells
        rowElement.style.setProperty("--dv-numbers", String(cells === null || cells.columns.length > 1 ? 1 : Math.max(cells.numbers.length, 1)))
        // the note of a quiet run lies on the colour of its lines; a heading keeps its own
        if (value.remark?.quiet === true) {
          rowElement.dataset.quiet = ""
        }
      }
      // the note at an end of a move: which end, on the colour of a move (diff-view.css)
      const words = this.note(file, value)
      if (value.remark?.move != null) {
        rowElement.dataset.move = value.remark.move.at
        if (value.tone === "remark" && sideBySide(painterFile)) {
          // a column per side: the note lies over the column of its lines, the other one filled as beside them
          const filler = element("span", "dv-note-filler")
          rowElement.dataset.over = value.remark.move.at === "source" ? "old" : "new"
          rowElement.append(...(value.remark.move.at === "source" ? [words, filler] : [filler, words]))
          return { element: rowElement, cells: [], textsRead: true }
        }
      }
      rowElement.append(words)
      return { element: rowElement, cells: [], textsRead: true }
    }
    rowElement.dataset.kind = value.kind
    const cells: PaintedRow["cells"] = []
    const { ids } = painterFile
    const rowCells = painterFile.cells!
    if (rowCells.typed) {
      rowElement.dataset.typed = ""
    }
    if (rowCells.columns.length > 1) {
      // a column per side: Split, a merge
      for (const side of rowCells.columns) {
        const line = value.lines[side] ?? null
        const half = this.half(side, rowCells.cell(value, side), value, rowCells.resolved(value, side) ? resolvedEdges(value) : null)
        half.append(this.number(line))
        half.append(this.code(file, painterFile, value, side, line, tables, height))
        rowElement.append(half)
        const id = ids[side]
        if (line !== null && id != null) {
          cells.push({ blobId: id, line })
        }
      }
    } else {
      // Unified and Inline: one side, both line numbers (a file with one side: its own); a deleted row shows the old line
      const side = value.kind === "deleted" ? OLD : NEW
      const half = this.half(NEW, rowCells.cell(value, side), value, null)
      half.append(...rowCells.numbers.map(numberSide => this.number(value.lines[numberSide] ?? null)))
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

  private half(side: number, cell: string, { tint, run, move }: LineRow, resolved: string | null): HTMLElement {
    const half = element("div", "dv-half", "diff-side")
    half.dataset.side = String(side)
    half.dataset.cell = cell
    // a line of a move: which end of it the line is at, and whether the other end is in another file, for a screen
    // reader (diff-view.css)
    if (move !== null && cell === "moved") {
      half.dataset.move = move.at
      if (move.file !== null) {
        half.dataset.across = ""
      }
    }
    if (tint !== "full" && cell !== "context" && cell !== "filler") {
      half.dataset.tint = tint
      // a line of a quiet run of the marks: what it changed in, for a screen reader (diff-view.css)
      if (run !== null) {
        half.dataset.run = run
      }
    }
    if (resolved !== null) {
      half.dataset.resolved = resolved
    }
    return half
  }

  private number(line: number | null): HTMLElement {
    const cell = element("span", "dv-num", "diff-line-number")
    cell.setAttribute("aria-hidden", "true")
    if (line !== null) {
      cell.textContent = String(line + 1)
    }
    return cell
  }

  /**
   * The button of a fold, which expands it: "⋯ 62 unchanged lines", the lines the whitespace policy hides there ("1 line
   * hidden by Whitespace"), the scope around them (`foldWords`); the count is of the unchanged lines, never of a block
   * another section shows (`foldTargets` names those) nor of one the policy hides
   */
  private foldButton(file: number, fold: FoldRow): HTMLElement {
    const button = element("button", "dv-fold-button", "diff-fold")
    button.setAttribute("type", "button")
    button.tabIndex = -1
    button.dataset.file = String(file)
    button.dataset.gap = String(fold.gap)
    const { lines } = foldWords(fold)
    const what = lines === "" ? "the folded lines" : lines
    button.setAttribute("aria-label", fold.label === null ? `Expand ${what}` : `Expand ${what} in ${fold.label}`)
    const text = element("span", "")
    text.textContent = lines === "" ? "⋯" : `⋯ ${lines}`
    button.append(text)
    if (fold.label !== null) {
      const label = element("span", "dv-fold-label", "diff-fold-label")
      label.textContent = fold.label
      button.append(label)
    }
    return button
  }

  /**
   * The blocks another section shows that a fold hides, after a `·` a button for each place they are shown at, which
   * opens it as the row of such a block does: "· 1 change in part 2" (`foldWords`)
   */
  private foldTargets(file: number, fold: FoldRow): HTMLElement[] {
    return foldWords(fold).places.flatMap(({ block, words }) => {
      const separator = element("span", "dv-fold-separator")
      separator.textContent = "·"
      const button = element("button", "dv-fold-target")
      button.setAttribute("type", "button")
      button.tabIndex = -1
      button.dataset.file = String(file)
      button.dataset.block = String(block)
      button.dataset.testid = "diff-fold-elsewhere"
      button.textContent = words
      return [separator, button]
    })
  }

  /**
   * What the diff says of the lines after a note (`NoteRow.remark`), in the words of its presenter: "Formatting only ·
   * 3 lines". A note that opens (Inline shows its run once, or leaves out the lines a move took) is a button with a
   * chevron, which shows the old lines or hides them again, and where a key does the same for the notes in view
   * (`PainterModel.notesKey`) its hint; the hits of the find in the old lines it stands for are named after its
   * words. The note at an end of a move names the other end after its words ("14 lines moved" "to line 210"), a
   * button that shows it, then what else happened to the lines ("re-indented"); the hint of the key of the moves is
   * on the button of the one note the key takes (`markMoveKey`). The other end of a move between two files is named by
   * its file and its line ("Moved" "to Report.kt:12"), plain words where the view cannot show that file. The note of renames sets the two spellings of each
   * rename it words in the font of the code ("Renamed total → sum · 12 sites") and counts the others ("and 2 more"),
   * which a hover lists and a screen reader hears after the count, in a note of plain words as in the name of one
   * that opens; after its words, what the subject says of each rename worded: the file that declares it ("declared in
   * Store.kt", a button that shows the declaration where the view can show that file) and what the index of the
   * repository proved of its uses ("all uses in this change"). Not focusable, as a fold: a review has hundreds. The
   * keys of the view reach a note that opens and the place of a move (`notesKey`, `moveKey`); the place of a
   * declaration has no key, and the menu of the code on its note shows it (`declarationAt`)
   */
  private remark(file: number, note: NoteRow, remark: Remark): Node[] {
    const { label, renamed, target, detail, more, reach, name, toggle } = presentRemark(this.factsOf(file, remark))
    const words = element(toggle === null ? "span" : "button", toggle === null ? "dv-remark" : "dv-remark dv-remark-button", "diff-remark")
    words.dataset.kind = remark.kind
    if (toggle !== null) {
      words.setAttribute("type", "button")
      words.tabIndex = -1
      words.dataset.file = String(file)
      words.setAttribute("aria-expanded", String(remark.open === true))
      words.setAttribute("aria-label", `${name}: ${toggle}`)
      words.title = toggle
      const key = this.model?.notesKey
      if (key !== undefined) {
        // drawn after the words as generated content (diff-view.css): no text of the row, as the type of a line
        words.dataset.key = key
        words.setAttribute("aria-keyshortcuts", key)
      }
      words.append(element("span", "dv-remark-chevron"))
    }
    const text = element("span", "dv-remark-label")
    text.textContent = label
    words.append(text)
    const said = (className: string, content: string, testId?: string) => {
      const span = element("span", className, testId)
      span.textContent = content
      return span
    }
    for (const [at, rename] of (renamed ?? []).entries()) {
      // the two spellings of the name in the font of the code, as the name of a heading; the arrow and the sites in the font of the text
      words.append(
        at === 0 ? document.createTextNode(" ") : said("dv-remark-detail", " · "),
        said("dv-remark-name", rename.old),
        said("dv-remark-detail", " → "),
        said("dv-remark-name", rename.new),
        said("dv-remark-detail", ` · ${rename.sites}`),
      )
    }
    const parts: Node[] = [words]
    const reaching = said("dv-remark-detail", ` · ${detail}`)
    /** The words of a place another row or another file shows: a button of its own beside the one that opens the note */
    const place = (text: string, hover: string, testId: string) => {
      const jump = element("button", "dv-remark-target", testId)
      jump.setAttribute("type", "button")
      jump.tabIndex = -1
      jump.dataset.file = String(file)
      jump.textContent = text
      jump.setAttribute("aria-label", hover)
      jump.title = hover
      return jump
    }
    if (renamed !== null) {
      if (more !== null) {
        // the renames named after the ones worded: counted, each told on a hover, and to a screen reader after the count
        const others = said("dv-remark-detail", ` · ${more.words}`, "diff-remark-more")
        others.title = more.names
        others.append(said("sr-only", more.said))
        words.append(others)
      }
      // what the subject says of each rename worded, after the words of the note: where another file declares it, a
      // button that shows the declaration where the view can, and what the index proved of its uses
      for (const told of reach) {
        parts.push(said("dv-remark-detail", " · "))
        if (told.of !== null) {
          parts.push(said("dv-remark-name", told.of), document.createTextNode(" "))
        }
        if (told.declared !== null) {
          if (told.declared.jump) {
            const jump = place(told.declared.words, told.declared.name, "diff-rename-declared")
            jump.dataset.rename = String(told.rename)
            parts.push(jump)
          } else {
            const plain = said("dv-remark-detail", told.declared.words, "diff-rename-declared")
            plain.title = told.declared.name
            parts.push(plain)
          }
        }
        if (told.said !== "") {
          const proof = said("dv-remark-detail", told.declared === null ? told.said : ` · ${told.said}`, "diff-rename-proof")
          proof.dataset.proof = told.proof ?? ""
          parts.push(proof)
        }
      }
    } else if (target === null) {
      words.append(reaching)
    } else {
      // the other end of the move: a button that shows it, with its key; plain words for an end in a file the view
      // does not show
      const other = target.jump ? place(target.words, target.name, "diff-move-target") : said("dv-remark-detail", target.words, "diff-move-place")
      // plain words say in full which file and line they mean, on a hover
      other.title = target.name
      parts.push(document.createTextNode(" "), other)
      if (detail !== "") {
        parts.push(reaching)
      }
    }
    // the hits in the old lines the note stands for: the find lands on the note, which says how many it holds
    const hits = this.model?.hits?.get(file)
    let matches = 0
    let current = false
    const [oldStart, oldEnd] = note.lines[OLD]!
    for (let line = oldStart; hits !== undefined && line < oldEnd; line++) {
      const found = hits.get(lineKey(OLD, line))
      matches += (found?.ranges.length ?? 0) / 2
      current ||= (found?.current ?? -1) >= 0
    }
    if (matches === 0) {
      return parts
    }
    const hit = element("span", current ? "dv-remark-hits dv-hit dv-hit-current" : "dv-remark-hits dv-hit", "diff-remark-hits")
    hit.dataset.hit = current ? "current" : ""
    hit.textContent = formatHiddenMatches(matches, oldEnd - oldStart)
    parts.push(hit)
    return parts
  }

  /**
   * A remark as its presenter words it (`RemarkFacts`): with the file of the other end of a move between two files,
   * and of each rename what the subject says of it, the file that declares it among it, each file as the view knows it
   * (`PainterFile.otherFile`)
   */
  private factsOf(file: number, remark: Remark): RemarkFacts {
    const other = (index: number): OtherFile => this.model?.files[file]?.otherFile?.(index) ?? UNKNOWN_FILE
    const { move, renames } = remark
    return {
      kind: remark.kind,
      lines: remark.lines,
      open: remark.open,
      move:
        move === null
          ? null
          : {
              at: move.at,
              move: move.move,
              other: move.other,
              file: move.file === null ? null : other(move.file),
              declaration: move.declaration ?? null,
              reordered: move.reordered ?? null,
            },
      renames:
        renames === null
          ? null
          : {
              renames: renames.renames.map((rename, at) => {
                const reach = renames.reach[at]
                return {
                  old: rename.old,
                  new: rename.new,
                  sites: rename.sites,
                  proof: reach?.proof ?? null,
                  declared: reach?.declared == null ? null : { file: other(reach.declared.file), line: reach.declared.line, moved: reach.declared.moved ?? null },
                  across: reach?.across ?? null,
                }
              }),
              hides: renames.hides,
            },
    }
  }

  /** Whether the view can show the other end of a move: an end of the file itself, or one in a file the view shows a place of */
  private reaches(file: number, end: MoveEnd): boolean {
    return end.file === null || this.model?.files[file]?.otherFile?.(end.file)?.shown === true
  }

  /**
   * The words of a note row: the heading of a change, with what the diff says of the run its block starts with; that
   * remark alone; or a button to the section that shows a block
   */
  private note(file: number, note: NoteRow): HTMLElement {
    if (note.tone === "remark") {
      const row = element("span", "dv-note-label")
      if (note.remark !== null) {
        appendAll(row, this.remark(file, note, note.remark))
      }
      return row
    }
    if (note.tone === "unit") {
      const heading = element("span", "dv-note-label")
      heading.dataset.testid = "diff-unit-heading"
      // the name of a declaration in the code font, what changed and the marks between them in the font of the text
      const words = (text: string, alone = false) => {
        const span = element("span", alone ? "dv-note-words dv-note-alone" : "dv-note-words")
        span.textContent = text
        heading.append(span)
      }
      for (const [index, part] of (note.heading ?? [{ name: null, words: note.label }]).entries()) {
        if (index > 0) {
          words(" · ")
        }
        if (part.name !== null) {
          const name = element("span", "dv-note-name")
          name.textContent = part.name
          heading.append(name)
        }
        if (part.words !== "") {
          // a heading of no declaration ("Changes outside a declaration") is the heading itself, in the text colour
          words(part.name === null ? part.words : ` — ${part.words}`, part.name === null)
        }
      }
      if (note.remark === null) {
        return heading
      }
      // the block starts with a run: its remark in the row of the heading, after it
      const row = element("span", "dv-note-label")
      heading.className = "dv-note-heading"
      const separator = element("span", "dv-note-words")
      separator.textContent = " · "
      row.append(heading, separator)
      appendAll(row, this.remark(file, note, note.remark))
      return row
    }
    const button = element("button", "dv-note-button")
    button.setAttribute("type", "button")
    button.tabIndex = -1
    button.dataset.file = String(file)
    button.dataset.testid = "diff-unit-elsewhere"
    button.textContent = note.label
    return button
  }

  /** The code of one side of a row: plain text, or the parts of its tokens and ranges; wrapped in visual lines */
  private code(file: number, painterFile: PainterFile, value: LineRow, side: number, line: number | null, tables: readonly (LineTable | null)[], height: number): HTMLElement {
    const cell = element("span", "dv-code", "diff-code")
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
    // a renamed name of the line says what it was, or what it became, on a hover; a struck-through part is of the old line
    const named = painterFile.cells!.renamed(side, line)
    const oldLine = value.lines[OLD]
    const struck = value.kind === "inline" && oldLine != null ? painterFile.cells!.renamed(OLD, oldLine) : []
    const nameOf =
      named.length === 0 && struck.length === 0
        ? undefined
        : (part: LinePart | InlinePart) => (part.deleted ? renamedPart(struck, OLD, part.start) : renamedPart(named, side, part.start))
    if (this.model!.softWraps && height > this.model!.layout.metrics.rowHeight) {
      this.paintWrapped(text, parts, textOf, painterFile.codeWidths[side] ?? 0, nameOf)
    } else {
      appendAll(text, partNodes(parts, textOf, nameOf))
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
      const pair = inlinePair(lineText(oldTable, oldLine), content, cells.ranges(value, OLD) ?? [], cells.ranges(value, NEW) ?? [])
      // the hits of the old line: in its deleted parts, and on into the text the row shows from the new line
      const oldHits = this.model?.hits?.get(file)?.get(lineKey(OLD, oldLine)) ?? null
      // the old line of an edited line of a move is found where it left, every hit of it: the row of its pair marks
      // the ones it shows of that line alone, the struck-through parts; the text both lines share is the new line's
      return inlineParts(pair, runsAt(OLD, oldLine), runsAt(NEW, line), hits, value.move === null || oldHits === null ? oldHits : ownOldHits(pair, oldHits))
    }
    // a line that Inline shows once stands for its old lines: a hit of theirs is marked where the find says the line stands for it
    return lineParts(content.length, runsAt(side, line), cells.ranges(value, side), hits, value.kind === "once" ? (hits?.stands ?? null) : null)
  }

  private paintWrapped(
    text: HTMLElement,
    parts: readonly (LinePart | InlinePart)[],
    textOf: (part: LinePart | InlinePart) => string,
    width: number,
    nameOf?: (part: LinePart | InlinePart) => NamePart | null,
  ): void {
    const full = parts.map(textOf).join("")
    const breaks = [...wrapBreaks(full, width, this.model!.textMetrics), full.length]
    let offset = 0
    let start = 0
    let partIndex = 0
    for (const end of breaks) {
      const line = element("span", "dv-vl", "diff-visual-line")
      const lineParts: (LinePart | InlinePart)[] = []
      // the parts are in the order of the composed text: cut them at the break
      while (partIndex < parts.length) {
        const part = parts[partIndex]!
        const length = part.end - part.start
        const partStart = offset
        const partEnd = offset + length
        if (length === 0 && partStart === end && end !== full.length) {
          // a marker at a break stands at the start of the next visual line
          break
        }
        const cutStart = Math.max(partStart, start)
        const cutEnd = Math.min(partEnd, end)
        if (cutStart < cutEnd) {
          lineParts.push({ ...part, start: part.start + cutStart - partStart, end: part.start + cutEnd - partStart })
        } else if (length === 0 && partStart >= start && (partStart < end || end === full.length)) {
          // a marker of an empty side, in the visual line where its offset stands
          lineParts.push(part)
        }
        if (partEnd > end) {
          break
        }
        offset = partEnd
        partIndex++
      }
      appendAll(line, partNodes(lineParts, textOf, nameOf))
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
    // the user selects now: the name a reveal selected is selected no more, and no paint selects it again. Another
    // button than the first selects nothing by itself: the menu of the code it opens acts on the name of the old line
    // while the selection is that name (`onSelectionChange` lets it go when the press selects a word)
    this.revealedName = null
    if (event.button === 0) {
      this.letRevealedOldGo()
    }
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
    // a key of the user that moves or types: the name a reveal selected is not selected again by a paint (⌘A selects
    // all, Back reveals another). A modifier alone and the copy act on the name; the keys of the code (Go to
    // Declaration, Find Usages) read it for as long as the selection is the name (`onSelectionChange`)
    if (!isModifierKey(event) && !isSystemCopyKey(event)) {
      this.revealedName = null
    }
    const model = this.model
    const scroller = this.root
    if (model === null || scroller === null || event.altKey || event.shiftKey || !(event.metaKey || event.ctrlKey) || event.code !== "KeyA") {
      if (!(event.metaKey || event.ctrlKey)) {
        this.selectAll = null
      }
      return
    }
    const file = model.layout.fileAtTop(Math.max(this.viewTop(), 0))
    const host = this.hosts.get(file)
    if (host === undefined) {
      return
    }
    event.preventDefault()
    document.getSelection()?.selectAllChildren(host)
    this.selectAll = { file, side: sideBySide(model.files[file]) ? Number(scroller.dataset.selectSide ?? NEW) : NEW }
  }

  /** A copy gives the lines of the model between the ends of the selection, also of rows no longer painted */
  /**
   * The word of code under a point of the viewport: a name to go to the declaration of; null outside the code and
   * beside its text. The caret of a point beside the text (the number of a line, the blank end of a row, the space
   * under the last row) is the nearest place of the text, and the word there is not under the point
   */
  wordAt(clientX: number, clientY: number): CodeWord | null {
    const caret = caretAt(clientX, clientY)
    const word = caret === null ? null : this.wordOfPoint(caret.node, caret.offset)
    if (word === null) {
      return null
    }
    const rowHeight = this.model!.layout.metrics.rowHeight
    return word.rects().some(rect => {
      // the text of a line is lower than its row: a point between two lines is on the nearer one
      const slack = Math.max(rowHeight - rect.height, 0) / 2
      return rect.left <= clientX && clientX <= rect.right && rect.top - slack <= clientY && clientY <= rect.bottom + slack
    })
      ? word
      : null
  }

  /** The word at the start of the selection in the view, or at the caret a click left */
  wordAtSelection(): CodeWord | null {
    const selection = document.getSelection()
    if (selection === null || selection.rangeCount === 0 || selection.anchorNode === null) {
      return null
    }
    // the name of the old line a reveal selected in an Inline row: the selection may start in the text of the new line
    if (this.revealedOld !== null && this.holdsRevealedOld()) {
      return this.revealedOld.word
    }
    const range = selection.getRangeAt(0)
    return this.wordOfPoint(range.startContainer, range.startOffset)
  }

  private wordOfPoint(node: Node, offset: number): CodeWord | null {
    const model = this.model
    if (model === null || this.root === null || !this.root.contains(node)) {
      return null
    }
    const point = pointOf(node, offset)
    const painterFile = point === null ? undefined : model.files[point.file]
    if (point === null || painterFile?.rows == null) {
      return null
    }
    const value = rowAt(painterFile.rows, point.row)
    // a point in a deleted part of an Inline row is a point of the old line, which the cell of the new line shows
    const side = point.struck === null ? point.side : OLD
    const line = isLineRow(value) ? value.lines[side] : null
    const table = this.tablesOf(painterFile)[side]
    if (line == null || table == null) {
      return null
    }
    const [start, end] = wordBounds(lineText(table, line), point.struck ?? point.offset)
    if (start === end) {
      return null
    }
    const element = node instanceof Element ? node : node.parentElement
    return this.wordOf(point.file, painterFile, value, side, line, start, end, element?.closest<HTMLElement>(".dv-code") ?? null)
  }

  /** The word `[start, end)` of a line of a side that the row `value` shows in `cell` */
  private wordOf(file: number, painterFile: PainterFile, value: Row, side: number, line: number, start: number, end: number, cell: HTMLElement | null): CodeWord {
    const table = this.tablesOf(painterFile)[side]
    const pair = side === OLD ? this.inlinePairOf(painterFile, value) : null
    return {
      file,
      side,
      line,
      start,
      end,
      text: table == null ? "" : lineText(table, line).slice(start, end),
      rects: () => {
        if (cell === null) {
          return []
        }
        // a word of the old line of an Inline row: where the row shows it, struck through or in the text of the new line
        return pair === null ? textRects(cell, start, end, NOT_OF_THE_LINE) : textRects(cell, inlineOffset(pair, OLD, start), inlineOffset(pair, OLD, end, true), NOT_OF_THE_ROW)
      },
    }
  }

  /** The two lines of the row of a similar pair in Inline (`inlinePair`); null for another row, or while a text is not read */
  private inlinePairOf(painterFile: PainterFile, value: Row): InlinePair | null {
    const [oldTable, newTable] = this.tablesOf(painterFile)
    if (value.kind !== "inline" || painterFile.cells === null || oldTable == null || newTable == null) {
      return null
    }
    return inlinePair(
      lineText(oldTable, value.lines[OLD]!),
      lineText(newTable, value.lines[NEW]!),
      painterFile.cells.ranges(value, OLD) ?? [],
      painterFile.cells.ranges(value, NEW) ?? [],
    )
  }

  /**
   * The end of a move at a point of the viewport, for the menu of the code: of a line of a move, or of the note at
   * its end; null where the row under the point is of no move
   */
  moveAt(clientX: number, clientY: number): { file: number; end: MoveEnd } | null {
    return this.moveOfRow(document.elementFromPoint(clientX, clientY)?.closest<HTMLElement>(".dv-row") ?? null)
  }

  /**
   * The end of a move at the start of the selection of the view, for the menu of the code a key opens: of the line of
   * a move the selection starts in; null where nothing of the view is selected (after a jump to an end, which selects
   * nothing: the key of the moves goes on from there)
   */
  moveAtSelection(): { file: number; end: MoveEnd } | null {
    const anchor = document.getSelection()?.anchorNode ?? null
    const element = anchor instanceof Element ? anchor : (anchor?.parentElement ?? null)
    return this.moveOfRow(element?.closest<HTMLElement>(".dv-row") ?? null)
  }

  /**
   * The end of a move the view stands at, by the section that shows it: where a jump, Back or Forward landed, or the
   * note N or P stopped under; null once the view moved on to a place that is no end of a move. The key of the moves
   * takes its note first (`keyedMove`)
   */
  setMoveHand(hand: MoveHand | null): void {
    this.moveHand = hand
    if (this.root !== null) {
      this.markMoveKey(this.viewTop(), this.viewHeight())
    }
  }

  /**
   * The note of a move the key of the moves takes now (`moveNoteOfKey`): the one at the end the view stands at while
   * it is in view, else the first note of a move in view; null with none in view
   */
  keyedMove(): { file: number; end: MoveEnd; row: number } | null {
    const model = this.model
    return model === null || this.root === null ? null : moveNoteOfKey(model.layout, this.viewTop(), this.viewHeight(), this.moveHand, (file, end) => this.reaches(file, end))
  }

  /**
   * The hint of the key of the moves (`PainterModel.moveKey`), on the button of the one note the key takes now in a
   * view at `top`, `height` high: after every paint, with the place the paint read, so it follows the scroll and the
   * place the view stands at; no note shows a key that would act on another one. Drawn after the words of the button
   * as generated content (diff-view.css), no text of the row
   */
  private markMoveKey(top: number, height: number): void {
    const model = this.model
    const key = model?.moveKey
    const taken = model === null || key === undefined ? null : moveNoteOfKey(model.layout, top, height, this.moveHand, (file, end) => this.reaches(file, end))
    const button = taken === null ? null : (this.painted.get(taken.file)?.get(taken.row)?.element.querySelector<HTMLElement>(".dv-remark-target") ?? null)
    if (button === this.keyedMoveButton) {
      return
    }
    if (this.keyedMoveButton !== null) {
      delete this.keyedMoveButton.dataset.key
      this.keyedMoveButton.removeAttribute("aria-keyshortcuts")
    }
    if (button !== null && key !== undefined) {
      button.dataset.key = key
      button.setAttribute("aria-keyshortcuts", key)
    }
    this.keyedMoveButton = button
  }

  /** What a painted row of the view shows, and its file; null for an element that is no row of the view */
  private valueOfRow(row: HTMLElement | null): { file: number; value: ReturnType<typeof rowAt> } | null {
    const file = Number(row?.parentElement?.dataset.file)
    const rows = this.model?.files[file]?.rows
    if (row === null || this.root === null || !this.root.contains(row) || rows == null || row.dataset.row === undefined) {
      return null
    }
    return { file, value: rowAt(rows, Number(row.dataset.row)) }
  }

  private moveOfRow(row: HTMLElement | null): { file: number; end: MoveEnd } | null {
    const found = this.valueOfRow(row)
    if (found === null) {
      return null
    }
    const { file, value } = found
    const end = value.kind === "fold" ? null : isLineRow(value) ? value.move : (value.remark?.move ?? null)
    return end === null ? null : { file, end }
  }

  /**
   * The declaration the note of renames at a point of the viewport names as a place the view can show ("declared in
   * Store.kt", a button), for the menu of the code: the note, the number of that rename among its renames, and what
   * the button says in full; null where the row under the point is no such note
   */
  declarationAt(clientX: number, clientY: number): { file: number; note: RenameNote; rename: number; hover: string } | null {
    const found = this.valueOfRow(document.elementFromPoint(clientX, clientY)?.closest<HTMLElement>(".dv-row") ?? null)
    const remark = found === null || found.value.kind !== "note" ? null : found.value.remark
    if (found === null || remark?.renames == null) {
      return null
    }
    const told = presentRemark(this.factsOf(found.file, remark)).reach.find(it => it.declared?.jump === true)
    return told === undefined ? null : { file: found.file, note: remark.renames, rename: told.rename, hover: told.declared!.name }
  }

  /**
   * Marks the row a reveal of a line landed on (`rowOfPlace`): now if it is painted, else once a paint paints it, and
   * again after a paint that dropped it. A later call replaces it; a scroll, a press or a key of the user ends it, as
   * a name a reveal selects
   */
  land(file: number, side: number, line: number): void {
    this.letRevealedNameGo()
    this.letLandedGo()
    // what was selected in the view is of the place the view left: the line is its place now, which Back and Forward keep
    const selection = document.getSelection()
    if (selection !== null && selection.anchorNode !== null && this.root?.contains(selection.anchorNode) === true) {
      selection.removeAllRanges()
      this.keptSelection = null
    }
    const key = this.model?.files[file]?.key
    if (key !== undefined) {
      this.landed = { key, side, line, element: null }
      this.markLanded()
    }
  }

  private letLandedGo(): void {
    delete this.landed?.element?.dataset.landed
    this.landed = null
  }

  private markLanded(): void {
    const landed = this.landed
    const file = landed === null ? -1 : (this.model?.layout.indexOf(landed.key) ?? -1)
    const rows = this.model?.files[file]?.rows
    if (landed === null || rows == null) {
      return
    }
    const element = this.painted.get(file)?.get(rowOfPlace(rows, landed.side, landed.line))?.element ?? null
    if (element !== null && element !== landed.element) {
      delete landed.element?.dataset.landed
      element.dataset.landed = ""
      landed.element = element
    }
  }

  /**
   * Selects the columns `[start, end)` of a line of a side, or the first whole word `word` there when given, the name a
   * reveal shows: now if its row is painted with its
   * text, else once a paint paints it so (the text of a file outside the review comes after its layout, later under
   * load), and again after a paint that dropped it. A later call replaces it; a press or a key in the view, or a
   * selection elsewhere, ends it
   */
  selectWord(file: number, side: number, line: number, start: number, end: number, word: string | null = null): void {
    this.letRevealedNameGo()
    this.letLandedGo()
    this.revealedName = { file, side, line, start, end, word, selected: false }
    this.selectRevealedName()
  }

  /** The name a reveal selected is selected no more: the deleted parts of its Inline row are not selectable again */
  private letRevealedNameGo(): void {
    this.revealedName = null
    this.letRevealedOldGo()
  }

  private letRevealedOldGo(): void {
    delete this.revealedOld?.row.dataset.revealedOld
    this.revealedOld = null
  }

  /** The selection of the page is the name of the old line that a reveal selected in its Inline row */
  private holdsRevealedOld(): boolean {
    const selection = document.getSelection()
    const revealed = this.revealedOld
    if (revealed === null || selection === null || selection.isCollapsed || selection.rangeCount === 0) {
      return false
    }
    // by the text of the row before each end, not by the nodes: an engine may put an end at the same place of the next node
    const range = selection.getRangeAt(0)
    return (
      revealed.cell.contains(range.startContainer) &&
      revealed.cell.contains(range.endContainer) &&
      textOffset(revealed.cell, range.startContainer, range.startOffset, NOT_OF_THE_ROW) === revealed.from &&
      textOffset(revealed.cell, range.endContainer, range.endOffset, NOT_OF_THE_ROW) === revealed.to
    )
  }

  private selectRevealedName(): void {
    const name = this.revealedName
    const painterFile = name === null ? undefined : this.model?.files[name.file]
    const rows = painterFile?.rows
    if (name === null || painterFile === undefined || rows == null) {
      return
    }
    const row = rowOfLine(rows, name.side, name.line)
    const entry = this.painted.get(name.file)?.get(row)
    if (entry === undefined || !entry.textsRead || holdsSelection(entry.element)) {
      return
    }
    const value = rowAt(rows, row)
    // the row of a similar pair in Inline shows its old line in the cell of the new one
    const pair = name.side === OLD ? this.inlinePairOf(painterFile, value) : null
    const own = entry.element.querySelector<HTMLElement>(`.dv-code[data-side="${pair === null ? name.side : NEW}"]`)
    // one column shows an unchanged line once, by its new line: the old line has no cell of its own
    const shown = own === null && pair === null && name.side === OLD && value.kind === "context" ? NEW : name.side
    const cell = shown === name.side ? own : entry.element.querySelector<HTMLElement>(`.dv-code[data-side="${shown}"]`)
    const tables = this.tablesOf(painterFile)
    const table = tables[name.side]
    // a row that stands for the line without showing it (a fold not expanded yet) has no name to select
    if (cell === null || table == null || !isLineRow(value) || value.lines[name.side] !== name.line) {
      return
    }
    // a name without its columns is the first whole word of that name in the line: the line of the model, since the
    // text of the cell of an Inline row holds the deleted parts of the old line too
    const text = lineText(table, name.line)
    const at = name.word === null ? -1 : wordColumn(text, name.word)
    let [start, end] = at < 0 ? [name.start, name.end] : [at, at + name.word!.length]
    if (shown !== name.side) {
      // the columns are of the old line, the cell holds the new one: the same text but for the whitespace of a block
      // the policy ignores (another indent), where the name stands at other columns
      const line = value.lines[shown]
      const shownTable = tables[shown]
      if (line == null || shownTable == null) {
        return
      }
      const shownText = lineText(shownTable, line)
      start = sameTextOffset(text, shownText, start)
      end = sameTextOffset(text, shownText, end, true)
    }
    let range: Range
    if (pair === null) {
      range = textRange(cell, start, end, NOT_OF_THE_LINE)
    } else {
      if (start >= end) {
        return
      }
      // where the row shows the name: in the text it shares with the new line, or struck through in a deleted part,
      // which is selectable for as long as the name is selected
      const from = inlineOffset(pair, OLD, start)
      const to = inlineOffset(pair, OLD, end, true)
      this.letRevealedOldGo()
      entry.element.dataset.revealedOld = ""
      this.revealedOld = { row: entry.element, cell, from, to, word: this.wordOf(name.file, painterFile, value, OLD, name.line, start, end, cell) }
      range = textRange(cell, from, to, NOT_OF_THE_ROW)
    }
    const selection = document.getSelection()
    selection?.removeAllRanges()
    selection?.addRange(range)
    name.selected = true
  }

  /**
   * The selection changed. Its ends are kept while they are in the view (`keptSelection`). It is no more the name of
   * the old line a reveal selected in an Inline row: the deleted
   * parts of the row are not selectable again. It moved out of the view (the user selected in another one): the name a
   * reveal selected is let go. A reveal still waiting for the paint of its row keeps through the caret of a field: a
   * field moves it as its value is set, the search a reveal of a hidden file clears, and that change may come after
   * the reveal and before the paint (under load). Once the name is selected, a click into a field lets it go
   */
  private onSelectionChange(): void {
    const selection = document.getSelection()
    const anchor = selection?.anchorNode ?? null
    const focus = selection?.focusNode ?? null
    if (selection === null || selection.rangeCount === 0 || anchor === null || focus === null) {
      // no selection at all is no gesture of the user (a click leaves a caret): the engine cleared it as the focus left
      // (`keptSelection`). What the view holds for the selection waits for the return of the focus
      return
    }
    this.keptSelection =
      this.root !== null && this.root.contains(anchor) && this.root.contains(focus)
        ? { anchor, anchorOffset: selection.anchorOffset, focus, focusOffset: selection.focusOffset }
        : null
    if (this.revealedOld !== null && !this.holdsRevealedOld()) {
      this.letRevealedOldGo()
    }
    if (this.revealedName === null || this.root === null || this.root.contains(anchor)) {
      return
    }
    if (!this.revealedName.selected && selection.isCollapsed && isFieldCaret(anchor, selection.anchorOffset)) {
      return
    }
    this.letRevealedNameGo()
  }

  /**
   * The focus is in the view again: the selection the engine cleared as the focus left (`keptSelection`) is put back,
   * if its rows are still the painted ones, each end at an offset its node still has: a repaint meanwhile may have taken
   * the children or the text an end counted (WebKit threw "The index is not in the allowed range" at the focus). An
   * engine that keeps the selection (Chromium) has nothing to put back
   */
  private restoreSelection(): void {
    const kept = this.keptSelection
    const selection = document.getSelection()
    if (kept === null || selection === null || selection.rangeCount > 0) {
      return
    }
    const holds = (node: Node, offset: number) => this.root?.contains(node) === true && offset <= (node instanceof CharacterData ? node.length : node.childNodes.length)
    if (holds(kept.anchor, kept.anchorOffset) && holds(kept.focus, kept.focusOffset)) {
      selection.setBaseAndExtent(kept.anchor, kept.anchorOffset, kept.focus, kept.focusOffset)
    } else {
      this.keptSelection = null
    }
  }

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
    if (this.revealedOld !== null && this.holdsRevealedOld()) {
      // the name of the old line that a reveal selected in an Inline row: a copy of the lines takes the new line
      text = this.revealedOld.word.text
    } else if (this.selectAll !== null) {
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
    // a selection that holds no text of a line (a fold, a note) is the browser's to copy: never an empty clipboard
    if (text === "") {
      return
    }
    event.clipboardData.setData("text/plain", text)
    event.preventDefault()
  }

  private onClick(event: MouseEvent): void {
    const button =
      event.target instanceof Element ? event.target.closest<HTMLElement>(".dv-fold-button, .dv-note-button, .dv-fold-target, .dv-remark-button, .dv-remark-target") : null
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
    if (value.kind === "fold" && button.dataset.block !== undefined) {
      // a block another section shows, hidden in a fold: as the row of such a block, it opens that section
      const block = Number(button.dataset.block)
      const label = value.elsewhere.find(it => it.block === block)?.label ?? ""
      this.callbacks.onNote(file, { kind: "note", tone: "elsewhere", label, heading: null, block, lines: [], remark: null })
    } else if (value.kind === "fold") {
      this.keepFocus()
      this.callbacks.onFold(file, value, event.altKey)
    } else if (value.kind === "note" && button.classList.contains("dv-remark-target")) {
      if (button.dataset.rename !== undefined) {
        // the file a note of renames names as the one that declares a name: the declaration there
        if (value.remark?.renames != null) {
          this.callbacks.onDeclarationJump(file, value.remark.renames, Number(button.dataset.rename))
        }
      } else if (value.remark?.move != null) {
        // the place a note of a move names: the other end of the move
        this.callbacks.onMoveJump(file, value.remark.move)
      }
    } else if (value.kind === "note" && value.remark !== null && value.remark.open !== null) {
      this.keepFocus()
      this.callbacks.onNoteToggle(file, value, event.altKey)
    } else if (value.kind === "note") {
      this.callbacks.onNote(file, value)
    }
  }

  /** A button of a row goes with the rows it stood for (a fold, a note that opens): the diffs keep the focus, so their keys still work */
  private keepFocus(): void {
    if (this.scrollParent === null) {
      this.root?.focus({ preventScroll: true })
    }
  }
}

/**
 * How many views each scroller holds as blocks of it (`DiffPainter.attach` with a scroll parent): while it holds one
 * it is marked `data-dv-scroll-parent`, which turns the engine's own scroll anchoring off in it (diff-view.css), and
 * the mark goes with the last view
 */
const viewsOf = new WeakMap<HTMLElement, number>()

/** A file of the subject the view knows nothing of: named without a name, never shown */
const UNKNOWN_FILE: OtherFile = { path: "another file", name: "another file", shown: false }

/** What the part at column `start` of a line of a side is of a renamed name (`NamePart`); null where no site of `named` holds the column */
function renamedPart(named: readonly RenameSite[], side: number, start: number): NamePart | null {
  const site = named.find(it => it.start <= start && start < it.end)
  return site === undefined ? null : { hover: describeRenamedName(side === OLD ? "old" : "new", site.rename), starts: site.start === start }
}

/** Appends the nodes one by one: a line may have more parts than a call takes arguments */
function appendAll(parent: HTMLElement, nodes: readonly Node[]): void {
  for (const node of nodes) {
    parent.append(node)
  }
}

/** A key that is a modifier alone: it starts a chord and does nothing by itself */
function isModifierKey(event: KeyboardEvent): boolean {
  return event.key === "Meta" || event.key === "Control" || event.key === "Alt" || event.key === "Shift" || event.key === "AltGraph" || event.key === "CapsLock"
}

/** The copy of the system, which copies the selection: ⌘C or Ctrl+C by the key of the layout, Ctrl+Insert */
function isSystemCopyKey(event: KeyboardEvent): boolean {
  return (event.metaKey || event.ctrlKey) && !event.altKey && (event.key.toLowerCase() === "c" || event.key === "Insert")
}

/** An end of a non-empty selection is in `element` */
function holdsSelection(element: HTMLElement): boolean {
  const selection = document.getSelection()
  return selection !== null && !selection.isCollapsed && (element.contains(selection.anchorNode) || element.contains(selection.focusNode))
}

/** The characters of a name: letters, digits, `_` and `$` */
const NAME_CHARACTER = /[\p{L}\p{N}_$]/u

/** The name around a column of a line, `[start, end)`; empty where none is */
export function wordBounds(content: string, column: number): [number, number] {
  let start = Math.min(column, content.length)
  let end = start
  while (start > 0 && NAME_CHARACTER.test(content[start - 1]!)) {
    start--
  }
  while (end < content.length && NAME_CHARACTER.test(content[end]!)) {
    end++
  }
  // a number is no name
  return start < end && /\p{N}/u.test(content[start]!) ? [start, start] : [start, end]
}

/** The caret at a point: `caretPositionFromPoint` where the engine has it, else WebKit's `caretRangeFromPoint` */
function caretAt(clientX: number, clientY: number): { node: Node; offset: number } | null {
  const position = document.caretPositionFromPoint?.(clientX, clientY)
  if (position != null) {
    return { node: position.offsetNode, offset: position.offset }
  }
  const range = document.caretRangeFromPoint?.(clientX, clientY)
  return range == null ? null : { node: range.startContainer, offset: range.startOffset }
}

/** What a code cell holds beside the text of its line: the deleted parts of the old line in an Inline row, the mark of a file without a final line end */
const NOT_OF_THE_LINE = "[data-inline-deleted], .dv-eof"
/** What a code cell holds beside the text of an Inline row, the deleted parts of the old line with it (`inlineOffset`) */
const NOT_OF_THE_ROW = ".dv-eof"
/** Every text of an element counts: a selector no element matches */
const NOT_COUNTED_NOWHERE = ":not(*)"

/**
 * The point of a selection after `offset` characters of the text of an element, every text of it counted
 * (`textOffset` with `NOT_COUNTED_NOWHERE`): in the text node that holds the character there, at the end of the
 * last one for the end of the text, and at the element itself where it holds no text
 */
function textPoint(element: HTMLElement, offset: number): { node: Node; offset: number } {
  let length = 0
  let last: Node | null = null
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
  for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
    const size = text.textContent?.length ?? 0
    if (offset < length + size) {
      return { node: text, offset: Math.max(offset - length, 0) }
    }
    length += size
    last = text
  }
  return last === null ? { node: element, offset: 0 } : { node: last, offset: last.textContent?.length ?? 0 }
}

/**
 * Calls `visit` with each text node of a code cell that holds a part of `[start, end)` of its text, and that part as
 * offsets in the node; the text in `excluded` elements is not counted. A range without text is the place of `start`
 * in the node that holds it
 */
function forEachText(cell: HTMLElement, start: number, end: number, excluded: string, visit: (text: Node, from: number, to: number) => void): void {
  const empty = start >= end
  let length = 0
  const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT)
  for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
    if (text.parentElement?.closest(excluded) != null) {
      continue
    }
    const size = text.textContent?.length ?? 0
    if (empty ? start <= length + size : start < length + size) {
      visit(text, Math.max(start - length, 0), empty ? Math.max(start - length, 0) : Math.min(end - length, size))
    }
    length += size
    if (empty ? start <= length : end <= length) {
      return
    }
  }
}

/**
 * The range `[start, end)` of the text of a code cell, the text in `excluded` elements not counted: from the node that
 * holds its first character to the one that holds its last, so no text before or after it that is not counted (a
 * deleted part of an Inline row beside the name) lies in the range
 */
function textRange(cell: HTMLElement, start: number, end: number, excluded: string): Range {
  const range = document.createRange()
  let started = false
  forEachText(cell, start, end, excluded, (text, from, to) => {
    if (!started) {
      range.setStart(text, from)
      started = true
    }
    range.setEnd(text, to)
  })
  return range
}

/** Where `[start, end)` of the text of a code cell is drawn, in viewport coordinates: no text in `excluded` elements, also between its parts */
function textRects(cell: HTMLElement, start: number, end: number, excluded: string): DOMRect[] {
  const rects: DOMRect[] = []
  const range = document.createRange()
  forEachText(cell, start, end, excluded, (text, from, to) => {
    range.setStart(text, from)
    range.setEnd(text, to)
    for (const rect of range.getClientRects()) {
      rects.push(rect)
    }
  })
  return rects
}

/** The text of a code cell before a point of a selection, the text in `excluded` elements not counted */
function textOffset(cell: HTMLElement, node: Node, offset: number, excluded: string): number {
  const boundary = document.createRange()
  boundary.setStart(node, offset)
  boundary.collapse(true)
  let length = 0
  const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT)
  for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
    const skipped = text.parentElement?.closest(excluded) != null
    if (text === node) {
      return length + (skipped ? 0 : offset)
    }
    const size = text.textContent?.length ?? 0
    // a text that ends after the point is not before it
    if (boundary.comparePoint(text, size) > 0) {
      break
    }
    if (!skipped) {
      length += size
    }
  }
  return length
}

/**
 * A point of a selection in the model: the file and the row of its row element, the side of its code cell and the
 * offset in the line the cell shows. `struck`: for a point in a deleted part of an Inline row, its offset in the old
 * line (`offset` is then where the part stands in the new line); null elsewhere
 */
function pointOf(node: Node | null, offset: number): (CopyPoint & { side: number; struck: number | null }) | null {
  const element = node === null ? null : node instanceof Element ? node : node.parentElement
  const row = element?.closest<HTMLElement>(".dv-row")
  const file = row?.parentElement?.dataset.file
  if (row === null || row === undefined || file === undefined) {
    return null
  }
  const cell = element?.closest<HTMLElement>(".dv-code") ?? row.querySelector<HTMLElement>(".dv-code[data-side]")
  const inCell = cell !== null && cell.contains(node)
  const deleted = inCell ? (element?.closest<HTMLElement>("[data-inline-deleted]") ?? null) : null
  let struck: number | null = null
  if (deleted !== null) {
    // in the text of the part, or at the part itself: before it or after it
    const inside = node === deleted ? (offset > 0 ? (deleted.textContent?.length ?? 0) : 0) : offset
    struck = Number(deleted.dataset.inlineDeleted) + inside
  }
  return {
    file: Number(file),
    row: Number(row.dataset.row),
    // without the deleted parts of an Inline row: a point in one stands where the part does in the new line
    offset: inCell ? textOffset(cell, node!, offset, NOT_OF_THE_LINE) : 0,
    side: Number(cell?.dataset.side ?? NEW),
    struck,
  }
}

/**
 * The edges of a resolved block that a row draws (`data-resolved`): IntelliJ draws the border of a resolved change
 * above its first line and below its last one
 */
function resolvedEdges(row: LineRow): string {
  const first = row.blockRow === 0
  const last = row.blockRow === row.blockRows - 1
  return first && last ? "both" : first ? "first" : last ? "last" : "inside"
}

/** A file shows a column per side: Split, a merge */
function sideBySide(file: PainterFile | undefined): boolean {
  return (file?.cells?.columns.length ?? 1) > 1
}

/** The UTF-16 column of the first whole-word `word` in `text`; -1 when the text has none */
function wordColumn(text: string, word: string): number {
  const part = (char: string | undefined) => char !== undefined && /[\p{L}\p{N}_$]/u.test(char)
  for (let at = text.indexOf(word); at >= 0; at = text.indexOf(word, at + 1)) {
    if (!part(text[at - 1]) && !part(text[at + word.length])) {
      return at
    }
  }
  return -1
}
