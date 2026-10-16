import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { getPullRequestItem } from "@/hooks/useBranchContextActions"
import { useRowRobot } from "@/hooks/useRowRobot"
import type { PatronusRobot } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import { getRowRobotStep } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import type { Branch } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"
import { usePushHold } from "@/stores/patronus"
import { usePullRequest } from "@/stores/pull-requests"

/**
 * The Safe Push slot of a branch row, after the push button, one fixed width in every row, so the push buttons and "⋯"
 * line up down the table: the next step after the robot of the branch, which the Status cell shows (`getRowRobotStep`):
 * - "Safe Push…" without a robot, for one of an earlier state of the branch, and after a dry run;
 * - "Restart Safe Push" after a failed or canceled robot of the branch as it is, "Safe Push…" where Patronus can't
 *   restart it (a Safe Push of a temporary ref);
 * - "Open Pull Request", or "Create Pull Request", after a robot that needs the code-owner review: the approvals count
 *   in the pull request;
 * - nothing while a robot runs (Cancel is in "⋯" and the details of the robot), while a Safe Push of a branch built on
 *   this one lands it (`PushHold`, its Status cell says "lands with oauth"), and after a merge.
 * Nothing in a repository that is not on Space.
 */
export interface SafePushSlotProps {
  branch: Branch
  /** The hover of "Safe Push…" while it can start (`SafePushButton`) */
  description?: string
  /** Opens the Safe Push form instead of the row's own (`SafePushButton`) */
  onOpen?: () => void
}

export function SafePushSlot({ branch, description, onOpen }: SafePushSlotProps) {
  const { available, robot, slot, needsReview } = useRowRobot(branch)
  const hold = usePushHold(branch.name)
  if (!available && slot.kind === "none") {
    return null
  }
  const step = hold === null ? getRowRobotStep(slot) : "none"
  return (
    <div className="flex w-40 items-center" data-testid="safe-push-slot" data-step={step} data-stack-path="landing">
      {step === "safePush" && <SafePushButton branch={branch} shortcut={onOpen === undefined ? "S" : undefined} description={description} onOpen={onOpen} />}
      {step === "restart" && robot !== undefined && <RestartStep branch={branch} robot={robot} needsReview={needsReview} />}
      {step === "pullRequest" && <PullRequestStep branch={branch} />}
    </div>
  )
}

/** "Restart Safe Push" of the robot, as its details and the branch menu offer it (`getRobotActions`) */
function RestartStep({ branch, robot, needsReview }: { branch: Branch; robot: PatronusRobot; needsReview: boolean }) {
  const processing = useIsRowProcessing(branch.name)
  const restart = getRobotActions(robot, branch, needsReview).find(action => action.id === "restart-robot" || action.id === "push-and-restart-robot")
  return restart === undefined ? <SafePushButton branch={branch} shortcut="S" /> : <StepButton action={restart} disabled={processing} />
}

/** "Open Pull Request" or "Create Pull Request", as the branch menu names them; "Safe Push…" off GitHub */
function PullRequestStep({ branch }: { branch: Branch }) {
  // subscribed, so the step follows a pull request opened meanwhile
  usePullRequest(branch.name)
  const item = getPullRequestItem(branch)
  return item === null ? <SafePushButton branch={branch} shortcut="S" /> : <StepButton action={item} />
}

/**
 * An action of the branch as a ghost `xs` button of the Actions cell; the hover says why a disabled one is, and the key
 * of the focused row that runs it (O)
 */
function StepButton({ action, disabled = false }: { action: MenuItem; disabled?: boolean }) {
  const isDisabled = action.disabled === true || disabled
  const Icon = action.icon
  const button = (
    <Button
      variant="ghost"
      size="xs"
      disabled={isDisabled}
      // the hover says why; Base UI keeps the button focusable, so the dimming is ours
      focusableWhenDisabled
      className={cn("min-w-0", isDisabled && "opacity-50")}
      data-testid="safe-push-step"
      onClick={event => {
        // the row toggles on a click
        event.stopPropagation()
        action.onSelect()
      }}
    >
      {Icon !== undefined && <Icon data-icon="inline-start" />}
      <span className="truncate">{action.label}</span>
    </Button>
  )
  const keys = isDisabled ? [] : (action.kbds ?? [])
  if (action.description === undefined && keys.length === 0) {
    return button
  }
  return (
    <Tooltip>
      <TooltipTrigger render={button} />
      <TooltipContent>
        {action.description}
        {keys.map(key => (
          <Kbd key={key}>{key.toUpperCase()}</Kbd>
        ))}
      </TooltipContent>
    </Tooltip>
  )
}
