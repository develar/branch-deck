import { useLayoutEffect, useRef } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { CircleHelpIcon, ShieldCheckIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { useClosePreview } from "@/hooks/useClosePreview"
import { requestSafePush } from "@/hooks/useSafePush"
import { useStackNextStep } from "@/hooks/useStackNextStep"
import type { MenuItem } from "@/lib/menu"
import { formatStackPushing } from "@/lib/stack"
import { transferLabel } from "@/lib/transfer-progress"
import { stackReadinessKey } from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import { getStartActionItems } from "@/presenters/actions"
import type { Branch } from "@/stores/branch-sync"
import type { SafePushPlacement } from "@/stores/safe-push"
import { stopStart, useLandProgress, useLandRun } from "@/stores/safe-push"

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
  /**
   * In the step column of a row or of an island header: as wide while a start runs as at rest, so a click moves no
   * column; its push shows as the compact push of the row does, the spinner and "45%", "Pushing 45%" (the branches
   * it pushes too) its accessible name and its hover
   */
  compact?: boolean
  className?: string
}

/** A click right after the button took the place of another step under the pointer is a click meant for that step */
const STEP_CHANGE_GUARD_MS = 700

/** Stop Waiting of a start that waits for another holder of the repository lock (`waiting` names whom for) */
function stopStartItem(op: string, waiting: string): MenuItem {
  return { id: "stop-waiting", label: "Stop Waiting", description: `${waiting}; stop and push nothing`, onSelect: () => void stopStart(op) }
}

/**
 * "Safe Push" ("Safe Push · 3" for a stacked branch, which lands with the branches it is built on), the start of the
 * branch's next step (`getBranchNextStep`): a click starts at once when there is nothing to say, else opens the popover
 * at the slot (`requestSafePush`); the hover says what it lands. While a start runs, its step is the label: "Checking…",
 * "Pushing 45%" ("Pushing 2 branches 45%" for a stack; the stack branch of the whole stack "Pushing 45%"), "Starting…":
 * the one indicator of its push; compact, the spinner and "45%", as the compact push of the row. Disabled with the reason on hover as every surface
 * disables it. Inside the branch preview the preview closes first: the start is the row's. The second click of a
 * double click, and a click on the button that just took the place of another step under the pointer (a poll changed
 * the step), are ignored: they were meant for what was there before.
 */
export function SafePushButton({
  branch,
  variant = "ghost",
  shortcut,
  icon = true,
  hint,
  description,
  placement = "row",
  wholeStack,
  compact = false,
  className,
}: SafePushButtonProps) {
  const closePreview = useClosePreview()
  const branchStep = useBranchNextStep(wholeStack === undefined ? branch.name : "")
  // of a whole stack that forks, the step of its stack branch; a straight chain's is its last branch's, whose button that is
  const stackStep = useStackNextStep(wholeStack ?? "")
  const step = wholeStack === undefined ? branchStep : stackStep?.leaf === null ? stackStep.step : null
  // the start of the whole stack runs under its key, and pushes its stack branch alone
  const key = wholeStack === undefined ? branch.name : stackReadinessKey(wholeStack)
  const starting = useLandRun(key)
  // how far its push is, as the channel of this window's start tells it
  const progress = useLandProgress(starting?.op ?? null)
  // until when a click is ignored: the button took the place of another step under a pointer that stayed (null: not mounted yet)
  const guardUntil = useRef<number | null>(null)
  const item = step?.item?.id === "safe-push" ? step.item : null
  const blocker = step?.safePushBlocker ?? null
  const disabled = blocker !== null || starting !== null
  const pushing = starting?.step === "pushing" ? transferLabel(wholeStack === undefined ? formatStackPushing(branch.name, starting.pushes) : "Pushing", progress) : null
  // whom its wait for the repository lock is for, as the view names the holder: every window says it
  const waiting = starting?.waitingFor ?? null
  // another process runs the start (an agent's `bd safe-push`): its words take the place of its step
  const elsewhere = starting?.by ?? null
  // a start whose answer was lost: its actions beside the slot, as Rust offers them
  const lost = starting?.lost ?? null
  const lostOffered = useRowPart(lost === null ? "" : `op:${lost.id}`, "actions")
  const lostItems = lost === null ? [] : getStartActionItems(lost, lostOffered)
  const label =
    starting === null
      ? (item?.label ?? "Safe Push")
      : elsewhere?.unknown === true
        ? elsewhere.label
        : elsewhere !== null
          ? `${elsewhere.label}…`
          : pushing !== null
            ? compact
              ? progress === null
                ? null
                : `${progress.percent}%`
              : pushing
            : starting.step === "waiting"
              ? "Waiting…"
              : starting.step === "checking"
                ? "Checking…"
                : "Starting…"
  const buttonRef = useRef<HTMLButtonElement | null>(null)
  const restWidth = useRef<number | null>(null)
  const busy = starting !== null
  // compact: the width at rest holds while a start runs, so a click moves no column ("Safe Push · 3" is wider than
  // "Checking…", the minimum covers "Safe Push", narrower than "Checking…")
  useLayoutEffect(() => {
    const button = buttonRef.current
    if (!compact || button === null) {
      return
    }
    if (busy) {
      button.style.minWidth = restWidth.current === null ? "" : `${restWidth.current}px`
    } else {
      button.style.minWidth = ""
      restWidth.current = button.getBoundingClientRect().width
    }
  }, [compact, busy, label])
  // it waits for another holder of the repository lock: Stop Waiting beside it, the hover says whom for
  const stopWaiting = waiting !== null && starting?.op != null && elsewhere === null ? stopStartItem(starting.op, waiting) : null
  const button = (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant={variant}
            size="xs"
            disabled={disabled}
            // the hover says why; Base UI keeps the button focusable, so the dimming is ours
            focusableWhenDisabled
            aria-busy={(starting !== null && lost === null) || undefined}
            // the push a compact label says by its percent only, the wait by whom it is for
            aria-label={waiting ?? (compact && pushing !== null && elsewhere === null ? pushing : undefined)}
            className={cn("tabular-nums", compact && "min-w-26 justify-start", disabled && starting === null && "opacity-50", className)}
            data-testid="safe-push-button"
            ref={element => {
              buttonRef.current = element
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
        {lost !== null ? (
          <CircleHelpIcon data-icon="inline-start" />
        ) : starting !== null ? (
          <Spinner data-icon="inline-start" />
        ) : (
          (hint ?? (icon && <ShieldCheckIcon data-icon="inline-start" />))
        )}
        {label}
      </TooltipTrigger>
      <TooltipContent>
        {/* the branches a compact label leaves out: "Pushing 2 branches 45%"; whom the wait is for */}
        {waiting ?? elsewhere?.description ?? (pushing !== null && pushing !== label ? pushing : (blocker ?? description ?? item?.description))}
        {blocker === null && shortcut !== undefined && <Kbd>{shortcut}</Kbd>}
      </TooltipContent>
    </Tooltip>
  )
  // the actions beside the slot are no label the cap of the step track truncates: the track takes their width
  if (lostItems.length > 0) {
    return (
      <span className="inline-flex items-center gap-1" data-beside-slot="">
        {button}
        {lostItems.map(lostItem => (
          <MenuItemButton key={lostItem.id} item={lostItem} variant="outline" data-testid={lostItem.id} />
        ))}
      </span>
    )
  }
  if (stopWaiting === null) {
    return button
  }
  return (
    <span className="inline-flex items-center gap-1" data-beside-slot="">
      {button}
      <MenuItemButton item={stopWaiting} variant="outline" data-testid="stop-waiting" />
    </span>
  )
}
