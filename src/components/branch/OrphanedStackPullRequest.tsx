import { useContext } from "react"
import { cn } from "cn"
import { GitPullRequestClosedIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { ClosePreviewContext } from "@/hooks/useClosePreview"
import { describeOrphanedStackPullRequest } from "@/lib/whole-stack"
import { useStackBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { openInline } from "@/stores/inline-row-action"
import { useStackLandedBy } from "@/stores/landed-stacks"
import { useStackPullRequest } from "@/stores/pull-requests"

/** Why the pull request of the whole stack of `root` no longer matches it (`describeOrphanedStackPullRequest`), null while it does */
function useOrphanedStackPullRequest(root: string): string | null {
  return describeOrphanedStackPullRequest(root, useStackPullRequest(root), useStackBranch(root), useStackLandedBy(root))
}

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
 * "Close Pull Request #12…" of the pull request of the whole stack of `root`: the confirmation opens in the root's row
 * (an open preview closes first); `link` in a sentence, `ghost` beside the actions of the island header
 */
export function ClosePullRequestButton({ root, number, variant = "link", className }: { root: string; number: number; variant?: "link" | "ghost"; className?: string }) {
  const closePreview = useContext(ClosePreviewContext)
  return (
    <Button
      variant={variant}
      size="xs"
      className={cn(variant === "link" && "h-auto p-0", className)}
      data-testid="close-stack-pull-request"
      onClick={() => {
        closePreview?.()
        requestReveal({ branchName: root, commitIds: [] })
        openInline("close-stack-pull-request", root)
      }}
    >
      <GitPullRequestClosedIcon data-icon="inline-start" />
      {`Close Pull Request #${number}…`}
    </Button>
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
        <ClosePullRequestButton root={root} number={pullRequest.number} />
      </AlertDescription>
    </Alert>
  )
}
