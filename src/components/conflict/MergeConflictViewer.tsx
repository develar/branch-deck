import { useMemo } from "react"
import { getConflictMarkerCommits } from "@/components/conflict/conflict-marker-commits"
import { ConflictCommitContext } from "@/components/conflict/ConflictCommitContext"
import { ConflictingFileListCard } from "@/components/conflict/ConflictingFileListCard"
import { ConflictResolutionHint } from "@/components/conflict/ConflictResolutionHint"
import { DivergenceInsight } from "@/components/conflict/DivergenceInsight"
import { MissingCommitsDetails } from "@/components/conflict/MissingCommitsDetails"
import { NestedPanel } from "@/components/shared/NestedPanel"
import { useConflictBase } from "@/hooks/useConflictBase"
import type { MergeConflictInfo } from "@/lib/bindings"
import { formatConflictTitle, getMergeBase } from "@/lib/merge-conflict"

export interface MergeConflictViewerProps {
  conflict: MergeConflictInfo
  branchName?: string
  /** What the commit does not apply onto, when it is not the base of the branch ("the amended commit") */
  onto?: string
}

/**
 * Merge conflict of a commit, in panels nested in the expanded row: the title, the divergence insight and a resolution
 * hint, then the conflicting files, then the missing commits.
 */
export function MergeConflictViewer({ conflict, branchName, onto }: MergeConflictViewerProps) {
  const conflictMarkerCommits = useMemo(() => getConflictMarkerCommits(conflict), [conflict])
  const conflictBase = useConflictBase(branchName) ?? undefined
  const target = onto ?? conflictBase
  const analysis = conflict.conflictAnalysis
  const missingCommitCount = analysis.missingCommits.length

  return (
    <div className="flex flex-col gap-4">
      <NestedPanel className="gap-4 bd-padding-card">
        {/* the commit row above already shows the commit, so only the title is said here */}
        <ConflictCommitContext title={formatConflictTitle(conflict.commit.message, target)} />
        <DivergenceInsight missingCommitCount={missingCommitCount} mergeBase={getMergeBase(analysis)} divergenceSummary={analysis.divergenceSummary} />
        <ConflictResolutionHint />
      </NestedPanel>

      <ConflictingFileListCard conflicts={conflict.conflictingFiles} conflictInfo={conflict} conflictMarkerCommits={conflictMarkerCommits} branchName={branchName} onto={target} />

      {missingCommitCount > 0 && <MissingCommitsDetails missingCommits={analysis.missingCommits} conflict={conflict} branchName={branchName} onto={target} />}
    </div>
  )
}
