import { useEffect, useRef, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ArrowUpIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { DroppedCommitList } from "@/components/branch/DroppedCommitList"
import { RobotActionButton } from "@/components/branch/PatronusChip"
import { HoverDescription } from "@/components/shared/HoverDescription"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { getRobotItems, useRobotParts } from "@/hooks/useBranchNextStep"
import type { PushButtonAction } from "@/hooks/usePushButton"
import { usePushButton, useStackBranchPushAlone } from "@/hooks/usePushButton"
import { pushBranch, pushFromButton } from "@/lib/git/push"
import { blockingBranchOf, formatStackPushing, stackPushSubject } from "@/lib/stack"
import type { PushProgress } from "@/lib/transfer-progress"
import { stopWaitingPushItem, transferAmountNotInLine, transferLabel, transferPercent } from "@/lib/transfer-progress"
import { ghostButtonColorClass } from "@/lib/ui-types"
import { STACK_BRANCH_PUSHING, formatWholeStackBranch, stackReadinessKey } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { useBranchPushProgress, useIsBranchPushing, useStackPush } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { useObjectBusy, useOperations } from "@/stores/operations"
import { usePushHold } from "@/stores/patronus"
import type { PushConfirmRequest, PushConfirmTarget } from "@/stores/push-confirm"
import { clearPushConfirm, usePushConfirmRequest } from "@/stores/push-confirm"
import { useSafePushStarting } from "@/stores/safe-push"

export interface BranchPushButtonProps {
  branch: Branch
  /** The text takes the push colour of the branch (`getPushColor`); off in the preview footer */
  colored?: boolean
  /** Before the label instead of the upload icon, e.g. the key of the preview footer */
  hint?: ReactNode
  /** The surface whose confirm requests this button answers (`pushOrConfirm`) */
  confirmTarget?: PushConfirmTarget
  /**
   * The branch row: an up arrow and the count ("↑ 3"), the percent while the push runs ("45%"); the label ("Force Push
   * (1)") is its accessible name and leads its description. The preview footer keeps the label.
   */
  compact?: boolean
  /** The key that pushes where the button is (P on the focused row of the branch table), after the label of the description */
  shortcut?: string
  /**
   * The row of an island root: a stack push started from it (Push with Children) and a push of the stack branch of its
   * whole stack show in the island header, the button only the push of the branch itself
   */
  stackPushInHeader?: boolean
  /**
   * Why the button is disabled where it keeps its place without acting (`firstPushWait`: a first push the landing of
   * the branch makes, the answer it waits for); its description says it
   */
  waits?: string | null
}

/**
 * Push button of an active branch ("Push (2)", "Force Push (1)", "Push Stack (3)" for a stacked branch
 * whose ancestors have work for the remote); the full description is shown on hover, also while the button is disabled
 * ("Everything is pushed.", why it is blocked), with the remote commits a force push drops and the other push of a
 * stacked branch (the branch alone, or Push Stack), and the pull requests into a virtual branch it retargets to the
 * baseline first. A push that drops remote commits asks first: the click opens the description with a destructive
 * Force Push, which pushes. While the push runs, the label has its percent ("Pushing 45%", "Pushing 3 branches 45%" for
 * the one push of a stack, "Pushing login 45%" for a stack push of another branch; on the root of a small stack
 * "Pushing the whole stack 45%" for the push of the stack branch of its whole stack alone, unless the start of its Safe
 * Push pushes it, which its button in the slot shows) and the hover the last
 * progress line of git, with the amount and the rate when that line lacks them. A click pushes and leaves the hover as
 * it is (`HoverDescription`); only the question before a push that drops remote commits opens it on the click.
 */
export function BranchPushButton({ branch, colored = true, hint, confirmTarget, compact = false, shortcut, stackPushInHeader = false, waits = null }: BranchPushButtonProps) {
  // a stack push from this row shows what it pushes, unless the island header shows it
  const startedStackPush = useStackPush(branch.name)
  const stackPush = stackPushInHeader ? null : startedStackPush
  const ownPush = useIsBranchPushing(branch.name) || stackPush !== null
  // the push of the stack branch of the whole stack the branch heads, alone: where the pushes of a small stack show (the
  // start of its Safe Push shows its push in the slot)
  const stackBranchPush = useStackBranchPushAlone(branch.name) && !stackPushInHeader && !ownPush
  const isPushing = ownPush || stackBranchPush
  // a Safe Push pushes as the first step of its start: its button in the slot shows the push, this one is disabled
  // without it
  const starting = useSafePushStarting(branch.name)
  const formPushes = starting?.step === "pushing" && compact
  const showsPush = isPushing && !formPushes
  // the one push of a stack reports its progress for each of its branches, the stack branch under its key
  const progress = useBranchPushProgress(stackBranchPush ? stackReadinessKey(branch.name) : (stackPush?.[0] ?? branch.name))
  const processing = useObjectBusy(branch.name) !== null
  const push = usePushButton(branch, isPushing)
  const request = usePushConfirmRequest(branch.name, confirmTarget)
  const disabled = push.disabled || processing || starting !== null || (waits !== null && !isPushing)

  const [open, setOpen] = useState(false)
  // the click on a push that drops remote commits: the description asks before the push
  const [clicked, setClicked] = useState(false)
  // a request (the action of the remote-moved toast, P in the preview) opens the description as that click does
  const [shownRequest, setShownRequest] = useState<PushConfirmRequest | null>(null)
  if (request !== shownRequest) {
    setShownRequest(request)
    if (request !== null) {
      setOpen(true)
    }
  }
  const confirming = push.confirm && !isPushing && (clicked || request !== null)
  const label = showsPush ? transferLabel(stackBranchPush ? STACK_BRANCH_PUSHING : formatStackPushing(branch.name, stackPush), progress) : push.text

  const triggerRef = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    if (request !== null) {
      triggerRef.current?.scrollIntoView({ block: "nearest" })
    }
  }, [request])

  const close = () => {
    setOpen(false)
    setClicked(false)
    if (request !== null) {
      clearPushConfirm(request)
    }
  }

  const confirmPush = () => {
    setClicked(false)
    if (request !== null) {
      clearPushConfirm(request)
    }
    // the description stays open and shows the progress, as after the click on a safe push
    void pushBranch(branch.name, request?.options)
  }

  return (
    <HoverDescription
      open={open}
      onOpenChange={(nextOpen, details) => {
        if (nextOpen) {
          setOpen(true)
        }
        // a confirm waits for its answer: leaving the button keeps it, Escape or a click elsewhere dismisses it
        else if (!confirming || details.reason !== "trigger-hover") {
          close()
        }
      }}
    >
      <PopoverTrigger
        ref={triggerRef}
        openOnHover
        delay={300}
        render={
          <Button
            size="xs"
            variant="ghost"
            // a running push keeps its colour beside the spinner, as Update and Sync do
            // compact: one width in every row ("⟳ 100%" fits), the arrows in a column
            className={cn("tabular-nums", compact && "min-w-16 justify-start", colored && ghostButtonColorClass(push.color), !showsPush && "aria-disabled:opacity-50")}
            disabled={disabled}
            // the description stays reachable on hover while the button is disabled
            focusableWhenDisabled
            aria-busy={showsPush || undefined}
            aria-label={compact ? label : undefined}
            data-testid="push-button"
            // hovered or focused, the stack graph marks the branches the push sends (`stack-view.ts`)
            data-stack-path="push"
            onClick={event => {
              event.stopPropagation()
              if (confirming) {
                // the button again: the confirm closes, nothing is pushed
                event.preventBaseUIHandler()
                close()
              } else if (push.confirm) {
                event.preventBaseUIHandler()
                setClicked(true)
                setOpen(true)
              } else {
                pushFromButton(branch.name)
              }
            }}
          />
        }
      >
        {compact ? (
          <CompactPushLabel isPushing={showsPush} percent={transferPercent(progress)} count={push.count} />
        ) : (
          <>
            {isPushing ? <Spinner data-icon="inline-start" /> : (hint ?? <UploadIcon data-icon="inline-start" />)}
            {label}
          </>
        )}
      </PopoverTrigger>
      <PopoverContent
        data-testid="push-description"
        className="w-auto max-w-sm gap-2 p-3 text-sm leading-snug"
        // React passes a click in the portal up to the branch row, which would toggle it
        onClick={event => event.stopPropagation()}
      >
        {isPushing ? (
          <PushProgressDescription subject={stackBranchPush ? formatWholeStackBranch(branch.name) : stackPushSubject(branch.name, stackPush)} progress={progress} />
        ) : (
          <>
            {/* the compact button shows only the count: the description names the push */}
            {compact && (
              <div className="flex items-center gap-1.5 font-medium" data-testid="push-label">
                {push.text}
                {shortcut !== undefined && !push.disabled && <Kbd>{shortcut}</Kbd>}
              </div>
            )}
            <div className="break-words whitespace-pre-line">{waits ?? push.tooltip}</div>
            {push.dropped !== null && <DroppedCommitList dropped={push.dropped} />}
            {push.retarget !== null && (
              <div data-testid="push-retarget" className={cn("break-words whitespace-pre-line", push.retarget.warning && "text-warning")}>
                {push.retarget.text}
              </div>
            )}
            {confirming && (
              <Button variant="destructive" size="xs" className="self-start" data-testid="push-confirm" disabled={disabled} onClick={confirmPush}>
                <UploadIcon data-icon="inline-start" />
                {push.text}
              </Button>
            )}
            {push.disabled && <PushHeldActions branch={branch} close={close} />}
            {push.action !== null && <PushAlternative action={push.action} disabled={disabled} close={close} />}
            {/* the children's push goes also while the branch's own push has nothing to send */}
            {push.childrenAction !== null && <PushAlternative action={push.childrenAction} disabled={isPushing || processing} close={close} />}
          </>
        )}
      </PopoverContent>
    </HoverDescription>
  )
}

/** The robot actions a push description of a held branch offers, by id: the restart its text names, the robot */
const HOLD_ACTIONS: ReadonlySet<string> = new Set(["restart-robot", "push-and-restart-robot", "open-robot"])

/**
 * What the reason of a push that can't go names, as buttons: "Show auth", the parent a blocked branch waits for; the
 * robot that holds the branch (`PushHold`), its restart while it runs on the branch itself ("…or restart it on the
 * current branch") and "Open in Patronus", from the one list of its actions (`getGatedRobotActions`)
 */
function PushHeldActions({ branch, close }: { branch: Branch; close: () => void }) {
  const hold = usePushHold(branch.name)
  const robotParts = useRobotParts(hold?.robot)
  const operations = useOperations()
  const blocker = blockingBranchOf(branch)
  if (blocker !== null) {
    return (
      <Button
        variant="outline"
        size="xs"
        className="self-start"
        onClick={() => {
          close()
          requestReveal({ branchName: blocker, commitIds: [] })
        }}
      >
        {`Show ${blocker}`}
      </Button>
    )
  }
  if (hold === null) {
    return null
  }
  const own = hold.kind === "own"
  const actions = getRobotItems(hold.robot, operations, robotParts).filter(it => HOLD_ACTIONS.has(it.id) && (own || it.id === "open-robot"))
  return (
    <div className="flex flex-wrap gap-2" data-testid="push-hold-actions">
      {actions.map(action => (
        <RobotActionButton key={action.id} action={action} />
      ))}
    </div>
  )
}

/** The content of the compact button: "↑ 3" (the arrow alone without a count), the spinner and "45%" while pushing */
export function CompactPushLabel({ isPushing, percent, count }: { isPushing: boolean; percent: number | null; count: number }) {
  if (isPushing) {
    return (
      <>
        <Spinner data-icon="inline-start" />
        {percent !== null && `${percent}%`}
      </>
    )
  }
  return (
    <>
      <ArrowUpIcon data-icon="inline-start" />
      {count > 0 && count}
    </>
  )
}

/** Another push the description offers below a rule: its text, then its button */
function PushAlternative({ action, disabled, close }: { action: PushButtonAction; disabled: boolean; close: () => void }) {
  return (
    <div data-testid="push-alternative" className="flex flex-col gap-2 border-t border-border pt-2">
      <span className="break-words">{action.description}</span>
      <Button
        variant="outline"
        size="xs"
        className="self-start"
        disabled={disabled}
        onClick={() => {
          close()
          action.run()
        }}
      >
        <UploadIcon data-icon="inline-start" />
        {action.label}
      </Button>
    </div>
  )
}

/**
 * The last progress line of git, "Pushing feature-auth…" ("Pushing 3 branches…") until git reports a stage, and what was
 * sent so far and how fast when that line doesn't say it
 */
export function PushProgressDescription({ subject, progress }: { subject: string; progress: PushProgress | null }) {
  const amount = transferAmountNotInLine(progress)
  // a push that waits for another holder of the repository lock: whom for, and Stop Waiting
  const stopWaiting = stopWaitingPushItem(progress)
  return (
    <div className="flex flex-col gap-0.5" data-testid="push-progress">
      <span className="break-words">{progress?.line ?? `Pushing ${subject}…`}</span>
      {amount !== null && <span className="text-muted-foreground tabular-nums">{amount}</span>}
      {stopWaiting !== null && (
        <div className="flex justify-end pt-1">
          <MenuItemButton item={stopWaiting} variant="outline" data-testid="stop-waiting" />
        </div>
      )}
    </div>
  )
}
