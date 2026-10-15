import { ShieldCheckIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useOpenRowForm } from "@/hooks/useOpenRowForm"
import { canStartRobot, SAFE_PUSH_DESCRIPTION } from "@/lib/patronus"
import type { Branch } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"
import { useBranchRobot, useLandingAncestor, useLandingSpan } from "@/stores/patronus"

export interface SafePushButtonProps {
  branch: Branch
  /** `ghost` in the row and the preview footer, `outline` among the buttons of the robot details */
  variant?: "ghost" | "outline"
  /** The key that opens the form where the button is (S on the focused row of the branch table), shown on hover */
  shortcut?: string
  /** Without the shield: in the Safe Push slot of the row, the shield of the finished robot stands before it */
  icon?: boolean
  className?: string
}

/**
 * "Safe Push…": opens the Safe Push form below the row, which checks the code-owner review, offers the pull request or
 * the merge request to open or create and starts the robot; the hover says so. Disabled while a robot runs on the commit
 * the branch is at, a robot lands an ancestor of a stacked branch or the branch with a branch built on it, or the branch
 * is blocked (`canStartRobot`), with the reason on hover, and while the row runs an inline action. Inside the branch
 * preview the preview closes first (`useOpenRowForm`): the form is a form of the row.
 */
export function SafePushButton({ branch, variant = "ghost", shortcut, icon = true, className }: SafePushButtonProps) {
  const openForm = useOpenRowForm()
  const robot = useBranchRobot(branch.name)
  const processing = useIsRowProcessing(branch.name)
  const landingAncestor = useLandingAncestor(branch.name)
  const landingSpan = useLandingSpan(branch.name)
  const start = canStartRobot(branch, robot, landingAncestor, landingSpan)
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
            className={cn(disabled && "opacity-50", className)}
            data-testid="safe-push-button"
            onClick={(event) => {
              // the row toggles on a click
              event.stopPropagation()
              openForm("safe-push", branch.name)
            }}
          />
        )}
      >
        {icon && <ShieldCheckIcon data-icon="inline-start" />}
        Safe Push…
      </TooltipTrigger>
      <TooltipContent>
        {start.ok ? SAFE_PUSH_DESCRIPTION : start.reason}
        {start.ok && shortcut !== undefined && <Kbd>{shortcut}</Kbd>}
      </TooltipContent>
    </Tooltip>
  )
}
