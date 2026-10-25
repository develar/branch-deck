import { lazy, Suspense } from "react"
import type { ReactNode } from "react"
import { Spinner } from "@/components/ui/spinner"
import { CommitList } from "@/components/commit/CommitList"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { RowActions } from "@/components/shared/RowActions"
import type { Commit } from "@/lib/bindings"
import type { ActiveInline, InlineActionType } from "@/stores/inline-row-action"
import { closeInline, portalTargetIdFor } from "@/stores/inline-row-action"

const InlineDeleteConfirmationInput = lazy(() => import("@/components/branch/InlineDeleteConfirmationInput").then(it => ({ default: it.InlineDeleteConfirmationInput })))

/** The Actions cell of the row of a kept branch: its "⋯" menu, `menu` the same items as the right-click */
export function KeptBranchActionsCell({ label, disabled, busy = null, menu }: { label: string; disabled: boolean; busy?: string | null; menu: ReactNode }) {
  return (
    <td className="w-px px-4 py-(--bd-cell-y) whitespace-nowrap">
      <RowActions>
        <MoreActionsMenu name={label} disabled={disabled} busy={busy} menu={menu} />
      </RowActions>
    </td>
  )
}

interface KeptBranchRowPartsProps {
  /** The full name of the branch, the key of its row */
  name: string
  columnCount: number
  /** The inline form of the row that is open: its portal row is there */
  inline: ActiveInline | null
  processing: boolean
  /** The inline form of the delete confirmation */
  deleteForm: InlineActionType
  deleteTitle: string
  /** What the delete loses */
  deleteHelpText?: string
  onDelete: () => void
  /** The content below the expanded row; nothing when collapsed */
  expanded: ReactNode
}

/**
 * The rows below the row of a branch kept outside HEAD (archived, unapplied): the portal of its inline form, the delete
 * confirmation (its name typed), and its expansion
 */
export function KeptBranchRowParts({ name, columnCount, inline, processing, deleteForm, deleteTitle, deleteHelpText, onDelete, expanded }: KeptBranchRowPartsProps) {
  return (
    <>
      {inline !== null && (
        <tr>
          <td id={portalTargetIdFor(name)} colSpan={columnCount} className="relative overflow-hidden p-0">
            {processing && <InlineRowProgressBar />}
            {inline?.type === deleteForm && (
              <Suspense fallback={null}>
                <InlineDeleteConfirmationInput key={`delete:${name}`} branchName={name} title={deleteTitle} helpText={deleteHelpText} onSubmit={onDelete} onCancel={closeInline} />
              </Suspense>
            )}
          </td>
        </tr>
      )}
      {expanded !== null && expanded !== false && (
        // as the commits of a row of the branch table
        <tr data-testid="branch-commits-row" data-branch-of={name}>
          <td colSpan={columnCount} className="px-0 py-(--bd-cell-y)">
            {expanded}
          </td>
        </tr>
      )}
    </>
  )
}

interface KeptBranchCommitsProps {
  isLoading: boolean
  loadError: string | undefined
  commits: readonly Commit[] | undefined
}

/** The commits of a branch kept outside HEAD (archived, unapplied) below its expanded row: loading, an error, the list */
export function KeptBranchCommits({ isLoading, loadError, commits }: KeptBranchCommitsProps) {
  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Spinner className="size-4" />
        Loading commits…
      </div>
    )
  }
  if (loadError !== undefined && loadError !== "") {
    return <p className="text-sm text-destructive">{`Cannot load the commits: ${loadError}`}</p>
  }
  if (commits !== undefined && commits.length > 0) {
    return <CommitList commits={commits} variant="compact" showFileCount={false} />
  }
  return <div className="text-sm text-muted-foreground">No commits found</div>
}
