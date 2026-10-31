import { memo, useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactElement, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { ChevronRightIcon, FolderIcon, FolderOpenIcon, GitCommitHorizontalIcon, LayersIcon, TargetIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { FileRowMark } from "@/components/files/FileRowMark"
import { LazyContextMenu } from "@/components/shared/LazyContextMenu"
import type { CommitFile } from "@/lib/bindings"
import { copyText, isCopyKey } from "@/lib/clipboard"
import { formatCommitCount } from "@/lib/commit"
import { formatCommitFilePath, formatFileCount, formatFileStatus } from "@/lib/commit-files"
import { copiedText } from "@/lib/file-actions"
import { fileActionTarget, getVisibleRows } from "@/lib/file-tree"
import type { FileTreeAlso, FileTreeCommit, FileTreeDirectory, FileTreeFile, FileTreeNode } from "@/lib/file-tree"
import { formatShortHash } from "@/lib/hash"
import type { MenuItemGroups } from "@/lib/menu"
import type { ReviewPlace } from "@/lib/review-place"
import { presentRowFacts } from "@/presenters/review-plan"
import type { DependencyBadge, FileBadges } from "@/presenters/review-plan"
import { formatCommitRole } from "@/presenters/review-steps"

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
  /** The files of the chosen owner chip: the other rows are muted; none muted without */
  marks?: FileTreeMarks | null
  /** What the commit rows of the story of a range review do (`FileTreeCommit`); a tree without them has none */
  commitRows?: FileTreeCommitRows | null
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
  /** The name of the chip, on the rows of the steps: "Design 2" */
  readonly label: string
  /** The paths of its files */
  readonly paths: ReadonlySet<string>
  /** How many of its files each step shows, by the key of the step's row (`presentOwnerMarks`); 0 mutes the step */
  readonly counts: ReadonlyMap<string, number>
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
 * the row's mark (`FileRowMark`).
 */
export function FileTree(props: FileTreeProps) {
  const { nodes, "aria-label": label, collapsed, onToggleDirectory, mode, fileOwners, badges = null, marks = null, commitRows = null } = props
  // a file of a navigation tree has no diff to open
  const expandedFiles = props.mode === "inline" ? props.expandedFiles : null
  const selectedKey = props.mode === "navigate" ? props.selectedKey : null
  const onToggleFile = props.mode === "inline" ? props.onToggleFile : undefined
  const onSelectFile = props.mode === "navigate" ? props.onSelectFile : undefined
  const onOpenFile = props.mode === "inline" ? props.onOpenFile : undefined
  const rows = getVisibleRows(nodes, collapsed)
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
    if (node.kind === "commit") {
      commitRows?.onSelect(node)
    } else if (node.kind !== "directory") {
      onSelectFile?.(node.file, node.key)
    }
  }

  // memoized by the compiler: the rows are memoized
  const toggle = (node: FileTreeNode) => {
    setFocusedKey(node.key)
    if (node.kind === "directory") {
      onToggleDirectory(node.key)
    } else if (node.kind === "commit") {
      commitRows?.onSelect(node)
    } else if (onSelectFile !== undefined) {
      onSelectFile(node.file, node.key)
    } else if (node.kind === "file") {
      onToggleFile?.(node.key)
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
        void copyText(node.kind === "commit" ? node.commit.id : copiedText(fileActionTarget(node)))
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
      if (markFacts(node, badges) !== null) {
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
        } else if (node.kind === "commit") {
          commitRows?.onOpen(node)
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

  return (
    <div role="tree" aria-label={label} className="flex flex-col" onKeyDown={handleKeyDown}>
      {rows.map(node => (
        <FileTreeRow
          key={node.key}
          node={node}
          isOpen={isRowOpen(node, collapsed, expandedFiles)}
          isSelected={mode === "navigate" ? node.key === selectedKey : undefined}
          inTabOrder={node.key === tabKey}
          rowElements={rowElements}
          onToggle={toggle}
          onFocusRow={setFocusedKey}
          owner={node.kind === "directory" || node.kind === "commit" ? undefined : fileOwners?.get(node.file.path)}
          fileBadges={node.kind === "file" ? badges?.files.get(node.key) : undefined}
          dependencyBadge={badges?.dependencies.get(node.key)}
          onSelectPlace={badges?.onSelectPlace}
          card={card?.key === node.key ? (card.byKey ? "key" : "hover") : null}
          onCardChange={changeCard}
          muted={marks !== null && isMuted(node, marks)}
          ownerMark={marks === null ? null : ownerMarkOf(node, marks)}
          commitMenu={node.kind === "commit" ? commitRows?.menu : undefined}
          content={props.mode === "inline" && node.kind === "file" && props.expandedFiles.has(node.key) ? props.renderFileContent(node.file) : null}
        />
      ))}
    </div>
  )
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
 * A row the chosen owner chip plays down: one without its files; a commit of a step as its step, a commit of no step
 * never (it holds no file)
 */
function isMuted(node: FileTreeNode, marks: FileTreeMarks): boolean {
  if (node.kind === "commit") {
    return node.chapter !== null && marks.counts.get(node.parentKey ?? "") === 0
  }
  return node.kind === "directory" && node.group === "commits" ? false : markedCount(node, marks) === 0
}

/** The files of the chosen owner in a row: of a step its count, of a directory or a group its files, of a file 1 or 0, of a commit none */
function markedCount(node: FileTreeNode, marks: FileTreeMarks): number {
  if (node.kind === "commit") {
    return 0
  }
  if (node.kind !== "directory") {
    return marks.paths.has(node.file.path) ? 1 : 0
  }
  return marks.counts.get(node.key) ?? node.children.reduce((sum, child) => sum + markedCount(child, marks), 0)
}

/** The count chip of a step row that shows files of the chosen owner: "Design" and 2 */
function ownerMarkOf(node: FileTreeNode, marks: FileTreeMarks): OwnerMark | null {
  const count = node.kind === "directory" ? (marks.counts.get(node.key) ?? 0) : 0
  return count === 0 ? null : { label: marks.label, count }
}

interface OwnerMark {
  label: string
  count: number
}

/** A directory is open unless collapsed; a file is open while its diff shows, `undefined` where it has none to show */
function isRowOpen(node: FileTreeNode, collapsed: ReadonlySet<string>, expandedFiles: ReadonlySet<string> | null): boolean | undefined {
  if (node.kind === "directory") {
    return !collapsed.has(node.key)
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
  onToggle: (node: FileTreeNode) => void
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
  /** Holds no file of the chosen owner chip: played down, it stays in the story */
  muted: boolean
  /** The files of the chosen owner a step row shows: "Design 2" */
  ownerMark: OwnerMark | null
  /** The right-click menu of a commit row */
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
  onFocusRow,
  owner,
  fileBadges,
  dependencyBadge,
  onSelectPlace,
  card,
  onCardChange,
  muted,
  ownerMark,
  commitMenu,
  content,
}: FileTreeRowProps) {
  const indent = `${node.depth * INDENT_REM}rem`
  const facts = presentRowFacts(fileBadges, dependencyBadge)
  const counts = rowCounts(node)
  // the mark is no tab stop: the description says every fact of its card
  const described = facts === null || facts.description === "" ? counts : `${counts}, ${facts.description}`
  const description = ownerMark === null ? described : `${described}, ${formatFileCount(ownerMark.count)} of ${ownerMark.label}`
  // a file of a review also in another concern: the row tells it from the row of the file
  const label = node.kind === "also" ? `${node.name}, ${node.also}` : node.name
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
      aria-selected={node.kind === "directory" ? undefined : isSelected}
      aria-label={label}
      aria-description={description}
      tabIndex={inTabOrder ? 0 : -1}
      data-tree-key={node.key}
      data-muted={muted ? "" : undefined}
      className="group/tree-item flex flex-col outline-none"
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
              "group/row relative flex h-6 cursor-default items-center gap-1.5 rounded-md pr-1 text-sm transition-colors select-none group-focus-visible/tree-item:ring-2 group-focus-visible/tree-item:ring-ring/50 group-focus-visible/tree-item:ring-inset hover:bg-muted",
              isSelected && "bg-primary/15 hover:bg-primary/15",
              // its parts fade, not the row: the selection and the hover keep their colour
              muted && "*:opacity-60",
            )}
            style={{ paddingLeft: `calc(${indent} + ${ROW_START})` }}
            onClick={() => onToggle(node)}
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
        {isOpen === undefined ? (
          // a file of a navigation tree opens nothing: the space keeps the names aligned
          <span className="size-3.5 shrink-0" />
        ) : (
          <ChevronRightIcon className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform duration-150", isOpen && "rotate-90")} />
        )}
        {node.kind === "directory" ? (
          <DirectoryRowContent node={node} isOpen={isOpen === true} ownerMark={ownerMark} />
        ) : node.kind === "also" ? (
          <AlsoRowContent node={node} owner={owner} />
        ) : node.kind === "commit" ? (
          <CommitRowContent node={node} />
        ) : (
          <FileRowContent node={node} owner={owner} />
        )}
        {facts !== null && onSelectPlace !== undefined && (
          <FileRowMark
            facts={facts}
            onSelectPlace={onSelectPlace}
            open={card !== null}
            openedByKey={card === "key"}
            onOpenChange={open => onCardChange(node.key, open)}
            focusRow={() => rowElements.current.get(node.key)?.focus()}
          />
        )}
        {node.kind === "directory" ? node.group !== "commits" && <DirectoryLineCounts node={node} /> : node.kind === "file" && <FileLineCounts node={node} />}
      </RowMenu>
      {content !== null && (
        <div className="my-1 overflow-hidden rounded-lg border border-border" style={{ marginLeft: indent }}>
          {content}
        </div>
      )}
    </div>
  )
})

/** What a row says after its name to a screen reader: the status and the lines of a file, the files of a group, the role of a commit */
function rowCounts(node: FileTreeNode): string {
  if (node.kind === "commit") {
    const { role, item } = node.commit
    return [formatCommitRole(role), item?.since, item?.review, item?.branch, `commit ${formatShortHash(node.commit.id)}`].filter(part => part != null).join(", ")
  }
  if (node.kind === "directory") {
    return node.group === "commits" ? formatCommitCount(node.commitCount ?? 0) : `${formatFileCount(node.fileCount)}, +${node.additions} −${node.deletions}`
  }
  return `${formatFileStatus(node.file.status)}, +${node.file.additions} −${node.file.deletions}`
}

/** The right-click menu of a row: the actions of its file or directory, of a commit its menu (`FileTreeCommitRows.menu`) */
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
  if (node.kind === "commit") {
    return (
      <LazyContextMenu menu={() => commitMenu?.(node) ?? []} onOpen={onOpen} render={render}>
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
 * A commit in the group of its step: its subject, its role ("follows up", "no story"), what it is since the user's
 * review, the branch of a commit of the whole stack; the hash on hover
 */
function CommitRowContent({ node }: { node: FileTreeCommit }) {
  const { commit } = node
  const role = formatCommitRole(commit.role)
  return (
    <>
      <GitCommitHorizontalIcon className="size-4 shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate" title={commit.subject} data-testid="review-commit-subject">
        {commit.subject}
      </span>
      {role !== null && (
        <span className="shrink-0 text-xs text-muted-foreground" data-testid="review-commit-role">
          {role}
        </span>
      )}
      {commit.item?.since !== undefined && (
        <Badge variant="info" data-testid="review-commit-since">
          {commit.item.since}
        </Badge>
      )}
      {commit.item?.review !== undefined && (
        <Badge variant="outline" data-testid="review-commit-review">
          {commit.item.review}
        </Badge>
      )}
      {commit.item?.branch !== undefined && (
        <span className="max-w-24 shrink-0 truncate text-xs text-muted-foreground" data-testid="review-commit-branch">
          {commit.item.branch}
        </span>
      )}
      {/* the subject gets the width: the hash shows on hover, and the heading of the commit alone has it */}
      <span className="hidden font-mono text-xs text-muted-foreground group-hover/row:inline" data-testid="review-commit-hash">
        {formatShortHash(commit.id)}
      </span>
    </>
  )
}

function DirectoryRowContent({ node, isOpen, ownerMark }: { node: FileTreeDirectory; isOpen: boolean; ownerMark: OwnerMark | null }) {
  // a group of a review is no directory: its name is what it holds; a concern is a task of the range
  const Icon = node.group === "concern" ? TargetIcon : node.group !== undefined ? LayersIcon : isOpen ? FolderOpenIcon : FolderIcon
  return (
    <>
      <Icon className="size-4 shrink-0 text-muted-foreground" />
      <span
        className={cn("min-w-0 flex-1 truncate", node.group === "concern" && "font-medium")}
        title={node.title ?? (node.group === undefined ? node.key.slice(0, -1) : node.name)}
        data-group={node.group}
      >
        {node.name}
      </span>
      {ownerMark !== null && (
        <Badge variant="secondary" data-testid="review-owner-mark" className="max-w-[45%] shrink text-muted-foreground">
          <span className="min-w-0 truncate">{ownerMark.label}</span>
          <span className="tabular-nums">{ownerMark.count}</span>
        </Badge>
      )}
    </>
  )
}

function DirectoryLineCounts({ node }: { node: FileTreeDirectory }) {
  return (
    <span className="shrink-0 text-xs text-muted-foreground" data-testid="directory-line-counts">
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
        <FilePathLabel path={file.path} oldPath={file.oldPath} status={file.status} directory={node.inGroup === true} letterSlot="icon" />
      </span>
    </>
  )
}

/** A file in a concern that is not the one of its main row: its name played down, "also" after it */
function AlsoRowContent({ node, owner }: { node: FileTreeAlso; owner: string | undefined }) {
  const { file } = node
  const path = formatCommitFilePath(file)
  return (
    <span className="flex min-w-0 flex-1 items-center gap-1.5 text-muted-foreground" title={`${owner === undefined ? path : `${path}\nOwned by ${owner}`}\n${node.home}`}>
      <span className="flex min-w-0">
        <FilePathLabel path={file.path} oldPath={file.oldPath} status={file.status} directory={node.inGroup} letterSlot="icon" />
      </span>
      <span className="shrink-0 text-xs">also</span>
    </span>
  )
}

function FileLineCounts({ node: { file } }: { node: FileTreeFile }) {
  return (
    (file.additions > 0 || file.deletions > 0) && (
      <span className="shrink-0 text-xs">
        <LineCounts data-testid="file-line-counts" additions={file.additions} deletions={file.deletions} colored />
      </span>
    )
  )
}
