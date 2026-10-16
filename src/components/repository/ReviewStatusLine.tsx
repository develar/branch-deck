import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { RelativeTime } from "@/components/shared/RelativeTime"

export interface ReviewStatusLineProps {
  /** Why the service can't be asked, or null when it answers */
  hint: string | null
  /** What fixes it, after the hint: "Sign In…" (`useAvailabilityAction`) */
  hintAction?: { label: string, onClick: () => void } | null
  /** "Pull requests checked", followed by how long ago (`checkedAt`, it ages while shown); shown without a hint */
  checkedLabel: string
  /** After the hint too: "GitHub: offline · checked 5 minutes ago", for rows kept from an earlier answer */
  ageWithHint?: boolean
  /** When the service last answered, `Date.now()` */
  checkedAt: number
  isRefreshing: boolean
  onRefresh: () => void
  /** The accessible name of the Refresh button, naming what it refreshes: "Refresh pull requests"; two such lines sit in one footer */
  refreshLabel: string
  testId: string
  refreshTestId: string
}

/** A footer line of the outgoing overview: when a review service was last asked, or a quiet hint why it can't be, and a refresh button */
export function ReviewStatusLine({ hint, hintAction, checkedLabel, checkedAt, ageWithHint = false, isRefreshing, onRefresh, refreshLabel, testId, refreshTestId }: ReviewStatusLineProps) {
  return (
    <div className="flex items-center justify-between gap-3" data-testid={testId}>
      {/* wraps: a hint is read in full */}
      <span className="min-w-0 text-muted-foreground">
        {hint !== null && hint}
        {(hint === null || (ageWithHint && checkedAt !== 0)) && (
          <>
            {hint === null ? `${checkedLabel} ` : ` · ${checkedLabel.toLowerCase()} `}
            <RelativeTime time={checkedAt} />
          </>
        )}
      </span>
      {hint !== null && hintAction != null && (
        <Button variant="outline" size="xs" className="shrink-0" data-testid={`${testId}-action`} onClick={hintAction.onClick}>{hintAction.label}</Button>
      )}
      <Button variant="ghost" size="xs" disabled={isRefreshing} aria-label={refreshLabel} data-testid={refreshTestId} onClick={onRefresh}>
        {isRefreshing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
        Refresh
      </Button>
    </div>
  )
}
