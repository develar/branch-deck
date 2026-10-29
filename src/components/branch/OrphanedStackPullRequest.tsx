import { useContext } from "react"
import { cn } from "cn"
import { GitPullRequestClosedIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ClosePreviewContext } from "@/hooks/useClosePreview"
import { useOperationWords } from "@/hooks/useOperationWords"
import { useOrphanedStackPullRequest } from "@/hooks/useOrphanedStackPullRequest"
import { CLOSE_STACK_PULL_REQUEST, getCloseStackPullRequestItem } from "@/hooks/useWholeStackActions"
import { stackReadinessKey } from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import { useStackPullRequest } from "@/stores/pull-requests"

/**
 * "#12 no longer matches the air stack: auth landed" in the header of the stack island: the pull request of the whole
 * stack shows a stack that is no more; "Close Pull Request #12…" beside the header's actions (the line is inside its
 * toggle) and in the "⋯", asked
 */
export function OrphanedStackPullRequestLine({ root, className }: { root: string; className?: string }) {
  const text = useOrphanedStackPullRequest(root)
  if (text === null) {
    return null
  }
  return (
    <span className={cn("text-warning", className)} data-testid="orphaned-stack-pull-request">
      {text}
    </span>
  )
}

/**
 * "Close Pull Request #12…" of the pull request of the whole stack of `root`, which no longer matches it (`orphaned`):
 * the confirmation opens in the root's row (an open preview closes first); `link` in a sentence, `ghost` beside the
 * actions of the island header. Disabled with the reason in its hover as Rust decides it (`getCloseStackPullRequestItem`:
 * "Waits for the first sync" on the frame of an earlier run), none while Rust doesn't offer it
 */
export function ClosePullRequestButton({
  root,
  number,
  orphaned,
  variant = "link",
  className,
}: {
  root: string
  number: number
  orphaned: string
  variant?: "link" | "ghost"
  className?: string
}) {
  const closePreview = useContext(ClosePreviewContext)
  const actions = useRowPart(stackReadinessKey(root), "actions")
  // the operations of the session only while the close runs, waits or is busy behind one, whose words it says
  const words = useOperationWords(actions, CLOSE_STACK_PULL_REQUEST)
  const item = getCloseStackPullRequestItem(root, number, orphaned, actions, words, closePreview ?? undefined)
  if (item === null) {
    return null
  }
  const button = (
    <Button
      variant={variant}
      size="xs"
      className={cn(variant === "link" && "h-auto p-0", className)}
      data-testid="close-stack-pull-request"
      disabled={item.disabled === true}
      onClick={() => item.onSelect({ surface: null })}
    >
      <GitPullRequestClosedIcon data-icon="inline-start" />
      {item.label}
    </Button>
  )
  if (item.disabled !== true) {
    return button
  }
  return (
    <Tooltip>
      {/* a span: a disabled button gets no pointer events */}
      <TooltipTrigger render={<span className="inline-flex" />}>{button}</TooltipTrigger>
      <TooltipContent>{item.description}</TooltipContent>
    </Tooltip>
  )
}

/**
 * The same in the preview of the root, with "Close Pull Request #12…": the confirmation opens in the root's row (the
 * preview closes), the pull request is never closed by itself
 */
export function OrphanedStackPullRequestNotice({ root }: { root: string }) {
  const text = useOrphanedStackPullRequest(root)
  const pullRequest = useStackPullRequest(root)
  if (text === null || pullRequest === undefined) {
    return null
  }
  return (
    <Alert variant="warning" data-testid="orphaned-stack-pull-request">
      <TriangleAlertIcon />
      <AlertDescription className="flex flex-wrap items-center gap-x-2">
        {`${text}.`}
        <ClosePullRequestButton root={root} number={pullRequest.number} orphaned={text} />
      </AlertDescription>
    </Alert>
  )
}
