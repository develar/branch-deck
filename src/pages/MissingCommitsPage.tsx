import { Card } from "@/components/ui/card"
import { ConflictExplanation } from "@/components/conflict/ConflictExplanation"
import { ConflictWindowPage } from "@/components/conflict/ConflictWindowPage"
import { MissingCommitsDetails } from "@/components/conflict/MissingCommitsDetails"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { MissingCommitsWindowData } from "@/types/sub-window"

export function MissingCommitsPage() {
  const data = useSubWindowData<MissingCommitsWindowData>()

  return (
    <ConflictWindowPage
      title="Missing commits"
      data-testid="missing-commits-window"
      commit={
        data && {
          hash: data.conflictCommitHash,
          message: data.conflictCommitMessage,
          authorTime: data.conflictCommitAuthorTime,
          committerTime: data.conflictCommitCommitterTime,
        }
      }
      branchName={data?.branchName}
      onto={data?.onto}
      loadingMessage="Loading missing commits…"
      fixSteps={data?.fixSteps}
      insight={data && <ConflictExplanation owners={data.owners} branchName={data.branchName} builtOn={data.builtOn} revealable={false} />}
    >
      {data &&
        (data.missingCommits.length > 0 ? (
          <MissingCommitsDetails missingCommits={data.missingCommits} owners={data.owners} isInWindow />
        ) : (
          <Card className="gap-0 py-0">
            <p className="bd-padding-card text-sm text-muted-foreground">No missing commits found for this conflict.</p>
          </Card>
        ))}
    </ConflictWindowPage>
  )
}
