import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import type { NextStepTone } from "@/lib/branch-next-step"
import type { MenuItem } from "@/lib/menu"
import type { Branch } from "@/stores/branch-sync"

/**
 * The next step of a branch row, after the push button, one fixed width in every row, so the push buttons and "⋯" line
 * up down the table: the step of the first row of the [Next Step](../../../spec/branch/next-step.spec.md) table that
 * matches (`getBranchNextStep`), the same that disables "Safe Push…" everywhere else:
 * - "Safe Push…"; "Restart Safe Push" after a failed or canceled robot of the branch as it is; "Safe Push ‹root›…" of a
 *   stacked branch; "Retarget #7 to master" of a pull request into another base; "Create Pull Request…" or "Request
 *   Review…" where the approvals that count are missing;
 * - a quiet step, the state as its label, which opens the pull request it waits in: "Awaiting Ilya Korennoy" (or
 *   "Awaiting 2 Reviews"), "Changes Requested" in `destructive`, "Draft";
 * - nothing while the Status cell says why: a sync, a failed sync, a blocked branch, a robot that runs or merged, a
 *   Safe Push that lands the branch (`PushHold`), a branch that landed.
 * Nothing in a repository that is not on Space.
 */
export interface NextStepSlotProps {
  branch: Branch
  /**
   * The header of a stack island: the Safe Push of the last branch of its main line, whose form leads root first, with
   * the hover of the header; the step of the branch otherwise
   */
  stackHeader?: { description: string; onOpen: () => void }
}

export function NextStepSlot({ branch, stackHeader }: NextStepSlotProps) {
  const step = useBranchNextStep(branch.name)
  if (step === null || step.kind === "hidden") {
    return null
  }
  const header = stackHeader !== undefined && step.kind === "stackRoot"
  const item = header ? null : step.item
  return (
    <div className="flex w-40 items-center" data-testid="next-step-slot" data-step={step.kind} data-stack-path="landing">
      {header && <SafePushButton branch={branch} description={stackHeader.description} onOpen={stackHeader.onOpen} />}
      {item !== null &&
        (item.id === "safe-push" ? (
          <SafePushButton branch={branch} shortcut={stackHeader === undefined ? "S" : undefined} description={stackHeader?.description} onOpen={stackHeader?.onOpen} />
        ) : (
          <StepButton action={item} tone={step.tone} showKeys={stackHeader === undefined} />
        ))}
    </div>
  )
}

const toneClass: Readonly<Record<NextStepTone, string>> = {
  default: "",
  quiet: "text-muted-foreground",
  destructive: "text-destructive hover:text-destructive",
}

/**
 * An action of the branch as a ghost `xs` button of the Actions cell, its label truncated to the slot; the hover says
 * the whole text (why a disabled one is), and the key of the focused row that runs it (O) where the slot is a row's
 */
function StepButton({ action, tone, showKeys }: { action: MenuItem; tone: NextStepTone; showKeys: boolean }) {
  const isDisabled = action.disabled === true
  const Icon = action.icon
  const button = (
    <Button
      variant="ghost"
      size="xs"
      disabled={isDisabled}
      // the hover says why; Base UI keeps the button focusable, so the dimming is ours
      focusableWhenDisabled
      className={cn("max-w-full min-w-0", toneClass[tone], isDisabled && "opacity-50")}
      data-testid="next-step"
      data-tone={tone}
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
  const keys = isDisabled || !showKeys ? [] : (action.kbds ?? [])
  if (action.description === undefined && keys.length === 0) {
    return button
  }
  return (
    <Tooltip>
      <TooltipTrigger render={button} />
      <TooltipContent className="whitespace-pre-line" data-testid="next-step-hover">
        {action.description}
        {keys.map(key => (
          <Kbd key={key}>{key.toUpperCase()}</Kbd>
        ))}
      </TooltipContent>
    </Tooltip>
  )
}
