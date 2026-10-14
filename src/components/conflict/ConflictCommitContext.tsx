import { TriangleAlertIcon } from "lucide-react"
import { CommitHeader } from "@/components/commit/CommitHeader"

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
    <div className="flex items-start gap-2">
      {/* one line high: centred on the title */}
      <span className="flex h-lh shrink-0 items-center text-sm">
        <TriangleAlertIcon className="size-4 text-destructive" />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="text-sm font-medium text-foreground">{title || "Cherry-pick conflict"}</p>
        <CommitHeader
          subject={commitMessage.split("\n", 1)[0]!}
          hash={commitHash}
          authorTime={commitAuthorTime}
          committerTime={commitCommitterTime}
          extra={branchName ? [<span key="branch">{branchName}</span>] : undefined}
        />
      </div>
    </div>
  )
}
