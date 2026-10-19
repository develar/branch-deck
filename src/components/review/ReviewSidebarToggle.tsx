import { cn } from "cn"
import { PanelLeftIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { setReviewSidebar, useAppSettings } from "@/stores/settings/appSettings"

/**
 * Hides or shows the sidebar of a review (the commits and the files) so the diffs take the whole window, as F does;
 * the choice is kept for every review and never changes by itself
 */
export function ReviewSidebarToggle() {
  const shown = useAppSettings(state => state.reviewSidebar)
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Sidebar"
            aria-pressed={shown}
            data-testid="review-sidebar-toggle"
            className={cn(shown && "text-primary hover:text-primary")}
            onClick={() => setReviewSidebar(!shown)}
          />
        }
      >
        <PanelLeftIcon />
      </TooltipTrigger>
      <TooltipContent className="flex items-center gap-1.5">
        {shown ? "Hide the sidebar" : "Show the sidebar"}
        <Kbd>F</Kbd>
      </TooltipContent>
    </Tooltip>
  )
}
