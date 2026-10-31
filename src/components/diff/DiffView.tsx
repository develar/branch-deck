import { memo, startTransition, useEffect, useEffectEvent, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState } from "react"
import type { CSSProperties, KeyboardEvent, MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent, ReactNode, Ref } from "react"
import { flushSync } from "react-dom"
import { cn } from "cn"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { DiffPainter } from "@/components/diff/diff-painter"
import type { CodeWord, PainterCallbacks, PainterFile } from "@/components/diff/diff-painter"
import { DiffFindBar } from "@/components/diff/DiffFindBar"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import type { BlobRequest, CommitFile, DeclarationSite } from "@/lib/bindings"
import { codeMenuItems } from "@/lib/diff/code-menu"
import type { FileDiff, TextDiff } from "@/lib/diff/file-diff-model"
import { composeResult, resultTextId } from "@/lib/diff/file-merge-model"
import type { FileMerge, MergeMiddle, TextMerge } from "@/lib/diff/file-merge-model"
import { findHits, hitsByLine } from "@/lib/diff/find"
import { getHighlightClient } from "@/lib/diff/highlight/highlighter"
import { DiffLayout, visualLinePrefix } from "@/lib/diff/layout"
import { inlineParts } from "@/lib/diff/line-parts"
import { getLineTable, lineText, maxColumns } from "@/lib/diff/line-table"
import { DIFF_FONT_SIZE, DIFF_TAB_SIZE, measureFont, rowHeightOf, textMetricsOf } from "@/lib/diff/metrics"
import { diffCells, fileLayoutOf, mergeCells, oneSideOf } from "@/lib/diff/row-cells"
import type { RowCells } from "@/lib/diff/row-cells"
import {
  buildFileRows,
  buildMergeRows,
  EXPANDED,
  getFoldLevels,
  isLineRow,
  MIDDLE,
  NEW,
  nextFoldLevel,
  OLD,
  rowAt,
  rowOfLine,
  sectionFoldKey,
  setFoldLevels,
} from "@/lib/diff/rows"
import type { FileRows, FoldKey, FoldRow, SectionUnits } from "@/lib/diff/rows"
import { blobText, fileNotice, isEmptyNotice, languageOf, mergeNotice, startsCollapsed } from "@/lib/diff/view-files"
import { visualLineCount } from "@/lib/diff/wrap"
import type { TextMetrics } from "@/lib/diff/wrap"
import type { MenuItem } from "@/lib/menu"
import { isMac } from "@/lib/platform"
import { formatCollapse } from "@/presenters/collapse"
import { blobTextsStore, putDerivedText, requestBlobTexts } from "@/stores/blob-texts"
import type { BlobTextEntry } from "@/stores/blob-texts"
import { useAppSettings } from "@/stores/settings/appSettings"
import { MIN_SPLIT_RATIO, setDiffSplitRatio, useDiffLigatures, useDiffSplitRatio } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import "./diff-view.css"

/*
 * The file cards of a view with headers (docs/design/diff-view.md, "File cards"), as IntelliJ's combined diff: a gap of
 * 10 px between two cards and around them, an inset of 12 px at the sides, a border of 1 px. The header holds the top
 * border of its card.
 */
const CARD_HEADER_HEIGHT = 48
const CARD_GAP = 10
const CARD_INSET = 12
const CARD_BORDER = 1
/** The font of the rows, as `.dv-rows` of diff-view.css */
const ROWS_FONT = `${DIFF_FONT_SIZE}px "JetBrains Mono Variable", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`
/** The files after the window whose texts are read ahead */
const READ_AHEAD_FILES = 20

export interface DiffViewFile {
  /** The file of git's list, with the line counts of the engine */
  file: CommitFile
  /** Its diff; undefined when the engine has none for it, or while it is pending */
  diff: FileDiff | undefined
  /** The diff has not arrived yet: the header shows, the rows follow */
  pending?: boolean
  /** Why the review folds the file ("Imports only", "Same edit as Reader.kt"): it starts collapsed, its header says why */
  folded?: string
  /** The review plays down the blocks of the imports alone: pale, and N and P pass them (`buildFileRows`) */
  quietImports?: boolean
  /**
   * A section of a review (docs/design/diff-view.md, "Sections"): the key that tells it from another section of the same
   * file, the heading of the change each block starts, and for each block another section shows the key of that section,
   * which a click on its row opens
   */
  section?: DiffSection
}

export interface DiffSection {
  readonly key: string
  readonly units: SectionUnits
  readonly targets: ReadonlyMap<number, string>
  /** The words of the header: the group the section is of ("Same edit in 3 files") */
  readonly heading: string | null
}

/** A conflicting file in three columns: left (ours), the middle (`DiffViewProps.middle`), right (theirs) */
export interface MergeViewFile {
  path: string
  /** Its merge; undefined when the engine has none for it, or while it is pending. Never a one-sided one */
  merge: FileMerge | undefined
  pending?: boolean
}

export interface DiffHeaderProps {
  file: CommitFile
  /** Why the file shows no rows, or how its line ends changed */
  notice: string | null
  /** The notice says why the file shows no rows: nothing to expand */
  empty: boolean
  /** Why the file starts collapsed: the review folds it (`DiffViewFile.folded`), or the words of Rust's `collapse` */
  folded: string | null
  /** The group of a section of a review (`DiffSection.heading`) */
  heading: string | null
  collapsed: boolean
  onToggle: () => void
  /** Shows all the unchanged lines of the file; undefined when it folds none */
  onExpandAll?: () => void
}

export interface DiffViewHandle {
  /** Shows the file at the top */
  scrollToFile: (path: string) => void
  /**
   * Shows the next (1) or previous (-1) block of changed lines, from the last one shown in the file at the top; into
   * the next file at the end of one. Collapsed files are skipped. Returns the path of the file it shows
   */
  moveChange: (direction: 1 | -1) => string | undefined
  /** Shows the next (1) or previous (-1) section at the top, from the one at the top now; returns the path of its file */
  moveSection: (direction: 1 | -1) => string | undefined
  focus: () => void
  /**
   * Shows a declaration of the view: its file open, the fold around it expanded, its name selected; `from` is the place
   * Back returns to. False for a file the view does not show
   */
  revealDeclaration: (place: DeclarationPlace, from: DeclarationPlace | null) => boolean
}

/** A name in a side of a file of the view: a declaration to show, or the place a jump to one started */
export interface DeclarationPlace {
  path: string
  side: "old" | "new"
  /** The line from 0 and the UTF-16 columns of the name */
  line: number
  column: number
  endColumn: number
  /** The name, when its columns are not known: the first whole word of it in the line is selected */
  name?: string
}

export interface DiffViewProps {
  ref?: Ref<DiffViewHandle>
  files: readonly (DiffViewFile | MergeViewFile)[]
  /** The layout of the diffs; a merge always shows three columns */
  mode: DiffViewMode
  /** What the middle column of a merge shows */
  middle?: MergeMiddle
  softWraps: boolean
  /** The file shown first, at the top */
  initialPath: string | null
  /** The accessible name of the scroller */
  label: string
  /** The file whose header is at the top, as the user scrolls */
  onVisibleFileChange?: (path: string) => void
  /** The rows of the first screen are painted with their texts, and highlighted or 80 ms late */
  onFirstScreen?: () => void
  /**
   * Go to Declaration (spec/commit/symbol-navigation.spec.md): a ⌘-click (Ctrl-click off macOS) on a name, ⌘B on the
   * selection, the item of the menu of the code; `at` is where to show a choice of declarations
   */
  onGoToDeclaration?: (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace) => void
  /** Find Usages: ⌥F7 (Alt+F7 off macOS) on the selection, the item of the menu of the code; the arguments as above */
  onFindUsages?: (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace) => void
  /** A place shown once the view mounts: the declaration a view of a file outside the review opens at */
  initialPlace?: DeclarationPlace
  /** Back with no jump left to return along: the view of a file outside the review closes */
  onHistoryEnd?: () => void
  /** A file shown by itself, not a change: no tint of an added file */
  plain?: boolean
  /** The sticky header of each file; without it the view shows its files open, a notice in a row under each */
  renderHeader?: (props: DiffHeaderProps) => ReactNode
  /** "own": the view scrolls itself (the review windows); "parent": it is a block of the scroller it lies in */
  scroll?: "own" | "parent"
  /** The files are the uncommitted changes: the new side is read by its path in the working tree */
  worktree?: boolean
}

interface ViewMetrics {
  rowHeight: number
  columnWidth: number
  text: TextMetrics
}

function viewMetrics(): ViewMetrics {
  const font = measureFont(ROWS_FONT)
  return { rowHeight: rowHeightOf(font, window.devicePixelRatio || 1), columnWidth: font.columnWidth, text: textMetricsOf(font) }
}

interface PreparedFile {
  kind: "diff" | "merge"
  /** The key of its section: its path, or the key of a section of a review (`DiffSection`) */
  key: string
  path: string
  /** The file of git's list, for the header; a stand-in for a merge */
  file: CommitFile
  diff: FileDiff | undefined
  merge: FileMerge | undefined
  pending: boolean
  /** The text diff or the text merge whose rows show */
  model: TextDiff | TextMerge | null
  /** What the levels of its folds are kept by: the model, or of a section its own key */
  foldKey: FoldKey | null
  cells: RowCells | null
  rows: FileRows | null
  /** The blob id of the text of each side the rows show; the result of a merge is a text the view composes */
  ids: readonly (string | null)[]
  /** The path of each side: the old path for the old side of a rename */
  paths: readonly string[]
  /** The path in the working tree of each side that is a file of it */
  worktreePaths: readonly (string | null)[]
  /** The texts the view reads: the blobs of the sides, the new side of an uncommitted change by its path too */
  reads: readonly BlobRequest[]
  collapsed: boolean
  /** Why the file starts collapsed: the review folds it (`DiffViewFile.folded`), or the words of Rust's `collapse` */
  folded: string | null
  section: DiffSection | null
  /** The columns of the line numbers: the digits of the largest one, and its padding */
  gutterColumns: number
}

interface PrepareOptions {
  mode: DiffViewMode
  middle: MergeMiddle
  folds: ReadonlyMap<FoldKey, readonly number[]>
  /** The files start collapsed or open as `startsCollapsed` says, except these */
  toggled: ReadonlySet<string> | null
  worktree: boolean
}

/** What the view shows of a file: its rows, the texts of its sides, its cells */
function prepareFile(entry: DiffViewFile | MergeViewFile, { mode, middle, folds, toggled, worktree }: PrepareOptions): PreparedFile {
  const pending = entry.pending ?? false
  if ("merge" in entry) {
    const { merge } = entry
    const text = merge?.content.kind === "text" ? merge.content : null
    const rows = text === null ? null : buildMergeRows(text, middle, folds.get(text) ?? getFoldLevels(text))
    // the result of a copy that resolved the conflict by words is the blob the sync committed, else the one the engine proposes
    const result = text === null ? null : (merge!.resolvedId ?? resultTextId(text))
    const ids = merge === undefined ? [] : [merge.leftId, result !== null && middle === "result" ? result : merge.baseId, merge.rightId]
    const reads = text === null ? [] : [merge!.leftId, merge!.baseId, merge!.rightId, merge!.resolvedId].flatMap(id => (id === null ? [] : [{ id, path: null }]))
    const digits = String(Math.max(text?.leftLineCount ?? 0, text?.baseLineCount ?? 0, text?.rightLineCount ?? 0, 1)).length
    const file = mergeStandIn(entry.path)
    return {
      kind: "merge",
      key: entry.path,
      path: entry.path,
      file,
      diff: undefined,
      merge,
      pending,
      model: text,
      foldKey: text,
      cells: text === null ? null : mergeCells(text, middle),
      rows,
      ids,
      paths: ids.map(() => entry.path),
      worktreePaths: ids.map(() => null),
      reads,
      collapsed: false,
      folded: null,
      section: null,
      gutterColumns: digits + 2,
    }
  }
  const { file, diff } = entry
  const section = entry.section ?? null
  const key = section?.key ?? file.path
  const text = diff?.content.kind === "text" ? diff.content : null
  const foldKey = text === null ? null : section === null ? text : sectionFoldKey(text, section.key)
  // an added or a deleted file shows its one side, as IntelliJ's one-sided viewer: no empty half beside it in Split
  const oneSide = diff === undefined ? null : oneSideOf(diff)
  const rows = text === null ? null : buildFileRows(text, fileLayoutOf(mode, oneSide), folds.get(foldKey!) ?? getFoldLevels(foldKey!), entry.quietImports, section?.units ?? null)
  const digits = String(Math.max(text?.oldLineCount ?? 0, text?.newLineCount ?? 0, 1)).length
  // without a header nothing collapses: the row above the view is the header
  const collapsed = toggled !== null && !pending && (startsCollapsed(diff) || entry.folded !== undefined) !== toggled.has(key)
  const ids = diff === undefined ? [] : [diff.oldId, diff.newId]
  const reads: BlobRequest[] = []
  if (text !== null && diff!.oldId !== null) {
    reads.push({ id: diff!.oldId, path: null })
  }
  if (text !== null && diff!.newId !== null) {
    reads.push({ id: diff!.newId, path: worktree ? diff!.path : null })
  }
  return {
    kind: "diff",
    key,
    path: file.path,
    file,
    diff,
    merge: undefined,
    pending,
    model: text,
    foldKey,
    cells: text === null ? null : diffCells(text, mode, oneSide),
    rows,
    ids,
    // the old side of a rename has the grammar of its old path
    paths: diff === undefined ? [] : [diff.oldPath ?? diff.path, diff.path],
    worktreePaths: diff === undefined ? [] : [null, worktree ? diff.path : null],
    reads,
    collapsed,
    folded: entry.folded ?? formatCollapse(diff?.collapse ?? null),
    section,
    gutterColumns: digits + 2,
  }
}

/** Whether a section shows `line` of a side: no other section of its file shows the block of it */
function showsLine(file: PreparedFile, side: number, line: number): boolean {
  const blocks = file.diff?.content.kind === "text" ? file.diff.content.blocks : []
  for (const index of file.section?.units.elsewhere.keys() ?? []) {
    const block = blocks[index]
    const [start, end] = side === OLD ? [block?.oldStart ?? 0, block?.oldEnd ?? 0] : [block?.newStart ?? 0, block?.newEnd ?? 0]
    if (start <= line && line < end) {
      return false
    }
  }
  return true
}

/**
 * The section of `files` that shows `path`: the one that shows its `line` of a side; without a line its first open
 * section (the change of its own before a group a section folds); else its first
 */
function sectionOf(files: readonly PreparedFile[], path: string, at?: { side: number; line: number }): number {
  const first = files.findIndex(file => file.path === path)
  if (first < 0) {
    return first
  }
  if (at === undefined) {
    const open = files.findIndex(file => file.path === path && !file.collapsed)
    return open < 0 ? first : open
  }
  const showing = files.findIndex(file => file.path === path && showsLine(file, at.side, at.line))
  return showing < 0 ? first : showing
}

/** The file of git's list a merge stands for in a header */
function mergeStandIn(path: string): CommitFile {
  return { path, status: "modified", oldPath: null, additions: 0, deletions: 0, binary: false, oldId: null, newId: null, submodule: false }
}

/**
 * The diffs of files in one virtualized scroller (docs/design/diff-view.md): a sticky header per file, the rows from
 * the blocks of the engine, the unchanged lines folded as IntelliJ, plain text until the tokens arrive. React renders
 * the sections of the files in view; `DiffPainter` paints their rows in the scroll event.
 */
export function DiffView({
  ref,
  files,
  mode,
  middle = "result",
  softWraps,
  initialPath,
  label,
  onVisibleFileChange,
  onFirstScreen,
  renderHeader,
  scroll = "own",
  worktree = false,
  onGoToDeclaration,
  onFindUsages,
  initialPlace,
  onHistoryEnd,
  plain = false,
}: DiffViewProps) {
  // each file a card under its header: the review windows
  const cards = renderHeader !== undefined
  const headerHeight = cards ? CARD_HEADER_HEIGHT : 0
  const zoom = useAppSettings(state => state.zoom)
  const ligatures = useDiffLigatures()
  // the share of the old side in Split: the stored one, or the one of a drag of the divider until it ends
  const storedSplit = useDiffSplitRatio()
  const [draggedSplit, setDraggedSplit] = useState<number | null>(null)
  const split = mode === "split" ? (draggedSplit ?? storedSplit) : 0.5
  // the files the user expanded or collapsed against the way they start
  const [toggled, setToggled] = useState<ReadonlySet<string>>(() => new Set())
  // the fold levels this view changed; `setFoldLevels` keeps them for the next view of the same diff
  const [folds, setFolds] = useState<ReadonlyMap<FoldKey, readonly number[]>>(() => new Map())
  const [windowRange, setWindowRange] = useState<readonly [number, number]>([0, 0])
  const [metrics, setMetrics] = useState<ViewMetrics>(() => ({ rowHeight: 20, columnWidth: 7.8, text: { columnWidth: 7.8, tabSize: DIFF_TAB_SIZE, widthOf: () => 7.8 } }))
  const [viewWidth, setWidth] = useState(0)
  // the width of the rows: the view less the insets and the borders of the cards
  const width = cards ? Math.max(viewWidth - 2 * (CARD_INSET + CARD_BORDER), 0) : viewWidth
  const scroller = useRef<HTMLDivElement>(null)
  const visiblePath = useRef<string | undefined>(initialPath ?? (files[0] === undefined ? undefined : pathOf(files[0])))
  // the section at the top: of a file its path, of a section of a review its key
  const visibleKey = useRef<string | undefined>(files[0] === undefined ? undefined : keyOf(files[0]))
  // the block of changed lines N and P stepped to last, in the section at the top
  const changeCursor = useRef<{ key: string; index: number } | null>(null)
  const [painter] = useState(() => new DiffPainter(EMPTY_CALLBACKS, getHighlightClientOrNull, () => blobTextsStore.getState().byId))
  // the find in the diffs of a view with its own scroller (⌘F): the query, the folded lines, the current hit (-1 before
  // the first step)
  const [find, setFind] = useState<{ open: boolean; query: string; folded: boolean; current: number }>({ open: false, query: "", folded: false, current: -1 })
  const findInput = useRef<HTMLInputElement>(null)
  // the hit a step chose, shown once the layout has its row
  const reveal = useRef<number | null>(null)

  const prepared = useMemo(
    () => files.map(entry => prepareFile(entry, { mode, middle, folds, toggled: renderHeader === undefined ? null : toggled, worktree })),
    [files, mode, middle, toggled, folds, renderHeader, worktree],
  )

  // the texts this render reads: of the files in the window, and of every file the soft wraps lay out; a text of
  // another file does not render the view again
  const [first, last] = windowRange
  const watchedIds = useMemo(() => textIdsOf(softWraps || find.open ? prepared : prepared.slice(first, last + 1)), [prepared, softWraps, find.open, first, last])
  const watched = useStore(
    blobTextsStore,
    useShallow(state => watchedIds.map(id => state.byId.get(id))),
  )
  const texts = useMemo(() => new Map(watchedIds.flatMap((id, index) => (watched[index] === undefined ? [] : [[id, watched[index]] as const]))), [watchedIds, watched])

  const visualLines = useMemo(() => {
    if (!softWraps || width === 0) {
      return prepared.map(() => undefined)
    }
    return prepared.map(file => {
      const { rows, cells } = file
      if (rows === null || cells === null || file.collapsed) {
        return undefined
      }
      const sides = file.ids.map(id => blobText(texts, id))
      if (sides.some(text => text === null)) {
        return undefined
      }
      const tables = file.ids.map((id, side) => (id === null ? null : getLineTable(id, sides[side]!)))
      const widths = codeWidthsOf(file, width, metrics.columnWidth, split)
      const sideBySide = cells.columns.length > 1
      return visualLinePrefix(rows.rowCount, row => {
        const value = rowAt(rows, row)
        if (!isLineRow(value)) {
          return 1
        }
        if (value.kind === "inline") {
          const oldLine = lineText(tables[OLD]!, value.lines[OLD]!)
          const newLine = lineText(tables[NEW]!, value.lines[NEW]!)
          const parts = inlineParts(oldLine.length, null, cells.ranges(value, OLD) ?? [], newLine.length, null, cells.ranges(value, NEW) ?? [])
          return visualLineCount(parts.map(part => (part.side === OLD ? oldLine : newLine).slice(part.start, part.end)).join(""), widths[NEW]!, metrics.text)
        }
        let lines = 1
        value.lines.forEach((line, side) => {
          const table = tables[side]
          const shown = sideBySide || side === (value.kind === "deleted" ? OLD : NEW)
          if (line !== null && table !== null && table !== undefined && shown) {
            lines = Math.max(lines, visualLineCount(lineText(table, line), widths[side]!, metrics.text))
          }
        })
        return lines
      })
    })
  }, [prepared, softWraps, width, texts, metrics, split])

  // a view without headers shows the notice of a file in a row of the layout; the key keeps the notices one array
  // while they stay the same, so a text that arrives lays out nothing again
  const noticeKey = prepared.map(file => (renderHeader === undefined ? (noticeOf(file, texts) ?? "") : "")).join(NOTICE_SEPARATOR)
  const notices = useMemo(() => noticeKey.split(NOTICE_SEPARATOR).map(notice => (notice === "" ? null : notice)), [noticeKey])

  const layout = useMemo(
    () =>
      new DiffLayout(
        prepared.map((file, index) => ({
          key: file.key,
          rows: file.collapsed || file.rows === null || file.rows.rowCount === 0 ? null : file.rows,
          notice: notices[index] !== null,
          visualLines: visualLines[index],
        })),
        { rowHeight: metrics.rowHeight, headerHeight, cards: cards ? { gap: CARD_GAP, padding: CARD_GAP, borderBottom: CARD_BORDER } : undefined },
      ),
    [prepared, visualLines, metrics, headerHeight, notices, cards],
  )

  // the hits of the find in the lines the rows show, from the texts read so far
  const hits = useMemo(() => {
    if (!find.open || find.query === "") {
      return []
    }
    const searched = prepared.map((file, index) => {
      const tables = file.ids.map(id => {
        const text = blobText(texts, id)
        return id === null || text === null ? null : getLineTable(id, text)
      })
      return { rows: layout.files[index]!.rows, tables }
    })
    const sideBySide = mode === "split" || prepared.some(file => file.kind === "merge")
    return findHits(searched, find.query, { sideBySide, folded: find.folded })
  }, [find.open, find.query, find.folded, prepared, layout, texts, mode])
  const current = Math.min(find.current, hits.length - 1)
  const hitLines = useMemo(() => (hits.length === 0 ? undefined : hitsByLine(hits, current)), [hits, current])

  // the painter gets the model before the paint, so the anchor keeps its place in the new layout
  useLayoutEffect(() => {
    const painterFiles = prepared.map((file, index): PainterFile => ({
      key: file.key,
      ids: file.ids,
      cells: file.cells,
      rows: layout.files[index]!.rows,
      languages: file.paths.map(path => languageOf(path)),
      paths: file.paths,
      worktreePaths: file.worktreePaths,
      codeWidths: codeWidthsOf(file, width, metrics.columnWidth, split),
    }))
    painter.setModel({ files: painterFiles, layout, softWraps, textMetrics: metrics.text, hits: hitLines })
  }, [painter, prepared, layout, softWraps, metrics, width, hitLines, split])

  // the hit a step chose: a third of the viewport below the header, never above its file
  useLayoutEffect(() => {
    const hit = reveal.current === null ? undefined : hits[reveal.current]
    const rows = hit === undefined ? null : layout.files[hit.file]!.rows
    if (hit === undefined || rows === null) {
      return
    }
    reveal.current = null
    const top = layout.rowTop(hit.file, rowOfLine(rows, hit.side, hit.line)) - layout.stickyTop - headerHeight - (scroller.current?.clientHeight ?? 0) / 3
    painter.scrollTo(Math.max(top, layout.fileTop(hit.file)))
  }, [find.current, hits, layout, painter, headerHeight])

  // the find reads the texts of every file it searches, behind the texts on screen
  useEffect(() => {
    if (find.open) {
      requestBlobTexts(
        prepared.flatMap(file => readsOf(file)),
        "background",
      )
    }
  }, [find.open, prepared])

  // the result of a merge is composed once the texts of its three sides are read
  useEffect(() => {
    for (const file of prepared) {
      const { merge, model } = file
      const id = file.ids[MIDDLE] ?? null
      if (
        file.kind !== "merge" ||
        merge === undefined ||
        model === null ||
        !("baseLineCount" in model) ||
        middle !== "result" ||
        id === null ||
        merge.resolvedId !== null ||
        blobTextsStore.getState().byId.get(id)?.kind === "loaded"
      ) {
        continue
      }
      const sides = [merge.leftId, merge.baseId, merge.rightId].map(sideId => ({ id: sideId, text: blobText(texts, sideId) }))
      if (sides.every(side => side.text !== null)) {
        const [left, base, right] = sides.map(side => getLineTable(side.id ?? "", side.text!))
        putDerivedText(id, composeResult(model, [left!, base!, right!]))
      }
    }
  }, [prepared, texts, middle])

  // the next or the previous hit; a hit in a fold expands the fold first
  const stepHit = (direction: 1 | -1) => {
    if (hits.length === 0) {
      return
    }
    const next = current < 0 ? (direction === 1 ? 0 : hits.length - 1) : (current + direction + hits.length) % hits.length
    const hit = hits[next]!
    const text = prepared[hit.file]?.foldKey ?? null
    if (hit.gap >= 0 && text !== null && (getFoldLevels(text)[hit.gap] ?? 0) < EXPANDED) {
      const levels = Array.from({ length: Math.max(getFoldLevels(text).length, hit.gap + 1) }, (_, gap) => getFoldLevels(text)[gap] ?? 0)
      levels[hit.gap] = EXPANDED
      setFoldLevels(text, levels)
      setFolds(previous => new Map(previous).set(text, levels))
    }
    reveal.current = next
    setFind(previous => ({ ...previous, current: next }))
  }

  const closeFind = () => {
    setFind(previous => ({ ...previous, open: false, current: -1 }))
    scroller.current?.focus()
  }

  // ⌘F (Ctrl+F off macOS) opens the find, ⌘G steps through its hits
  const onFindKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!(event.metaKey || event.ctrlKey) || event.altKey) {
      return
    }
    if (event.code === "KeyF" && !event.shiftKey) {
      event.preventDefault()
      setFind(previous => ({ ...previous, open: true }))
      requestAnimationFrame(() => findInput.current?.select())
      queueMicrotask(() => findInput.current?.focus())
    } else if (event.code === "KeyG" && find.open) {
      event.preventDefault()
      stepHit(event.shiftKey ? -1 : 1)
    }
  }

  // every fold of a file shows all its lines: "Expand All" of the header, ⌥-click on a fold
  const expandAll = (file: PreparedFile | undefined) => {
    const text = file?.foldKey ?? null
    const gapCount = file?.rows?.gapCount ?? 0
    if (text === null || gapCount === 0) {
      return
    }
    const levels = Array.from({ length: gapCount }, () => EXPANDED)
    setFoldLevels(text, levels)
    setFolds(previous => new Map(previous).set(text, levels))
  }

  const expandFold = (file: PreparedFile | undefined, fold: FoldRow) => {
    const text = file?.foldKey ?? null
    if (text === null || file?.rows == null) {
      return
    }
    const levels = Array.from({ length: Math.max(getFoldLevels(text).length, fold.gap + 1) }, (_, gap) => getFoldLevels(text)[gap] ?? 0)
    levels[fold.gap] = nextFoldLevel(file.rows, fold.gap, fold.level)
    setFoldLevels(text, levels)
    setFolds(previous => new Map(previous).set(text, levels))
  }

  const toggleFile = (key: string) =>
    setToggled(previous => {
      const next = new Set(previous)
      if (!next.delete(key)) {
        next.add(key)
      }
      return next
    })

  /** Shows the section at `index` at the top, N and P from its top */
  const showSection = (index: number) => {
    const file = prepared[index]!
    changeCursor.current = { key: file.key, index: -1 }
    visibleKey.current = file.key
    visiblePath.current = file.path
    painter.scrollTo(layout.fileTop(index))
  }

  useLayoutEffect(() => {
    const callbacks: PainterCallbacks = {
      onWindow: (first, last, when) => {
        if (when === "flush") {
          // the scroll event needs the new sections in its frame
          flushSync(() => setWindowRange([first, last]))
        } else if (when === "now") {
          setWindowRange([first, last])
        } else {
          startTransition(() => setWindowRange([first, last]))
        }
      },
      onTopFile: index => {
        // the section the view showed last (a file of the tree, N, J) stays the one shown while it is on screen: a
        // section near the end, which a scroll to it cannot bring to the top, is not the one above it
        const cursor = changeCursor.current === null ? -1 : layout.indexOf(changeCursor.current.key)
        const element = scroller.current
        const bottom = element === null ? index : layout.fileAt(element.scrollTop + element.clientHeight)
        const file = prepared[cursor >= index && cursor <= bottom ? cursor : index]
        if (file !== undefined && file.key !== visibleKey.current) {
          visibleKey.current = file.key
          // the user scrolled into another section: N starts from its top
          if (changeCursor.current?.key !== file.key) {
            changeCursor.current = null
          }
        }
        if (file !== undefined && file.path !== visiblePath.current) {
          visiblePath.current = file.path
          onVisibleFileChange?.(file.path)
        }
      },
      onFold: (index, fold, all) => (all ? expandAll(prepared[index]) : expandFold(prepared[index], fold)),
      onNote: (index, note) => {
        const target = prepared[index]?.section?.targets.get(note.block)
        const shown = target === undefined ? -1 : layout.indexOf(target)
        if (shown >= 0) {
          if (prepared[shown]!.collapsed) {
            toggleFile(prepared[shown]!.key)
          }
          showSection(shown)
        }
      },
      onFirstScreen: () => onFirstScreen?.(),
      // the corrected widths are in the cache of the text metrics: a new object lays the rows out again
      onWidthsCorrected: () => setMetrics(previous => ({ ...previous, text: { ...previous.text } })),
    }
    painter.setCallbacks(callbacks)
  })

  // the font, the zoom and the screen give the row height; the width of the view the soft wraps
  useLayoutEffect(() => {
    const element = scroller.current
    if (element === null) {
      return
    }
    const measure = () => setMetrics(viewMetrics())
    measure()
    void document.fonts.load(ROWS_FONT).then(measure, measure)
    const resolution = matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`)
    resolution.addEventListener("change", measure)
    const resize = new ResizeObserver(([entry]) => setWidth(entry?.contentRect.width ?? 0))
    resize.observe(element)
    return () => {
      resolution.removeEventListener("change", measure)
      resize.disconnect()
    }
  }, [zoom])

  // only when the view mounts: a later file is the tree's or the keys'
  const scrollToInitialFile = useEffectEvent(() => {
    const index = initialPath === null ? -1 : sectionOf(prepared, initialPath)
    if (index > 0) {
      painter.scrollTo(layout.fileTop(index))
    }
  })

  // the painter scrolls, paints and listens on the scroller; the review opens at its file
  useLayoutEffect(() => {
    const element = scroller.current
    if (element === null) {
      return
    }
    const parent = scroll === "parent" ? scrollParentOf(element) : null
    painter.attach(element, listener => blobTextsStore.subscribe(listener), parent)
    if (parent === null) {
      scrollToInitialFile()
      takeFocus(element)
    }
    return () => painter.detach()
  }, [painter, scroll])

  // the texts of the files in the window at once (also the ones a Find still queues), then of the files after it
  useEffect(() => {
    requestBlobTexts(prepared.slice(first, last + 1).flatMap(file => readsOf(file)))
    const ahead = setTimeout(
      () =>
        requestBlobTexts(
          prepared.slice(last + 1, last + 1 + READ_AHEAD_FILES).flatMap(file => readsOf(file)),
          "background",
        ),
      50,
    )
    return () => clearTimeout(ahead)
  }, [prepared, first, last])

  const scrollToChange = (file: number, row: number) => {
    const top = layout.rowTop(file, row) - layout.stickyTop - headerHeight - 4 * metrics.rowHeight
    painter.scrollTo(Math.max(top, layout.fileTop(file)))
  }

  // Go to Declaration: the name under the pointer with the modifier held, its underline, the jump, Back and Forward
  const links = useRef<HTMLDivElement>(null)
  const pointer = useRef<{ x: number; y: number } | null>(null)
  const history = useRef<{ back: DeclarationPlace[]; forward: DeclarationPlace[] }>({ back: [], forward: [] })
  const pendingReveal = useRef<DeclarationPlace | null>(null)
  const [revealTick, setRevealTick] = useState(0)
  const navigates = onGoToDeclaration !== undefined && scroll === "own"
  // ⌘ on macOS, where a Ctrl-click is the context menu; Ctrl elsewhere, the Meta key too
  const modifierHeld = (event: { metaKey: boolean; ctrlKey: boolean }) => event.metaKey || (!isMac && event.ctrlKey)

  /** Where a word is, as the backend reads it: a side of a file of a diff, never of a merge */
  const siteOf = (word: CodeWord): { site: DeclarationSite; place: DeclarationPlace } | null => {
    const file = prepared[word.file]
    const id = file?.ids[word.side]
    if (file === undefined || file.kind !== "diff" || id == null) {
      return null
    }
    const side = word.side === OLD ? "old" : "new"
    const path = file.paths[word.side] ?? file.path
    return {
      site: { path, side, blob: { id, path: file.worktreePaths[word.side] ?? null }, line: word.line, column: word.start },
      place: { path: file.path, side, line: word.line, column: word.start, endColumn: word.end },
    }
  }

  /** The underline of a name to go to, as a link of IntelliJ; none for null */
  const showLink = (word: CodeWord | null) => {
    const layer = links.current
    const target = scroller.current
    if (layer === null || target === null) {
      return
    }
    const rects = word === null || siteOf(word) === null ? [] : word.rects()
    layer.replaceChildren(
      ...rects.map(rect => {
        const line = document.createElement("div")
        line.className = "dv-link"
        line.style.cssText = `left:${rect.left}px;top:${rect.bottom - 1}px;width:${rect.width}px`
        return line
      }),
    )
    target.style.cursor = rects.length > 0 ? "pointer" : ""
  }

  /** Shows a place of the view: its file open, its fold expanded, then (`revealTick`) scrolled to and selected */
  const revealPlace = (place: DeclarationPlace): boolean => {
    const side = place.side === "old" ? OLD : NEW
    const index = sectionOf(prepared, place.path, { side, line: place.line })
    const file = prepared[index]
    if (file === undefined) {
      return false
    }
    if (file.collapsed) {
      toggleFile(file.key)
    }
    const text = file.foldKey
    const fold = file.rows?.segments.find(segment => segment.kind === "fold" && segment.row.lines[side]![0] <= place.line && place.line < segment.row.lines[side]![1])
    if (text !== null && fold?.kind === "fold" && (getFoldLevels(text)[fold.row.gap] ?? 0) < EXPANDED) {
      const levels = Array.from({ length: Math.max(getFoldLevels(text).length, fold.row.gap + 1) }, (_, gap) => getFoldLevels(text)[gap] ?? 0)
      levels[fold.row.gap] = EXPANDED
      setFoldLevels(text, levels)
      setFolds(previous => new Map(previous).set(text, levels))
    }
    pendingReveal.current = place
    setRevealTick(tick => tick + 1)
    return true
  }

  /** Go to Declaration or Find Usages of a word: the reveal of a target keeps the place of the name for Back */
  const goTo = (word: CodeWord | null, at: { x: number; y: number }, action = onGoToDeclaration) => {
    const found = word === null ? null : siteOf(word)
    if (found !== null) {
      action?.(
        found.site,
        at,
        place => {
          if (sectionOf(prepared, place.path) < 0) {
            return false
          }
          history.current.back.push(found.place)
          history.current.forward = []
          return revealPlace(place)
        },
        found.place,
      )
    }
  }

  const onCodeMouseMove = (event: ReactMouseEvent<HTMLDivElement>) => {
    pointer.current = { x: event.clientX, y: event.clientY }
    showLink(modifierHeld(event) ? painter.wordAt(event.clientX, event.clientY) : null)
  }

  const onCodeClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (!modifierHeld(event) || event.button !== 0) {
      return
    }
    const word = painter.wordAt(event.clientX, event.clientY)
    if (word !== null && siteOf(word) !== null) {
      event.preventDefault()
      event.stopPropagation()
      showLink(null)
      goTo(word, { x: event.clientX, y: event.clientY })
    }
  }

  /** The items of the menu of the code: built when it opens, for the name under the pointer */
  const [codeMenu, setCodeMenu] = useState<MenuItem[]>([])
  const openCodeMenu = () => {
    const at = pointer.current ?? { x: 0, y: 0 }
    const word = painter.wordAtSelection() ?? painter.wordAt(at.x, at.y)
    const target = word === null ? null : siteOf(word)
    setCodeMenu(
      codeMenuItems({
        name: target === null ? null : word!.text,
        goToDeclaration: () => goTo(word, at),
        findUsages: onFindUsages === undefined ? null : () => goTo(word, at, onFindUsages),
      }),
    )
  }

  // the place the view opens at, once its file is laid out
  const initialShown = useRef(false)
  const revealInitial = useEffectEvent((place: DeclarationPlace) => revealPlace(place))
  useLayoutEffect(() => {
    if (initialPlace !== undefined && !initialShown.current && sectionOf(prepared, initialPlace.path) >= 0) {
      initialShown.current = true
      revealInitial(initialPlace)
    }
  }, [layout, prepared, initialPlace])

  // the place a reveal waits for: scrolled to a third of the viewport below the header, then its name selected
  useLayoutEffect(() => {
    const place = pendingReveal.current
    const side = place?.side === "old" ? OLD : NEW
    const index = place === null ? -1 : sectionOf(prepared, place.path, { side, line: place.line })
    const rows = index < 0 ? null : layout.files[index]!.rows
    if (place === null || rows === null) {
      return
    }
    pendingReveal.current = null
    const top = layout.rowTop(index, rowOfLine(rows, side, place.line)) - layout.stickyTop - headerHeight - (scroller.current?.clientHeight ?? 0) / 3
    painter.scrollTo(Math.max(top, layout.fileTop(index)))
    // the keys of the view (Back) work at the place shown, whether a choice closed or a view of a file opened
    scroller.current?.focus({ preventScroll: true })
    // once the row of the name is painted with its text, whenever that is
    painter.selectWord(index, side, place.line, place.column, place.endColumn, place.name ?? null)
  }, [layout, prepared, revealTick, painter, headerHeight])

  /** Back (⌘[) and Forward (⌘]): the places of the jumps */
  const stepHistory = (direction: "back" | "forward") => {
    const from = direction === "back" ? history.current.back : history.current.forward
    const to = direction === "back" ? history.current.forward : history.current.back
    const place = from.pop()
    if (place === undefined) {
      if (direction === "back") {
        onHistoryEnd?.()
      }
      return
    }
    const word = painter.wordAtSelection()
    const current = word === null ? null : siteOf(word)
    if (current !== null) {
      to.push(current.place)
    }
    revealPlace(place)
  }

  const onNavigationKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!navigates) {
      return
    }
    const command = isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey
    const findsUsages = event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey && event.code === "F7" && onFindUsages !== undefined
    if ((command && !event.altKey && !event.shiftKey && event.code === "KeyB") || findsUsages) {
      const word = painter.wordAtSelection()
      const rect = word?.rects()[0]
      event.preventDefault()
      goTo(word, rect === undefined ? (pointer.current ?? { x: 0, y: 0 }) : { x: rect.left, y: rect.bottom }, findsUsages ? onFindUsages : onGoToDeclaration)
    } else if (isMac && command && !event.altKey && (event.code === "BracketLeft" || event.code === "BracketRight")) {
      event.preventDefault()
      stepHistory(event.code === "BracketLeft" ? "back" : "forward")
    } else if (!isMac && command && event.altKey && (event.code === "ArrowLeft" || event.code === "ArrowRight")) {
      event.preventDefault()
      stepHistory(event.code === "ArrowLeft" ? "back" : "forward")
    }
  }

  useImperativeHandle(ref, () => ({
    scrollToFile: path => {
      const index = sectionOf(prepared, path)
      if (index >= 0) {
        showSection(index)
      }
    },
    moveChange: direction => {
      const order = prepared.map(file => file.key)
      const start = changeCursor.current ?? { key: visibleKey.current ?? order[0] ?? "", index: -1 }
      let fileIndex = order.indexOf(start.key)
      let index = start.index + direction
      if (fileIndex === -1) {
        // the file at the top is filtered out: from the first file, or back from the last one
        fileIndex = direction === 1 ? 0 : order.length - 1
        index = direction === 1 ? 0 : Number.MAX_SAFE_INTEGER
      } else if (direction === -1 && start.index === -1) {
        // from the top of a file going back: the last block of the file before it
        fileIndex--
        index = Number.MAX_SAFE_INTEGER
      }
      while (fileIndex >= 0 && fileIndex < order.length) {
        const targets = layout.files[fileIndex]!.rows?.changeStarts ?? []
        const clamped = index === Number.MAX_SAFE_INTEGER ? targets.length - 1 : index
        const target = targets[clamped]
        if (target !== undefined && clamped >= 0) {
          const { key, path } = prepared[fileIndex]!
          changeCursor.current = { key, index: clamped }
          visibleKey.current = key
          visiblePath.current = path
          scrollToChange(fileIndex, target)
          return path
        }
        fileIndex += direction
        index = direction === 1 ? 0 : Number.MAX_SAFE_INTEGER
      }
      return undefined
    },
    moveSection: direction => {
      const at = prepared.findIndex(file => file.key === visibleKey.current)
      const next = prepared[at + direction] === undefined || at < 0 ? -1 : at + direction
      if (next < 0) {
        return undefined
      }
      showSection(next)
      return prepared[next]!.path
    },
    focus: () => scroller.current?.focus(),
    revealDeclaration: (place, from) => {
      if (sectionOf(prepared, place.path) < 0) {
        return false
      }
      if (from !== null) {
        history.current.back.push(from)
        history.current.forward = []
      }
      return revealPlace(place)
    },
  }))

  // a section renders again only when its own props change: a scroll into the next file mounts that file alone
  const sections = prepared.slice(first, last + 1).map((file, offset) => {
    const index = first + offset
    const rows = layout.files[index]!.rows
    const widthOf = (id: string | null) => {
      const text = blobText(texts, id)
      return id === null || text === null ? 0 : (maxColumns(getLineTable(id, text), DIFF_TAB_SIZE) + 2) * metrics.columnWidth
    }
    return (
      <FileSection
        key={file.key}
        index={index}
        file={file}
        rows={rows}
        notice={noticeOf(file, texts)}
        noticeRow={layout.files[index]!.notice}
        top={layout.offsets[index]!}
        height={layout.fileHeight(index)}
        rowsHeight={layout.rowsHeight(index)}
        card={cards}
        rowHeight={metrics.rowHeight}
        columnWidth={metrics.columnWidth}
        softWraps={softWraps}
        width={width}
        split={split}
        sideWidths={file.ids.map(widthOf).join(" ")}
        painter={painter}
        renderHeader={renderHeader}
        onToggle={toggleFile}
        onExpandAll={expandAll}
      />
    )
  })

  // the divider of Split, as IntelliJ's DiffSplitter: the line between the halves of a row, 4 px to each side of it
  // (`.dv-half + .dv-half[data-side="1"]::before` gives the cursor; the one column of a file with one side has none); a
  // drag sets the share, a double click makes the sides equal
  const onDividerOf = (event: { clientX: number; clientY: number }) => {
    // by the point, not the target: the drag before a double click captured the pointer
    const half = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>('.dv-row > .dv-half + .dv-half[data-side="1"]') ?? null
    return half !== null && Math.abs(event.clientX - half.getBoundingClientRect().left) <= 4
  }
  const dragSplit = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !onDividerOf(event)) {
      return
    }
    event.preventDefault()
    event.stopPropagation()
    const element = event.currentTarget
    const start = element.getBoundingClientRect().left + (cards ? CARD_INSET + CARD_BORDER : 0)
    const shareAt = (clientX: number) => Math.min(Math.max((clientX - start) / Math.max(width, 1), MIN_SPLIT_RATIO), 1 - MIN_SPLIT_RATIO)
    let share = split
    element.setPointerCapture(event.pointerId)
    const move = (moved: PointerEvent) => {
      share = shareAt(moved.clientX)
      setDraggedSplit(share)
    }
    const end = () => {
      element.removeEventListener("pointermove", move)
      element.removeEventListener("pointerup", end)
      element.removeEventListener("pointercancel", end)
      setDiffSplitRatio(share)
      setDraggedSplit(null)
    }
    element.addEventListener("pointermove", move)
    element.addEventListener("pointerup", end)
    element.addEventListener("pointercancel", end)
  }

  const view = (
    <div
      ref={scroller}
      role="region"
      aria-label={label}
      tabIndex={scroll === "own" ? 0 : undefined}
      className={cn(scroll === "own" ? "dv-scroller min-h-0 flex-1" : "dv-embedded", cards && "dv-cards")}
      data-ligatures={ligatures ? undefined : "off"}
      data-split={mode === "split" ? "" : undefined}
      style={mode === "split" ? ({ "--dv-split": String(split) } as CSSProperties) : undefined}
      onPointerDown={scroll === "own" && mode === "split" ? event => dragSplit(event) : undefined}
      onDoubleClick={scroll === "own" && mode === "split" ? event => onDividerOf(event) && setDiffSplitRatio(0.5) : undefined}
      data-plain={plain ? "" : undefined}
      onMouseMove={navigates ? onCodeMouseMove : undefined}
      onMouseLeave={navigates ? () => showLink(null) : undefined}
      onClickCapture={navigates ? onCodeClick : undefined}
      onScroll={navigates ? () => showLink(null) : undefined}
      data-testid="diff-view"
    >
      {navigates ? (
        <ContextMenu onOpenChange={open => open && openCodeMenu()}>
          {/* the trigger turns off the selection of its content; the code keeps it (the copy, ⌘B) */}
          <ContextMenuTrigger render={<div className="dv-document select-text" style={{ height: layout.height }} />}>{sections}</ContextMenuTrigger>
          <ContextMenuContent>
            <ContextMenuItems groups={[codeMenu]} />
          </ContextMenuContent>
        </ContextMenu>
      ) : (
        <div className="dv-document" style={{ height: layout.height }}>
          {sections}
        </div>
      )}
      {navigates && <div ref={links} className="dv-links" aria-hidden />}
    </div>
  )
  if (scroll === "parent") {
    return view
  }
  return (
    <div
      className="flex h-full min-h-0 flex-col"
      onKeyDown={event => {
        onFindKeys(event)
        onNavigationKeys(event)
      }}
      onKeyUp={navigates ? event => !modifierHeld(event) && showLink(null) : undefined}
    >
      {find.open && (
        <DiffFindBar
          query={find.query}
          onQueryChange={query => setFind(previous => ({ ...previous, query, current: -1 }))}
          count={hits.length}
          current={current}
          folded={find.folded}
          onFoldedChange={folded => setFind(previous => ({ ...previous, folded, current: -1 }))}
          onStep={stepHit}
          onClose={closeFind}
          inputRef={findInput}
        />
      )}
      {view}
    </div>
  )
}

interface FileSectionProps {
  index: number
  file: PreparedFile
  rows: FileRows | null
  notice: string | null
  noticeRow: boolean
  top: number
  height: number
  rowsHeight: number
  /** The file is a card: a border, rounded corners, its header the top of the card */
  card: boolean
  rowHeight: number
  columnWidth: number
  softWraps: boolean
  width: number
  /** The share of the old side in Split */
  split: number
  /**
   * The width of the widest line of the text of each side in CSS px, 0 while a text is not read; one string, so the
   * memo of the section compares it by value
   */
  sideWidths: string
  painter: DiffPainter
  renderHeader: DiffViewProps["renderHeader"]
  onToggle: (path: string) => void
  onExpandAll: (file: PreparedFile) => void
}

/** The section of a file: as tall as the file, its header sticky, the rows element the painter fills */
const FileSection = memo(function FileSection(props: FileSectionProps) {
  const {
    index,
    file,
    rows,
    notice,
    noticeRow,
    top,
    height,
    rowsHeight,
    card,
    rowHeight,
    columnWidth,
    softWraps,
    width,
    split,
    sideWidths,
    painter,
    renderHeader,
    onToggle,
    onExpandAll,
  } = props
  return (
    <section
      className={card ? "dv-section dv-card" : "dv-section"}
      aria-label={file.path}
      data-testid="diff-view-file"
      data-path={file.path}
      data-section={file.section?.key}
      // the colours a plugin of IntelliJ gives a role in its language only (diff-view.css)
      data-language={languageOf(file.path)}
      // a card with its header alone (diff-view.css): an attribute, as a `:has()` of the rows costs Chromium a style
      // recalc of the card on each row the painter adds
      data-collapsed={card && rows === null && !noticeRow ? "" : undefined}
      style={{ top, height, "--dv-gutter": `${file.gutterColumns * columnWidth}px`, "--dv-row-height": `${rowHeight}px` } as CSSProperties}
    >
      {renderHeader !== undefined && (
        <div className="dv-header">
          {renderHeader({
            file: file.file,
            notice,
            empty: file.pending || isEmptyNotice(file.diff, file.rows),
            folded: file.folded,
            heading: file.section?.heading ?? null,
            collapsed: file.collapsed,
            onToggle: () => onToggle(file.key),
            onExpandAll: rows !== null && rows.segments.some(segment => segment.kind === "fold") ? () => onExpandAll(file) : undefined,
          })}
        </div>
      )}
      {noticeRow && (
        <p className="dv-notice" data-testid={file.kind === "merge" ? "merge-notice" : rows === null ? "diff-notice" : "file-line-ends"}>
          {notice}
        </p>
      )}
      {rows !== null && (
        <>
          <div
            className="dv-rows"
            style={{ height: rowsHeight }}
            ref={host => {
              painter.attachRows(index, host)
              return () => {
                if (host !== null) {
                  painter.detachRows(index, host)
                }
              }
            }}
          />
          {!softWraps && (
            <HorizontalBars file={file} sideWidths={sideWidths.split(" ").map(Number)} columnWidth={columnWidth} codeWidths={codeWidthsOf(file, width, columnWidth, split)} />
          )}
        </>
      )}
    </section>
  )
})

/** The sticky scrollbar of each column whose widest line is wider than it */
function HorizontalBars({ file, sideWidths, columnWidth, codeWidths }: { file: PreparedFile; sideWidths: readonly number[]; columnWidth: number; codeWidths: readonly number[] }) {
  const columns = file.cells?.columns ?? []
  const sideBySide = columns.length > 1
  // one column shows the line of either side
  const widths = sideBySide ? columns.map(side => sideWidths[side] ?? 0) : [Math.max(0, ...sideWidths)]
  if (columns.length === 0 || widths.every((contentWidth, index) => contentWidth <= codeWidths[columns[index]!]!)) {
    return null
  }
  return (
    <div className="dv-hscrolls">
      {columns.map((side, index) => (
        <div key={side} className="dv-hscroll" data-side={side}>
          <div style={{ width: widths[index]! + file.gutterColumns * columnWidth * (sideBySide ? 1 : (file.cells?.numbers.length ?? 2)), height: 1 }} />
        </div>
      ))}
    </div>
  )
}

/** The blob ids of the texts the files that show rows paint and read */
function textIdsOf(files: readonly PreparedFile[]): string[] {
  return [...new Set(files.flatMap(file => (file.collapsed ? [] : [...file.ids, ...file.reads.map(read => read.id)].filter((id): id is string => id !== null))))]
}

/** The texts a file reads while it shows rows */
function readsOf(file: PreparedFile): readonly BlobRequest[] {
  return file.collapsed ? [] : file.reads
}

/** The path of a file of the props */
function pathOf(entry: DiffViewFile | MergeViewFile): string {
  return "merge" in entry ? entry.path : entry.file.path
}

/** The key of the section of a file of the props (`PreparedFile.key`) */
function keyOf(entry: DiffViewFile | MergeViewFile): string {
  return "merge" in entry ? entry.path : (entry.section?.key ?? entry.file.path)
}

const NOTICE_SEPARATOR = "\u0001"

/** The notice of a file: "Loading…" while its diff or merge is pending */
function noticeOf(file: PreparedFile, texts: ReadonlyMap<string, BlobTextEntry>): string | null {
  if (file.pending) {
    return "Loading…"
  }
  return file.kind === "merge" ? mergeNotice(file.merge, file.rows, texts) : fileNotice(file.diff, file.rows, texts)
}

/** The nearest ancestor that scrolls vertically: the scroller a view without its own lies in */
function scrollParentOf(element: HTMLElement): HTMLElement | null {
  for (let parent = element.parentElement; parent !== null; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent)
    if (overflowY === "auto" || overflowY === "scroll" || overflowY === "overlay") {
      return parent
    }
  }
  return document.scrollingElement instanceof HTMLElement ? document.scrollingElement : null
}

/**
 * The width of the code of each side in CSS px: its column, less the line numbers, the bar of a change and the
 * padding; one column shows both line numbers, or the one of a file with one side
 */
function codeWidthsOf(file: PreparedFile, width: number, columnWidth: number, split: number): number[] {
  const gutter = file.gutterColumns * columnWidth
  // the padding before the code; the colour of a line fills its gutter, so no bar takes a width
  const chrome = columnWidth
  const columns = file.cells?.columns.length ?? 1
  if (columns === 2) {
    // Split: the old side takes its share of the row (`--dv-split`), the new side the rest less its 1-px border
    const old = split * width
    return [old - gutter - chrome, width - old - 1 - gutter - chrome]
  }
  // a border of 1 px between two columns
  const code = columns > 1 ? (width - (columns - 1)) / columns - gutter - chrome : width - (file.cells?.numbers.length ?? 2) * gutter - chrome
  return file.ids.map(() => code)
}

const EMPTY_CALLBACKS: PainterCallbacks = {
  onWindow: () => {},
  onTopFile: () => {},
  onFold: () => {},
  onNote: () => {},
  onFirstScreen: () => {},
  onWidthsCorrected: () => {},
}

function getHighlightClientOrNull() {
  try {
    return getHighlightClient()
  } catch {
    return null
  }
}

/**
 * The diffs take the focus, so the arrows, PageUp and PageDown scroll them: from the review, from what holds it or
 * from the body, and from anything of its dialog, but not from what the user works in beside the diffs (the commits,
 * the owner chips: `data-review-keeps-focus`), which keeps its keys, nor from the trigger of an open popup (the "⋯" of
 * the commit heading): its menu closes when the focus leaves it
 */
function takeFocus(element: HTMLElement): void {
  const active = document.activeElement
  const root = element.closest("[data-review-root]")
  const dialog = element.closest("[role=dialog]")
  const keeps = active?.closest("[data-review-keeps-focus]") != null || active?.getAttribute("aria-expanded") === "true"
  if (
    active === null ||
    active === document.body ||
    root === null ||
    (root.contains(active) && !keeps) ||
    active.contains(root) ||
    (dialog !== null && dialog.contains(active) && !keeps && active.closest("[role=listbox]") === null)
  ) {
    element.focus()
  }
}
