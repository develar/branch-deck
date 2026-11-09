import { memo, useEffect, useRef, useState } from "react"
import type { ComponentProps, KeyboardEvent, ReactElement, ReactNode, RefObject } from "react"
import { cn } from "cn"
import {
  CableIcon,
  ChevronRightIcon,
  FlaskConicalIcon,
  FolderIcon,
  FolderOpenIcon,
  GitCommitHorizontalIcon,
  LayersIcon,
  PackageIcon,
  ScrollTextIcon,
  TargetIcon,
} from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CommitRow, CommitRowBadges } from "@/components/commit/CommitRow"
import type { CommitRowFacts } from "@/components/commit/CommitRow"
import { CommitRowMark } from "@/components/commit/CommitRowMark"
import { CommitSubjectText } from "@/components/commit/CommitSubject"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { FileRowMark } from "@/components/files/FileRowMark"
import { BreakableCode } from "@/components/shared/CodeText"
import { LazyContextMenu } from "@/components/shared/LazyContextMenu"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import type { RowCardState } from "@/components/shared/RowCard"
import { useRowCardHover } from "@/hooks/useRowCardHover"
import type { RowCardHover } from "@/hooks/useRowCardHover"
import type { CommitFile } from "@/lib/bindings"
import { copyText, isCopyKey } from "@/lib/clipboard"
import type { CardCommit } from "@/lib/commit"
import { formatCommitCount, toCardCommit } from "@/lib/commit"
import { formatCommitFilePath, formatFileCount, formatFileStatus } from "@/lib/commit-files"
import { copiedText } from "@/lib/file-actions"
import { fileActionTarget, getVisibleRows, isTreeHeading } from "@/lib/file-tree"
import type { FileTreeAlso, FileTreeCommit, FileTreeDirectory, FileTreeFile, FileTreeNode } from "@/lib/file-tree"
import { formatShortHash } from "@/lib/hash"
import type { MenuItemGroups } from "@/lib/menu"
import type { ReviewPlace } from "@/lib/review-place"
import { presentRowFacts } from "@/presenters/review-plan"
import type { DependencyBadge, FileBadges } from "@/presenters/review-plan"
import { formatCommitRole, formatModuleState, formatSeen } from "@/presenters/review-steps"

interface FileTreeBaseProps {
  nodes: readonly FileTreeNode[]
  "aria-label": string
  collapsed: ReadonlySet<string>
  /** Must keep its identity: the rows are memoized */
  onToggleDirectory: (key: string) => void
  /** The code owner of a file by its path, in the hover of its row (the header of its diff shows it) */
  fileOwners?: ReadonlyMap<string, string>
  /** The facts of the review plan on the rows, one mark each (`FileRowMark`); none without a plan */
  badges?: FileTreeBadges | null
  /** The counts of files from the chosen owner chip on each step */
  marks?: FileTreeMarks | null
  /** The step of the selected file or step; its rows form one highlighted group */
  activeGroupKey?: string | null
  /** What the commit rows of the story of a range review do (`FileTreeCommit`); a tree without them has none */
  commitRows?: FileTreeCommitRows | null
  /**
   * The files the reader saw of each large step and of each of its groups, by the key of its row (spec/branch/review-tour.spec.md,
   * "In the Review Window"); a row shows them from the first one
   */
  progress?: ReadonlyMap<string, FileTreeProgress> | null
}

/** How many files of a row the reader saw, of how many */
export interface FileTreeProgress {
  readonly seen: number
  readonly total: number
}

/** What a commit row does (spec/branch/review-tour.spec.md, "In the Review Window") */
export interface FileTreeCommitRows {
  /** A click, or the focus: the step of the commit at the top of the diffs; must keep its identity, the rows are memoized */
  readonly onSelect: (node: FileTreeCommit) => void
  /** Enter: the commit alone */
  readonly onOpen: (node: FileTreeCommit) => void
  /** Its right-click menu: Show Commit Alone, then the menu of the commit; must keep its identity */
  readonly menu: (node: FileTreeCommit) => MenuItemGroups
}

/** What an owner chip marks in a tree (spec/branch/branch-review.spec.md, "Code owners") */
export interface FileTreeMarks {
  /** The name of the chip, for the step's accessible description */
  readonly label: string
  /** How many of its files each step shows, by the key of the step's row (`presentOwnerMarks`) */
  readonly counts: ReadonlyMap<string, { shown: number; total: number }>
}

/** The badges of the rows of a tree, by the key of the row */
export interface FileTreeBadges {
  readonly files: ReadonlyMap<string, FileBadges>
  /** The dependency changes of a module, on its directory row or on the row of its build file */
  readonly dependencies: ReadonlyMap<string, DependencyBadge>
  /** Shows another file from the card of a row's mark; must keep its identity: the rows are memoized */
  readonly onSelectPlace: (place: ReviewPlace) => void
}

/** A file row expands its diff under it (the commit preview) */
export interface InlineFileTreeProps extends FileTreeBaseProps {
  mode: "inline"
  /** The files whose diff is shown under their row */
  expandedFiles: ReadonlySet<string>
  /** Must keep its identity: the rows are memoized */
  onToggleFile: (key: string) => void
  /** The diff of an expanded file, under its row */
  renderFileContent: (file: CommitFile) => ReactNode
  /** Enter on a file row; without it Enter toggles the diff, as Space does */
  onOpenFile?: (file: CommitFile) => void
}

/** A file row selects the file, and the selection follows the focus (the review, beside the diffs) */
export interface NavigationFileTreeProps extends FileTreeBaseProps {
  mode: "navigate"
  selectedKey: string | null
  /** The file of a row, and the key of the row: a file of a review shows also in other concerns; must keep its identity, the rows are memoized */
  onSelectFile: (file: CommitFile, key: string) => void
}

export type FileTreeProps = InlineFileTreeProps | NavigationFileTreeProps

/** The indent of a tree level; a guide line per level keeps the levels apart */
const INDENT_REM = 0.75
/** The space before the chevron of a row, and the middle of the chevron (`size-3.5`): where the guide of its level is */
const ROW_START = "0.25rem"
const CHEVRON_MIDDLE = "7px"

/**
 * The changed files grouped by directory: a directory row collapses its subtree, a file row expands its diff under it
 * or selects the file. One row is in the tab order (roving focus): ↑/↓, Home and End move between the rows, → opens
 * and ← closes or goes to the parent, Space and Enter toggle. The keys the tree handles are `preventDefault`ed, so the
 * dialog around it leaves them alone; the others (letters, keys with a modifier) reach it. A row's name comes first: the
 * path, the rename and the owner are in its hover, Copy Path in its right-click menu and on ⌘C. `I` opens the card of
 * the row's mark (`FileRowMark`, of a commit with a body `CommitRowMark`). The group of a step of the commits is the
 * commit that starts it (`FileTreeDirectory.commit`): a click selects it as a commit row, its chevron, Space, → and ←
 * fold it, Enter, ⌘C, `I` and its menu are the commit's.
 */
export function FileTree(props: FileTreeProps) {
  const {
    nodes,
    "aria-label": label,
    collapsed,
    onToggleDirectory,
    mode,
    fileOwners,
    badges = null,
    marks = null,
    activeGroupKey = null,
    commitRows = null,
    progress = null,
  } = props
  // a file of a navigation tree has no diff to open
  const expandedFiles = props.mode === "inline" ? props.expandedFiles : null
  const selectedKey = props.mode === "navigate" ? props.selectedKey : null
  const onToggleFile = props.mode === "inline" ? props.onToggleFile : undefined
  const onSelectFile = props.mode === "navigate" ? props.onSelectFile : undefined
  const onOpenFile = props.mode === "inline" ? props.onOpenFile : undefined
  const rows = getVisibleRows(nodes, collapsed)
  // the indent of each row: its depth less the headings above it, whose rows keep their indent
  const indents = headingIndents(rows)
  const [focusedKey, setFocusedKey] = useState<string | null>(null)
  // the row in the tab order: the focused one while it shows, then the selected one, otherwise the first
  const tabKey = [focusedKey, selectedKey].find(key => key !== null && rows.some(row => row.key === key)) ?? rows[0]?.key ?? null
  const rowElements = useRef(new Map<string, HTMLElement>())
  // the one open card of a row's mark, by the key of its row: by the hover of the mark or by `I` on the row
  const [card, setCard] = useState<OpenCard | null>(null)

  // memoized by the compiler: the rows are memoized; the hover of the mark of the card `I` opened keeps it the key's
  const changeCard = (key: string, open: boolean) => {
    setCard(current => (open ? (current?.key === key ? current : { key, byKey: false }) : current?.key === key ? null : current))
  }

  // a file selected elsewhere (the diffs scrolled, J and K) scrolls into view
  useEffect(() => {
    if (selectedKey !== null) {
      rowElements.current.get(selectedKey)?.scrollIntoView({ block: "nearest" })
    }
  }, [selectedKey])

  const focusRow = (node: FileTreeNode) => {
    setFocusedKey(node.key)
    rowElements.current.get(node.key)?.focus()
    // the selection follows the focus
    if (isCommitRow(node)) {
      commitRows?.onSelect(commitOf(node))
    } else if (node.kind !== "directory") {
      onSelectFile?.(node.file, node.key)
    }
  }

  // memoized by the compiler: the rows are memoized
  const toggle = (node: FileTreeNode) => {
    setFocusedKey(node.key)
    if (node.kind === "directory") {
      // a step of its commit alone folds nothing: it selects its commit
      if (isLeafGroup(node)) {
        commitRows?.onSelect(node.commit)
      } else {
        onToggleDirectory(node.key)
      }
    } else if (node.kind === "commit") {
      commitRows?.onSelect(node)
    } else if (onSelectFile !== undefined) {
      onSelectFile(node.file, node.key)
    } else if (node.kind === "file") {
      onToggleFile?.(node.key)
    }
  }

  // memoized by the compiler: the rows are memoized; a click on a row: the group of a step of the commits shows its step
  // as its commit does, opened, another row toggles
  const activate = (node: FileTreeNode, open: boolean | undefined) => {
    if (node.kind === "directory" && node.commit !== undefined) {
      // a folded step opens; a step of its commit alone folds nothing (`isRowOpen` undefined)
      setFocusedKey(node.key)
      if (open === false) {
        onToggleDirectory(node.key)
      }
      commitRows?.onSelect(node.commit)
    } else {
      toggle(node)
    }
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement
    const key = target.dataset.treeKey
    // ⌘C on a row copies its path, as Copy Path of its menu; on a commit its hash
    if (key !== undefined && isCopyKey(event)) {
      const node = rows.find(row => row.key === key)
      if (node !== undefined) {
        event.preventDefault()
        void copyText(isCommitRow(node) ? commitOf(node).commit.id : copiedText(fileActionTarget(node)))
      }
      return
    }
    // only the keys of a row, not of the diff under it; a key with a modifier is the dialog's
    if (key === undefined || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
      return
    }
    const index = rows.findIndex(row => row.key === key)
    const node = rows[index]
    if (node === undefined) {
      return
    }
    // the physical key, in any keyboard layout: the card of the row's mark, a row without a mark leaves it alone
    if (event.code === "KeyI") {
      if (markFacts(node, badges) !== null || cardCommitOf(node) !== null) {
        event.preventDefault()
        setCard({ key: node.key, byKey: true })
      }
      return
    }
    const isOpen = isRowOpen(node, collapsed, expandedFiles)
    let handled = true
    switch (event.key) {
      case "ArrowDown":
      case "ArrowUp": {
        // also at the first and the last row: the arrows never leave the tree
        const next = rows[index + (event.key === "ArrowDown" ? 1 : -1)]
        if (next !== undefined) {
          focusRow(next)
        }
        break
      }
      case "Home":
        focusRow(rows[0]!)
        break
      case "End":
        focusRow(rows.at(-1)!)
        break
      case "ArrowRight":
        if (isOpen === false) {
          toggle(node)
        } else if (node.kind === "directory" && node.children[0] !== undefined) {
          focusRow(node.children[0])
        }
        break
      case "ArrowLeft": {
        const parent = rows.find(row => row.key === node.parentKey)
        if (isOpen === true) {
          toggle(node)
        } else if (parent !== undefined) {
          focusRow(parent)
        }
        break
      }
      case "Enter":
        if (node.kind === "file" && onOpenFile !== undefined) {
          onOpenFile(node.file)
        } else if (isCommitRow(node)) {
          commitRows?.onOpen(commitOf(node))
        } else {
          toggle(node)
        }
        break
      case " ":
        toggle(node)
        break
      default:
        handled = false
    }
    if (handled) {
      event.preventDefault()
    }
  }

  const renderRow = (node: FileTreeNode, sticky: RowSticky, activeGroup: boolean) => (
    <FileTreeRow
      key={node.key}
      node={node}
      isOpen={isRowOpen(node, collapsed, expandedFiles)}
      isSelected={mode === "navigate" ? node.key === selectedKey : undefined}
      inTabOrder={node.key === tabKey}
      rowElements={rowElements}
      onToggle={toggle}
      onActivate={activate}
      onFocusRow={setFocusedKey}
      owner={node.kind === "directory" || node.kind === "commit" ? undefined : fileOwners?.get(node.file.path)}
      fileBadges={node.kind === "file" ? badges?.files.get(node.key) : undefined}
      dependencyBadge={badges?.dependencies.get(node.key)}
      onSelectPlace={badges?.onSelectPlace}
      card={card?.key === node.key ? (card.byKey ? "key" : "hover") : null}
      onCardChange={changeCard}
      ownerMark={marks === null ? null : ownerMarkOf(node, marks)}
      commitMenu={isCommitRow(node) ? commitRows?.menu : undefined}
      content={props.mode === "inline" && node.kind === "file" && props.expandedFiles.has(node.key) ? props.renderFileContent(node.file) : null}
      sticky={sticky}
      activeGroup={activeGroup}
      indentDepth={indents.get(node.key) ?? node.depth}
      progress={progress?.get(node.key) ?? null}
    />
  )
  // the rows of a step under its row: each group of a large step one block, whose heading sticks below the step's row
  const renderStepRows = (list: readonly FileTreeNode[], activeGroup: boolean): ReactNode[] => {
    const blocks: ReactNode[] = []
    for (let at = 0; at < list.length; at++) {
      const row = list[at]!
      if (row.kind === "directory" && row.group === "section") {
        const inside: FileTreeNode[] = []
        while (list[at + 1] !== undefined && list[at + 1]!.depth > row.depth) {
          inside.push(list[++at]!)
        }
        blocks.push(
          <div key={row.key} role="none">
            {renderRow(row, "section", activeGroup)}
            {inside.map(it => renderRow(it, false, activeGroup))}
          </div>,
        )
      } else {
        blocks.push(renderRow(row, false, activeGroup))
      }
    }
    return blocks
  }
  const renderedRows: ReactNode[] = []
  for (let index = 0; index < rows.length; index++) {
    const node = rows[index]!
    if (node.kind === "directory" && node.group === "concern") {
      const group: FileTreeNode[] = []
      const activeGroup = activeGroupKey === node.key
      while (rows[index + 1] !== undefined && rows[index + 1]!.depth > node.depth) {
        group.push(rows[++index]!)
      }
      renderedRows.push(
        <div key={node.key} role="none" className="relative" ref={group.some(isTreeHeading) ? stickBelowStepRow : undefined}>
          {renderRow(node, "step", activeGroup)}
          <div role="none" className="relative">
            {activeGroup && (
              // the line of the selected step at its chevron, from under its row to its last row: above the headings of
              // its groups (z-5), under its own row when that row sticks (z-10), so it never crosses its chevron
              <span
                aria-hidden
                className="pointer-events-none absolute inset-y-0 z-[7] w-0.5 rounded-full bg-primary/60"
                style={{ left: `calc(${ROW_START} + ${node.depth * INDENT_REM}rem + ${CHEVRON_MIDDLE})` }}
                data-testid="tree-step-line"
              />
            )}
            {renderStepRows(group, activeGroup)}
          </div>
        </div>,
      )
    } else {
      renderedRows.push(renderRow(node, false, false))
    }
  }

  return (
    <div role="tree" aria-label={label} className="flex flex-col" onKeyDown={handleKeyDown}>
      {renderedRows}
    </div>
  )
}

/** How a row sticks to the top of the tree: the row of a step, the heading of a group of a large step below it, or not */
type RowSticky = "step" | "section" | false

/** The indent of each row by its key: its depth less the headings above it (`isTreeHeading`), whose rows keep their indent */
function headingIndents(rows: readonly FileTreeNode[]): Map<string, number> {
  const above: { depth: number; heading: boolean }[] = []
  const indents = new Map<string, number>()
  for (const row of rows) {
    while (above.length > 0 && above.at(-1)!.depth >= row.depth) {
      above.pop()
    }
    indents.set(row.key, row.depth - above.filter(it => it.heading).length)
    if (row.kind === "directory") {
      above.push({ depth: row.depth, heading: isTreeHeading(row) })
    }
  }
  return indents
}

/**
 * Keeps the height of the row of a step on the block of its rows (`--tree-step-row`), so the heading of a group of a
 * large step sticks right below it however many lines its title wraps to
 */
function stickBelowStepRow(block: HTMLDivElement | null): (() => void) | undefined {
  const row = block?.querySelector<HTMLElement>(":scope > [role=treeitem]")
  if (block === null || block === undefined || row === null || row === undefined) {
    return undefined
  }
  const observer = new ResizeObserver(() => block.style.setProperty("--tree-step-row", `${row.offsetHeight}px`))
  observer.observe(row)
  return () => observer.disconnect()
}

/** The group of a step of the commits that lands nothing and has no other commit: its commit alone */
function isLeafGroup(node: FileTreeDirectory): node is StepCommitGroup {
  return node.commit !== undefined && node.children.length === 0
}

/** The group of a step of the commits: the commit that starts it */
type StepCommitGroup = FileTreeDirectory & { commit: FileTreeCommit }

/** A row that is a commit: a commit row, the group of a step of the commits */
function isCommitRow(node: FileTreeNode): node is FileTreeCommit | StepCommitGroup {
  return node.kind === "commit" || (node.kind === "directory" && node.commit !== undefined)
}

/** The commit a row is */
function commitOf(row: FileTreeCommit | StepCommitGroup): FileTreeCommit {
  return row.kind === "commit" ? row : row.commit
}

/** The open card of a row's mark: its row, and whether `I` opened it (the focus is in it) */
interface OpenCard {
  key: string
  byKey: boolean
}

/** The facts of a row's one mark, null for a row without a mark: the same the row shows (`FileTreeRow`) */
function markFacts(node: FileTreeNode, badges: FileTreeBadges | null) {
  return badges === null ? null : presentRowFacts(node.kind === "file" ? badges.files.get(node.key) : undefined, badges.dependencies.get(node.key))
}

/**
 * The card of a commit row and of the group of a step of the commits, null for another row and for a commit without a
 * body: the commit of the range it is
 */
function cardCommitOf(node: FileTreeNode): CardCommit | null {
  const commit = isCommitRow(node) ? commitOf(node).commit.item?.commit : undefined
  return commit?.hasBody === true ? toCardCommit(commit) : null
}

/** The count chip of a step row that shows files of the chosen owner: "4 of 7" */
function ownerMarkOf(node: FileTreeNode, marks: FileTreeMarks): OwnerMark | null {
  const count = node.kind === "directory" ? marks.counts.get(node.key) : undefined
  return count === undefined || count.shown === 0 ? null : { label: marks.label, count: count.shown, total: count.total }
}

interface OwnerMark {
  label: string
  count: number
  total: number
}

/**
 * A directory is open unless collapsed; a file is open while its diff shows, `undefined` where it has none to show, as
 * the group of a step of the commits that holds no row: its commit alone, it folds nothing
 */
function isRowOpen(node: FileTreeNode, collapsed: ReadonlySet<string>, expandedFiles: ReadonlySet<string> | null): boolean | undefined {
  if (node.kind === "directory") {
    return isLeafGroup(node) ? undefined : !collapsed.has(node.key)
  }
  if (node.kind === "also" || node.kind === "commit") {
    return undefined
  }
  return expandedFiles === null ? undefined : expandedFiles.has(node.key)
}

interface FileTreeRowProps {
  node: FileTreeNode
  isOpen: boolean | undefined
  /** In a navigation tree, whether the row is the selected file; `undefined` in an inline tree */
  isSelected: boolean | undefined
  inTabOrder: boolean
  rowElements: RefObject<Map<string, HTMLElement>>
  /** Its chevron; must keep its identity */
  onToggle: (node: FileTreeNode) => void
  /** A click on it; must keep its identity */
  onActivate: (node: FileTreeNode, open: boolean | undefined) => void
  onFocusRow: (key: string) => void
  /** The code owner of a file */
  owner: string | undefined
  /** The facts of the review plan, the row's one mark */
  fileBadges: FileBadges | undefined
  dependencyBadge: DependencyBadge | undefined
  onSelectPlace: ((place: ReviewPlace) => void) | undefined
  /** The card of the mark when it is open, and by what: the hover of the mark or `I` on the row */
  card: "hover" | "key" | null
  /** Opens or closes the card of a row's mark; must keep its identity */
  onCardChange: (key: string, open: boolean) => void
  /** The files of the chosen owner a step row shows: "4 of 7" */
  ownerMark: OwnerMark | null
  sticky: RowSticky
  activeGroup: boolean
  /** Its indent, in levels: its depth less the headings above it */
  indentDepth: number
  /** The files of a step or a group the reader saw */
  progress: FileTreeProgress | null
  /** The right-click menu of a commit row and of the group of a step of the commits */
  commitMenu: FileTreeCommitRows["menu"] | undefined
  /** The diff under an expanded file */
  content: ReactNode
}

const FileTreeRow = memo(function FileTreeRow({
  node,
  isOpen,
  isSelected,
  inTabOrder,
  rowElements,
  onToggle,
  onActivate,
  onFocusRow,
  owner,
  fileBadges,
  dependencyBadge,
  onSelectPlace,
  card,
  onCardChange,
  ownerMark,
  commitMenu,
  content,
  sticky,
  activeGroup,
  indentDepth,
  progress,
}: FileTreeRowProps) {
  const indent = `${indentDepth * INDENT_REM}rem`
  const isConcern = node.kind === "directory" && node.group === "concern"
  const section = node.kind === "directory" ? node.section : undefined
  // the files of a step or a group the reader saw, from the first one
  const seen = progress !== null && progress.seen > 0 ? formatSeen(progress.seen, progress.total) : null
  const facts = presentRowFacts(fileBadges, dependencyBadge)
  const cardCommit = cardCommitOf(node)
  // the one card of the row's mark, of its facts or of its commit
  const cardState: RowCardState = {
    open: card !== null,
    openedByKey: card === "key",
    onOpenChange: open => onCardChange(node.key, open),
    focusRow: () => rowElements.current.get(node.key)?.focus(),
  }
  // the hover of the title of a commit opens its card too
  const hover = useRowCardHover(cardState)
  // the "⋯" of a commit in its card: the menu of its row
  const cardMenu =
    cardCommit !== null && commitMenu !== undefined && isCommitRow(node) ? (
      <MoreActionsMenu name={formatShortHash(cardCommit.id)} menu={() => commitMenu(commitOf(node))} />
    ) : undefined
  const commitCard = cardCommit === null ? null : { commit: cardCommit, state: cardState, menu: cardMenu }
  const counts = rowCounts(node)
  // the mark is no tab stop: the description says every fact of its card; a group its module, what it also holds
  const described = [
    section?.state == null ? null : formatModuleState(section.state),
    section?.also ?? null,
    counts,
    facts === null || facts.description === "" ? null : facts.description,
    seen,
  ]
    .filter(part => part !== null && part !== "")
    .join(", ")
  const description = ownerMark === null ? described : `${described}, ${ownerMark.count} of ${formatFileCount(ownerMark.total)}, ${ownerMark.label}`
  // a file of a review also in another concern: the row tells it from the row of the file
  const label = node.kind === "also" ? `${node.name}, ${node.also}` : node.name
  const disclosure =
    isOpen === undefined ? (
      // a file of a navigation tree opens nothing: the space keeps the names aligned
      <span className="my-[3px] size-3.5 shrink-0" />
    ) : (
      // the chevron folds the row, also where a click on a row selects it (the group of a step of the commits)
      <ChevronRightIcon
        className={cn("my-[3px] size-3.5 shrink-0 text-muted-foreground transition-transform duration-150", isOpen && "rotate-90")}
        data-testid="tree-row-chevron"
        onClick={event => {
          event.stopPropagation()
          onToggle(node)
        }}
      />
    )
  const rowMark =
    facts !== null && onSelectPlace !== undefined ? (
      <FileRowMark facts={facts} onSelectPlace={onSelectPlace} selected={isSelected === true} {...cardState} />
    ) : (
      commitCard !== null && node.kind === "directory" && <CommitRowMark commit={commitCard.commit} size="tree" menu={commitCard.menu} popupHover={hover.popup} {...cardState} />
    )
  const lineCounts =
    node.kind === "directory" ? node.group !== "commits" && node.fileCount > 0 && <DirectoryLineCounts node={node} /> : node.kind === "file" && <FileLineCounts node={node} />
  // the counts of the heading of a group: the files of the chosen owner, the mark of its dependency changes, the files
  // seen, its lines
  const groupFacts = section !== undefined && (
    <>
      {ownerMark !== null && <OwnerMarkBadge mark={ownerMark} />}
      {rowMark}
      {seen !== null && <SeenProgress text={seen} />}
      {lineCounts}
    </>
  )
  const rowContent =
    node.kind === "directory" && isTreeHeading(node) ? (
      <SectionRowContent node={node} />
    ) : node.kind === "directory" ? (
      <DirectoryRowContent node={node} isOpen={isOpen === true} titleHover={commitCard === null ? undefined : hover.text} />
    ) : node.kind === "also" ? (
      <AlsoRowContent node={node} owner={owner} />
    ) : node.kind === "commit" ? (
      <CommitRowContent node={node} card={commitCard} />
    ) : (
      <FileRowContent node={node} owner={owner} />
    )
  return (
    <div
      ref={element => {
        if (element === null) {
          rowElements.current.delete(node.key)
        } else {
          rowElements.current.set(node.key, element)
        }
      }}
      role="treeitem"
      aria-level={node.depth + 1}
      aria-expanded={isOpen}
      // a step of a review is selected while its opening is at the top of the diffs; a directory never
      aria-selected={node.kind === "directory" && node.group !== "concern" ? undefined : isSelected}
      aria-label={label}
      aria-description={description}
      tabIndex={inTabOrder ? 0 : -1}
      data-tree-key={node.key}
      data-active-group={activeGroup ? "" : undefined}
      className={cn("group/tree-item flex flex-col outline-none", sticky === "step" && "sticky top-0 z-10 bg-canvas", sticky === "section" && "sticky z-[5] bg-canvas")}
      // the heading of a group sticks right below the row of its step
      style={sticky === "section" ? { top: "var(--tree-step-row, 0px)" } : undefined}
      onFocus={event => {
        if (event.target === event.currentTarget) {
          onFocusRow(node.key)
        }
      }}
    >
      <RowMenu
        node={node}
        commitMenu={commitMenu}
        // the row takes the focus, so ⌘C copies the same path
        onOpen={() => rowElements.current.get(node.key)?.focus()}
        render={
          <div
            className={cn(
              // a title of a step or a commit wraps as needed: the row grows, its chevron, icon and end stay on the first
              "group/row relative flex min-h-6 cursor-default items-start gap-1.5 rounded-md py-0.5 pr-1 text-sm transition-colors select-none bd-focus-inset group-focus-visible/tree-item:bd-focus hover:bg-muted",
              (isConcern || section !== undefined) && "flex-col items-stretch gap-0",
              activeGroup && "bg-primary/5",
              isSelected && "bg-primary/15 hover:bg-primary/15",
            )}
            style={{ paddingLeft: `calc(${indent} + ${ROW_START})` }}
            onClick={() => onActivate(node, isOpen)}
          />
        }
      >
        {node.depth > 0 && (
          // a line per level above the row, under the chevron of each parent
          <span
            aria-hidden
            className="pointer-events-none absolute inset-y-0"
            style={{
              left: `calc(${ROW_START} + ${CHEVRON_MIDDLE})`,
              width: indent,
              backgroundImage: "linear-gradient(to right, var(--color-border) 1px, transparent 1px)",
              backgroundSize: `${INDENT_REM}rem 100%`,
            }}
          />
        )}
        {isConcern ? (
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="flex min-w-0 items-start gap-1.5">
              {disclosure}
              {rowContent}
            </span>
            <span className="flex min-w-0 items-center gap-1.5 pl-5">
              {ownerMark !== null && <OwnerMarkBadge mark={ownerMark} />}
              {node.commit !== undefined && <CommitRowBadges facts={factsOf(node.commit)} />}
              {rowMark}
              {lineCounts}
              {seen !== null && <SeenProgress text={seen} />}
            </span>
          </span>
        ) : section !== undefined ? (
          // the heading of a group; of a module the commits add or delete, or one that holds small modules, its name and
          // its badge alone on the first line, the modules it also holds muted below, then its counts
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="flex min-w-0 items-start gap-1.5">
              {disclosure}
              {rowContent}
              {section.state === null && section.also === null && groupFacts}
            </span>
            {(section.state !== null || section.also !== null) && (
              <span className="flex min-w-0 items-center gap-1.5 pl-[2.625rem]">
                <span className="min-w-0 flex-1 truncate text-xs leading-5 text-muted-foreground" data-testid="review-group-also">
                  {section.also}
                </span>
                {groupFacts}
              </span>
            )}
          </span>
        ) : (
          <>
            {disclosure}
            {rowContent}
            {rowMark}
            {lineCounts}
          </>
        )}
      </RowMenu>
      {content !== null && (
        <div className="my-1 overflow-hidden rounded-lg border border-border" style={{ marginLeft: indent }}>
          {content}
        </div>
      )}
    </div>
  )
})

/**
 * What a row says after its name to a screen reader: the status and the lines of a file, the files of a group, the role
 * of a commit; the group of a step of the commits its files, then its commit
 */
function rowCounts(node: FileTreeNode): string {
  if (node.kind === "commit") {
    return commitFacts(node)
  }
  if (node.kind === "directory") {
    if (node.group === "commits") {
      return formatCommitCount(node.commitCount ?? 0)
    }
    // a group that lands nothing: its commits say what it is
    const parts = [
      node.fileCount > 0 ? `${formatFileCount(node.fileCount)}, +${node.additions} −${node.deletions}` : null,
      node.commit === undefined ? null : commitFacts(node.commit),
    ]
    return parts.filter(part => part !== null).join(", ")
  }
  return `${formatFileStatus(node.file.status)}, +${node.file.additions} −${node.file.deletions}`
}

/** The role of a commit, what it is since the user's review, its branch, its hash */
function commitFacts(node: FileTreeCommit): string {
  const { role, item } = node.commit
  return [formatCommitRole(role), item?.since, item?.review, item?.branch, `commit ${formatShortHash(node.commit.id)}`].filter(part => part != null).join(", ")
}

/**
 * The right-click menu of a row: the actions of its file or directory, of a commit and of the group of a step of the
 * commits the menu of the commit (`FileTreeCommitRows.menu`)
 */
function RowMenu({
  node,
  commitMenu,
  onOpen,
  render,
  children,
}: {
  node: FileTreeNode
  commitMenu: FileTreeCommitRows["menu"] | undefined
  onOpen: () => void
  render: ReactElement
  children: ReactNode
}) {
  if (isCommitRow(node)) {
    const commit = commitOf(node)
    return (
      <LazyContextMenu menu={() => commitMenu?.(commit) ?? []} onOpen={onOpen} render={render}>
        {children}
      </LazyContextMenu>
    )
  }
  return (
    <FileContextMenu target={() => fileActionTarget(node)} copyKey onOpen={onOpen} render={render}>
      {children}
    </FileContextMenu>
  )
}

/**
 * A commit in the group of its step: the one commit row of a sidebar (`CommitRow`), by the title the review gives it,
 * which gets the width (its role, "follows up" or "no story", is its indent under the step, and the description of the
 * row says it), what it is since the user's review, the branch of a commit of the whole stack, the mark of its card;
 * its subject and hash in the hover of its title
 */
function CommitRowContent({ node, card }: { node: FileTreeCommit; card: ComponentProps<typeof CommitRow>["card"] }) {
  return (
    <>
      <GitCommitHorizontalIcon className="my-0.5 size-4 shrink-0 text-muted-foreground" />
      <span className="flex min-w-0 flex-1 flex-col">
        <CommitRow subject={node.commit.subject} title={node.name} hash={node.commit.id} facts={factsOf(node)} card={card} wholeSubject subjectTestId="review-commit-subject" />
      </span>
    </>
  )
}

/** What a commit of the tree is since the user's review, its review, its branch */
function factsOf({ commit: { item } }: FileTreeCommit): CommitRowFacts {
  return { since: item?.since, review: item?.review, branch: item?.branch }
}

/** The hover of a group: the hover of a step (its evidence), the subject and the hash of the commit a step is, a directory's path */
function groupTitle(node: FileTreeDirectory): string {
  const commit = node.commit?.commit
  if (commit !== undefined) {
    return `${commit.subject}\n${formatShortHash(commit.id)}`
  }
  return node.title ?? (node.group === undefined ? node.key.slice(0, -1) : node.name)
}

function DirectoryRowContent({
  node,
  isOpen,
  titleHover,
}: {
  node: FileTreeDirectory
  isOpen: boolean
  /** The hover of the title of the step of a commit: it opens the card of the commit (`useRowCardHover`) */
  titleHover: RowCardHover["text"] | undefined
}) {
  // a group of a review is no directory: its name is what it holds; a concern is a task of the range, the step of a
  // commit that commit
  const Icon =
    node.commit !== undefined ? GitCommitHorizontalIcon : node.group === "concern" ? TargetIcon : node.group !== undefined ? LayersIcon : isOpen ? FolderOpenIcon : FolderIcon
  const path = node.group === undefined && node.commit === undefined && node.lead === undefined
  const name = node.name.slice(node.lead?.length ?? 0)
  return (
    <>
      <Icon className="my-0.5 size-4 shrink-0 text-muted-foreground" />
      {/* the number of a step hangs before its title, which wraps under its own start with the whole width left */}
      {node.lead !== undefined && <span className="shrink-0 font-medium tabular-nums">{node.lead.trimEnd()}</span>}
      <span
        className={cn(
          "min-w-0 flex-1",
          // the title of a step or a task wraps as many lines as needed; a path is cut
          node.group === "concern" ? "font-medium" : "truncate",
          // a code name at the size of the row: JetBrains Mono has the x-height of Inter (docs/design/ui-theme.md)
          node.code === true && "font-mono",
        )}
        dir={path ? "rtl" : undefined}
        data-testid={path ? "review-directory-path" : undefined}
        // the card of a commit says its subject and hash: no native hover over it
        title={titleHover === undefined ? groupTitle(node) : undefined}
        data-group={node.group}
        {...titleHover}
      >
        {/* a code name wraps at its dots, never inside a word but one wider than the row */}
        {node.code === true ? (
          <BreakableCode text={name} humps />
        ) : path ? (
          <bdi dir="ltr">{name}</bdi>
        ) : node.commit !== undefined ? (
          <CommitSubjectText subject={name} prefixClassName="font-mono font-normal text-muted-foreground" />
        ) : (
          name
        )}
      </span>
    </>
  )
}

/** The files of the chosen owner a step or a group shows: "4 of 7" */
function OwnerMarkBadge({ mark }: { mark: OwnerMark }) {
  return (
    <Badge variant="secondary" data-testid="review-owner-mark" className="shrink-0 text-muted-foreground">
      <span className="tabular-nums">
        {mark.count} of {mark.total}
      </span>
    </Badge>
  )
}

/** "3 / 11 seen": the files of a step or a group the reader saw */
function SeenProgress({ text }: { text: string }) {
  return (
    <span className="shrink-0 text-xs leading-5 text-muted-foreground tabular-nums" data-testid="review-seen-progress">
      {text}
    </span>
  )
}

/** The icon of a group of a large step: what it holds */
function SectionIcon({ node }: { node: FileTreeDirectory }) {
  const className = "my-0.5 size-4 shrink-0 text-muted-foreground"
  switch (node.section?.kind) {
    case "contract":
      return <ScrollTextIcon className={className} />
    case "folder":
      return <FolderIcon className={className} />
    case "mechanical":
      return node.section.name === "Wiring" ? <CableIcon className={className} /> : <LayersIcon className={className} />
    default:
      return <PackageIcon className={className} />
  }
}

/**
 * A group of a large step, the heading of its files: its name (a module by its label as code), its module new or
 * deleted, the small modules it also holds muted; the tests at the end of a group a muted heading
 */
function SectionRowContent({ node }: { node: FileTreeDirectory }) {
  const section = node.section
  if (section === undefined) {
    return (
      <>
        <FlaskConicalIcon className="my-0.5 size-4 shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 truncate text-xs leading-5 font-medium text-muted-foreground" title={groupTitle(node)} data-group={node.group}>
          {node.name}
        </span>
      </>
    )
  }
  return (
    <>
      <SectionIcon node={node} />
      <span className="flex min-w-0 flex-1 items-center gap-1.5">
        <span className={cn("min-w-0 truncate font-medium", section.code && "font-mono")} title={groupTitle(node)} data-group={node.group} data-testid="review-group-name">
          {node.name}
        </span>
        {section.state !== null && (
          <Badge variant={section.state === "added" ? "success" : "secondary"} data-testid="review-group-state">
            {formatModuleState(section.state)}
          </Badge>
        )}
      </span>
    </>
  )
}

function DirectoryLineCounts({ node }: { node: FileTreeDirectory }) {
  return (
    <span className="shrink-0 text-xs leading-5 text-muted-foreground" data-testid="directory-line-counts">
      <LineCounts additions={node.additions} deletions={node.deletions} colored={false} />
    </span>
  )
}

function FileRowContent({ node, owner }: { node: FileTreeFile; owner: string | undefined }) {
  const { file } = node
  const path = formatCommitFilePath(file)
  return (
    <>
      {/* the letter where a directory has its folder icon, so the names align; the name keeps its width, the old name of a rename gives way first */}
      <span className="flex min-w-0 flex-1" title={owner === undefined ? path : `${path}\nOwned by ${owner}`}>
        {/* a file of a group is in no directory row: its directory follows its name */}
        <FilePathLabel path={file.path} oldPath={file.oldPath} status={file.status} directory={node.directory ?? node.inGroup === true} letterSlot="icon" wholeDirectory />
      </span>
    </>
  )
}

/** A file in a chapter that is not the one of its main row: its name played down, the chapter of its own row after it ("in step 2") */
function AlsoRowContent({ node, owner }: { node: FileTreeAlso; owner: string | undefined }) {
  const { file } = node
  const path = formatCommitFilePath(file)
  return (
    <span className="flex min-w-0 flex-1 items-center gap-1.5 text-muted-foreground" title={`${owner === undefined ? path : `${path}\nOwned by ${owner}`}\n${node.home}`}>
      <span className="flex min-w-0">
        <FilePathLabel path={file.path} oldPath={file.oldPath} status={file.status} directory={node.directory ?? node.inGroup} letterSlot="icon" wholeDirectory />
      </span>
      {/* "in step 2"; a chapter of no number by its name, which gives way at half the row, its whole name in the hover */}
      <span className="max-w-1/2 shrink-0 truncate text-xs" data-testid="review-also-home">
        {node.homeTag}
      </span>
    </span>
  )
}

function FileLineCounts({ node: { file } }: { node: FileTreeFile }) {
  return (
    (file.additions > 0 || file.deletions > 0) && (
      <span className="shrink-0 text-xs leading-5">
        <LineCounts data-testid="file-line-counts" additions={file.additions} deletions={file.deletions} colored />
      </span>
    )
  )
}
