import { ReviewStatusLine } from "@/components/repository/ReviewStatusLine"
import { copyText } from "@/lib/clipboard"
import { formatPatronusAvailabilityHint, formatSafePushLoginCommand } from "@/lib/patronus"
import { isWindows } from "@/lib/platform"
import { refreshPatronus, useRepositoryPatronus } from "@/stores/patronus"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * Footer line of the landing overview: when Patronus was last asked, or a quiet hint why it can't be (the Safe Push
 * CLI is not logged in, with Copy Command; Patronus fails), and a refresh button (the last 7 days). Nothing for a
 * repository that is not on Space.
 */
export function PatronusStatusLine() {
  const { availability, fetchedAt, isRefreshing } = useRepositoryPatronus()
  const repositoryPath = useSelectedProjectPath()
  if (availability === null || availability.status === "notPatronus") {
    return null
  }
  // the hint names a command: it copies, into the repository, ready to paste
  const hintAction =
    availability.status === "notLoggedIn" && repositoryPath !== undefined
      ? { label: "Copy Command", onClick: () => void copyText(formatSafePushLoginCommand(repositoryPath, isWindows)) }
      : null
  return (
    <ReviewStatusLine
      hint={formatPatronusAvailabilityHint(availability)}
      hintAction={hintAction}
      checkedLabel="Patronus checked"
      checkedAt={fetchedAt}
      isRefreshing={isRefreshing}
      onRefresh={() => void refreshPatronus("manual")}
      refreshLabel="Refresh Patronus"
      testId="patronus-status-line"
      refreshTestId="patronus-refresh"
    />
  )
}
