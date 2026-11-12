import { memo, startTransition, useEffect, useEffectEvent, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState } from "react"
import type { CSSProperties, KeyboardEvent, MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent, ReactNode, Ref } from "react"
import { flushSync } from "react-dom"
import { cn } from "cn"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { DiffPainter } from "@/components/diff/diff-painter"
import type { CodeWord, PainterCallbacks, PainterFile } from "@/components/diff/diff-painter"
import { DiffFileMap } from "@/components/diff/DiffFileMap"
import type { DiffFileMapHandle, FileMapEntry } from "@/components/diff/DiffFileMap"
import { DiffFindBar } from "@/components/diff/DiffFindBar"
import { useMeasuredHeights } from "@/components/diff/use-measured-heights"
import type { MeasuredHeights } from "@/components/diff/use-measured-heights"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import type { BlobRequest, CommitFile, DeclarationSite, ReviewTitle } from "@/lib/bindings"
import { noteKey } from "@/lib/diff/block-parts"
import type { MoveEnd, RenameNote } from "@/lib/diff/block-parts"
import { codeMenuItems, codeMenuKeyPoint } from "@/lib/diff/code-menu"
import type { FileDiff, TextDiff } from "@/lib/diff/file-diff-model"
import { PROLOGUE_SECTION } from "@/lib/diff/file-map"
import { composeResult, resultTextId } from "@/lib/diff/file-merge-model"
import type { FileMerge, MergeMiddle, TextMerge } from "@/lib/diff/file-merge-model"
import { findHits, hitsByLine, mergeHits, rowOfHit } from "@/lib/diff/find"
import type { DiffFindHit, FindHit, TextFindHit } from "@/lib/diff/find"
import { findTextHits, revealTextHit } from "@/lib/diff/find-text"
import { getHighlightClient } from "@/lib/diff/highlight/highlighter"
import { inlineRowText, widestInlineRow } from "@/lib/diff/inline-rows"
import { DiffLayout, endsWithFooter, EPILOGUE_KEY, moveHandOf, PROLOGUE_KEY, visualLinePrefix } from "@/lib/diff/layout"
import type { LayoutPart } from "@/lib/diff/layout"
import { getLineTable, lineText, maxColumns } from "@/lib/diff/line-table"
import { DIFF_FONT_SIZE, DIFF_TAB_SIZE, measureFont, rowHeightOf, textMetricsOf } from "@/lib/diff/metrics"
import { diffCells, fileLayoutOf, mergeCells, oneSideOf } from "@/lib/diff/row-cells"
import type { RowCells } from "@/lib/diff/row-cells"
import {
  buildFileRows,
  buildMergeRows,
  EXPANDED,
  getFoldLevels,
  getOpenNotes,
  isLineRow,
  MIDDLE,
  NEW,
  nextFoldLevel,
  noteOfLine,
  notesOfRows,
  OLD,
  rowAt,
  rowOfLine,
  rowOfPlace,
  sectionFoldKey,
  setFoldLevels,
  setOpenNotes,
} from "@/lib/diff/rows"
import type { FileRows, FoldKey, FoldRow, NoteRow, SectionUnits } from "@/lib/diff/rows"
import { blobText, fileNotice, isEmptyNotice, languageOf, mergeNotice, startsCollapsed } from "@/lib/diff/view-files"
import { visualLineCount } from "@/lib/diff/wrap"
import type { TextMetrics } from "@/lib/diff/wrap"
import type { MenuItem } from "@/lib/menu"
import { isMenuKey, openMenuByKey } from "@/lib/menu-key"
import { isMac } from "@/lib/platform"
import { formatCollapse } from "@/presenters/collapse"
import { fileNameAmong } from "@/presenters/diff-marks"
import type { StoryMark } from "@/presenters/review-story"
import { blobTextsStore, putDerivedText, requestBlobTexts } from "@/stores/blob-texts"
import type { BlobTextEntry } from "@/stores/blob-texts"
import { useAppSettings } from "@/stores/settings/appSettings"
import { MIN_SPLIT_RATIO, setDiffSplitRatio, useDiffLigatures, useDiffSplitRatio } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import "./diff-view.css"

/*
 * The file cards of a view with headers (docs/design/diff-view.md, "File cards"), as IntelliJ's combined diff: a gap of
 * 10 px between two cards and around them, in the canvas colour, an inset of 12 px at the sides, a border of 1 px. The
 * header holds the top border of its card, a band of its own colour (diff-view.css). A section that starts a step of the
 * review plan has its opening on the canvas between the gap above its card and the card (`renderOpening`), as tall as
 * its words; one that starts a group a line of a fixed height there (`renderDivider`). A prologue may come before the
 * first section (`prologue`), an epilogue after the last card (`epilogue`). A long card and the last one end with a line one row high that names what comes after it
 * (`renderFooter`, `endsWithFooter`). Their own scroller hides its scrollbar for the map of the files beside it
 * (`DiffFileMap`, "The map of files").
 */
const CARD_HEADER_HEIGHT = 48
const CARD_GAP = 10
const CARD_INSET = 12
const CARD_BORDER = 1
const CARD_DIVIDER = 32
/** The height of an opening before its first measure, which comes before the paint */
const OPENING_ESTIMATE = 32
/** The font of the rows, as `.dv-rows` of diff-view.css */
const ROWS_FONT = `${DIFF_FONT_SIZE}px "JetBrains Mono Variable", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`
/** The files after the window whose texts are read ahead */
const READ_AHEAD_FILES = 20
const EMPTY_TEXT_HITS: readonly TextFindHit[] = []
/** The inner ranges of a file whose cells are not known yet: it has no rows to search either */
const NO_RANGES = (): null => null

type FindHitIdentity = { kind: "text"; node: Node; offset: number } | { kind: "diff"; file: number; side: number; line: number; start: number; end: number }

function identityOf(hit: FindHit): FindHitIdentity {
  return hit.kind === "text"
    ? { kind: "text", node: hit.range.startContainer, offset: hit.range.startOffset }
    : { kind: "diff", file: hit.file, side: hit.side, line: hit.line, start: hit.start, end: hit.end }
}

function matchesIdentity(hit: FindHit, identity: FindHitIdentity): boolean {
  if (hit.kind === "text" && identity.kind === "text") {
    return hit.range.startContainer === identity.node && hit.range.startOffset === identity.offset
  }
  if (hit.kind === "diff" && identity.kind === "diff") {
    return hit.file === identity.file && hit.side === identity.side && hit.line === identity.line && hit.start === identity.start && hit.end === identity.end
  }
  return false
}

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
  /** The group the section is of, on its first file ("Same edit in 3 files"): the divider above its card names it */
  readonly heading: string | null
  /**
   * The step of the review plan the section starts, on its first section: its id, and the words of the opening above
   * its card, its number ("Step 2") and its title ("The open keeps a stored layout", `Chapter.title`)
   */
  readonly step: { readonly id: string; readonly number: string; readonly title: ReviewTitle } | null
  /**
   * The steps of no card of their own told right before `step`, in their order: each its opening above the opening of
   * `step`, in the one layer of openings of the section (each element of it `data-opening-step` its id), J one stop each
   */
  readonly before?: readonly { readonly id: string; readonly number: string; readonly title: ReviewTitle }[]
  /**
   * The opening of the group of a large step the section starts (`presentGroupOpenings`), by its key: above its card in
   * the layer of its openings, in place of the divider of `heading`
   */
  readonly group?: string | null
  /** The words of the step the section is of, on every section of it: the hover of the map of the files names it */
  readonly stepHeading: string | null
  /** The blocks of the stories that name the section (`storyMarks`): its header marks it, a click shows the block */
  readonly story?: readonly StoryMark[]
  /** The units it shows (`NarrativeSection.unitIds`): a place of one of them is shown in it */
  readonly unitIds?: ReadonlySet<string>
  /**
   * Of a file the story shows in two sections or more, which one this is ("part 2 of 3", `index` from 0) and the lines
   * its blocks change: its header says them in place of the counts of the file
   */
  readonly part?: { readonly index: number; readonly count: number; readonly additions: number; readonly deletions: number } | null
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
  /** The section of the story the header heads; null for a file of no review */
  section: DiffSection | null
  /** Why the file shows no rows, or how its line ends changed */
  notice: string | null
  /** The notice says why the file shows no rows: nothing to expand */
  empty: boolean
  /** Why the file starts collapsed: the review folds it (`DiffViewFile.folded`), or the words of Rust's `collapse` */
  folded: string | null
  collapsed: boolean
  onToggle: () => void
  /** Shows all the unchanged lines of the file; undefined when it folds none */
  onExpandAll?: () => void
  /**
   * What is left to read of the file, or of its section: the stops of N and P among its blocks of changed lines, as
   * Rust says where the reading of each starts (`FileRows.changeStarts`); null where every block is a stop
   */
  toRead: { readonly toRead: number; readonly changes: number } | null
}

export interface DiffViewHandle {
  /** Shows the file at the top */
  scrollToFile: (path: string) => void
  /**
   * Shows the section of a review at the top, by its key (`DiffSection.key`): its opening (the default), of a step of
   * no card told before it (`DiffSection.before`) by its id, or its card with the opening above the view; the opening
   * of a step in the epilogue (`DiffViewProps.epilogue`) by its id, the key of the last section
   */
  scrollToSection: (key: string, part?: "opening" | "card", step?: string) => void
  /** Shows the prologue at the top; nothing without one */
  scrollToPrologue: () => void
  /**
   * Shows the next (1) or previous (-1) block of changed lines, from the last one shown in the file at the top; into
   * the next file at the end of one. Collapsed files, and with `accept` the files it refuses, are skipped. Returns the
   * file it shows and the key of its section; undefined, and nothing moves, when no block is left in that direction
   */
  moveChange: (direction: 1 | -1, accept?: (path: string) => boolean) => { path: string; key: string } | undefined
  /**
   * The key of the notes (`REVIEW_KEYS.notes`): shows the old lines of every note in view that opens (Inline shows
   * its run once), or hides them once they all show; the view stays at its top row. A note is in view when its row is
   * whole below the stuck header. Nothing changes when no such note is in view
   */
  toggleNotes: () => void
  /**
   * The key of the moves (`REVIEW_KEYS.moveEnd`): shows the other end of a move from the note of one of its ends in
   * view, the one note that shows the hint of the key: the note of the end the view stands at while it is in view
   * (where a jump, Back or Forward landed, the note N or P stopped under), else the first one, so two presses go
   * there and back; the end it starts from is kept for Back. A note is in view when its row is whole below the stuck
   * header. The other end of a move between two files is in the file it names: the key takes its note where the view
   * shows that file. Nothing changes when no such note is in view
   */
  jumpMove: () => void
  /**
   * The section at the top and its part there, at its opening the step whose opening it is: the one the view showed
   * last, or the one scrolled to (`onVisibleSectionChange`); undefined without files
   */
  visibleStop: () => { key: string; part: LayoutPart; step?: string } | undefined
  focus: () => void
  openFind: () => void
  stepFind: (direction: 1 | -1) => void
  /**
   * Shows a declaration of the view: its file open, the fold around it expanded, its name selected; `from` is the place
   * Back returns to. Returns the key of the section that shows it; undefined for a file the view does not show
   */
  revealDeclaration: (place: DeclarationPlace, from: DeclarationPlace | null) => string | undefined
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
  /** The unit of the review it is of (`ReviewPlace.unit`): shown in the section of that unit */
  unit?: string
  /**
   * The place is the line itself and no name in it (an end of a move): a reveal selects nothing, opens no note the
   * line is behind, and lands on the row that says what the line is (`rowOfPlace`), which it marks
   */
  wholeLine?: boolean
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
  /**
   * The section at the top, its key and its file, as the user scrolls: of a file its path, of a section of a review its
   * key; and its part at the top then, its opening or its card, or the prologue above the first section. Told as the
   * section or its part changes
   */
  onVisibleSectionChange?: (key: string, path: string, part: LayoutPart, step?: string) => void
  /** The rows of the first screen are painted with their texts, and highlighted or 80 ms late */
  onFirstScreen?: () => void
  /**
   * The cards that passed entirely through the viewport (`DiffLayout.passedCards`), each by its file and its key (of a
   * section of a review its own), each once a view, as a scroll brings them: the progress of a review
   */
  onCardsPassed?: (cards: readonly { readonly path: string; readonly key: string }[]) => void
  /**
   * Go to Declaration (openspec/specs/commit/symbol-navigation/spec.md): a ⌘-click (Ctrl-click off macOS) on a name, ⌘B on the
   * selection, the item of the menu of the code; `at` is where to show a choice of declarations
   */
  onGoToDeclaration?: (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace) => void
  /** Find Usages: ⌥F7 (Alt+F7 off macOS) on the selection, the item of the menu of the code; the arguments as above */
  onFindUsages?: (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace) => void
  /** A place shown once the view mounts: the declaration a view of a file outside the review opens at */
  initialPlace?: DeclarationPlace
  /**
   * A place the surface around the view asks it to show (`revealDeclaration` as a prop, for a view that mounts with
   * it): shown once the view shows its file, and again whenever the prop is another object. The other end of a move
   * between two files, or the declaration of a name, that another view of the same subject jumped to (`elsewhere`)
   */
  shownPlace?: DeclarationPlace
  /**
   * The other views of the files of the same subject, where each file has a view of its own (the diffs under the rows
   * of the tree of a commit's files): whether one of them can show a file, by its path, and the call that shows a
   * place there. A note that names a place of another file of the subject ("Moved to Report.kt:12", "declared in
   * Store.kt") is a button where this view shows that file itself or `shows` says so, else plain words
   */
  elsewhere?: DiffViewElsewhere
  /** Back with no jump left to return along: the view of a file outside the review closes */
  onHistoryEnd?: () => void
  /** A file shown by itself, not a change: no tint of an added file */
  plain?: boolean
  /** The sticky header of each file; without it the view shows its files open, a notice in a row under each */
  renderHeader?: (props: DiffHeaderProps) => ReactNode
  /**
   * What comes before the first section, with the headers only: a block of the document, always mounted, as tall as
   * its content (measured in place), the space above the cards above it and a gap below it
   */
  prologue?: ReactNode
  /**
   * What comes after the last card, with the headers only: the openings of the steps of no card told after it, each an
   * element `data-opening-step` of its step; always mounted, as tall as its content (measured in place), a gap above it
   * and the space below the cards below it. An opening of the last section to the view, J and the tree
   */
  epilogue?: ReactNode
  /**
   * The opening on the canvas above the card of a section that starts a step (`DiffSection.step`), with the headers
   * only: always mounted, as tall as its content (measured in place); without it no section has one
   */
  renderOpening?: (section: DiffSection) => ReactNode
  /**
   * The divider of a fixed height on the canvas above the card of a section that starts a group (`DiffSection.heading`)
   * and no step, with the headers only; without it no section has one
   */
  renderDivider?: (section: DiffSection) => ReactNode
  /**
   * The line at the end of a long card and of the last one (`endsWithFooter`), by the key of its section and its file,
   * with the headers only; without it no card has one
   */
  renderFooter?: (key: string, path: string) => ReactNode
  /** The files the view plays down (of another owner than the chosen chip's): their ticks on the map of the files paler */
  muted?: (path: string) => boolean
  /** "own": the view scrolls itself (the review windows); "parent": it is a block of the scroller it lies in */
  scroll?: "own" | "parent"
  /**
   * The key by which the surface around the view calls `toggleNotes` (H in a review): a note that opens shows it after
   * its words and names it to a screen reader; absent where no key does, so no hint shows where none works
   */
  notesKey?: string
  /**
   * The key by which the surface around the view calls `jumpMove` (G in a review): the button of the one note of a
   * move the key takes shows it after its words and names it to a screen reader, and the menu of the code beside its
   * item where the menu opened on that end; absent where no key does
   */
  moveKey?: string
  /** The files are the uncommitted changes: the new side is read by its path in the working tree */
  worktree?: boolean
}

/** What shows the files of a subject that a view does not show itself (`DiffViewProps.elsewhere`) */
export interface DiffViewElsewhere {
  readonly shows: (path: string) => boolean
  readonly show: (place: DeclarationPlace) => void
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
  /** The notes this view opened, by the key of the folds of their file (`getOpenNotes`) */
  notes: ReadonlyMap<FoldKey, ReadonlySet<number>>
  /** The files start collapsed or open as `startsCollapsed` says, except these */
  toggled: ReadonlySet<string> | null
  worktree: boolean
}

/** What the view shows of a file: its rows, the texts of its sides, its cells */
function prepareFile(entry: DiffViewFile | MergeViewFile, { mode, middle, folds, notes, toggled, worktree }: PrepareOptions): PreparedFile {
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
  const rows =
    text === null
      ? null
      : buildFileRows(
          text,
          fileLayoutOf(mode, oneSide),
          folds.get(foldKey!) ?? getFoldLevels(foldKey!),
          entry.quietImports,
          section?.units ?? null,
          notes.get(foldKey!) ?? getOpenNotes(foldKey!),
        )
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
 * The section of `files` that shows `path`: the one of the unit of the place, else the one that shows its `line` of a
 * side; without a line its first open section (the change of its own before a group a section folds); else its first
 */
function sectionOf(files: readonly PreparedFile[], path: string, at?: { side: number; line: number; unit?: string | undefined }): number {
  const first = files.findIndex(file => file.path === path)
  if (first < 0) {
    return first
  }
  if (at === undefined) {
    const open = files.findIndex(file => file.path === path && !file.collapsed)
    return open < 0 ? first : open
  }
  // the section of the unit the place is of: a line of its name another section of the file shows too is its there
  const unit = at.unit
  const own = unit === undefined ? -1 : files.findIndex(file => file.path === path && file.section?.unitIds?.has(unit) === true)
  if (own >= 0) {
    return own
  }
  const showing = files.findIndex(file => file.path === path && showsLine(file, at.side, at.line))
  return showing < 0 ? first : showing
}

/**
 * Another file of the subject of a file as its notes name it (`PainterFile.otherFile`), by its index among `paths`
 * (`FileDiff.subjectPaths`): its path, its name among them, and whether a view can show a place of it (`reaches`).
 * Undefined for a file of no subject
 */
function otherFileOf(paths: readonly string[] | undefined, reaches: (path: string) => boolean): PainterFile["otherFile"] {
  if (paths === undefined) {
    return undefined
  }
  return index => {
    const path = paths[index]
    return path === undefined ? null : { path, name: fileNameAmong(paths, index), shown: reaches(path) }
  }
}

/** The paths of the other files of its subject that the marks of a diff name: where its lines went or came from, where a name it uses is declared */
function namedFilesOf(diff: FileDiff | undefined): string[] {
  const subject = diff?.content.kind === "text" ? diff.content.marks.subject : null
  const paths = diff?.subjectPaths
  if (subject == null || paths === undefined) {
    return []
  }
  return [...subject.movedOut, ...subject.movedIn, ...subject.used].flatMap(named => paths[named.file] ?? [])
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
  onVisibleSectionChange,
  onFirstScreen,
  onCardsPassed,
  renderHeader,
  prologue,
  epilogue,
  renderOpening,
  renderDivider,
  renderFooter,
  muted,
  scroll = "own",
  notesKey,
  moveKey,
  worktree = false,
  onGoToDeclaration,
  onFindUsages,
  initialPlace,
  shownPlace,
  elsewhere,
  onHistoryEnd,
  plain = false,
}: DiffViewProps) {
  // each file a card under its header: the review windows; their own scroller has the map of the files for a scrollbar
  const cards = renderHeader !== undefined
  const mapped = cards && scroll === "own"
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
  // the notes of the marks this view opened, kept as the fold levels are (`setOpenNotes`)
  const [notes, setNotes] = useState<ReadonlyMap<FoldKey, ReadonlySet<number>>>(() => new Map())
  const [windowRange, setWindowRange] = useState<readonly [number, number]>([0, 0])
  const [metrics, setMetrics] = useState<ViewMetrics>(() => ({ rowHeight: 20, columnWidth: 7.8, text: { columnWidth: 7.8, tabSize: DIFF_TAB_SIZE, widthOf: () => 7.8 } }))
  const [viewWidth, setWidth] = useState(0)
  // the height of the view, which the space past the end of the document is sized for (`DiffLayout.pastEnd`): a resize
  // lays the document out again, a scroll never
  const [viewHeight, setViewHeight] = useState(0)
  // the width of the rows: the view less the insets and the borders of the cards
  const width = cards ? Math.max(viewWidth - 2 * (CARD_INSET + CARD_BORDER), 0) : viewWidth
  const scroller = useRef<HTMLDivElement>(null)
  const fileMap = useRef<DiffFileMapHandle>(null)
  const visiblePath = useRef<string | undefined>(initialPath ?? (files[0] === undefined ? undefined : pathOf(files[0])))
  // the section at the top: of a file its path, of a section of a review its key; and its part at the top. Undefined
  // until a scroll or a choice reports one: the view says then what is at its top (`visibleNow`), the prologue where a
  // review opens
  const visibleKey = useRef<string | undefined>(undefined)
  const visiblePart = useRef<LayoutPart>("card")
  // at the opening of a section, the step whose opening is at the top: of the steps told before it, or its own
  const visibleStep = useRef<string | undefined>(undefined)
  // the prologue and the openings of the steps: their heights, measured in place
  const { heights: measured, measure } = useMeasuredHeights()
  // the epilogue, whose openings J and the tree show
  const epilogueElement = useRef<HTMLDivElement | null>(null)
  // the edges of the cards that were in view (`DiffLayout.passedCards`)
  const passedEdges = useRef(new Map<string, number>())
  // the block of changed lines N and P stepped to last, in the section at the top
  const changeCursor = useRef<{ key: string; index: number } | null>(null)
  const [painter] = useState(() => new DiffPainter(EMPTY_CALLBACKS, getHighlightClientOrNull, () => blobTextsStore.getState().byId))
  // the find in the review of a view with its own scroller (⌘F): the query, the folded lines, the current hit (-1 before
  // the first step)
  const [find, setFind] = useState<{ open: boolean; query: string; folded: boolean; current: number }>({ open: false, query: "", folded: false, current: -1 })
  const [textSearch, setTextSearch] = useState<{ query: string; hits: readonly TextFindHit[] }>({ query: "", hits: [] })
  const findInput = useRef<HTMLInputElement>(null)
  const currentIdentity = useRef<FindHitIdentity | null>(null)
  // the hit a step chose, shown once its fold is open and its layout is measured
  const reveal = useRef<FindHitIdentity | null>(null)

  const prepared = useMemo(
    () => files.map(entry => prepareFile(entry, { mode, middle, folds, notes, toggled: renderHeader === undefined ? null : toggled, worktree })),
    [files, mode, middle, toggled, folds, notes, renderHeader, worktree],
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
          const text = inlineRowText(
            lineText(tables[OLD]!, value.lines[OLD]!),
            lineText(tables[NEW]!, value.lines[NEW]!),
            cells.ranges(value, OLD) ?? [],
            cells.ranges(value, NEW) ?? [],
          )
          return visualLineCount(text, widths[NEW]!, metrics.text)
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

  // whether the cards end with a line, not its words: a chip that changes what it names lays out nothing again
  const footers = cards && renderFooter !== undefined
  const opens = cards && renderOpening !== undefined
  const divides = cards && renderDivider !== undefined
  const prologued = cards && prologue !== undefined && prologue !== null
  const epilogued = cards && epilogue !== undefined && epilogue !== null
  // the files with a side wider than its column: their sticky scrollbar takes a strip of its own below their rows
  // (`LayoutFile.scrollbar`); one string, so a text that changes no width lays out nothing again
  const scrollbarKey =
    softWraps || width === 0
      ? ""
      : prepared
          .map(file =>
            layoutRowsOf(file) !== null && scrollerWidths(file, sideWidthsOf(file, texts, metrics.columnWidth), codeWidthsOf(file, width, metrics.columnWidth, split)) !== null
              ? "1"
              : "0",
          )
          .join("")
  const scrollbars = useMemo(() => Array.from(scrollbarKey, flag => flag === "1"), [scrollbarKey])
  const layout = useMemo(
    () =>
      new DiffLayout(
        prepared.map((file, index) => {
          const rows = layoutRowsOf(file)
          return {
            key: file.key,
            rows,
            notice: notices[index] !== null,
            visualLines: visualLines[index],
            opening: opens && (file.section?.step != null || file.section?.group != null) ? (measured.get(file.key) ?? OPENING_ESTIMATE) : undefined,
            divider: divides && file.section?.heading != null && file.section.group == null,
            footer: footers && endsWithFooter(rows, index === prepared.length - 1),
            scrollbar: scrollbars[index] === true,
          }
        }),
        { rowHeight: metrics.rowHeight, headerHeight, cards: cards ? { gap: CARD_GAP, padding: CARD_GAP, borderBottom: CARD_BORDER, divider: CARD_DIVIDER } : undefined },
        prologued ? (measured.get(PROLOGUE_KEY) ?? 0) : 0,
        epilogued ? (measured.get(EPILOGUE_KEY) ?? 0) : 0,
        // the review windows scroll past the end, so J, K, N, P and a row bring any section to the top
        mapped ? viewHeight : 0,
      ),
    [prepared, visualLines, metrics, headerHeight, notices, cards, opens, divides, prologued, epilogued, measured, footers, scrollbars, mapped, viewHeight],
  )

  // the sections as the map of the files shows them: where a step starts, which files the chip plays down
  const mapSections = useMemo(
    (): readonly FileMapEntry[] =>
      mapped
        ? prepared.map(file => ({
            key: file.key,
            path: file.path,
            oldPath: file.file.oldPath,
            status: file.file.status,
            step: file.section?.step != null,
            stepHeading: file.section?.stepHeading ?? null,
            muted: muted?.(file.path) ?? false,
          }))
        : [],
    [mapped, prepared, muted],
  )

  useLayoutEffect(() => {
    const clear = () => setTextSearch(previous => (previous.query === find.query && previous.hits.length === 0 ? previous : { query: find.query, hits: [] }))
    if (!find.open || find.query === "") {
      clear()
      return
    }
    const scrollElement = scroller.current
    if (scrollElement === null) {
      clear()
      return
    }
    const layers: Array<{ root: HTMLElement; file: number }> = []
    const measuredLayers = new Map<string, HTMLElement>()
    scrollElement.querySelectorAll<HTMLElement>("[data-measure]").forEach(root => {
      const key = root.dataset.measure
      if (key !== undefined) {
        measuredLayers.set(key, root)
      }
    })
    const addLayer = (key: string, file: number) => {
      const root = measuredLayers.get(key)
      if (root !== undefined) {
        layers.push({ root, file })
      }
    }
    if (prologued) {
      addLayer(PROLOGUE_KEY, -1)
    }
    prepared.forEach((file, index) => {
      if (layout.files[index]!.opening !== undefined && file.section !== null) {
        addLayer(file.key, index)
      }
    })
    if (epilogued && epilogueElement.current !== null) {
      layers.push({ root: epilogueElement.current, file: prepared.length })
    }
    const update = () => setTextSearch({ query: find.query, hits: layers.flatMap(({ root, file }) => findTextHits(root, find.query, file)) })
    update()
    let frame: number | null = null
    const observer = new MutationObserver(() => {
      if (frame !== null) {
        return
      }
      frame = requestAnimationFrame(() => {
        frame = null
        update()
      })
    })
    for (const { root } of layers) {
      observer.observe(root, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ["hidden"] })
    }
    return () => {
      observer.disconnect()
      if (frame !== null) {
        cancelAnimationFrame(frame)
      }
    }
  }, [find.open, find.query, prepared, layout, prologued, epilogued])

  const textHits = find.open && textSearch.query === find.query ? textSearch.hits : EMPTY_TEXT_HITS
  // the hits of the find in the review: story text and the rows the diff painter shows
  const diffHits = useMemo(() => {
    if (!find.open || find.query === "") {
      return []
    }
    const searched = prepared.map((file, index) => {
      const tables = file.ids.map(id => {
        const text = blobText(texts, id)
        return id === null || text === null ? null : getLineTable(id, text)
      })
      return { rows: layout.files[index]!.rows, tables, ranges: file.cells?.ranges ?? NO_RANGES, renamed: file.cells?.renamed, renamedRun: file.cells?.renamedRun }
    })
    const sideBySide = mode === "split" || prepared.some(file => file.kind === "merge")
    return findHits(searched, find.query, { sideBySide, folded: find.folded })
  }, [find.open, find.query, find.folded, prepared, layout, texts, mode])
  const hits = useMemo(() => (find.open && find.query !== "" ? mergeHits(textHits, diffHits) : []), [find.open, find.query, textHits, diffHits])
  const current = Math.min(find.current, hits.length - 1)
  const diffHitsOnly = useMemo(() => hits.filter((hit): hit is DiffFindHit => hit.kind === "diff"), [hits])
  const currentHit = current < 0 ? undefined : hits[current]
  const currentDiff = currentHit?.kind === "diff" ? diffHitsOnly.indexOf(currentHit) : -1
  const hitLines = useMemo(() => (diffHitsOnly.length === 0 ? undefined : hitsByLine(diffHitsOnly, currentDiff)), [diffHitsOnly, currentDiff])

  useLayoutEffect(() => {
    const identity = currentIdentity.current
    if (identity === null) {
      return
    }
    const index = hits.findIndex(hit => matchesIdentity(hit, identity))
    if (index < 0) {
      currentIdentity.current = null
      reveal.current = null
      setFind(previous => (previous.current === -1 ? previous : { ...previous, current: -1 }))
    } else {
      setFind(previous => (previous.current === index ? previous : { ...previous, current: index }))
    }
  }, [hits])

  useLayoutEffect(() => {
    if (typeof CSS === "undefined" || CSS.highlights === undefined || typeof Highlight === "undefined") {
      return
    }
    const highlights = CSS.highlights
    highlights.delete("bd-find")
    highlights.delete("bd-find-current")
    if (find.open && textHits.length > 0) {
      highlights.set("bd-find", new Highlight(...textHits.map(hit => hit.range)))
      if (currentHit?.kind === "text") {
        highlights.set("bd-find-current", new Highlight(currentHit.range))
      }
    }
    return () => {
      highlights.delete("bd-find")
      highlights.delete("bd-find-current")
    }
  }, [find.open, textHits, currentHit])

  // the files the view shows: a note that names a place of one of them is a button that shows it
  const shownPaths = useMemo(() => new Set(prepared.map(file => file.path)), [prepared])
  // what shows the files beside the view, as the last render gave it: read when a note is painted or pressed
  const elsewhereNow = useRef(elsewhere)
  useLayoutEffect(() => {
    elsewhereNow.current = elsewhere
  })
  // which of the files the notes of the view name the views beside it show: the rows are painted again when one of
  // these answers changes, never because the surface around the view rendered again
  const elsewhereKey =
    elsewhere === undefined
      ? ""
      : prepared
          .map(file =>
            namedFilesOf(file.diff)
              .map(path => (elsewhere.shows(path) ? "1" : "0"))
              .join(""),
          )
          .join(",")

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
      otherFile: otherFileOf(file.diff?.subjectPaths, path => shownPaths.has(path) || elsewhereNow.current?.shows(path) === true),
    }))
    painter.setModel({ files: painterFiles, layout, softWraps, textMetrics: metrics.text, hits: hitLines, notesKey, moveKey })
  }, [painter, prepared, shownPaths, elsewhereKey, layout, softWraps, metrics, width, hitLines, split, notesKey, moveKey])

  // the hit a step chose: a third of the viewport below the header; a text hit waits until its folds open
  useLayoutEffect(() => {
    const identity = reveal.current
    if (identity === null) {
      return
    }
    const hit = hits.find(candidate => matchesIdentity(candidate, identity))
    if (hit === undefined) {
      return
    }
    if (hit.kind === "diff") {
      const rows = layout.files[hit.file]!.rows
      if (rows === null) {
        return
      }
      reveal.current = null
      const top = layout.rowTop(hit.file, rowOfHit(rows, hit)) - layout.stickyTop - headerHeight - (scroller.current?.clientHeight ?? 0) / 3
      painter.scrollTo(Math.max(top, layout.stopTop(hit.file)))
      return
    }
    if (hit.folds.some(fold => fold.hasAttribute("hidden"))) {
      return
    }
    const element = scroller.current
    if (element === null) {
      return
    }
    const rangeRect = hit.range.getBoundingClientRect()
    const scrollerRect = element.getBoundingClientRect()
    const top = rangeRect.top - scrollerRect.top + element.scrollTop - layout.stickyTop - headerHeight - element.clientHeight / 3
    reveal.current = null
    painter.scrollTo(Math.max(top, 0))
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
    const identity = identityOf(hit)
    currentIdentity.current = identity
    reveal.current = identity
    if (hit.kind === "text") {
      revealTextHit(hit)
    } else {
      const text = prepared[hit.file]?.foldKey ?? null
      if (hit.gap >= 0 && text !== null && (getFoldLevels(text)[hit.gap] ?? 0) < EXPANDED) {
        const levels = Array.from({ length: Math.max(getFoldLevels(text).length, hit.gap + 1) }, (_, gap) => getFoldLevels(text)[gap] ?? 0)
        levels[hit.gap] = EXPANDED
        setFoldLevels(text, levels)
        setFolds(previous => new Map(previous).set(text, levels))
      }
    }
    setFind(previous => ({ ...previous, current: next }))
  }

  const closeFind = () => {
    currentIdentity.current = null
    reveal.current = null
    setFind(previous => ({ ...previous, open: false, current: -1 }))
    scroller.current?.focus()
  }

  const openFind = () => {
    setFind(previous => ({ ...previous, open: true }))
    requestAnimationFrame(() => {
      findInput.current?.focus()
      findInput.current?.select()
    })
  }

  const stepFind = (direction: 1 | -1) => {
    if (find.open) {
      stepHit(direction)
    }
  }

  // ⌘F (Ctrl+F off macOS) opens the find, ⌘G steps through its hits
  const onFindKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!(event.metaKey || event.ctrlKey) || event.altKey) {
      return
    }
    if (event.code === "KeyF" && !event.shiftKey) {
      event.preventDefault()
      openFind()
    } else if (event.code === "KeyG" && find.open) {
      event.preventDefault()
      stepFind(event.shiftKey ? -1 : 1)
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

  /** The notes of a file that are open become `open`: its rows are built again, the old lines they stand for shown or not */
  const openNotes = (file: PreparedFile, open: ReadonlySet<number>) => {
    const key = file.foldKey
    if (key !== null) {
      setOpenNotes(key, open)
      setNotes(previous => new Map(previous).set(key, open))
    }
  }

  /**
   * A click on a note that opens: the old lines of its run show, or go again; with `all` (⌥-click, as on a fold)
   * every such note of the file follows it
   */
  const toggleNote = (file: PreparedFile | undefined, note: NoteRow, all: boolean) => {
    const key = file?.foldKey ?? null
    const remark = note.remark
    if (file === undefined || key === null || remark === null || remark.open === null) {
      return
    }
    const at = noteKey(remark)
    if (all) {
      openNotes(file, new Set(remark.open ? [] : (file.rows?.notes ?? []).map(it => it.key)))
      return
    }
    const open = new Set(getOpenNotes(key))
    if (!open.delete(at)) {
      open.add(at)
    }
    openNotes(file, open)
  }

  /**
   * The key of the notes: the notes in view that open show their old lines, or hide them once they all show. In view:
   * the row whole below the header stuck at the top; a note counts as in view with a line it hides or shows (a line
   * that Inline shows once, a line under an opened note), wherever it stands itself: the one note of a rename may be
   * screens above a site of it (`notesOfRows`). A wholly quiet block is no stop of N and P, so the key takes what the
   * view shows, not a change the keys stepped to
   */
  const toggleNotesInView = () => {
    const view = scroller.current
    if (view === null) {
      return
    }
    const inView: { file: PreparedFile; note: number }[] = []
    const top = view.scrollTop + layout.stickyTop + headerHeight
    const bottom = view.scrollTop + view.clientHeight
    for (let index = layout.fileAt(top), last = layout.fileAt(bottom); index <= last && index < prepared.length; index++) {
      const rows = layout.files[index]!.rows
      if (rows !== null) {
        const [from, to] = layout.rowsInView(index, view.scrollTop, view.clientHeight)
        for (const note of notesOfRows(rows, from, to)) {
          inView.push({ file: prepared[index]!, note })
        }
      }
    }
    const isOpen = ({ file, note }: { file: PreparedFile; note: number }) => file.foldKey !== null && getOpenNotes(file.foldKey).has(note)
    const open = !inView.every(isOpen)
    const byFile = new Map<PreparedFile, Set<number>>()
    for (const { file, note } of inView) {
      const notesOfFile = byFile.get(file) ?? new Set(file.foldKey === null ? [] : getOpenNotes(file.foldKey))
      if (open) {
        notesOfFile.add(note)
      } else {
        notesOfFile.delete(note)
      }
      byFile.set(file, notesOfFile)
    }
    for (const [file, notesOfFile] of byFile) {
      openNotes(file, notesOfFile)
    }
  }

  const toggleFile = (key: string) =>
    setToggled(previous => {
      const next = new Set(previous)
      if (!next.delete(key)) {
        next.add(key)
      }
      return next
    })

  /**
   * Shows the section at `index` at the top, its opening (none: its card) or its card, N and P from its top; the opening
   * of `step` in the epilogue as far up as the end of the document lets it
   */
  const showSection = (index: number, part: "opening" | "card" = "opening", step?: string) => {
    const file = prepared[index]!
    changeCursor.current = { key: file.key, index: -1 }
    const ending = part === "opening" && step !== undefined ? epilogueElements().find(element => element.dataset.openingStep === step) : undefined
    if (ending !== undefined) {
      visibleKey.current = file.key
      visiblePart.current = "opening"
      visibleStep.current = step
      visiblePath.current = file.path
      painter.scrollTo(Math.max(layout.epilogueTop + ending.offsetTop - layout.stickyTop, 0))
      fileMap.current?.sync()
      return
    }
    visibleKey.current = file.key
    visiblePart.current = part === "opening" && layout.files[index]!.opening !== undefined ? "opening" : "card"
    visibleStep.current = visiblePart.current === "opening" ? (step ?? file.section?.before?.[0]?.id ?? file.section?.step?.id) : undefined
    visiblePath.current = file.path
    painter.scrollTo(part === "opening" ? layout.fileTop(index) + openingOffset(file, step) : layout.stopTop(index))
    fileMap.current?.sync()
  }

  /** The element of the opening of `step` in the layer of openings of a section, of the steps told before its own */
  const openingElements = (file: PreparedFile): HTMLElement[] =>
    file.section?.before === undefined || file.section.before.length === 0
      ? []
      : [...(scroller.current?.querySelector(`[data-measure="${CSS.escape(file.key)}"]`)?.querySelectorAll<HTMLElement>("[data-opening-step]") ?? [])]
  /** Where the opening of `step` lies in the layer of openings of its section: 0 for the first, and without `step` */
  const openingOffset = (file: PreparedFile, step: string | undefined): number =>
    step === undefined ? 0 : (openingElements(file).find(element => element.dataset.openingStep === step)?.offsetTop ?? 0)
  /** The openings of the epilogue, each the element of its step */
  const epilogueElements = (): HTMLElement[] => [...(epilogueElement.current?.querySelectorAll<HTMLElement>("[data-opening-step]") ?? [])]
  /**
   * The step whose opening is at `y` of the document in the layer of openings of the section of `index`, of the epilogue
   * below the last card
   */
  const openingStepAt = (index: number, y: number): string | undefined => {
    const file = prepared[index]
    const ending = layout.epilogue > 0 && y >= layout.epilogueTop
    let found = ending ? undefined : file?.section?.step?.id
    if (file === undefined) {
      return found
    }
    const offset = y - (ending ? layout.epilogueTop : layout.sectionStart(index))
    for (const element of ending ? epilogueElements() : openingElements(file)) {
      if (element.offsetTop <= offset) {
        found = element.dataset.openingStep
      } else {
        break
      }
    }
    return found
  }

  /**
   * The stop the view shows: the one it showed last, else, while nothing scrolled it nor chose a section, the one at its
   * top, so the first J of a review that opens on its overview goes on from the overview
   */
  const visibleNow = (): { key: string; part: LayoutPart; step?: string } | undefined => {
    if (visibleKey.current !== undefined) {
      return { key: visibleKey.current, part: visiblePart.current, ...(visiblePart.current === "opening" && visibleStep.current !== undefined && { step: visibleStep.current }) }
    }
    const element = scroller.current
    const index = element === null ? -1 : layout.fileShownAt(element.scrollTop, element.clientHeight)
    const file = prepared[index]
    if (element === null || file === undefined) {
      return undefined
    }
    const part = layout.partShownAt(element.scrollTop, element.clientHeight)
    const step = part === "opening" ? openingStepAt(index, element.scrollTop + layout.stickyTop + headerHeight / 2) : undefined
    return { key: file.key, part, ...(step !== undefined && { step }) }
  }

  /** Shows the prologue at the top, the section of `file` (the first) the one it is above */
  const showPrologue = (file: PreparedFile) => {
    changeCursor.current = null
    visibleKey.current = file.key
    visiblePart.current = "prologue"
    visibleStep.current = undefined
    painter.scrollTo(0)
    fileMap.current?.sync()
  }

  /**
   * A click on the map of the files: the section under it at the top (`PROLOGUE_SECTION`: the prologue), the tree and
   * the bar of the step following, as a click in the tree shows it. The diffs take the focus, as a click on the line at
   * the end of a card gives it them: a focus in a card the jump unmounts would take the keys with it, so J goes on from
   * the section shown
   */
  const showMapSection = (index: number) => {
    const file = prepared[index === PROLOGUE_SECTION ? 0 : index]
    if (file === undefined) {
      return
    }
    scroller.current?.focus({ preventScroll: true })
    if (index === PROLOGUE_SECTION) {
      showPrologue(file)
    } else {
      showSection(index)
    }
    onVisibleSectionChange?.(file.key, file.path, visiblePart.current, visibleStep.current)
    onVisibleFileChange?.(file.path)
  }

  // the places of the jumps of the view (Go to Declaration, the other end of a move), and the place a reveal waits for
  const history = useRef<{ back: DeclarationPlace[]; forward: DeclarationPlace[] }>({ back: [], forward: [] })
  const pendingReveal = useRef<DeclarationPlace | null>(null)
  // the line a reveal showed for itself last (an end of a move), until a name is revealed: the place of the view then
  const landedPlace = useRef<DeclarationPlace | null>(null)
  const [revealTick, setRevealTick] = useState(0)
  const navigates = onGoToDeclaration !== undefined && scroll === "own"

  /** Shows a place of the view: its file open, its fold expanded, then (`revealTick`) scrolled to and selected */
  /** Shows `place`: the key of the section that shows it, undefined for a file the view does not show */
  const revealPlace = (place: DeclarationPlace): string | undefined => {
    const side = place.side === "old" ? OLD : NEW
    const index = sectionOf(prepared, place.path, { side, line: place.line, unit: place.unit })
    const file = prepared[index]
    if (file === undefined) {
      return undefined
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
    // an old line that Inline shows once, by its new line: its note opens, as its fold does, so its name can be selected;
    // a line shown for itself (an end of a move) is the note that stands for it
    const note = side === OLD && text !== null && file.rows !== null && place.wholeLine !== true ? noteOfLine(file.rows, place.line) : null
    if (note !== null && text !== null && !getOpenNotes(text).has(note)) {
      openNotes(file, new Set(getOpenNotes(text)).add(note))
    }
    pendingReveal.current = place
    // where Forward goes on from after a Back: a line shown for itself selects nothing a place could be read from
    landedPlace.current = place.wholeLine === true ? place : null
    // and the end of a move the view stands at, whatever showed it (a jump, Back, Forward): the key of the moves goes
    // on from its note; a name shown is no end of a move
    painter.setMoveHand(place.wholeLine === true ? { key: file.key, side, line: place.line } : null)
    setRevealTick(tick => tick + 1)
    return file.key
  }

  /**
   * Shows a place a note names (the other end of a move, the declaration of a name another file declares), from the
   * place `from` of the note: in this view where it shows the file of the place, and `from` is then the place Back
   * returns to where the view has Back; else in the view of that file beside this one (`elsewhere`), which has none.
   * Nothing where neither shows the file: the note is plain words there
   */
  const showPlace = (place: DeclarationPlace, from: DeclarationPlace) => {
    if (sectionOf(prepared, place.path) >= 0) {
      if (navigates) {
        history.current.back.push(from)
        history.current.forward = []
      }
      revealPlace(place)
    } else if (elsewhere?.shows(place.path) === true) {
      elsewhere.show(place)
    }
  }

  /** The path of the file of the other end of a move: the file itself, or of a move between two files the one it names */
  const otherPathOf = (file: PreparedFile, end: MoveEnd): string | undefined => (end.file === null ? file.path : file.diff?.subjectPaths?.[end.file])

  /** Whether the view can show the other end of a move from `file`: in a file it shows, or one a view beside it shows */
  const reachesMove = (file: PreparedFile | undefined, end: MoveEnd): boolean => {
    const path = file === undefined ? undefined : otherPathOf(file, end)
    return path !== undefined && (shownPaths.has(path) || elsewhere?.shows(path) === true)
  }

  /**
   * Shows the other end of a move from one of its ends (a click on the place its note names, the key of the moves, the
   * item of the menu of the code): the note of that end a third of the viewport below the header, marked, the file
   * open. Where the view has Back, this end is the place it returns to. Both ends of a move within a file are in that
   * file, of a review maybe in two of its sections; the other end of a move between two files is in the file its note
   * names
   */
  const jumpMove = (file: PreparedFile | undefined, end: MoveEnd) => {
    const other = file === undefined ? undefined : otherPathOf(file, end)
    if (file === undefined || other === undefined) {
      return
    }
    // the first line of an end, the blank lines that moved with the lines among them: what its note stands before
    const place = (path: string, at: MoveEnd["at"], [line]: readonly [number, number]): DeclarationPlace => ({
      path,
      side: at === "source" ? "old" : "new",
      line,
      column: 0,
      endColumn: 0,
      wholeLine: true,
    })
    showPlace(place(other, end.at === "source" ? "destination" : "source", end.other), place(file.path, end.at, end.lines))
  }

  /**
   * Shows the declaration of a name that another file of the subject declares, from the note of its rename in this
   * file (a click on "declared in Store.kt"): the new name selected in its line there, the site the note stands at
   * the place Back returns to
   */
  const jumpDeclaration = (file: PreparedFile | undefined, note: RenameNote, at: number) => {
    const rename = note.renames[at]
    const declared = note.reach[at]?.declared
    const path = declared == null ? undefined : file?.diff?.subjectPaths?.[declared.file]
    if (file === undefined || rename === undefined || declared == null || path === undefined) {
      return
    }
    const name = (where: string, line: number): DeclarationPlace => ({ path: where, side: "new", line, column: 0, endColumn: 0, name: rename.new })
    // the first site of the rename in this file, where its note stands or a line of its block
    const site = rename.newSites[0] ?? note.lines[NEW]
    showPlace(name(path, declared.line), name(file.path, site))
  }

  /**
   * The key of the moves: from the note of an end of a move in view to its other end. The one note that shows the hint
   * of the key (`DiffPainter.keyedMove`): the note of the end the view stands at while it is in view (where a jump,
   * Back or Forward landed, so a second press goes back; the note N or P stopped under), else the first one in view
   */
  const jumpMoveInView = () => {
    const from = painter.keyedMove()
    if (from !== null) {
      jumpMove(prepared[from.file], from.end)
    }
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
      onTopFile: (index, part) => {
        // the section at the top, which a review window's scroll past the end brings any section to; of a view that ends
        // at its end the section it showed last (a file of the tree, N, J) stays the one shown while it is on screen: a
        // section near the end, which a scroll to it cannot bring to the top, is not the one above it; the prologue at
        // the top is the prologue
        const cursor = changeCursor.current === null || mapped ? -1 : layout.indexOf(changeCursor.current.key)
        const element = scroller.current
        const bottom = element === null ? index : layout.fileAt(element.scrollTop + element.clientHeight)
        const shown = part !== "prologue" && cursor >= index && cursor <= bottom ? cursor : index
        const file = prepared[shown]
        const shownPart = shown === index ? part : "card"
        // at a layer of openings of several steps, the one at the top
        const step = shownPart === "opening" && element !== null ? openingStepAt(shown, element.scrollTop + layout.stickyTop + headerHeight / 2) : undefined
        // another section, or another part of it: the tree selects the row of a step at its opening, of a file at its
        // card, and keeps its row at the prologue
        if (file !== undefined && (file.key !== visibleKey.current || shownPart !== visiblePart.current || step !== visibleStep.current)) {
          // the user scrolled into another section: N starts from its top
          if (file.key !== visibleKey.current && changeCursor.current?.key !== file.key) {
            changeCursor.current = null
          }
          visibleKey.current = file.key
          visiblePart.current = shownPart
          visibleStep.current = step
          onVisibleSectionChange?.(file.key, file.path, shownPart, step)
        }
        if (file !== undefined && file.path !== visiblePath.current) {
          visiblePath.current = file.path
          onVisibleFileChange?.(file.path)
        }
      },
      onScroll: (top, height) => {
        if (onCardsPassed !== undefined) {
          const passed = layout.passedCards(top, height, passedEdges.current, index => prepared[index]?.pending === false)
          if (passed.length > 0) {
            onCardsPassed(passed.map(index => ({ path: prepared[index]!.path, key: prepared[index]!.key })))
          }
        }
      },
      onFold: (index, fold, all) => (all ? expandAll(prepared[index]) : expandFold(prepared[index], fold)),
      onNoteToggle: (index, note, all) => toggleNote(prepared[index], note, all),
      onMoveJump: (index, end) => jumpMove(prepared[index], end),
      onDeclarationJump: (index, note, rename) => jumpDeclaration(prepared[index], note, rename),
      onNote: (index, note) => {
        const target = prepared[index]?.section?.targets.get(note.block)
        const shown = target === undefined ? -1 : layout.indexOf(target)
        if (shown >= 0) {
          const file = prepared[shown]!
          if (file.collapsed) {
            toggleFile(file.key)
          }
          showSection(shown)
          // the section shown is the one at the top, for the tree and the bar of the step, as a click on the map
          onVisibleSectionChange?.(file.key, file.path, visiblePart.current, visibleStep.current)
          onVisibleFileChange?.(file.path)
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
    const resize = new ResizeObserver(([entry]) => {
      setWidth(entry?.contentRect.width ?? 0)
      setViewHeight(entry?.contentRect.height ?? 0)
    })
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
    painter.scrollTo(Math.max(top, layout.stopTop(file)))
  }

  // Go to Declaration: the name under the pointer with the modifier held, its underline, the jump, Back and Forward
  const links = useRef<HTMLDivElement>(null)
  const pointer = useRef<{ x: number; y: number } | null>(null)
  /** The trigger of the menu of the code, and what a menu key opens it on: the name at the selection and the point under it */
  const codeDocument = useRef<HTMLDivElement>(null)
  const keyedMenu = useRef<{ word: CodeWord | null; at: { x: number; y: number } } | null>(null)
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
        line.dataset.testid = "diff-link"
        line.style.cssText = `left:${rect.left}px;top:${rect.bottom - 1}px;width:${rect.width}px`
        return line
      }),
    )
    target.style.cursor = rects.length > 0 ? "pointer" : ""
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
          return revealPlace(place) !== undefined
        },
        found.place,
      )
    }
  }

  /** The pointer moved over the code, or opens the menu of the code: the point the names are read at */
  const onCodeMouseMove = (event: ReactMouseEvent<HTMLDivElement>) => {
    pointer.current = { x: event.clientX, y: event.clientY }
    showLink(modifierHeld(event) ? painter.wordAt(event.clientX, event.clientY) : null)
  }

  /** A `contextmenu` in the view: the pointer's, at its point; the one a menu key dispatched is no move of the pointer */
  const onCodeContextMenu = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (keyedMenu.current === null) {
      onCodeMouseMove(event)
    }
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

  /**
   * The items of the menu of the code: built when it opens, for the name it opened on. The pointer opens it on the name
   * under it, whatever is selected elsewhere: its press selects no word in a part that is not selectable (the
   * struck-through old name of an Inline row), on Windows in no text, and where it is on no name the menu has none. A
   * menu key opens it on the name at the selection (`openCodeMenuByKey`)
   */
  const [codeMenu, setCodeMenu] = useState<MenuItem[]>([])
  const openCodeMenu = () => {
    const keyed = keyedMenu.current
    const at = keyed?.at ?? pointer.current ?? { x: 0, y: 0 }
    const word = keyed === null ? painter.wordAt(at.x, at.y) : keyed.word
    const target = word === null ? null : siteOf(word)
    // the end of a move the menu opened on: a line of a move or the note at its end; of a menu key the line of a move
    // the selection starts in
    const found = keyed === null ? painter.moveAt(at.x, at.y) : painter.moveAtSelection()
    // an end in another file of the subject that no view shows has no other end to show here
    const moved = found !== null && reachesMove(prepared[found.file], found.end) ? found : null
    // the key of the moves does the same only from the note it takes now: the item names it there alone
    const keyOfMove = moved !== null && moveKey !== undefined && painter.keyedMove()?.end === moved.end ? moveKey : null
    const movedFile = moved === null ? undefined : prepared[moved.file]
    const paths = movedFile?.diff?.subjectPaths
    setCodeMenu(
      codeMenuItems({
        name: target === null ? null : word!.text,
        goToDeclaration: () => goTo(word, at),
        findUsages: onFindUsages === undefined ? null : () => goTo(word, at, onFindUsages),
        moveEnd:
          moved === null
            ? null
            : {
                at: moved.end.at,
                go: () => jumpMove(movedFile, moved.end),
                key: keyOfMove,
                file: moved.end.file === null || paths === undefined ? null : fileNameAmong(paths, moved.end.file),
              },
      }),
    )
  }

  /**
   * The context menu key or Shift+F10 in the view: the menu of the code on the name at the selection, opened under the
   * start of that name as a list of ⌘B is. The engine's own `contextmenu` of the key would go to the view, not to the
   * trigger inside it, at the middle of the view
   */
  const openCodeMenuByKey = (event: KeyboardEvent<HTMLDivElement>, view: HTMLDivElement, trigger: HTMLDivElement) => {
    const word = painter.wordAtSelection()
    const selection = document.getSelection()
    const selected = selection !== null && selection.rangeCount > 0 && view.contains(selection.anchorNode) ? selection.getRangeAt(0).getClientRects()[0] : undefined
    keyedMenu.current = { word, at: codeMenuKeyPoint(word?.rects()[0] ?? selected, view.getBoundingClientRect()) }
    // the menu opens, and reads what the key opens it on, before this returns
    openMenuByKey(event, trigger, keyedMenu.current.at.x, keyedMenu.current.at.y)
    keyedMenu.current = null
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

  // a place the surface around the view asks for: once its file is laid out, and again for another one
  const askedPlace = useRef<DeclarationPlace | null>(null)
  useLayoutEffect(() => {
    if (shownPlace !== undefined && askedPlace.current !== shownPlace && sectionOf(prepared, shownPlace.path) >= 0) {
      askedPlace.current = shownPlace
      revealInitial(shownPlace)
    }
  }, [layout, prepared, shownPlace])

  // the place a reveal waits for: scrolled to a third of the viewport below the header, then its name selected
  useLayoutEffect(() => {
    const place = pendingReveal.current
    const side = place?.side === "old" ? OLD : NEW
    const index = place === null ? -1 : sectionOf(prepared, place.path, { side, line: place.line, unit: place.unit })
    const rows = index < 0 ? null : layout.files[index]!.rows
    if (place === null || rows === null) {
      return
    }
    pendingReveal.current = null
    // a line shown for itself: the row of words that says what it is (the note at an end of a move), else its own row
    const row = place.wholeLine === true ? rowOfPlace(rows, side, place.line) : rowOfLine(rows, side, place.line)
    const top = layout.rowTop(index, row) - layout.stickyTop - headerHeight - painter.viewportHeight() / 3
    if (scroll === "own") {
      painter.scrollTo(Math.max(top, layout.stopTop(index)))
      // the keys of the view (Back) work at the place shown, whether a choice closed or a view of a file opened
      scroller.current?.focus({ preventScroll: true })
    } else {
      // a block of the scroller of its parent: what stands above the view there stays above the row, the focus where it is
      painter.scrollTo(top)
    }
    if (place.wholeLine === true) {
      painter.land(index, side, place.line)
    } else {
      // once the row of the name is painted with its text, whenever that is
      painter.selectWord(index, side, place.line, place.column, place.endColumn, place.name ?? null)
    }
  }, [layout, prepared, revealTick, painter, headerHeight, scroll])

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
    // where the view is: the name selected, else the line the last reveal showed for itself (an end of a move)
    const word = painter.wordAtSelection()
    const current = (word === null ? null : siteOf(word))?.place ?? landedPlace.current
    if (current !== null) {
      to.push(current)
    }
    revealPlace(place)
  }

  const canStepHistory = (direction: "back" | "forward") =>
    direction === "back" ? history.current.back.length > 0 || onHistoryEnd !== undefined : history.current.forward.length > 0

  const onNavigationKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!navigates) {
      return
    }
    const command = isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey
    const findsUsages = event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey && event.code === "F7" && onFindUsages !== undefined
    // a menu key while the view itself has the focus: a control in it (the header of a file) has a menu of its own
    if (isMenuKey(event) && event.target === scroller.current && codeDocument.current !== null) {
      openCodeMenuByKey(event, scroller.current, codeDocument.current)
    } else if ((command && !event.altKey && !event.shiftKey && event.code === "KeyB") || findsUsages) {
      const word = painter.wordAtSelection()
      const rect = word?.rects()[0]
      event.preventDefault()
      goTo(word, rect === undefined ? (pointer.current ?? { x: 0, y: 0 }) : { x: rect.left, y: rect.bottom }, findsUsages ? onFindUsages : onGoToDeclaration)
    } else if (isMac && command && !event.altKey && (event.code === "BracketLeft" || event.code === "BracketRight")) {
      const direction = event.code === "BracketLeft" ? "back" : "forward"
      if (canStepHistory(direction)) {
        event.preventDefault()
        stepHistory(direction)
      }
    } else if (!isMac && command && event.altKey && (event.code === "ArrowLeft" || event.code === "ArrowRight")) {
      const direction = event.code === "ArrowLeft" ? "back" : "forward"
      if (canStepHistory(direction)) {
        event.preventDefault()
        stepHistory(direction)
      }
    }
  }

  useImperativeHandle(ref, () => ({
    scrollToFile: path => {
      const index = sectionOf(prepared, path)
      if (index >= 0) {
        showSection(index)
      }
    },
    scrollToSection: (key, part, step) => {
      const index = prepared.findIndex(file => file.key === key)
      if (index >= 0) {
        showSection(index, part, step)
      }
    },
    moveChange: (direction, accept) => {
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
        const { key, path } = prepared[fileIndex]!
        const targets = accept === undefined || accept(path) ? (layout.files[fileIndex]!.rows?.changeStarts ?? []) : []
        const clamped = index === Number.MAX_SAFE_INTEGER ? targets.length - 1 : index
        const target = targets[clamped]
        if (target !== undefined && clamped >= 0) {
          changeCursor.current = { key, index: clamped }
          // the stop under the note of a move (where its lines stand now): the key of the moves takes that note
          const note = layout.files[fileIndex]!.rows?.moveNotes.find(({ row }) => row === target - 1)
          painter.setMoveHand(note === undefined ? null : moveHandOf(key, note.end))
          visibleKey.current = key
          visiblePart.current = "card"
          visibleStep.current = undefined
          visiblePath.current = path
          scrollToChange(fileIndex, target)
          fileMap.current?.sync()
          return { path, key }
        }
        fileIndex += direction
        index = direction === 1 ? 0 : Number.MAX_SAFE_INTEGER
      }
      return undefined
    },
    scrollToPrologue: () => {
      const file = prepared[0]
      if (file !== undefined && layout.prologue > 0) {
        showPrologue(file)
      }
    },
    toggleNotes: toggleNotesInView,
    jumpMove: jumpMoveInView,
    visibleStop: visibleNow,
    focus: () => scroller.current?.focus(),
    openFind,
    stepFind,
    revealDeclaration: (place, from) => {
      if (sectionOf(prepared, place.path) < 0) {
        return undefined
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
    return (
      <FileSection
        key={file.key}
        index={index}
        file={file}
        rows={rows}
        notice={noticeOf(file, texts)}
        noticeRow={layout.files[index]!.notice}
        top={layout.offsets[index]!}
        dividerHeight={layout.files[index]!.opening === undefined ? layout.openingHeight(index) : 0}
        footer={layout.files[index]!.footer === true}
        scrollbar={layout.scrollbarHeight(index) > 0}
        height={layout.fileHeight(index)}
        rowsHeight={layout.rowsHeight(index)}
        card={cards}
        rowHeight={metrics.rowHeight}
        columnWidth={metrics.columnWidth}
        width={width}
        split={split}
        sideWidths={sideWidthsOf(file, texts, metrics.columnWidth).join(" ")}
        painter={painter}
        renderHeader={renderHeader}
        renderDivider={renderDivider}
        renderFooter={renderFooter}
        onToggle={toggleFile}
        onExpandAll={expandAll}
      />
    )
  })

  // the prologue and the openings of the steps: always mounted, not virtualized, few, each measured where it lies
  // (`useMeasuredHeights`); in the order of the document, each opening right before the card of its section, so the
  // tab order and a screen reader read an opening before its card (the positions are absolute: the order moves nothing)
  const openings = prepared.flatMap((file, index) => (layout.files[index]!.opening === undefined || file.section === null ? [] : [{ index, section: file.section }]))
  const content: ReactNode[] = []
  if (prologued) {
    content.push(
      <div
        key={PROLOGUE_KEY}
        ref={measure}
        className="dv-opening dv-prose dv-prologue"
        data-testid="diff-view-prologue"
        data-measure={PROLOGUE_KEY}
        style={{ top: layout.stickyTop }}
      >
        {prologue}
      </div>,
    )
  }
  let opened = 0
  for (let index = first; index <= first + sections.length; index++) {
    for (; opened < openings.length && (openings[opened]!.index <= index || index === first + sections.length); opened++) {
      const { index: at, section } = openings[opened]!
      content.push(<Opening key={`opening:${section.key}`} section={section} top={layout.sectionStart(at)} renderOpening={renderOpening!} measure={measure} />)
    }
    const section = sections[index - first]
    if (section !== undefined) {
      content.push(section)
    }
  }
  if (epilogued) {
    content.push(
      <div
        key={EPILOGUE_KEY}
        ref={element => {
          epilogueElement.current = element
          return measure(element)
        }}
        className="dv-opening"
        data-testid="diff-view-epilogue"
        data-measure={EPILOGUE_KEY}
        style={{ top: layout.epilogueTop }}
      >
        {epilogue}
      </div>,
    )
  }

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
    // the view says it drags the divider: the cursor keeps the resize one past the 8 px of the line, the code selects
    // nothing on the way, and a test sees that the press took the divider (diff-view.css)
    element.dataset.dragging = ""
    const move = (moved: PointerEvent) => {
      share = shareAt(moved.clientX)
      setDraggedSplit(share)
    }
    const end = () => {
      element.removeEventListener("pointermove", move)
      element.removeEventListener("pointerup", end)
      element.removeEventListener("pointercancel", end)
      delete element.dataset.dragging
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
      // busy while the diff of a file is read: its card says "Loading…", N and P find no change in it yet
      aria-busy={prepared.some(file => file.pending) || undefined}
      tabIndex={scroll === "own" ? 0 : undefined}
      className={cn(scroll === "own" ? "dv-scroller min-h-0 min-w-0 flex-1" : "dv-embedded", cards && "dv-cards")}
      data-ligatures={ligatures ? undefined : "off"}
      data-split={mode === "split" ? "" : undefined}
      style={mode === "split" ? ({ "--dv-split": String(split) } as CSSProperties) : undefined}
      onPointerDown={scroll === "own" && mode === "split" ? event => dragSplit(event) : undefined}
      onDoubleClick={scroll === "own" && mode === "split" ? event => onDividerOf(event) && setDiffSplitRatio(0.5) : undefined}
      data-plain={plain ? "" : undefined}
      onMouseMove={navigates ? onCodeMouseMove : undefined}
      onContextMenuCapture={navigates ? onCodeContextMenu : undefined}
      onMouseLeave={navigates ? () => showLink(null) : undefined}
      onClickCapture={navigates ? onCodeClick : undefined}
      onScroll={navigates ? () => showLink(null) : undefined}
      data-testid="diff-view"
    >
      {navigates ? (
        <ContextMenu onOpenChange={open => open && openCodeMenu()}>
          {/* the trigger turns off the selection of its content; the code keeps it (the copy, ⌘B) */}
          <ContextMenuTrigger ref={codeDocument} render={<div className="dv-document select-text" style={{ height: layout.height }} />}>
            {content}
          </ContextMenuTrigger>
          <ContextMenuContent>
            <ContextMenuItems groups={[codeMenu]} />
          </ContextMenuContent>
        </ContextMenu>
      ) : (
        <div className="dv-document" style={{ height: layout.height }}>
          {content}
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
          onQueryChange={query => {
            currentIdentity.current = null
            reveal.current = null
            setFind(previous => ({ ...previous, query, current: -1 }))
          }}
          count={hits.length}
          current={current}
          folded={find.folded}
          onFoldedChange={folded => {
            currentIdentity.current = null
            reveal.current = null
            setFind(previous => ({ ...previous, folded, current: -1 }))
          }}
          onStep={stepHit}
          onClose={closeFind}
          inputRef={findInput}
        />
      )}
      {mapped ? (
        // the map of the files beside the scroller, in place of its scrollbar (diff-view.css)
        <div className="flex min-h-0 flex-1">
          {view}
          <DiffFileMap
            ref={fileMap}
            scroller={() => scroller.current}
            scrollTo={top => {
              if (scroller.current !== null) {
                scroller.current.scrollTop = top
              }
            }}
            layout={layout}
            sections={mapSections}
            // the section the view showed last, or the prologue; one of files gone since (the story replaced the files), the
            // one at the top
            current={() => {
              const shown = layout.indexOf(visibleKey.current ?? "")
              if (visiblePart.current === "prologue" && layout.prologue > 0) {
                return PROLOGUE_SECTION
              }
              return shown >= 0 ? shown : layout.fileAtTop(scroller.current?.scrollTop ?? 0)
            }}
            onShowSection={showMapSection}
            onUserScroll={() => painter.followUserScroll()}
          />
        </div>
      ) : (
        view
      )}
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
  /** The divider of a group above the card, 0 for none: it ends at `top` */
  dividerHeight: number
  /** The card ends with a line below its rows (`LayoutFile.footer`), one row high */
  footer: boolean
  /** A side of its rows is wider than its column: the strip of their scrollbars below the rows (`LayoutFile.scrollbar`) */
  scrollbar: boolean
  height: number
  rowsHeight: number
  /** The file is a card: a border, rounded corners, its header the top of the card */
  card: boolean
  rowHeight: number
  columnWidth: number
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
  renderDivider: DiffViewProps["renderDivider"]
  renderFooter: DiffViewProps["renderFooter"]
  onToggle: (path: string) => void
  onExpandAll: (file: PreparedFile) => void
}

/**
 * The opening of a step above the card of its first section, at the start of the section, as tall as its words: the
 * layout has the height `useMeasuredHeights` measures of it. It renders again only when its own props change, not on
 * a scroll into the next files
 */
const Opening = memo(function Opening({
  section,
  top,
  renderOpening,
  measure,
}: {
  section: DiffSection
  top: number
  renderOpening: (section: DiffSection) => ReactNode
  measure: MeasuredHeights["measure"]
}) {
  return (
    <div ref={measure} className="dv-opening" data-testid="diff-view-opening" data-section={section.key} data-measure={section.key} style={{ top }}>
      {renderOpening(section)}
    </div>
  )
})

/**
 * The section of a file: as tall as the file, its header sticky, the rows element the painter fills, the line at the
 * end of a long card; the divider of a group above it
 */
const FileSection = memo(function FileSection(props: FileSectionProps) {
  const {
    index,
    file,
    rows,
    notice,
    noticeRow,
    top,
    dividerHeight,
    footer,
    scrollbar,
    height,
    rowsHeight,
    card,
    rowHeight,
    columnWidth,
    width,
    split,
    sideWidths,
    painter,
    renderHeader,
    renderDivider,
    renderFooter,
    onToggle,
    onExpandAll,
  } = props
  const { section } = file
  return (
    <>
      {dividerHeight > 0 && section !== null && renderDivider !== undefined && (
        // on the canvas between the gap above the card and the card, scrolled with it (diff-view.css)
        <div className="dv-divider" data-testid="diff-view-divider" data-section={section.key} style={{ top: top - dividerHeight, height: dividerHeight }}>
          {renderDivider(section)}
        </div>
      )}
      <section
        className={card ? "dv-section dv-card" : "dv-section"}
        aria-label={file.path}
        data-testid="diff-view-file"
        data-path={file.path}
        data-section={section?.key}
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
              section,
              notice,
              empty: file.pending || isEmptyNotice(file.diff, file.rows),
              folded: file.folded,
              collapsed: file.collapsed,
              onToggle: () => onToggle(file.key),
              onExpandAll: rows !== null && rows.segments.some(segment => segment.kind === "fold") ? () => onExpandAll(file) : undefined,
              // the rows of the file whether it is collapsed or not: a collapsed card says what it holds to read
              toRead: file.rows === null || file.rows.changeStarts.length >= file.rows.changes ? null : { toRead: file.rows.changeStarts.length, changes: file.rows.changes },
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
            {scrollbar && (
              <HorizontalBars file={file} sideWidths={sideWidths.split(" ").map(Number)} columnWidth={columnWidth} codeWidths={codeWidthsOf(file, width, columnWidth, split)} />
            )}
          </>
        )}
        {footer && renderFooter !== undefined && (
          // after the scrollbars, which stick to the bottom of the rows above it; not selectable; no line of its own under
          // a fold, which draws one (diff-view.css)
          <div className="dv-footer" data-testid="diff-view-footer" data-after-fold={rows?.segments.at(-1)?.kind === "fold" ? "" : undefined} style={{ height: rowHeight }}>
            {renderFooter(file.key, file.path)}
          </div>
        )}
      </section>
    </>
  )
})

/**
 * The sticky scrollbar of each column whose widest line is wider than it, in the strip the layout keeps for them below
 * the rows (`LayoutFile.scrollbar`): at the end of the file they lie under the last row, never over it
 */
function HorizontalBars({ file, sideWidths, columnWidth, codeWidths }: { file: PreparedFile; sideWidths: readonly number[]; columnWidth: number; codeWidths: readonly number[] }) {
  const columns = file.cells?.columns ?? []
  const sideBySide = columns.length > 1
  const widths = scrollerWidths(file, sideWidths, codeWidths)
  return (
    <div className="dv-hscrolls" data-testid="diff-view-scrollbars">
      {widths !== null &&
        columns.map((side, index) => (
          <div key={side} className="dv-hscroll" data-testid="diff-hscroll" data-side={side}>
            <div style={{ width: widths[index]! + file.gutterColumns * columnWidth * (sideBySide ? 1 : (file.cells?.numbers.length ?? 2)), height: 1 }} />
          </div>
        ))}
    </div>
  )
}

/** The rows a file lays out: none while it is collapsed or shows none */
function layoutRowsOf(file: PreparedFile): FileRows | null {
  return file.collapsed || file.rows === null || file.rows.rowCount === 0 ? null : file.rows
}

/**
 * The widest line of each text in columns, by its blob id: a file keeps the strip of its scrollbar once its text left
 * the window of the view, so the layout does not change as the view scrolls
 */
const widestColumns = new Map<string, number>()

/** The widest Inline row of a diff in columns (`widestInlineRow`), kept with the diff as `widestColumns` keeps a text's */
const widestInlineRows = new WeakMap<TextDiff, number>()

/**
 * The width of the widest line each side shows in CSS px, two columns of padding with it; 0 while its text is unread.
 * In Inline the new side shows the rows of the similar pairs too, each wider than its lines by the deleted parts
 */
function sideWidthsOf(file: PreparedFile, texts: ReadonlyMap<string, BlobTextEntry>, columnWidth: number): number[] {
  const columns = file.ids.map(id => {
    if (id === null) {
      return undefined
    }
    const text = blobText(texts, id)
    if (text !== null) {
      widestColumns.set(id, maxColumns(getLineTable(id, text), DIFF_TAB_SIZE))
    }
    return widestColumns.get(id)
  })
  const diff = file.rows?.mode === "inline" && file.diff?.content.kind === "text" ? file.diff.content : null
  if (diff !== null) {
    const [oldId, newId] = file.ids
    const oldText = blobText(texts, oldId ?? null)
    const newText = blobText(texts, newId ?? null)
    if (oldId != null && newId != null && oldText !== null && newText !== null) {
      widestInlineRows.set(diff, widestInlineRow(diff, getLineTable(oldId, oldText), getLineTable(newId, newText), DIFF_TAB_SIZE))
    }
    const inline = widestInlineRows.get(diff)
    if (inline !== undefined && columns[NEW] !== undefined) {
      columns[NEW] = Math.max(columns[NEW], inline)
    }
  }
  return columns.map(count => (count === undefined ? 0 : (count + 2) * columnWidth))
}

/** The width of the scroller of each column (one column shows the line of either side); null when every line fits */
function scrollerWidths(file: PreparedFile, sideWidths: readonly number[], codeWidths: readonly number[]): number[] | null {
  const columns = file.cells?.columns ?? []
  const widths = columns.length > 1 ? columns.map(side => sideWidths[side] ?? 0) : [Math.max(0, ...sideWidths)]
  return columns.length === 0 || widths.every((contentWidth, index) => contentWidth <= codeWidths[columns[index]!]!) ? null : widths
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
  onScroll: () => {},
  onFold: () => {},
  onNote: () => {},
  onNoteToggle: () => {},
  onMoveJump: () => {},
  onDeclarationJump: () => {},
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
