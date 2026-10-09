import { CircleCheckIcon, FileDiffIcon } from "lucide-react"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { MissingCommitsWindowData } from "@/types/sub-window"

export function MissingCommitsPage() {
  const missingCommitsData = useSubWindowData<MissingCommitsWindowData>()

  return (
    <div className="w-full px-4 py-4 sm:px-6 lg:px-8">
      {missingCommitsData
        ? (
            <div className="flex flex-col gap-4">
              {/* Commit Context */}
              <Card className="bg-muted">
                <CardHeader>
                  {/* TODO(Phase 7): <ConflictCommitContext title="Cannot apply commit" ... /> */}
                  <div className="text-sm text-muted-foreground">
                    ConflictCommitContext (TODO Phase 7):
                    {" "}
                    {missingCommitsData.conflictCommitMessage}
                    {" "}
                    on
                    {" "}
                    {missingCommitsData.branchName}
                  </div>
                </CardHeader>

                {/* Divergence Info (matching main view) */}
                {missingCommitsData.mergeBase && (
                  <CardContent>
                    <div className="flex flex-col gap-2 rounded-lg border border-warning bg-muted p-3">
                      <div className="flex items-center gap-2">
                        <FileDiffIcon className="size-4 text-warning" />
                        <span className="text-sm font-medium text-foreground">
                          Insight: Missing commits modified the conflicting files
                        </span>
                      </div>

                      <p className="text-sm text-muted-foreground">
                        Common ancestor:
                        {" "}
                        <CommitHashPopover
                          hash={missingCommitsData.mergeBase.hash}
                          subject={missingCommitsData.mergeBase.subject}
                          message={missingCommitsData.mergeBase.message}
                          author={missingCommitsData.mergeBase.author}
                          authorTime={missingCommitsData.mergeBase.time}
                        />
                        {" "}
                        <span className="text-muted-foreground">
                          (
                          {missingCommitsData.divergenceSummary.commitsAheadInSource}
                          {" "}
                          commits behind source,
                          {" "}
                          {missingCommitsData.divergenceSummary.commitsAheadInTarget}
                          {" "}
                          commits behind target)
                        </span>
                      </p>
                    </div>
                  </CardContent>
                )}
              </Card>

              {missingCommitsData.missingCommits.length > 0
                ? (
                    // TODO(Phase 7): <MissingCommitsDetails missingCommits isInWindow />
                    <div className="text-sm text-muted-foreground">
                      MissingCommitsDetails (TODO Phase 7):
                      {" "}
                      {missingCommitsData.missingCommits.length}
                      {" "}
                      missing commits
                    </div>
                  )
                : (
                    <Card>
                      <CardContent>
                        <div className="flex flex-col items-center gap-3 py-8 text-center">
                          <CircleCheckIcon className="size-12 text-success" />
                          <p className="text-sm text-muted-foreground">
                            No missing commits found for this conflict.
                          </p>
                        </div>
                      </CardContent>
                    </Card>
                  )}
            </div>
          )
        : (
            <div className="flex min-h-[400px] items-center justify-center">
              <div className="flex flex-col items-center gap-3 text-center">
                <Spinner className="size-8 text-muted-foreground" />
                <p className="text-sm text-muted-foreground">Loading missing commits data...</p>
              </div>
            </div>
          )}
    </div>
  )
}
