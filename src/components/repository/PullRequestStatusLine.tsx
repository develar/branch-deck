import { ReviewStatusLine } from "@/components/repository/ReviewStatusLine"
import { formatAvailabilityHint, getAvailabilityAction } from "@/lib/pull-request"
import { refreshPullRequests, useRepositoryPullRequests } from "@/stores/pull-requests"

/**
 * Footer line of the outgoing overview: when GitHub was last asked, or a quiet hint why it can't be (no GitHub token,
 * a rejected one, offline) with what fixes it ("Sign In…" opens Settings), and a refresh button. Nothing for a
 * repository that is not on GitHub.
 */
export function PullRequestStatusLine() {
  const { availability, fetchedAt, isRefreshing } = useRepositoryPullRequests()
  if (availability === null || availability.status === "notGitHub") {
    return null
  }
  return (
    <ReviewStatusLine
      hint={formatAvailabilityHint(availability)}
      hintAction={getAvailabilityAction(availability)}
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
