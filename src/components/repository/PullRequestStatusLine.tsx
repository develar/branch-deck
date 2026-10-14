import { ReviewStatusLine } from "@/components/repository/ReviewStatusLine"
import { formatAvailabilityHint } from "@/lib/pull-request"
import { formatRelativeTime } from "@/lib/time"
import { refreshPullRequests, useRepositoryPullRequests } from "@/stores/pull-requests"

/**
 * Footer line of the outgoing overview: when GitHub was last asked, or a quiet hint why it can't be (gh missing, not
 * logged in, offline), and a refresh button. Nothing for a repository that is not on GitHub.
 */
export function PullRequestStatusLine() {
  const { availability, fetchedAt, isRefreshing } = useRepositoryPullRequests()
  if (availability === null || availability.status === "notGitHub") {
    return null
  }
  return (
    <ReviewStatusLine
      hint={formatAvailabilityHint(availability)}
      checkedText={`Pull requests checked ${formatRelativeTime(fetchedAt)}`}
      isRefreshing={isRefreshing}
      onRefresh={() => void refreshPullRequests("manual")}
      testId="pull-request-status-line"
      refreshTestId="pull-request-refresh"
    />
  )
}
