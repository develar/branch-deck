import { useMemo } from "react"
import { FileDiffIcon, GitMergeIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import { getConflictMarkerCommits } from "@/components/conflict/conflict-marker-commits"
import { ConflictingFileListCard } from "@/components/conflict/ConflictingFileListCard"
import { ConflictResolutionHint } from "@/components/conflict/ConflictResolutionHint"
import { MissingCommitsDetails } from "@/components/conflict/MissingCommitsDetails"
import type { MergeConflictInfo } from "@/lib/bindings"

export interface MergeConflictViewerProps {
  conflict: MergeConflictInfo
  branchName?: string
}

/** Cherry-pick conflict of a commit row: the divergence insight and a resolution hint, then the conflicting files, then the missing commits. */
export function MergeConflictViewer({ conflict, branchName }: MergeConflictViewerProps) {
  const conflictMarkerCommits = useMemo(() => getConflictMarkerCommits(conflict), [conflict])
  const analysis = conflict.conflictAnalysis
  const missingCommitCount = analysis.missingCommits.length

  return (
    <div className="flex flex-col gap-4">
      {/* the commit that cannot be applied, why, and how to resolve it */}
      <Card className="gap-4 bd-padding-card shadow-none">
        {/* the commit row above already shows the commit, so only the title is repeated here */}
        <p className="flex items-center gap-3 text-sm font-medium text-foreground">
          <GitMergeIcon className="size-5 shrink-0 text-destructive" />
          Cherry-pick conflict detected
        </p>

        <div className="flex flex-col gap-1 rounded-lg bg-warning/5 px-4 py-3 text-sm text-muted-foreground ring-1 ring-warning/30">
          <p className="flex items-center gap-2 font-medium text-foreground">
            <FileDiffIcon className="size-4 text-warning" />
            {`${missingCommitCount} missing ${missingCommitCount === 1 ? "commit" : "commits"} modified the conflicting files`}
          </p>
          <p>
            {"Common ancestor "}
            <CommitHashPopover
              hash={analysis.mergeBaseHash}
              subject={analysis.mergeBaseSubject}
              message={analysis.mergeBaseMessage}
              author={analysis.mergeBaseAuthor}
              authorTime={analysis.mergeBaseTime}
            />
            {`: ${analysis.divergenceSummary.commitsAheadInSource} commits behind the source, ${analysis.divergenceSummary.commitsAheadInTarget} behind the target`}
          </p>
        </div>

        <ConflictResolutionHint />
      </Card>

      <ConflictingFileListCard
        conflicts={conflict.conflictingFiles}
        conflictInfo={conflict}
        conflictMarkerCommits={conflictMarkerCommits}
        branchName={branchName}
      />

      {missingCommitCount > 0 && (
        <MissingCommitsDetails missingCommits={analysis.missingCommits} conflict={conflict} branchName={branchName} />
      )}
    </div>
  )
}
