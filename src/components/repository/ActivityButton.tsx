import { HistoryIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { openActivity } from "@/lib/activity-window"

/**
 * The header button of Activity, what Branch Deck did in the repository: a click toggles its side panel (or opens its
 * window where the panel has no room). There while the log is empty too, which it opens ("Nothing yet", as the menus
 * and the palette do), so the first entry moves nothing in the header
 */
export function ActivityButton() {
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label="Activity" data-testid="activity-button" onClick={() => openActivity()} />}>
        <HistoryIcon />
      </TooltipTrigger>
      <TooltipContent>Activity</TooltipContent>
    </Tooltip>
  )
}
