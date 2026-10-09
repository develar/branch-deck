// TODO(Phase 6): temporary minimal commit list for the branch tables; replaced by the full port of CommitList.vue
//  (selection, context menu, CommitMessageWithPopover, CommitMetadata, LinkedText, file count).
import { lazy, memo, Suspense } from "react"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { TimeWithPopover } from "@/components/shared/TimeWithPopover"
import type { Commit, CommitSyncStatus } from "@/lib/bindings"
import type { SyncedCommit } from "@/stores/branch-sync"

const MergeConflictViewer = lazy(() => import("@/components/conflict/MergeConflictViewer").then(it => ({ default: it.MergeConflictViewer })))

export interface CommitListProps {
  commits: readonly (Commit | SyncedCommit)[]
  variant?: "compact" | "detailed" | "status"
  branchName?: string
  showFileCount?: boolean
  showAuthor?: boolean
  highlightTipCommit?: boolean
}

export function CommitList({ commits, variant = "compact", branchName, showAuthor = false, highlightTipCommit = false }: CommitListProps) {
  return (
    <div>
      <table className="w-full rounded-lg focus:outline-none" tabIndex={-1}>
        <tbody className="divide-y divide-border">
          {commits.map((commit, index) => (
            <CommitRow
              key={commit.originalHash}
              commit={commit}
              variant={variant}
              branchName={branchName}
              showAuthor={showAuthor}
              isAmendTarget={highlightTipCommit && index === 0}
            />
          ))}
        </tbody>
      </table>
    </div>
  )
}

interface CommitRowProps {
  commit: Commit | SyncedCommit
  variant: "compact" | "detailed" | "status"
  branchName: string | undefined
  showAuthor: boolean
  isAmendTarget: boolean
}

const CommitRow = memo(function CommitRow({ commit, variant, branchName, showAuthor, isAmendTarget }: CommitRowProps) {
  const status = variant === "status" && "status" in commit ? commit.status : undefined
  const error = "error" in commit ? commit.error : undefined
  const displayHash = ("hash" in commit && commit.hash) || commit.originalHash

  return (
    <tr data-row-id={commit.originalHash} data-selected="false" className="transition-colors hover:bg-muted">
      <td className="bd-padding-list-item">
        <div className="flex items-center gap-2">
          <span className="line-clamp-2 text-sm text-foreground">{commit.strippedSubject || commit.message}</span>
          {status === "Pending" && (
            <Badge variant="secondary">
              <Spinner />
              Pending
            </Badge>
          )}
          {status === "Error" && <Badge variant="destructive">Error</Badge>}
          {status === "Blocked" && <Badge variant="warning">Blocked</Badge>}
        </div>
        <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
          <span className="font-mono">{displayHash.slice(0, 8)}</span>
          <span>•</span>
          <TimeWithPopover authorTime={commit.authorTime} committerTime={commit.committerTime} />
          {showAuthor && (
            <>
              <span>•</span>
              <span>{commit.author}</span>
            </>
          )}
          {status && status !== "Unchanged" && (
            <>
              <span>•</span>
              <span className={statusClassName(status)}>{status === "Error" && error?.MergeConflict ? "Merge Conflict" : status}</span>
            </>
          )}
          {isAmendTarget && (
            <>
              <span>•</span>
              <span className="text-info">Changes will be amended to this commit</span>
            </>
          )}
        </div>
        {variant === "status" && error?.MergeConflict && (
          <div className="mt-2">
            <Suspense fallback={null}>
              <MergeConflictViewer conflict={error.MergeConflict} branchName={branchName} />
            </Suspense>
          </div>
        )}
        {variant === "status" && error?.Generic && (
          <p className="mt-2 rounded-md bg-destructive/10 px-2 py-1 text-xs text-destructive">{error.Generic}</p>
        )}
      </td>
    </tr>
  )
})

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
