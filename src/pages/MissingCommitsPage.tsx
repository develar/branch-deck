import { FileDiffIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import { Callout } from "@/components/shared/Callout"
import { ConflictCommitContext } from "@/components/conflict/ConflictCommitContext"
import { ConflictResolutionHint } from "@/components/conflict/ConflictResolutionHint"
import { MissingCommitsDetails } from "@/components/conflict/MissingCommitsDetails"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { MissingCommitsWindowData } from "@/types/sub-window"

export function MissingCommitsPage() {
  const missingCommitsData = useSubWindowData<MissingCommitsWindowData>()

  return (
    <div className="w-full px-4 py-4 sm:px-6 lg:px-8">
      {missingCommitsData
        ? (
            <div className="flex flex-col gap-4">
              {/* Commit context */}
              <Card className="gap-0 py-0">
                <div className="flex flex-col gap-4 p-4 sm:px-6">
                  <ConflictCommitContext
                    title="Cannot apply commit"
                    commitMessage={missingCommitsData.conflictCommitMessage}
                    commitHash={missingCommitsData.conflictCommitHash}
                    commitAuthorTime={missingCommitsData.conflictCommitAuthorTime}
                    commitCommitterTime={missingCommitsData.conflictCommitCommitterTime}
                    branchName={missingCommitsData.branchName}
                  />
                  <ConflictResolutionHint />
                </div>

                {/* Divergence insight (matching the main view) */}
                {missingCommitsData.mergeBase && (
                  <div className="border-t border-border bd-padding-card">
                    <Callout tone="warning" icon={FileDiffIcon} title="Insight: Missing commits modified the conflicting files">
                      <p>
                        {"Common ancestor: "}
                        <CommitHashPopover
                          hash={missingCommitsData.mergeBase.hash}
                          subject={missingCommitsData.mergeBase.subject}
                          message={missingCommitsData.mergeBase.message}
                          author={missingCommitsData.mergeBase.author}
                          authorTime={missingCommitsData.mergeBase.time}
                        />
                        {" "}
                        {`(${missingCommitsData.divergenceSummary.commitsAheadInSource} commits behind source, ${missingCommitsData.divergenceSummary.commitsAheadInTarget} commits behind target)`}
                      </p>
                    </Callout>
                  </div>
                )}
              </Card>

              {/* Missing commits (main content) */}
              {missingCommitsData.missingCommits.length > 0
                ? <MissingCommitsDetails missingCommits={missingCommitsData.missingCommits} isInWindow />
                : (
                    <Card className="gap-0 py-0">
                      <p className="bd-padding-card text-sm text-muted-foreground">No missing commits found for this conflict.</p>
                    </Card>
                  )}
            </div>
          )
        : (
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <Spinner className="size-4" />
              Loading missing commits…
            </p>
          )}
    </div>
  )
}
