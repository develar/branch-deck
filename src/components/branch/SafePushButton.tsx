import { ShieldCheckIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { canStartRobot, SAFE_PUSH_DESCRIPTION } from "@/lib/patronus"
import type { Branch } from "@/stores/branch-sync"
import { openInline, useIsRowProcessing } from "@/stores/inline-row-action"
import { useBranchRobot } from "@/stores/patronus"

export interface SafePushButtonProps {
  branch: Branch
  /** `ghost` in the row and the preview footer, `outline` among the buttons of the chip popover */
  variant?: "ghost" | "outline"
  /** Runs before the form opens, e.g. the preview closes: the form is a form of the row */
  onOpen?: () => void
}

/**
 * "Safe Push…": opens the Safe Push form below the row, which checks the code-owner review, offers a merge request and
 * starts the robot; the hover says so. Disabled while a robot runs on the commit the branch is at or the branch is
 * blocked (`canStartRobot`), with the reason on hover, and while the row runs an inline action.
 */
export function SafePushButton({ branch, variant = "ghost", onOpen }: SafePushButtonProps) {
  const robot = useBranchRobot(branch.name)
  const processing = useIsRowProcessing(branch.name)
  const start = canStartRobot(branch, robot)
  const disabled = !start.ok || processing
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button
            variant={variant}
            size="xs"
            disabled={disabled}
            // the hover says why; Base UI keeps the button focusable, so the dimming is ours
            focusableWhenDisabled
            className={cn(disabled && "opacity-50")}
            onClick={(event) => {
              // the row toggles on a click
              event.stopPropagation()
              onOpen?.()
              openInline("safe-push", branch.name)
            }}
          />
        )}
      >
        <ShieldCheckIcon data-icon="inline-start" />
        Safe Push…
      </TooltipTrigger>
      <TooltipContent>{start.ok ? SAFE_PUSH_DESCRIPTION : start.reason}</TooltipContent>
    </Tooltip>
  )
}
