import { ConflictCommitContext } from "@/components/conflict/ConflictCommitContext"
import { ConflictExplanation } from "@/components/conflict/ConflictExplanation"
import { ConflictFixPanel } from "@/components/conflict/ConflictFixPanel"
import { ConflictingFileListCard } from "@/components/conflict/ConflictingFileListCard"
import { ConflictResolutionHint } from "@/components/conflict/ConflictResolutionHint"
import { MissingCommitsDetails } from "@/components/conflict/MissingCommitsDetails"
import { NestedPanel } from "@/components/shared/NestedPanel"
import { useConflictBase } from "@/hooks/useConflictBase"
import { useMissingCommitOwners } from "@/hooks/useMissingCommitOwners"
import type { MergeConflictInfo } from "@/lib/bindings"
import { formatConflictTitle } from "@/lib/merge-conflict"

export interface MergeConflictViewerProps {
  conflict: MergeConflictInfo
  branchName?: string
  /** What the commit does not apply onto, when it is not the base of the branch ("the amended commit") */
  onto?: string
}

/**
 * Merge conflict of a commit, in panels nested in the expanded row: the title, the explanation (which commits outside
 * the branch changed the files first) and the fix (a resolution hint for an amend conflict), then the conflicting files,
 * then the missing commits.
 */
export function MergeConflictViewer({ conflict, branchName, onto }: MergeConflictViewerProps) {
  const conflictBase = useConflictBase(branchName) ?? undefined
  const target = onto ?? conflictBase
  const analysis = conflict.conflictAnalysis
  const missingCommitCount = analysis.missingCommits.length
  const owners = useMissingCommitOwners(analysis.missingCommits)
  // an amend conflict is with the amended commit, not with the base of the branch
  const builtOn = onto === undefined ? (conflictBase ?? null) : undefined

  return (
    <div className="flex flex-col gap-4">
      <NestedPanel className="gap-4 bd-padding-card">
        {/* the commit row above already shows the commit, so only the title is said here */}
        <ConflictCommitContext title={formatConflictTitle(conflict.commit.subjectParts, target)} />
        <ConflictExplanation owners={owners} branchName={branchName} builtOn={builtOn} revealable />
        {/* a fix regroups the commits of the sync; an amend conflict is with the amended commit */}
        {onto === undefined && branchName !== undefined ? (
          <ConflictFixPanel conflict={conflict} branchName={branchName} builtOn={builtOn ?? null} owners={owners} />
        ) : (
          <ConflictResolutionHint />
        )}
      </NestedPanel>

      <ConflictingFileListCard conflictInfo={conflict} branchName={branchName} onto={target} />

      {missingCommitCount > 0 && <MissingCommitsDetails conflict={conflict} owners={owners} branchName={branchName} onto={target} builtOn={builtOn} />}
    </div>
  )
}
