import { FileDiffIcon } from "lucide-react"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import { Callout } from "@/components/shared/Callout"
import { formatCommitCount } from "@/lib/commit"
import type { MergeBase } from "@/lib/merge-conflict"

export interface DivergenceInsightProps {
  missingCommitCount: number
  mergeBase: MergeBase
  divergenceSummary: {
    commitsAheadInSource: number
    commitsAheadInTarget: number
  }
}

/**
 * Why a commit conflicts: the missing commits that modified the conflicting files, and how far the source and the target
 * moved from their common ancestor. The expanded row and the missing-commits window show the same callout.
 */
export function DivergenceInsight({ missingCommitCount, mergeBase, divergenceSummary }: DivergenceInsightProps) {
  const title = missingCommitCount === 0
    ? "No missing commit modified the conflicting files"
    : `${missingCommitCount} missing ${missingCommitCount === 1 ? "commit" : "commits"} modified the conflicting files`
  return (
    <Callout tone="warning" icon={FileDiffIcon} title={title} data-testid="divergence-insight">
      <p>
        {"Common ancestor "}
        <CommitHashPopover hash={mergeBase.hash} subject={mergeBase.subject} message={mergeBase.message} author={mergeBase.author} authorTime={mergeBase.time} />
        {`: ${formatCommitCount(divergenceSummary.commitsAheadInSource)} behind the source, ${divergenceSummary.commitsAheadInTarget} behind the target`}
      </p>
    </Callout>
  )
}
