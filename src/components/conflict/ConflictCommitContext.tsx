import { GitMergeIcon } from "lucide-react"
import { CommitInfo } from "@/components/commit/CommitInfo"

export interface ConflictCommitContextProps {
  title?: string
  commitMessage: string
  commitHash: string
  commitAuthorTime: number
  commitCommitterTime: number
  branchName?: string
}

/** The commit that cannot be applied, shown at the top of the conflict sub-windows. */
export function ConflictCommitContext({ title, commitMessage, commitHash, commitAuthorTime, commitCommitterTime, branchName }: ConflictCommitContextProps) {
  return (
    <div className="flex items-start gap-3">
      <GitMergeIcon className="mt-0.5 size-5 shrink-0 text-destructive" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="text-sm font-medium text-foreground">{title || "Cherry-pick conflict"}</p>
        <p className="text-sm text-muted-foreground">{commitMessage}</p>
        <div className="flex items-center gap-3">
          <CommitInfo hash={commitHash} authorTime={commitAuthorTime} committerTime={commitCommitterTime} />
          {branchName && (
            <>
              <span className="text-xs text-muted-foreground">•</span>
              <span className="text-xs text-muted-foreground">{`Branch: ${branchName}`}</span>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
