import { TriangleAlertIcon } from "lucide-react"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import type { CommitMetadataProps } from "@/components/commit/CommitMetadata"

export interface ConflictCommitContextProps {
  /** `formatConflictTitle`: "“<subject>” does not apply cleanly onto <base>" */
  title: string
  /** The meta line of the commit under the title; the expanded row has the commit row above it instead */
  meta?: CommitMetadataProps
}

/** The commit that does not apply: the title of the conflict views, in the expanded row and in the sub-windows. */
export function ConflictCommitContext({ title, meta }: ConflictCommitContextProps) {
  return (
    <div className="flex items-start gap-2">
      {/* one line high: centred on the title */}
      <span className="flex h-lh shrink-0 items-center text-sm">
        <TriangleAlertIcon className="size-4 text-destructive" />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="text-sm font-medium text-pretty text-foreground" data-testid="conflict-title">
          {title}
        </p>
        {meta && <CommitMetadata {...meta} />}
      </div>
    </div>
  )
}
