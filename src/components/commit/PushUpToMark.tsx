import { useState } from "react"
import { ArrowUpIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { pushUpTo } from "@/lib/push-run"
import type { PushUpTo } from "@/presenters/push"
import { describePushUpTo, pushUpToIn } from "@/presenters/push"
import { useBranchSync, useStackPush } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { showPushUpTo } from "@/stores/stack-view"

/**
 * What Push Up to Here of the commit pushes as the view has it, and in what words (`pushUpToIn`), with the copy of the
 * commit it pushes; null for a commit the remote has or a merged Safe Push landed
 */
function usePushUpTo(branchName: string, originalHash: string): { plan: PushUpTo; description: string | null; hash: string } | null {
  // a string, so the store notifies only when the plan changes
  const key = useBranchSync(state => {
    const hash = state.branches[branchName]?.commits.find(commit => commit.originalHash === originalHash)?.hash
    const plan = hash === undefined ? null : pushUpToIn(state, branchName, hash)
    if (plan === null) {
      return ""
    }
    return JSON.stringify({ plan, hash, description: plan.ok ? describePushUpTo(plan.steps, name => state.branches[name]) : null })
  })
  return key === "" ? null : (JSON.parse(key) as { plan: PushUpTo; description: string | null; hash: string })
}

export interface PushUpToMarkProps {
  branchName: string
  originalHash: string
}

/**
 * The push arrow after the subject of a commit the remote lacks (`pushUpToCommitsIn`), in the commit list of a branch: a
 * click opens what Push Up to Here of it sends, the branches it is built on first ("Pushes 3 branches, parents first:",
 * a line per push, "login · 2 of 5 commits"), or why it can't, and the Push Up to Here button, whose hover marks the
 * nodes and the commits it sends
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
            // a click on the row selects its commit
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
  const pushing = useStackPush(branchName) !== null
  if (pushUp === null) {
    return <p className="text-muted-foreground">The remote has this commit.</p>
  }
  const { plan, description, hash } = pushUp
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
          // a sync that rebuilt the branch since gave the commit another id: the plan refuses it, nothing goes out
          void pushUpTo(branchName, hash)
        }}
        data-testid="push-up-to-button"
      >
        <UploadIcon data-icon="inline-start" />
        Push Up to Here
      </Button>
    </div>
  )
}
