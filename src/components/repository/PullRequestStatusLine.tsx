import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
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
  const hint = formatAvailabilityHint(availability)
  return (
    <div className="flex items-center justify-between gap-3" data-testid="pull-request-status-line">
      <span className="min-w-0 truncate text-muted-foreground" title={hint ?? undefined}>
        {hint ?? `Pull requests checked ${formatRelativeTime(fetchedAt)}`}
      </span>
      <Button
        variant="ghost"
        size="xs"
        disabled={isRefreshing}
        data-testid="pull-request-refresh"
        onClick={() => void refreshPullRequests("manual")}
      >
        {isRefreshing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
        Refresh
      </Button>
    </div>
  )
}
