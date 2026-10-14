import { lazy, Suspense, useState } from "react"
import { HistoryIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { useHasActivity } from "@/stores/activity"

const loadActivityLog = () => import("@/components/repository/ActivityLog")
const ActivityLog = lazy(() => loadActivityLog().then(it => ({ default: it.ActivityLog })))

/**
 * The header button of Activity, what Branch Deck did in the repository: a click opens the log. Hidden while the log of
 * the repository is empty.
 */
export function ActivityButton() {
  const hasActivity = useHasActivity()
  const [open, setOpen] = useState(false)
  // cleared: the next entry must not reopen the popover
  if (!hasActivity && open) {
    setOpen(false)
  }
  if (!hasActivity) {
    return null
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={(
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Activity"
            data-testid="activity-button"
            // the log is loaded on the way to the click
            onPointerEnter={() => void loadActivityLog()}
            onFocus={() => void loadActivityLog()}
          />
        )}
      >
        <HistoryIcon />
      </PopoverTrigger>
      <PopoverContent align="end" className="w-96 gap-0 overflow-hidden p-0" data-testid="activity-popover">
        <Suspense fallback={null}>
          <ActivityLog onClose={() => setOpen(false)} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}
