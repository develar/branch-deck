import { Card } from "@/components/ui/card"
import { ConflictWindowPage } from "@/components/conflict/ConflictWindowPage"
import { DivergenceInsight } from "@/components/conflict/DivergenceInsight"
import { MissingCommitsDetails } from "@/components/conflict/MissingCommitsDetails"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { MissingCommitsWindowData } from "@/types/sub-window"

export function MissingCommitsPage() {
  const data = useSubWindowData<MissingCommitsWindowData>()

  return (
    <ConflictWindowPage
      commit={data && {
        hash: data.conflictCommitHash,
        message: data.conflictCommitMessage,
        authorTime: data.conflictCommitAuthorTime,
        committerTime: data.conflictCommitCommitterTime,
      }}
      branchName={data?.branchName}
      onto={data?.onto}
      loadingMessage="Loading missing commits…"
      insight={data?.mergeBase && (
        <DivergenceInsight missingCommitCount={data.missingCommits.length} mergeBase={data.mergeBase} divergenceSummary={data.divergenceSummary} />
      )}
    >
      {data && (data.missingCommits.length > 0
        ? <MissingCommitsDetails missingCommits={data.missingCommits} isInWindow />
        : (
            <Card className="gap-0 py-0">
              <p className="bd-padding-card text-sm text-muted-foreground">No missing commits found for this conflict.</p>
            </Card>
          ))}
    </ConflictWindowPage>
  )
}
