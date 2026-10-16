import { useState } from "react"
import { ArrowUpIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { pushUpTo } from "@/lib/git/push"
import type { PushUpToPlan } from "@/lib/push-up-to"
import { describePushUpTo, pushUpToPlan } from "@/lib/push-up-to"
import { useBranchSync, useStackPushCurrent } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { usePushHolds } from "@/stores/patronus"
import { showPushUpTo } from "@/stores/stack-view"

/** What Push Up to Here of the commit pushes, and in what words (`pushUpToPlan`); null for a commit the remote has */
function usePushUpTo(branchName: string, originalHash: string): { plan: PushUpToPlan; description: string | null } | null {
  const holds = usePushHolds()
  // a string, so the store notifies only when the plan changes
  const key = useBranchSync(state => {
    const branchOf = (name: string) => state.branches[name]
    const plan = pushUpToPlan(branchName, originalHash, { branchOf, landingOf: name => holds.get(name)?.branch ?? null })
    if (plan === null) {
      return ""
    }
    return JSON.stringify({ plan, description: plan.ok ? describePushUpTo(plan.steps, branchOf) : null })
  })
  return key === "" ? null : (JSON.parse(key) as { plan: PushUpToPlan; description: string | null })
}

export interface PushUpToMarkProps {
  branchName: string
  originalHash: string
}

/**
 * The push arrow of a commit the remote lacks: a click opens what Push Up to Here of it sends, the branches it is built
 * on first ("Pushes 3 branches, parents first:", a line per push, "login · 2 of 5 commits"), or why it can't, and the
 * Push Up to Here button, whose hover marks the nodes and the commits it sends
 */
export function PushUpToMark({ branchName, originalHash }: PushUpToMarkProps) {
  const [open, setOpen] = useState(false)
  return (
    <Popover
      open={open}
      onOpenChange={next => {
        setOpen(next)
        if (!next) {
          showPushUpTo(null)
        }
      }}
    >
      <PopoverTrigger
        render={
          <button
            type="button"
            className="flex size-4 items-center justify-center rounded-lg text-primary outline-none hover:bg-primary/10 focus-visible:ring-3 focus-visible:ring-ring/30"
            aria-label="Push Up to Here…"
            data-testid="commit-push-mark"
            // the row toggles its commits on click
            onClick={event => event.stopPropagation()}
          />
        }
      >
        <ArrowUpIcon className="size-3" />
      </PopoverTrigger>
      <PopoverContent align="start" className="w-96" data-testid="push-up-to">
        {open && <PushUpToContent branchName={branchName} originalHash={originalHash} onDone={() => setOpen(false)} />}
      </PopoverContent>
    </Popover>
  )
}

function PushUpToContent({ branchName, originalHash, onDone }: PushUpToMarkProps & { onDone: () => void }) {
  const pushUp = usePushUpTo(branchName, originalHash)
  const pushing = useStackPushCurrent(branchName) !== null
  if (pushUp === null) {
    return <p className="text-muted-foreground">The remote has this commit.</p>
  }
  const { plan, description } = pushUp
  if (!plan.ok) {
    const show = plan.show
    return (
      <div className="flex flex-col gap-2">
        <p data-testid="push-up-to-reason">{plan.reason}</p>
        {show !== undefined && show !== branchName && (
          <Button
            variant="link"
            size="xs"
            className="h-auto self-start px-0"
            onClick={() => {
              onDone()
              requestReveal({ branchName: show, commitIds: [] })
            }}
          >
            {`Show ${show}`}
          </Button>
        )}
      </div>
    )
  }
  return (
    <div className="flex flex-col gap-3">
      <p className="whitespace-pre-line" data-testid="push-up-to-plan">
        {description}
      </p>
      <Button
        size="sm"
        className="self-end"
        disabled={pushing}
        onPointerEnter={() => showPushUpTo(plan.steps)}
        onPointerLeave={() => showPushUpTo(null)}
        onFocus={() => showPushUpTo(plan.steps)}
        onBlur={() => showPushUpTo(null)}
        onClick={() => {
          showPushUpTo(null)
          onDone()
          // a sync that rebuilt the branch since gave the commit another id: the push refuses it, nothing goes out
          void pushUpTo(plan.steps)
        }}
        data-testid="push-up-to-button"
      >
        <UploadIcon data-icon="inline-start" />
        Push Up to Here
      </Button>
    </div>
  )
}
