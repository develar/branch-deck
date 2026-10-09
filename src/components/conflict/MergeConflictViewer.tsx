import { useMemo } from "react"
import { FileDiffIcon, GitMergeIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import { CommitInfo } from "@/components/commit/CommitInfo"
import { getConflictMarkerCommits } from "@/components/conflict/conflict-marker-commits"
import { ConflictingFileListCard } from "@/components/conflict/ConflictingFileListCard"
import { MissingCommitsDetails } from "@/components/conflict/MissingCommitsDetails"
import type { MergeConflictInfo } from "@/lib/bindings"

export interface MergeConflictViewerProps {
  conflict: MergeConflictInfo
  branchName?: string
}

/** Cherry-pick conflict of a commit: overview with the divergence insight, missing commits and conflicting files. */
export function MergeConflictViewer({ conflict, branchName }: MergeConflictViewerProps) {
  const conflictMarkerCommits = useMemo(() => getConflictMarkerCommits(conflict), [conflict])
  const analysis = conflict.conflictAnalysis
  const missingCommitCount = analysis.missingCommits.length

  return (
    <div className="flex flex-col gap-4">
      {/* Conflict overview and missing commits insight */}
      <Card className="gap-0 overflow-hidden py-0">
        <div className="border-b border-border p-4 sm:px-6">
          <div className="flex items-start gap-3">
            <GitMergeIcon className="mt-0.5 size-5 shrink-0 text-destructive" />
            <div className="flex min-w-0 flex-1 flex-col gap-1">
              <p className="text-sm font-medium text-foreground">Cherry-pick conflict detected</p>
              <p className="text-sm text-muted-foreground">{`Cannot apply: ${conflict.commitMessage}`}</p>
              <CommitInfo hash={conflict.commitHash} authorTime={conflict.commitAuthorTime} committerTime={conflict.commitCommitterTime} />
            </div>
          </div>
        </div>

        <div className="bd-padding-card">
          <div className="flex flex-col gap-2 rounded-lg border border-warning bg-muted bd-padding-card">
            <div className="flex items-center gap-2">
              <FileDiffIcon className="size-4 text-warning" />
              <span className="text-sm font-medium text-foreground">Insight: Missing commits modified the conflicting files</span>
            </div>

            <div className="flex flex-col gap-1">
              <p className="text-sm text-muted-foreground">
                {`${missingCommitCount} ${missingCommitCount === 1 ? "commit is" : "commits are"} missing that altered the files in conflict.`}
              </p>

              <p className="text-sm text-muted-foreground">
                {"Common ancestor: "}
                <CommitHashPopover
                  hash={analysis.mergeBaseHash}
                  subject={analysis.mergeBaseSubject}
                  message={analysis.mergeBaseMessage}
                  author={analysis.mergeBaseAuthor}
                  authorTime={analysis.mergeBaseTime}
                />
                {" "}
                <span className="text-muted-foreground">
                  {`(${analysis.divergenceSummary.commitsAheadInSource} commits behind source, ${analysis.divergenceSummary.commitsAheadInTarget} commits behind target)`}
                </span>
              </p>
            </div>
          </div>
        </div>
      </Card>

      {missingCommitCount > 0 && (
        <MissingCommitsDetails missingCommits={analysis.missingCommits} conflict={conflict} branchName={branchName} />
      )}

      <ConflictingFileListCard
        conflicts={conflict.conflictingFiles}
        conflictInfo={conflict}
        conflictMarkerCommits={conflictMarkerCommits}
        branchName={branchName}
      />
    </div>
  )
}
