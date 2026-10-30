import { memo, useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { ChevronRightIcon, FolderIcon, FolderOpenIcon, LayersIcon } from "lucide-react"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { DependencyBadgeView, FileRowBadges } from "@/components/files/FileRowBadges"
import type { CommitFile } from "@/lib/bindings"
import { copyText, isCopyKey } from "@/lib/clipboard"
import { formatCommitFilePath, formatFileCount, formatFileStatus } from "@/lib/commit-files"
import { copiedText } from "@/lib/file-actions"
import { fileActionTarget, getVisibleRows } from "@/lib/file-tree"
import type { FileTreeDirectory, FileTreeFile, FileTreeNode } from "@/lib/file-tree"
import type { ReviewPlace } from "@/lib/review-place"
import type { DependencyBadge, FileBadges } from "@/presenters/review-plan"

interface FileTreeBaseProps {
  nodes: readonly FileTreeNode[]
  "aria-label": string
  collapsed: ReadonlySet<string>
  /** Must keep its identity: the rows are memoized */
  onToggleDirectory: (key: string) => void
  /** The code owner of a file by its path, in the hover of its row (the header of its diff shows it) */
  fileOwners?: ReadonlyMap<string, string>
  /** The badges of the review plan on the rows (`FileRowBadges`); none without a plan */
  badges?: FileTreeBadges | null
}

/** The badges of the rows of a tree, by the key of the row */
export interface FileTreeBadges {
  readonly files: ReadonlyMap<string, FileBadges>
  /** The dependency changes of a module, on its directory row or on the row of its build file */
  readonly dependencies: ReadonlyMap<string, DependencyBadge>
  /** Shows another file from a popover of a badge; must keep its identity: the rows are memoized */
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
  /** Must keep its identity: the rows are memoized */
  onSelectFile: (file: CommitFile) => void
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
 * path, the rename and the owner are in its hover, Copy Path in its right-click menu and on ⌘C.
 */
export function FileTree(props: FileTreeProps) {
  const { nodes, "aria-label": label, collapsed, onToggleDirectory, mode, fileOwners, badges = null } = props
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
    if (node.kind === "file") {
      onSelectFile?.(node.file)
    }
  }

  // memoized by the compiler: the rows are memoized
  const toggle = (node: FileTreeNode) => {
    setFocusedKey(node.key)
    if (node.kind === "directory") {
      onToggleDirectory(node.key)
    } else if (onSelectFile !== undefined) {
      onSelectFile(node.file)
    } else {
      onToggleFile?.(node.key)
    }
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement
    const key = target.dataset.treeKey
    // ⌘C on a row copies its path, as Copy Path of its menu
    if (key !== undefined && isCopyKey(event)) {
      const node = rows.find(row => row.key === key)
      if (node !== undefined) {
        event.preventDefault()
        void copyText(copiedText(fileActionTarget(node)))
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
          owner={node.kind === "file" ? fileOwners?.get(node.file.path) : undefined}
          fileBadges={node.kind === "file" ? badges?.files.get(node.key) : undefined}
          dependencyBadge={badges?.dependencies.get(node.key)}
          onSelectPlace={badges?.onSelectPlace}
          content={props.mode === "inline" && node.kind === "file" && props.expandedFiles.has(node.key) ? props.renderFileContent(node.file) : null}
        />
      ))}
    </div>
  )
}

/** A directory is open unless collapsed; a file is open while its diff shows, `undefined` where it has none to show */
function isRowOpen(node: FileTreeNode, collapsed: ReadonlySet<string>, expandedFiles: ReadonlySet<string> | null): boolean | undefined {
  if (node.kind === "directory") {
    return !collapsed.has(node.key)
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
  /** The badges of the review plan */
  fileBadges: FileBadges | undefined
  dependencyBadge: DependencyBadge | undefined
  onSelectPlace: ((place: ReviewPlace) => void) | undefined
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
  content,
}: FileTreeRowProps) {
  const indent = `${node.depth * INDENT_REM}rem`
  const description =
    node.kind === "directory"
      ? `${formatFileCount(node.fileCount)}, +${node.additions} −${node.deletions}`
      : `${formatFileStatus(node.file.status)}, +${node.file.additions} −${node.file.deletions}${fileBadges === undefined ? "" : `, ${fileBadges.description}`}`
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
      aria-selected={node.kind === "file" ? isSelected : undefined}
      aria-label={node.name}
      aria-description={description}
      tabIndex={inTabOrder ? 0 : -1}
      data-tree-key={node.key}
      className="group/tree-item flex flex-col outline-none"
      onFocus={event => {
        if (event.target === event.currentTarget) {
          onFocusRow(node.key)
        }
      }}
    >
      <FileContextMenu
        target={() => fileActionTarget(node)}
        copyKey
        // the row takes the focus, so ⌘C copies the same path
        onOpen={() => rowElements.current.get(node.key)?.focus()}
        render={
          <div
            className={cn(
              "group/row relative flex h-6 cursor-default items-center gap-1.5 rounded-md pr-1 text-sm transition-colors select-none group-focus-visible/tree-item:ring-2 group-focus-visible/tree-item:ring-ring/50 group-focus-visible/tree-item:ring-inset hover:bg-muted",
              isSelected && "bg-primary/15 hover:bg-primary/15",
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
        {node.kind === "directory" ? <DirectoryRowContent node={node} isOpen={isOpen === true} /> : <FileRowContent node={node} owner={owner} />}
        {dependencyBadge !== undefined && <DependencyBadgeView badge={dependencyBadge} />}
        {fileBadges !== undefined && onSelectPlace !== undefined && <FileRowBadges badges={fileBadges} onSelectPlace={onSelectPlace} />}
        {node.kind === "directory" ? <DirectoryLineCounts node={node} /> : <FileLineCounts node={node} />}
      </FileContextMenu>
      {content !== null && (
        <div className="my-1 overflow-hidden rounded-lg border border-border" style={{ marginLeft: indent }}>
          {content}
        </div>
      )}
    </div>
  )
})

function DirectoryRowContent({ node, isOpen }: { node: FileTreeDirectory; isOpen: boolean }) {
  // a group of a review is no directory: its name is what it holds
  const Icon = node.group !== undefined ? LayersIcon : isOpen ? FolderOpenIcon : FolderIcon
  return (
    <>
      <Icon className="size-4 shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate" title={node.group === undefined ? node.key.slice(0, -1) : node.name} data-group={node.group}>
        {node.name}
      </span>
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

function FileLineCounts({ node: { file } }: { node: FileTreeFile }) {
  return (
    (file.additions > 0 || file.deletions > 0) && (
      <span className="shrink-0 text-xs">
        <LineCounts data-testid="file-line-counts" additions={file.additions} deletions={file.deletions} colored />
      </span>
    )
  )
}
