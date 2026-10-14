import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

export interface ReviewStatusLineProps {
  /** Why the service can't be asked, or null when it answers */
  hint: string | null
  /** "Pull requests checked just now", shown without a hint */
  checkedText: string
  isRefreshing: boolean
  onRefresh: () => void
  testId: string
  refreshTestId: string
}

/** A footer line of the outgoing overview: when a review service was last asked, or a quiet hint why it can't be, and a refresh button */
export function ReviewStatusLine({ hint, checkedText, isRefreshing, onRefresh, testId, refreshTestId }: ReviewStatusLineProps) {
  return (
    <div className="flex items-center justify-between gap-3" data-testid={testId}>
      {hint === null
        ? <span className="min-w-0 truncate text-muted-foreground">{checkedText}</span>
        : (
            // the hint may be truncated: the whole text on hover
            <Tooltip>
              <TooltipTrigger render={<span className="min-w-0 truncate text-muted-foreground" />}>{hint}</TooltipTrigger>
              <TooltipContent>{hint}</TooltipContent>
            </Tooltip>
          )}
      <Button variant="ghost" size="xs" disabled={isRefreshing} data-testid={refreshTestId} onClick={onRefresh}>
        {isRefreshing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
        Refresh
      </Button>
    </div>
  )
}
