import { useRef } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ShieldCheckIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useClosePreview } from "@/hooks/useClosePreview"
import { requestSafePush } from "@/hooks/useSafePush"
import { useStartStep } from "@/hooks/useStackNextStep"
import { formatStackPushing } from "@/lib/stack"
import { transferLabel } from "@/lib/transfer-progress"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { useBranchPushProgress, useStackPush } from "@/stores/branch-sync"
import type { SafePushPlacement } from "@/stores/safe-push"
import { useSafePushStarting } from "@/stores/safe-push"

export interface SafePushButtonProps {
  branch: Branch
  /** `ghost` in the row and the preview footer, `outline` among the buttons of the robot details */
  variant?: "ghost" | "outline"
  /** The key that starts it where the button is (S on the focused row of the branch table), shown on hover */
  shortcut?: string
  /** Without the shield: in the Safe Push slot of the row, the shield of the finished robot stands before it */
  icon?: boolean
  /** Shown instead of the shield: the key of the branch preview (`<Kbd>S</Kbd>`), as its other footer buttons show theirs */
  hint?: ReactNode
  /** What the hover says while it can start, instead of the step's (the stack header: whose and what it lands) */
  description?: string
  /** Where its popover opens: the slot of the row, or of the header of the stack island */
  placement?: SafePushPlacement
  /**
   * The root whose whole stack it starts (`branch` is the root): its step, its start and its progress are the whole
   * stack's (`stackReadinessKey`); the branch's otherwise
   */
  wholeStack?: string
  className?: string
}

/** A click right after the button took the place of another step under the pointer is a click meant for that step */
const STEP_CHANGE_GUARD_MS = 700

/**
 * "Safe Push" ("Safe Push · 3" for a stacked branch, which lands with the branches it is built on), the start of the
 * branch's next step (`getBranchNextStep`): a click starts at once when there is nothing to say, else opens the popover
 * at the slot (`requestSafePush`); the hover says what it lands. While a start runs, its step is the label: "Checking…",
 * "Pushing 45%" ("Pushing 2 branches 45%" for a stack; the stack branch of the whole stack "Pushing 45%"), "Starting…":
 * the one indicator of its push. Disabled with the reason on hover as every surface
 * disables it. Inside the branch preview the preview closes first: the start is the row's. The second click of a
 * double click, and a click on the button that just took the place of another step under the pointer (a poll changed
 * the step), are ignored: they were meant for what was there before.
 */
export function SafePushButton({ branch, variant = "ghost", shortcut, icon = true, hint, description, placement = "row", wholeStack, className }: SafePushButtonProps) {
  const closePreview = useClosePreview()
  const step = useStartStep(branch.name, wholeStack)
  // the start of the whole stack runs under its key, and pushes its stack branch alone
  const key = wholeStack === undefined ? branch.name : stackReadinessKey(wholeStack)
  const starting = useSafePushStarting(key)
  const stackPush = useStackPush(wholeStack === undefined ? branch.name : "")
  // the push of a branch's start reports its progress for each branch of it; the whole stack's under its key
  const pushProgress = useBranchPushProgress(wholeStack === undefined ? (stackPush?.[0] ?? branch.name) : key)
  // until when a click is ignored: the button took the place of another step under a pointer that stayed (null: not mounted yet)
  const guardUntil = useRef<number | null>(null)
  const item = step?.item?.id === "safe-push" ? step.item : null
  const blocker = step?.safePushBlocker ?? null
  const disabled = blocker !== null || starting !== null
  const label =
    starting === null
      ? (item?.label ?? "Safe Push")
      : starting.step === "pushing"
        ? transferLabel(wholeStack === undefined ? formatStackPushing(branch.name, stackPush) : "Pushing", pushProgress)
        : starting.step === "checking" || starting.step === "target"
          ? "Checking…"
          : "Starting…"
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant={variant}
            size="xs"
            disabled={disabled}
            // the hover says why; Base UI keeps the button focusable, so the dimming is ours
            focusableWhenDisabled
            aria-busy={starting !== null || undefined}
            className={cn("tabular-nums", disabled && starting === null && "opacity-50", className)}
            data-testid="safe-push-button"
            ref={element => {
              if (element !== null && guardUntil.current === null) {
                guardUntil.current = element.matches(":hover") ? performance.now() + STEP_CHANGE_GUARD_MS : 0
              }
            }}
            onClick={event => {
              // the row toggles on a click
              event.stopPropagation()
              if (event.detail > 1 || performance.now() < (guardUntil.current ?? 0)) {
                return
              }
              closePreview?.()
              void requestSafePush(key, { mode: "rebaseAutosquash", via: "click", placement })
            }}
          />
        }
      >
        {starting !== null ? <Spinner data-icon="inline-start" /> : (hint ?? (icon && <ShieldCheckIcon data-icon="inline-start" />))}
        {label}
      </TooltipTrigger>
      <TooltipContent>
        {blocker ?? description ?? item?.description}
        {blocker === null && shortcut !== undefined && <Kbd>{shortcut}</Kbd>}
      </TooltipContent>
    </Tooltip>
  )
}
