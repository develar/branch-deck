import { useRef } from "react"
import { GaplessAnchors, RequestReviewSlot, SafePushAskPopover, StepButton } from "@/components/branch/NextStepSlot"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { useStackNextStep } from "@/hooks/useStackNextStep"
import { stackReadinessKey } from "@/lib/whole-stack"
import { useBranch, useStackBranch } from "@/stores/branch-sync"
import { useStackPullRequest } from "@/stores/pull-requests"
import { useSafePushAsk } from "@/stores/safe-push"

export interface WholeStackSlotProps {
  /** The root of the island's stack, which forks: its whole stack is its stack branch */
  root: string
  /** The last branch of its main line: while the stack lands in pieces, its Safe Push opens its popover here too */
  leaf: string
}

/**
 * The Safe Push slot of the header of a stack island whose stack forks: the next step of the whole stack
 * (`useStackNextStep`, the step of its stack branch `{prefix}/stack/<root>`), as the slot of a row renders a branch's:
 * "Safe Push · 7" (one robot lands every branch of it, side branches too; the hover says what), the restart of its
 * robot, "Create Pull Request of the Stack…", Request Review… of its pull request, the wait in it. Disabled with the
 * reason while its stack branch is not built ("The stack branch conflicts at …: the stack lands in pieces"). The popover
 * of its start opens here (`stack-header`), and so does the one of the Safe Push of the main line, which lands the stack
 * in pieces.
 */
export function WholeStackSlot({ root, leaf }: WholeStackSlotProps) {
  const next = useStackNextStep(root)
  const key = stackReadinessKey(root)
  const stackAsk = useSafePushAsk(key, "stack-header")
  const pieceAsk = useSafePushAsk(leaf, "stack-header")
  const rootBranch = useBranch(root)
  const stack = useStackBranch(root)
  const pullRequest = useStackPullRequest(root)
  const slotRef = useRef<HTMLDivElement>(null)
  const step = next?.leaf === null ? next.step : null
  if (step === null || rootBranch === undefined) {
    return null
  }
  const owner = { name: key, remoteStatus: stack?.remoteStatus ?? null }
  if (step.kind === "hidden") {
    // no step where Patronus doesn't answer; Request Review… of its pull request asks GitHub, from the "⋯": its anchor
    return pullRequest?.state !== "open" ? null : (
      <GaplessAnchors>
        <RequestReviewSlot owner={owner} pullRequest={pullRequest} step={null} />
      </GaplessAnchors>
    )
  }
  const item = step.item
  const requestReview = step.kind === "requestReview" && item?.disabled !== true ? item : null
  return (
    <div ref={slotRef} className="flex w-40 items-center" data-testid="next-step-slot" data-step={step.kind} data-whole-stack={root}>
      {item !== null &&
        (item.id === "safe-push" ? (
          <SafePushButton branch={rootBranch} wholeStack={root} placement="stack-header" />
        ) : requestReview !== null ? null : (
          <StepButton action={item} tone={step.tone} showKeys={false} />
        ))}
      <RequestReviewSlot owner={owner} pullRequest={pullRequest} step={requestReview} />
      <SafePushAskPopover ask={stackAsk ?? pieceAsk} anchor={slotRef} />
    </div>
  )
}
