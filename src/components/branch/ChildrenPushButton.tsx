import { cn } from "cn"
import { ArrowUpIcon } from "lucide-react"
import { useShallow } from "zustand/react/shallow"
import { Button } from "@/components/ui/button"
import { PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { CompactPushLabel, PushProgressDescription } from "@/components/branch/BranchPushButton"
import { HoverDescription } from "@/components/shared/HoverDescription"
import type { ChildrenPush } from "@/hooks/useBranchPushes"
import { useStackBranchPushAlone } from "@/hooks/useBranchPushes"
import { formatBranchCount } from "@/lib/branch"
import { pushWithChildren } from "@/lib/git/stack-push"
import { formatStackPushLine } from "@/lib/push-up-to"
import { formatAskFirst, formatPushWithChildrenLabel, stackPushSubject } from "@/lib/stack"
import { transferLabel, transferPercent } from "@/lib/transfer-progress"
import { ghostButtonColorClass } from "@/lib/ui-types"
import { STACK_BRANCH_PUSHING, formatStackBranchPushLine, formatWholeStackBranch, stackReadinessKey } from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import { formatPushHoldReason } from "@/presenters/reasons"
import type { StackBranch } from "@/stores/branch-sync"
import { useBranchPushProgress, useBranchSync, useIsStackBranchPushing, useStackBranch, useStackBranchPushProgress, useStackPush } from "@/stores/branch-sync"
import { usePushHold } from "@/stores/patronus"
import { useLandRun } from "@/stores/safe-push"

/** Why the push of a stack without branches to land is disabled: Rust offers no Push with Children of its root */
const NOTHING_TO_PUSH = "Nothing to push: the stack is on the remote"

export interface ChildrenPushButtonProps {
  /** The branch whose Push with Children it is: the root of an island, or a row whose own push has nothing to send */
  root: string
  push: ChildrenPush
  /** A Safe Push runs in the stack: a push won't land (`PushHold` of the root, not one of a merged robot) */
  held: boolean
  testId: string
  descriptionTestId: string
  /**
   * The button of a branch row: "with children" after the count where the view has the room, and the graph circles
   * the branches it sends while it is hovered (`data-stack-path`)
   */
  row?: boolean
}

/**
 * Push with Children of `root` as the compact push of a row: "↑ 41", the commits of the branches stacked on it that have
 * work for the remote, parents first, and the stack branch of the whole stack while it goes along, as Rust orders them
 * (`ChildrenPush`), in one push; a commit the branches and the stack branch both send counts once. The push of the
 * header of an island, and of a row whose own push has nothing to send, so its children are its one push. The hover
 * names it and lists what it sends, and the branches that ask first in their rows; a click pushes and leaves the hover
 * as it is (`HoverDescription`), so it shows the progress while the pointer stays. Disabled, "↑" with the reason in its
 * hover, while it would push nothing beyond the root (Rust's reason, `domain_policy::actions`: the stack is on the
 * remote, the root row's push is the push, the branches with work ask first, or only the stack branch has work and
 * doesn't go along) and while a Safe Push runs in the stack: a push won't land. While a push runs, the spinner and the
 * percent, its name ("Pushing 7 branches 45%"; "Pushing the whole stack 45%" for the stack branch alone) and the line
 * of git on hover: the one indicator of the pushes of the stack; the start of the Safe Push of the whole stack shows its
 * push in its own button, this one disabled meanwhile.
 */
export function ChildrenPushButton({ root, push, held, testId, descriptionTestId, row = false }: ChildrenPushButtonProps) {
  const stackPush = useStackPush(root)
  const current = stackPush === null ? null : stackPushSubject(root, stackPush)
  // the push of the stack branch alone (a step of the restart of its robot, of Request Review…, of the creation of its
  // pull request) shows here as the stack's pushes do; the Safe Push of the whole stack shows it in its own button, the
  // step of its start: one indicator, decided where the root's push button decides it
  // the start of the whole stack pushes its stack branch as a step: no second push of the stack meanwhile
  const startPushes = useLandRun(stackReadinessKey(root))?.step === "pushing"
  const stackBranchPushing = useIsStackBranchPushing(root) || startPushes
  const pushAlone = useStackBranchPushAlone(root)
  const alone = stackPush === null && pushAlone
  const busy = current !== null || alone
  // the one push of the stack reports its progress for each of its branches
  const branchProgress = useBranchPushProgress(stackPush?.[0] ?? root)
  const stackBranchProgress = useStackBranchPushProgress(root)
  const progress = alone ? stackBranchProgress : branchProgress
  const hold = usePushHold(root)
  // Push with Children of the root as Rust offers it (`domain_policy::actions`)
  const rootActions = useRowPart(root, "actions")
  const stack = useStackBranch(root)
  const { children } = push
  const withStackBranch = children?.stackBranch?.kind === "along" && stack !== undefined
  const pushOrder = children?.order ?? []
  const askFirst = children?.askFirst ?? []
  let blocker: string | null = null
  if (!busy) {
    if (held) {
      blocker = hold === null || hold.kind === "landed" ? "A Safe Push runs in the stack" : formatPushHoldReason(root, hold)
    } else {
      blocker = actionBlocker(rootActions, "push-with-children", { subject: root }) ?? (children?.offers === true ? null : NOTHING_TO_PUSH)
    }
  }
  let label = "Push with Children"
  if (current !== null) {
    label = transferLabel(`Pushing ${current}`, progress)
  } else if (alone) {
    label = transferLabel(STACK_BRANCH_PUSHING, progress)
  } else if (blocker === null) {
    label = formatPushWithChildrenLabel(pushOrder, withStackBranch)
  }
  // the stack branch holds the commits of the branches: what the remote lacks of both, each once
  const pushCommits = withStackBranch ? Math.max(push.pushCommits, stack.remoteStatus?.commitsAhead ?? 0) : push.pushCommits
  return (
    <HoverDescription>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <Button
            size="xs"
            variant="ghost"
            className={cn("min-w-16 justify-start tabular-nums", ghostButtonColorClass(blocker === null ? "primary" : "neutral"), !busy && "aria-disabled:opacity-50")}
            // a push of the stack branch runs: Push with Children would take it along
            disabled={busy || blocker !== null || stackBranchPushing}
            // the reason stays reachable on hover while the button is disabled
            focusableWhenDisabled
            aria-busy={busy || undefined}
            aria-label={label}
            data-testid={testId}
            // hovered or focused in a row, the stack graph marks the branches the push sends (`stack-view.ts`)
            data-stack-path={row ? "push" : undefined}
            onClick={event => {
              // the row and the header of an island toggle on a click
              event.stopPropagation()
              void pushWithChildren(root)
            }}
          />
        }
      >
        {row ? (
          <RowLabel busy={busy} percent={transferPercent(progress)} count={blocker === null ? pushCommits : 0} />
        ) : (
          <CompactPushLabel isPushing={busy} percent={transferPercent(progress)} count={blocker === null ? pushCommits : 0} />
        )}
      </PopoverTrigger>
      <PopoverContent data-testid={descriptionTestId} className="w-auto max-w-sm gap-2 p-3 text-sm leading-snug" onClick={event => event.stopPropagation()}>
        {busy && <PushProgressDescription subject={current ?? formatWholeStackBranch(root)} progress={progress} />}
        {!busy && blocker !== null && (
          <>
            <div className="font-medium">{label}</div>
            <div className="break-words">{blocker}</div>
          </>
        )}
        {!busy && blocker === null && (
          <>
            <div className="font-medium">{label}</div>
            <StackPushLines order={pushOrder} stack={withStackBranch ? stack : null} />
            {askFirst.length > 0 && <div className="text-muted-foreground">{`${formatAskFirst(askFirst)}.`}</div>}
          </>
        )}
      </PopoverContent>
    </HoverDescription>
  )
}

/** What the count of a row is: not the commits of the branch, which has none to push; the words where the view has the room */
const WITH_CHILDREN = "hidden text-muted-foreground @4xl/branch-view:inline"

/**
 * "↑ 4 with children" of a row, "⟳ 45% with children" while it pushes. One cell holds it and, invisible, the widest it
 * gets while it pushes ("⟳ 100% with children"), so the click changes no width of the push track: no column moves under
 * the pointer, and the hover of the progress stays open (ui-theme.md, "Live values hold their own width")
 */
function RowLabel({ busy, percent, count }: { busy: boolean; percent: number | null; count: number }) {
  return (
    <span className="grid items-center">
      <span aria-hidden className="invisible col-start-1 row-start-1 flex items-center gap-1">
        <ArrowUpIcon data-icon="inline-start" />
        100%
        <span className={WITH_CHILDREN}>with children</span>
      </span>
      <span className="col-start-1 row-start-1 flex items-center gap-1">
        <CompactPushLabel isPushing={busy} percent={percent} count={count} />
        <span className={WITH_CHILDREN}>with children</span>
      </span>
    </span>
  )
}

/**
 * "Pushes 3 branches, parents first:", a line per push in the words of a push (`formatStackPushLine`), the stack branch of
 * the whole stack last (`formatStackBranchPushLine`)
 */
function StackPushLines({ order, stack }: { order: readonly string[]; stack: StackBranch | null }) {
  const branches = useBranchSync(useShallow(state => order.flatMap(name => (state.branches[name] === undefined ? [] : [formatStackPushLine(state.branches[name])]))))
  const lines = stack === null ? branches : [...branches, formatStackBranchPushLine(stack)]
  const text = lines.length === 1 ? `Pushes ${lines[0]}` : `Pushes ${formatBranchCount(lines.length)}, parents first:\n${lines.join("\n")}`
  return <div className="break-words whitespace-pre-line">{text}</div>
}
