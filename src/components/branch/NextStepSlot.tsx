import { lazy, Suspense, useRef } from "react"
import type { ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RequestReviewPopover } from "@/components/branch/RequestReviewPopover"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import type { PullRequest } from "@/lib/bindings"
import type { NextStepTone } from "@/lib/branch-next-step"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { useStackBranch, useWholeStack } from "@/stores/branch-sync"
import { usePullRequest, useStackPullRequest } from "@/stores/pull-requests"
import { setRequestReviewOpen, useRequestReviewOpen } from "@/stores/request-review-popover"
import { useReviewReadiness } from "@/stores/review-readiness"
import type { SafePushAsk } from "@/stores/safe-push"
import { useSafePushAsk, useSafePushStarting } from "@/stores/safe-push"

const SafePushPopover = lazy(() => import("@/components/branch/SafePushPopover").then(it => ({ default: it.SafePushPopover })))

/**
 * The next step of a branch row, after the push button, in the step track of the Actions column, so the steps, the push
 * buttons and "⋯" line up down the table and its islands: the step of the first row of the
 * [Next Step](../../../spec/branch/next-step.spec.md) table that matches (`getBranchNextStep`), the same that disables
 * "Safe Push" everywhere else:
 * - "Safe Push" ("Safe Push · 3" for a stacked branch, which lands what it is built on with it), which starts at once
 *   when it has nothing to say, else opens its popover at the slot (`SafePushPopover`, the anchor of every surface's
 *   start of the branch: the menu, S, the preview, the review window, a toast);
 *   "Restart Safe Push" after a failed or canceled robot of the branch as it is; "Retarget #7 to master" of a pull
 *   request into another base; "Create Pull Request…" or "Request Review…" where the approvals that count are missing;
 * - a quiet step, the state as its label, which opens the pull request it waits in: "Awaiting Ilya Korennoy" (or
 *   "Awaiting 2 Reviews"), "Changes Requested" in `destructive`, "Draft";
 * - nothing while the Status cell says why: a sync, a failed sync, a blocked branch, a robot that runs or merged, a
 *   Safe Push that lands the branch (`PushHold`), a branch that landed;
 * - on the root of a small stack, while the start of its whole stack runs, the steps of that start.
 * Nothing in a repository that is not on Space, but the anchors of Request Review…, which asks GitHub
 * (`RequestReviewAnchors`).
 */
export interface NextStepSlotProps {
  branch: Branch
  /**
   * The header of a stack island: the step of the last branch of its main line, whose Safe Push lands the line in one
   * robot, its popover at the header's slot, with the hover of the header; the step of the row otherwise
   */
  stackHeader?: { description: string }
  /** The row of the root of an island: the Request Review… of its whole stack opens at the island's header, not here */
  wholeStackInHeader?: boolean
}

export function NextStepSlot({ branch, stackHeader, wholeStackInHeader = false }: NextStepSlotProps) {
  const step = useBranchNextStep(branch.name)
  const placement = stackHeader === undefined ? "row" : "stack-header"
  const ask = useSafePushAsk(branch.name, placement)
  const wholeStackAsk = useSafePushAsk(stackReadinessKey(branch.name), "row")
  // the start of the whole stack of a small stack runs: its steps show here, where it was asked, as the header of an
  // island shows them ("Checking…", "Pushing 45%", "Starting…"), in place of the root's own step
  const startsWholeStack = useSafePushStarting(stackHeader === undefined && !wholeStackInHeader ? stackReadinessKey(branch.name) : "") !== null
  const slotRef = useRef<HTMLDivElement>(null)
  if (step === null || step.kind === "hidden") {
    // no step where Patronus doesn't answer; Request Review… of an open pull request asks GitHub, from the menus
    return stackHeader === undefined ? <RequestReviewAnchors branch={branch} wholeStack={!wholeStackInHeader} /> : null
  }
  const item = step.item
  const requestReview = !startsWholeStack && step.kind === "requestReview" && item?.disabled !== true ? item : null
  return (
    <div
      ref={slotRef}
      className="flex min-w-0 items-center"
      data-testid="next-step-slot"
      data-step={step.kind}
      data-waits-for={step.waitsFor ?? undefined}
      data-stack-path="landing"
    >
      {startsWholeStack ? (
        <SafePushButton branch={branch} wholeStack={branch.name} placement="row" compact />
      ) : (
        item !== null &&
        (item.id === "safe-push" ? (
          <SafePushButton branch={branch} shortcut={stackHeader === undefined ? "S" : undefined} description={stackHeader?.description} placement={placement} compact />
        ) : requestReview !== null ? null : (
          <StepButton action={item} tone={step.tone} showKeys={stackHeader === undefined} />
        ))
      )}
      {stackHeader === undefined && <RequestReview branch={branch} step={requestReview} />}
      <SafePushAskPopover ask={ask} anchor={slotRef} />
      {/* the start of the whole stack of a small stack (no island): from the root's menu, at the root's row */}
      {stackHeader === undefined && <SafePushAskPopover ask={wholeStackAsk} anchor={slotRef} />}
      {/* and the Request Review… of its pull request, from "Whole Stack ›" of the root's menu */}
      {stackHeader === undefined && !wholeStackInHeader && <WholeStackRequestReviewAnchor root={branch.name} />}
    </div>
  )
}

/** The popover of a start asked at a slot (`useSafePushAsk`); nothing without one */
export function SafePushAskPopover({ ask, anchor }: { ask: SafePushAsk | null; anchor: RefObject<HTMLElement | null> }) {
  if (ask === null) {
    return null
  }
  return (
    <Suspense fallback={null}>
      <SafePushPopover key={`${ask.mode}:${ask.profileId ?? ""}:${ask.bypassReview === true}`} ask={ask} anchor={anchor} />
    </Suspense>
  )
}

/**
 * The Request Review… popover of the row's open pull request: around the step's button in row 9, else around an
 * anchor at the end of the slot, which the item of the branch menu opens (`openRequestReview`)
 */
function RequestReview({ branch, step }: { branch: Branch; step: MenuItem | null }) {
  return <RequestReviewSlot owner={branch} pullRequest={usePullRequest(branch.name)} step={step} />
}

/**
 * The Request Review… popover of an open pull request at a slot: of a branch, or of the whole stack (`owner` its
 * readiness key and the remote status of its stack branch, the header of its island)
 */
export function RequestReviewSlot({ owner, pullRequest, step }: { owner: Pick<Branch, "name" | "remoteStatus">; pullRequest: PullRequest | undefined; step: MenuItem | null }) {
  const readiness = useReviewReadiness(owner.name)?.readiness ?? null
  const open = useRequestReviewOpen(owner.name)
  if (pullRequest?.state !== "open") {
    return null
  }
  const trigger =
    step === null ? (
      // a target for the popover only: the menu item opens it
      <button type="button" tabIndex={-1} aria-hidden className="size-0 self-end overflow-hidden" data-testid="request-review-anchor" />
    ) : (
      <TooltipTrigger render={stepButton(step, "default", () => {})} />
    )
  const popover = (
    <RequestReviewPopover
      branch={owner}
      pullRequest={pullRequest}
      readiness={readiness}
      trigger={trigger}
      open={open}
      onOpenChange={nextOpen => setRequestReviewOpen(owner.name, nextOpen)}
    />
  )
  return step === null ? (
    popover
  ) : (
    <Tooltip>
      {popover}
      <TooltipContent className="whitespace-pre-line" data-testid="next-step-hover">
        {step.description}
      </TooltipContent>
    </Tooltip>
  )
}

/**
 * The anchors of the Request Review… popovers of a row without a next step (a repository without Patronus): of its
 * pull request and, on the root of a small stack, of the pull request of its whole stack, which the menus open
 * (`openRequestReview`); the push and "⋯" of the row stand where they stand in every other row (`GaplessAnchors`)
 */
function RequestReviewAnchors({ branch, wholeStack }: { branch: Branch; wholeStack: boolean }) {
  const own = usePullRequest(branch.name)?.state === "open"
  const forks = useWholeStack(branch.name)?.kind === "forking"
  const stackPullRequest = useStackPullRequest(branch.name)
  const ofStack = wholeStack && forks && stackPullRequest?.state === "open"
  if (!own && !ofStack) {
    return null
  }
  return (
    <GaplessAnchors>
      <RequestReview branch={branch} step={null} />
      {ofStack && <WholeStackRequestReviewAnchor root={branch.name} />}
    </GaplessAnchors>
  )
}

/**
 * Anchors of popovers in the step track of a row or a header (`ColumnTrack`): without a width, so a track that holds only
 * anchors in every row costs nothing, its gutter neither
 */
export function GaplessAnchors({ children }: { children: ReactNode }) {
  return (
    <div className="flex self-stretch" data-testid="request-review-anchors">
      {children}
    </div>
  )
}

/**
 * The Request Review… popover of the open pull request of the whole stack of `root`, at an anchor of a slot, where no
 * step of the whole stack shows it: the root's row of a small stack (no island), and the header of an island while a
 * Safe Push runs in the stack (its slot is empty). "Request Review…" of the whole stack's actions opens it
 * (`getWholeStackMenuItems`, `openRequestReview` of its key); nothing for a stack that doesn't fork.
 */
export function WholeStackRequestReviewAnchor({ root }: { root: string }) {
  const stack = useStackBranch(root)
  const forks = useWholeStack(root)?.kind === "forking"
  const pullRequest = useStackPullRequest(root)
  if (!forks || stack === undefined) {
    return null
  }
  return <RequestReviewSlot owner={{ name: stackReadinessKey(root), remoteStatus: stack.remoteStatus }} pullRequest={pullRequest} step={null} />
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
export function StepButton({ action, tone, showKeys }: { action: MenuItem; tone: NextStepTone; showKeys: boolean }) {
  const isDisabled = action.disabled === true
  const button = stepButton(action, tone, () => action.onSelect(FROM_MENU))
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

/** The ghost `xs` button of a step, `onClick` its action (none where a popover trigger opens it) */
function stepButton(action: MenuItem, tone: NextStepTone, onClick: () => void) {
  const isDisabled = action.disabled === true
  // the action of the slot runs (a retarget, a restart): its spinner in place of the icon, not dimmed
  const running = action.running === true
  const Icon = action.icon
  return (
    <Button
      variant="ghost"
      size="xs"
      disabled={isDisabled}
      aria-busy={running || undefined}
      // the hover says why; Base UI keeps the button focusable, so the dimming is ours
      focusableWhenDisabled
      className={cn("max-w-full min-w-0", toneClass[tone], isDisabled && !running && "opacity-50")}
      data-testid="next-step"
      data-tone={tone}
      onClick={event => {
        // the row toggles on a click
        event.stopPropagation()
        // the second click of a double click was meant for the step before (a refused start shows the step after it)
        if (event.detail > 1) {
          return
        }
        onClick()
      }}
    >
      {running ? <Spinner data-icon="inline-start" /> : Icon !== undefined && <Icon data-icon="inline-start" />}
      <span className="truncate">{action.compactLabel ?? action.label}</span>
    </Button>
  )
}
