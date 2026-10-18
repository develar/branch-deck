import { ReviewStatusLine } from "@/components/repository/ReviewStatusLine"
import { useAvailabilityAction } from "@/hooks/useAvailabilityAction"
import { formatAvailabilityHint } from "@/lib/pull-request"
import { refreshPullRequests, useRepositoryPullRequests } from "@/stores/pull-requests"

/**
 * Footer line of the landing overview: when GitHub was last asked, or a quiet hint why it can't be (no GitHub token,
 * a rejected one, offline) with what fixes it ("Sign In…", or "Settings…" in a build without the sign-in, opens
 * Settings), and a refresh button. Nothing for a repository that is not on GitHub.
 */
export function PullRequestStatusLine() {
  const { availability, fetchedAt, isRefreshing } = useRepositoryPullRequests()
  const hintAction = useAvailabilityAction(availability)
  if (availability === null || availability.status === "notGitHub") {
    return null
  }
  return (
    <ReviewStatusLine
      hint={formatAvailabilityHint(availability)}
      hintAction={hintAction}
      checkedLabel="Pull requests checked"
      checkedAt={fetchedAt}
      isRefreshing={isRefreshing}
      onRefresh={() => void refreshPullRequests("manual")}
      refreshLabel="Refresh pull requests"
      testId="pull-request-status-line"
      refreshTestId="pull-request-refresh"
    />
  )
}
