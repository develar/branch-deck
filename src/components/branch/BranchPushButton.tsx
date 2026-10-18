import { useEffect, useRef, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ArrowUpIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { DroppedCommitList } from "@/components/branch/DroppedCommitList"
import type { PushButtonAction } from "@/hooks/usePushButton"
import { usePushButton } from "@/hooks/usePushButton"
import type { TransferProgress } from "@/lib/bindings"
import { pushBranch, pushFromButton } from "@/lib/git/push"
import { formatStackPushing, stackPushSubject } from "@/lib/stack"
import { transferAmountNotInLine, transferLabel } from "@/lib/transfer-progress"
import { ghostButtonColorClass } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"
import { useBranchPushProgress, useIsBranchPushing, useStackPush } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"
import type { PushConfirmRequest, PushConfirmTarget } from "@/stores/push-confirm"
import { clearPushConfirm, usePushConfirmRequest } from "@/stores/push-confirm"
import { useSafePushStarting } from "@/stores/safe-push"

export interface BranchPushButtonProps {
  branch: Branch
  /** The text takes the push colour of the branch (`getPushColor`); off where a bar carries it (the outgoing overview) */
  colored?: boolean
  /** Before the label instead of the upload icon, e.g. the key of the preview footer */
  hint?: ReactNode
  /** The surface whose confirm requests this button answers (`pushOrConfirm`); none in the outgoing overview */
  confirmTarget?: PushConfirmTarget
  /**
   * The branch row: an up arrow and the count ("↑ 3"), the percent while the push runs ("45%"); the label ("Force Push
   * (1)") is its accessible name and leads its description. The preview footer and the outgoing overview keep the label.
   */
  compact?: boolean
  /** The key that pushes where the button is (P on the focused row of the branch table), after the label of the description */
  shortcut?: string
  /**
   * The row of an island root: a stack push started from it (Push with Children) shows in the island header, the
   * button only the push of the branch itself
   */
  stackPushInHeader?: boolean
}

/**
 * Push button of an active branch ("Push (2)", "Create Remote", "Force Push (1)", "Push Stack (3)" for a stacked branch
 * whose ancestors have work for the remote); the full description is shown on hover, also while the button is disabled
 * ("Everything is pushed.", why it is blocked), with the remote commits a force push drops and the other push of a
 * stacked branch (the branch alone, or Push Stack), and the pull requests into a virtual branch it retargets to the
 * baseline first. A push that drops remote commits asks first: the click opens the description with a destructive
 * Force Push, which pushes. While the push runs, the label has its percent ("Pushing 45%", "Pushing 3 branches 45%" for
 * the one push of a stack, "Pushing login 45%" for a stack push of another branch) and the hover the last progress line
 * of git, with the amount and the rate when that line lacks them.
 */
export function BranchPushButton({ branch, colored = true, hint, confirmTarget, compact = false, shortcut, stackPushInHeader = false }: BranchPushButtonProps) {
  // a stack push from this row shows what it pushes, unless the island header shows it
  const startedStackPush = useStackPush(branch.name)
  const stackPush = stackPushInHeader ? null : startedStackPush
  const isPushing = useIsBranchPushing(branch.name) || stackPush !== null
  // a Safe Push pushes as the first step of its start: its button in the slot shows the push, this one is disabled
  // without it
  const starting = useSafePushStarting(branch.name)
  const formPushes = starting?.step === "pushing" && compact
  const showsPush = isPushing && !formPushes
  // the one push of a stack reports its progress for each of its branches
  const progress = useBranchPushProgress(stackPush?.[0] ?? branch.name)
  const processing = useIsRowProcessing(branch.name)
  const push = usePushButton(branch, isPushing)
  const request = usePushConfirmRequest(branch.name, confirmTarget)
  const disabled = push.disabled || processing || starting !== null

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
  const label = showsPush ? transferLabel(formatStackPushing(branch.name, stackPush), progress) : push.text

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
    <Popover
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
          <CompactPushLabel isPushing={showsPush} percent={progress?.percent ?? null} count={push.count} />
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
          <PushProgressDescription subject={stackPushSubject(branch.name, stackPush)} progress={progress} />
        ) : (
          <>
            {/* the compact button shows only the count: the description names the push */}
            {compact && (
              <div className="flex items-center gap-1.5 font-medium" data-testid="push-label">
                {push.text}
                {shortcut !== undefined && !push.disabled && <Kbd>{shortcut}</Kbd>}
              </div>
            )}
            <div className="break-words whitespace-pre-line">{push.tooltip}</div>
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
            {push.action !== null && <PushAlternative action={push.action} disabled={disabled} close={close} />}
            {/* the children's push goes also while the branch's own push has nothing to send */}
            {push.childrenAction !== null && <PushAlternative action={push.childrenAction} disabled={isPushing || processing} close={close} />}
          </>
        )}
      </PopoverContent>
    </Popover>
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
export function PushProgressDescription({ subject, progress }: { subject: string; progress: TransferProgress | null }) {
  const amount = transferAmountNotInLine(progress)
  return (
    <div className="flex flex-col gap-0.5" data-testid="push-progress">
      <span className="break-words">{progress?.line ?? `Pushing ${subject}…`}</span>
      {amount !== null && <span className="text-muted-foreground tabular-nums">{amount}</span>}
    </div>
  )
}
