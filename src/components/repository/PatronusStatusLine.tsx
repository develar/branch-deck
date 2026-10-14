import { ReviewStatusLine } from "@/components/repository/ReviewStatusLine"
import { formatPatronusAvailabilityHint } from "@/lib/patronus"
import { refreshPatronus, useRepositoryPatronus } from "@/stores/patronus"

/**
 * Footer line of the outgoing overview: when Patronus was last asked, or a quiet hint why it can't be (the Safe Push
 * CLI is not logged in, Patronus fails), and a refresh button (the last 7 days). Nothing for a repository that is not
 * on Space.
 */
export function PatronusStatusLine() {
  const { availability, fetchedAt, isRefreshing } = useRepositoryPatronus()
  if (availability === null || availability.status === "notPatronus") {
    return null
  }
  return (
    <ReviewStatusLine
      hint={formatPatronusAvailabilityHint(availability)}
      checkedLabel="Patronus checked"
      checkedAt={fetchedAt}
      isRefreshing={isRefreshing}
      onRefresh={() => void refreshPatronus("manual")}
      testId="patronus-status-line"
      refreshTestId="patronus-refresh"
    />
  )
}
