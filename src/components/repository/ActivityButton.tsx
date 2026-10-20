import { HistoryIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { openActivity } from "@/lib/activity-window"
import { useHasActivity } from "@/stores/activity"

/**
 * The header button of Activity, what Branch Deck did in the repository: a click opens its window (or brings it to the
 * front). Hidden while the log of the repository is empty.
 */
export function ActivityButton() {
  const hasActivity = useHasActivity()
  if (!hasActivity) {
    return null
  }
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label="Activity" data-testid="activity-button" onClick={() => openActivity()} />}>
        <HistoryIcon />
      </TooltipTrigger>
      <TooltipContent>Activity</TooltipContent>
    </Tooltip>
  )
}
