import { CircleCheckIcon, FileDiffIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
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
                    <div className="flex flex-col gap-2 rounded-lg bg-warning/5 p-3 ring-1 ring-warning/30">
                      <div className="flex items-center gap-2">
                        <FileDiffIcon className="size-4 text-warning" />
                        <span className="text-sm font-medium text-foreground">
                          Insight: Missing commits modified the conflicting files
                        </span>
                      </div>

                      <p className="text-sm text-muted-foreground">
                        {"Common ancestor: "}
                        <CommitHashPopover
                          hash={missingCommitsData.mergeBase.hash}
                          subject={missingCommitsData.mergeBase.subject}
                          message={missingCommitsData.mergeBase.message}
                          author={missingCommitsData.mergeBase.author}
                          authorTime={missingCommitsData.mergeBase.time}
                        />
                        {" "}
                        <span className="text-muted-foreground">
                          {`(${missingCommitsData.divergenceSummary.commitsAheadInSource} commits behind source, ${missingCommitsData.divergenceSummary.commitsAheadInTarget} commits behind target)`}
                        </span>
                      </p>
                    </div>
                  </div>
                )}
              </Card>

              {/* Missing commits (main content) */}
              {missingCommitsData.missingCommits.length > 0
                ? <MissingCommitsDetails missingCommits={missingCommitsData.missingCommits} isInWindow />
                : (
                    <Card className="gap-0 py-0">
                      <div className="bd-padding-card">
                        <div className="flex flex-col items-center gap-3 py-8 text-center">
                          <CircleCheckIcon className="size-12 text-success" />
                          <p className="text-sm text-muted-foreground">No missing commits found for this conflict.</p>
                        </div>
                      </div>
                    </Card>
                  )}
            </div>
          )
        : (
            <div className="flex min-h-[400px] items-center justify-center">
              <div className="flex flex-col items-center gap-3 text-center">
                <Spinner className="size-8 text-muted-foreground" />
                <p className="text-sm text-muted-foreground">Loading missing commits data…</p>
              </div>
            </div>
          )}
    </div>
  )
}
