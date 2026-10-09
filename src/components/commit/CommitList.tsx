import { lazy, memo, Suspense, useCallback, useEffect, useRef, useState } from "react"
import type { ComponentProps, KeyboardEvent, MouseEvent, ReactNode, Ref } from "react"
import { LoaderCircleIcon } from "lucide-react"
import { cn } from "cn"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { CommitMessageWithPopover } from "@/components/commit/CommitMessageWithPopover"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import type { RowSelection } from "@/hooks/useRowSelection"
import type { BranchError, CommitSyncStatus } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash, getCommitSubject } from "@/lib/commit"
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

  /**
   * Makes the rows selectable (click, Cmd/Ctrl+click, Shift+click, Cmd/Ctrl+A, Escape).
   * The state is owned by the caller: `useRowSelection({ items: commits, getId: getCommitHash })`.
   */
  selection?: RowSelection<T>
  /** Shows a bar at the left edge of selected rows */
  highlightSelection?: boolean
  /**
   * Context menu for selectable rows. Called when the menu opens, with the selected commits if the row is selected,
   * otherwise with the row's commit only. Returning no items suppresses the menu.
   */
  contextMenuItems?: (commits: T[]) => MenuItemGroups
  /** Called before the selection shortcuts; call `preventDefault()` to suppress them */
  onKeyDown?: (event: KeyboardEvent<HTMLTableElement>) => void

  /** Rendered above the table (e.g. an inline form) */
  portalTarget?: ReactNode
  /** Rendered at the end of each commit cell; keep it stable (`useCallback`) so rows stay memoized */
  renderAfterCommit?: (commit: T, index: number) => ReactNode
  tableRef?: Ref<HTMLTableElement>
}

/** Commit table used by the branch tables, the unassigned commits card and the conflict views. */
export function CommitList<T extends CommitUnion>({
  commits,
  variant = "compact",
  branchName,
  showFileCount = false,
  showAuthor = false,
  highlightTipCommit = false,
  selection,
  highlightSelection = false,
  contextMenuItems,
  onKeyDown,
  portalTarget,
  renderAfterCommit,
  tableRef,
}: CommitListProps<T>) {
  const selectable = selection !== undefined

  // stable callbacks for the memoized rows, reading the latest props
  const latestRef = useRef({ selection, contextMenuItems })
  useEffect(() => {
    latestRef.current = { selection, contextMenuItems }
  })

  const getMenuGroups = useCallback((commit: CommitUnion): MenuItemGroups => {
    const { selection, contextMenuItems } = latestRef.current
    if (contextMenuItems === undefined || selection === undefined) {
      return []
    }
    const isRowSelected = selection.isSelected(getCommitHash(commit))
    const commits = isRowSelected && selection.selectedCount > 0 ? selection.selectedItems : [commit as T]
    return contextMenuItems(commits)
  }, [])

  // a row context menu opened with the mouse leaves the focus on the table; its keys (e.g. Escape) belong to the menu
  const isMenuOpenRef = useRef(false)
  const handleMenuOpenChange = useCallback((open: boolean) => {
    isMenuOpenRef.current = open
  }, [])

  const handleRowClick = selection?.handleRowClick
  const handleKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    // React bubbles events from portals (row context menus, popovers) through the component tree; only handle keys pressed in the table
    if (isMenuOpenRef.current || !event.currentTarget.contains(event.target as Node)) {
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
                highlightSelection={highlightSelection}
                onRowClick={handleRowClick}
                getMenuGroups={contextMenuItems !== undefined && selectable ? getMenuGroups : undefined}
                onMenuOpenChange={handleMenuOpenChange}
                renderAfterCommit={renderAfterCommit as CommitRowProps["renderAfterCommit"]}
              />
            )
          })}
        </tbody>
      </table>
    </div>
  )
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
  highlightSelection: boolean
  onRowClick: ((event: MouseEvent, id: string, index: number) => void) | undefined
  /** Set if the row has a context menu */
  getMenuGroups: ((commit: CommitUnion) => MenuItemGroups) | undefined
  onMenuOpenChange: (open: boolean) => void
  renderAfterCommit: ((commit: CommitUnion, index: number) => ReactNode) | undefined
}

const CommitRow = memo(function CommitRow(props: CommitRowProps) {
  const { commit, hash, index, selectable, selected, highlightSelection, onRowClick, getMenuGroups, onMenuOpenChange, renderAfterCommit } = props

  const rowProps = {
    "data-row-id": hash,
    "data-selected": String(selectable && selected),
    "className": cn(
      !selectable && "transition-colors hover:bg-muted",
      selectable && "relative cursor-pointer select-none hover:bg-muted",
      selectable && selected && "bg-primary/10 hover:bg-primary/15",
    ),
    "onClick": selectable && onRowClick ? (event: MouseEvent) => onRowClick(event, hash, index) : undefined,
  }

  const cell = (
    <td className="bd-padding-list-item">
      {selectable && selected && highlightSelection && (
        <div className="absolute top-1 bottom-1 left-0 w-1 rounded-r-sm bg-primary" />
      )}
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

function CommitCellContent({ commit, variant, branchName, showFileCount, showAuthor, isAmendTarget }: CommitRowProps) {
  const status = variant === "status" && "status" in commit ? commit.status : undefined
  const error = variant === "status" && "error" in commit ? commit.error : undefined
  const fileCount = showFileCount ? getFileCount(commit) : undefined

  return (
    <div>
      <div className="flex items-center gap-2">
        <CommitMessageWithPopover
          subject={getCommitSubject(commit)}
          message={commit.message}
          messageClassName="line-clamp-2 text-sm text-foreground"
        />
        {status === "Pending" && (
          <Badge variant="secondary">
            <LoaderCircleIcon data-icon="inline-start" className="animate-spin" />
            Pending
          </Badge>
        )}
        {status === "Error" && <Badge variant="destructive">Error</Badge>}
        {status === "Blocked" && <Badge variant="warning">Blocked</Badge>}
      </div>

      <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
        <CommitMetadata commit={commit} showAuthor={showAuthor} variant={variant === "status" ? "status" : "compact"} />

        {!!fileCount && (
          <>
            <span>•</span>
            <span>{`${fileCount} ${fileCount === 1 ? "file" : "files"}`}</span>
          </>
        )}

        {status && status !== "Unchanged" && (
          <>
            <span>•</span>
            <span className={statusClassName(status)}>{statusText(status, error)}</span>
          </>
        )}

        {isAmendTarget && (
          <>
            <span>•</span>
            <span className="text-info">Changes will be amended to this commit</span>
          </>
        )}
      </div>

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

function getFileCount(commit: CommitUnion): number | undefined {
  if ("fileDiffs" in commit && commit.fileDiffs?.length) {
    return commit.fileDiffs.length
  }
  if ("fileCount" in commit && typeof commit.fileCount === "number") {
    return commit.fileCount
  }
  return undefined
}

function statusClassName(status: CommitSyncStatus): string {
  switch (status) {
    case "Pending":
      return "text-muted-foreground/70"
    case "Error":
      return "text-destructive"
    case "Blocked":
      return "text-warning"
    case "Created":
      return "text-success"
    case "Unchanged":
      return "text-muted-foreground"
  }
}

function statusText(status: CommitSyncStatus, error: BranchError | null | undefined): string {
  return status === "Error" && error?.MergeConflict ? "Merge Conflict" : status
}
