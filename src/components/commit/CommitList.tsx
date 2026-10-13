import { lazy, memo, Suspense, useCallback, useEffect, useRef, useState } from "react"
import type { ComponentProps, DragEvent, KeyboardEvent, MouseEvent, ReactNode } from "react"
import { LoaderCircleIcon } from "lucide-react"
import { cn } from "cn"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CommitMessageButton } from "@/components/commit/CommitMessage"
import { CommitHeader } from "@/components/commit/CommitHeader"
import { CommitPreviewDialog } from "@/components/commit/CommitPreviewDialog"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { beginCommitDrag } from "@/hooks/useCommitDrag"
import { useCommitPreviewHotkey } from "@/hooks/useCommitPreviewHotkey"
import { useRewordCommit } from "@/hooks/useRewordCommit"
import type { RowSelection } from "@/hooks/useRowSelection"
import type { CommitUnion } from "@/lib/commit"
import { getCommitFileCount, getCommitHash, getCommitSubject, hasCommitMessageBody } from "@/lib/commit"
import type { MenuItemGroups } from "@/lib/menu"

const MergeConflictViewer = lazy(() => import("@/components/conflict/MergeConflictViewer").then(it => ({ default: it.MergeConflictViewer })))

export type CommitListVariant = "compact" | "detailed" | "status"

export interface CommitListProps<T extends CommitUnion> {
  commits: readonly T[]
  variant?: CommitListVariant
  /** For the status variant: passed to the merge conflict viewer */
  branchName?: string
  showFileCount?: boolean
  showAuthor?: boolean
  /** Marks the first commit as the amend target */
  highlightTipCommit?: boolean
  /** The commits are on the current branch: the preview can edit their messages */
  editable?: boolean

  /**
   * Makes the rows selectable (click, Cmd/Ctrl+click, Shift+click, Cmd/Ctrl+A, Escape).
   * The state is owned by the caller: `useRowSelection({ items: commits, getId: getCommitHash })`.
   */
  selection?: RowSelection<T>
  /**
   * Context menu for selectable rows. Called when the menu opens, with the selected commits if the row is selected,
   * otherwise with the row's commit only. Returning no items suppresses the menu.
   */
  contextMenuItems?: (commits: T[]) => MenuItemGroups
  /** Called before the selection shortcuts; call `preventDefault()` to suppress them */
  onKeyDown?: (event: KeyboardEvent<HTMLTableElement>) => void
  /**
   * Makes selectable rows draggable; called when a drag starts with the selected commits if the row is selected,
   * otherwise with the row's commit only
   */
  onDragCommits?: (commits: T[]) => void
  /** Called when a drag started in this list ends, dropped or not */
  onDragEnd?: () => void

  /** Rendered above the table (e.g. an inline form) */
  portalTarget?: ReactNode
  /** Rendered at the end of each commit cell; keep it stable (`useCallback`) so rows stay memoized */
  renderAfterCommit?: (commit: T, index: number) => ReactNode
}

/**
 * Commit table used by the branch tables, the unassigned commits card and the conflict views.
 * Space (over a row, or on the last clicked row of the focused table) and the row's message button open the commit preview.
 */
export function CommitList<T extends CommitUnion>({
  commits,
  variant = "compact",
  branchName,
  showFileCount = false,
  showAuthor = false,
  highlightTipCommit = false,
  editable = false,
  selection,
  contextMenuItems,
  onKeyDown,
  onDragCommits,
  onDragEnd,
  portalTarget,
  renderAfterCommit,
}: CommitListProps<T>) {
  const selectable = selection !== undefined

  // stable callbacks for the memoized rows, reading the latest props
  const latestRef = useRef({ selection, contextMenuItems, onDragCommits, onDragEnd })
  useEffect(() => {
    latestRef.current = { selection, contextMenuItems, onDragCommits, onDragEnd }
  })

  const getMenuGroups = useCallback((commit: CommitUnion): MenuItemGroups => {
    const { selection, contextMenuItems } = latestRef.current
    if (contextMenuItems === undefined || selection === undefined) {
      return []
    }
    return contextMenuItems(getActionCommits(selection, commit))
  }, [])

  const handleDragStart = useCallback((event: DragEvent, commit: CommitUnion) => {
    const { selection, onDragCommits } = latestRef.current
    if (selection === undefined || onDragCommits === undefined) {
      return
    }
    const commits = getActionCommits(selection, commit)
    beginCommitDrag(event, commits.length)
    onDragCommits(commits)
  }, [])
  const handleDragEnd = useCallback(() => latestRef.current.onDragEnd?.(), [])

  // a row context menu opened with the mouse leaves the focus on the table; its keys (e.g. Escape) belong to the menu
  const isMenuOpenRef = useRef(false)
  const handleMenuOpenChange = useCallback((open: boolean) => {
    isMenuOpenRef.current = open
  }, [])

  // the preview follows the commit, not its index, across a sync; after an edit, it waits for the reworded commit (`nextHash`)
  const [preview, setPreview] = useState<{ hash: string, nextHash?: string } | null>(null)
  let previewIndex = preview === null ? -1 : commits.findIndex(commit => getCommitHash(commit) === preview.hash)
  if (preview !== null && previewIndex === -1) {
    const nextHash = preview.nextHash
    previewIndex = nextHash === undefined ? -1 : commits.findIndex(commit => getCommitHash(commit) === nextHash)
    // a commit gone after a sync (also a reworded one that moved to another list) closes the preview for good
    setPreview(previewIndex === -1 ? null : { hash: nextHash! })
  }
  const openPreview = useCallback((hash: string) => {
    if (!isMenuOpenRef.current) {
      setPreview({ hash })
    }
  }, [])
  const { rewordCommit } = useRewordCommit()
  const saveMessage = (commit: CommitUnion, message: string) => rewordCommit(commit, message, (newCommitId) => {
    setPreview({ hash: getCommitHash(commit), nextHash: newCommitId })
  })
  const tableRef = useRef<HTMLTableElement>(null)
  useCommitPreviewHotkey(tableRef, () => latestRef.current.selection?.getAnchorId() ?? null, openPreview)

  const handleRowClick = selection?.handleRowClick
  const handleKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    // React bubbles events from portals (row context menus, popovers) through the component tree; only handle keys pressed in the table.
    // The preview takes the focus a frame after it opens; until then, its keys (Escape, arrows) must not reach the selection.
    if (isMenuOpenRef.current || previewIndex !== -1 || !event.currentTarget.contains(event.target as Node)) {
      return
    }
    onKeyDown?.(event)
    if (!event.defaultPrevented && selection !== undefined) {
      selection.handleKeyDown(event)
    }
  }

  return (
    <div>
      {portalTarget}
      <table
        ref={tableRef}
        className="w-full rounded-lg focus:outline-none"
        tabIndex={selectable ? 0 : -1}
        onKeyDown={handleKeyDown}
      >
        <tbody className="divide-y divide-border">
          {commits.map((commit, index) => {
            const hash = getCommitHash(commit)
            return (
              <CommitRow
                key={hash}
                commit={commit}
                hash={hash}
                index={index}
                variant={variant}
                branchName={branchName}
                showFileCount={showFileCount}
                showAuthor={showAuthor}
                isAmendTarget={highlightTipCommit && index === 0}
                selectable={selectable}
                selected={selection?.isSelected(hash) === true}
                onRowClick={handleRowClick}
                getMenuGroups={contextMenuItems !== undefined && selectable ? getMenuGroups : undefined}
                onDragStart={onDragCommits !== undefined && selectable ? handleDragStart : undefined}
                onDragEnd={handleDragEnd}
                onMenuOpenChange={handleMenuOpenChange}
                onOpenPreview={openPreview}
                renderAfterCommit={renderAfterCommit as CommitRowProps["renderAfterCommit"]}
              />
            )
          })}
        </tbody>
      </table>
      <CommitPreviewDialog
        commits={commits}
        index={previewIndex}
        onIndexChange={index => setPreview({ hash: getCommitHash(commits[index]!) })}
        onClose={() => setPreview(null)}
        onSaveMessage={editable ? saveMessage : undefined}
        showAuthor={showAuthor}
        showFileCount={showFileCount}
        showNewHash={variant === "status"}
      />
    </div>
  )
}

/** The commits a row action applies to: the selection if the row is selected, otherwise the row's commit only */
function getActionCommits<T extends CommitUnion>(selection: RowSelection<T>, commit: CommitUnion): T[] {
  const isRowSelected = selection.isSelected(getCommitHash(commit))
  return isRowSelected && selection.selectedCount > 0 ? selection.selectedItems : [commit as T]
}

interface CommitRowProps {
  commit: CommitUnion
  hash: string
  index: number
  variant: CommitListVariant
  branchName: string | undefined
  showFileCount: boolean
  showAuthor: boolean
  isAmendTarget: boolean
  selectable: boolean
  selected: boolean
  onRowClick: ((event: MouseEvent, id: string, index: number) => void) | undefined
  /** Set if the row has a context menu */
  getMenuGroups: ((commit: CommitUnion) => MenuItemGroups) | undefined
  /** Set if the row is draggable */
  onDragStart: ((event: DragEvent, commit: CommitUnion) => void) | undefined
  onDragEnd: () => void
  onMenuOpenChange: (open: boolean) => void
  onOpenPreview: (hash: string) => void
  renderAfterCommit: ((commit: CommitUnion, index: number) => ReactNode) | undefined
}

const CommitRow = memo(function CommitRow(props: CommitRowProps) {
  const { commit, hash, index, selectable, selected, onRowClick, getMenuGroups, onDragStart, onDragEnd, onMenuOpenChange, renderAfterCommit } = props

  const rowProps = {
    "data-row-id": hash,
    "data-selected": String(selectable && selected),
    // only selectable rows react to the pointer: hover on a row that does nothing on click is a false affordance
    "className": cn(
      selectable && "cursor-pointer select-none hover:bg-muted",
      selectable && selected && "bg-primary/10 hover:bg-primary/15",
    ),
    "onClick": selectable && onRowClick ? (event: MouseEvent) => onRowClick(event, hash, index) : undefined,
    ...(onDragStart === undefined
      ? {}
      : { draggable: true, onDragStart: (event: DragEvent) => onDragStart(event, commit), onDragEnd }),
  }

  const cell = (
    <td className="bd-padding-list-item">
      <CommitCellContent {...props} />
      {renderAfterCommit?.(commit, index)}
    </td>
  )

  if (getMenuGroups === undefined) {
    return <tr {...rowProps}>{cell}</tr>
  }
  return (
    <CommitRowWithContextMenu commit={commit} rowProps={rowProps} getMenuGroups={getMenuGroups} onOpenChange={onMenuOpenChange}>
      {cell}
    </CommitRowWithContextMenu>
  )
})

interface CommitRowWithContextMenuProps {
  commit: CommitUnion
  rowProps: ComponentProps<"tr"> & Record<`data-${string}`, string>
  getMenuGroups: (commit: CommitUnion) => MenuItemGroups
  onOpenChange: (open: boolean) => void
  children: ReactNode
}

function CommitRowWithContextMenu({ commit, rowProps, getMenuGroups, onOpenChange, children }: CommitRowWithContextMenuProps) {
  // computed when the menu opens, from the selection at that moment
  const [groups, setGroups] = useState<MenuItemGroups>([])

  return (
    <ContextMenu
      onOpenChange={(open, eventDetails) => {
        if (open) {
          const nextGroups = getMenuGroups(commit)
          if (nextGroups.every(group => group.length === 0)) {
            eventDetails.cancel()
            return
          }
          setGroups(nextGroups)
        }
        onOpenChange(open)
      }}
    >
      <ContextMenuTrigger render={<tr {...rowProps} />}>
        {children}
      </ContextMenuTrigger>
      {/* an inline form opened from the menu takes the focus */}
      <ContextMenuContent finalFocus={false}>
        <ContextMenuItems groups={groups} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

function CommitCellContent({ commit, hash, variant, branchName, showFileCount, showAuthor, isAmendTarget, onOpenPreview }: CommitRowProps) {
  const status = variant === "status" && "status" in commit ? commit.status : undefined
  const error = variant === "status" && "error" in commit ? commit.error : undefined
  const subject = getCommitSubject(commit)
  const hasBody = hasCommitMessageBody(subject, commit.message)

  const statusHash = variant === "status" && "hash" in commit ? commit.hash : undefined

  return (
    <div className="flex flex-col gap-1">
      <CommitHeader
        subject={subject}
        hash={hash}
        newHash={statusHash}
        author={showAuthor ? commit.author : undefined}
        authorTime={commit.authorTime}
        committerTime={commit.committerTime}
        fileCount={showFileCount ? getCommitFileCount(commit) : undefined}
        subjectAddon={(
          <>
            {hasBody && <CommitMessageButton onOpen={() => onOpenPreview(hash)} />}
            {status === "Pending" && (
              <Badge variant="secondary">
                <LoaderCircleIcon data-icon="inline-start" className="animate-spin" />
                Pending
              </Badge>
            )}
            {status === "Error" && <Badge variant="destructive">{error?.MergeConflict ? "Merge conflict" : "Error"}</Badge>}
            {status === "Blocked" && <Badge variant="warning">Blocked</Badge>}
            {isAmendTarget && (
              <Tooltip>
                <TooltipTrigger render={<Badge variant="info" />}>Amend target</TooltipTrigger>
                <TooltipContent>Changes will be amended to this commit</TooltipContent>
              </Tooltip>
            )}
          </>
        )}
      />

      {error && (
        <div className="mt-2">
          {error.MergeConflict
            ? (
                <Suspense fallback={null}>
                  <MergeConflictViewer conflict={error.MergeConflict} branchName={branchName} />
                </Suspense>
              )
            : error.Generic && (
              <Alert variant="destructive" className="border-transparent bg-destructive/10 py-2">
                <AlertDescription>
                  <p className="text-xs">{error.Generic}</p>
                </AlertDescription>
              </Alert>
            )}
        </div>
      )}
    </div>
  )
}
