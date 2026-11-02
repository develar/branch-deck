import type { ReactNode } from "react"
import { FileDiffIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CommitCard } from "@/components/commit/CommitCard"
import type { CardCommit } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"

/** A commit a preview reads: its card, its whole message; its copy in a virtual branch */
export interface PreviewCommit extends CardCommit {
  /** Before its review button: "Merge conflict" */
  readonly badge?: ReactNode
}

/**
 * The commits of a preview newest first, each with its whole message, so it reads like the description of a pull
 * request: the preview of a branch and of a kept branch. `onReview` reviews a commit, as D on its row does; `testId`
 * names the list (`<testId>-commits`, `<testId>-commit`, `<testId>-commit-review`).
 */
export function CommitsPreviewBody({
  commits,
  onReview,
  busy,
  testId,
}: {
  commits: readonly PreviewCommit[]
  onReview: ((commit: PreviewCommit) => void) | undefined
  /** The files of the commits are being read: their meta lines get them */
  busy: boolean
  testId: string
}) {
  return (
    <ol data-testid={`${testId}-commits`} aria-busy={busy || undefined} className="flex flex-col divide-y divide-border">
      {commits.map(commit => (
        <PreviewCommitItem key={commit.id} commit={commit} testId={testId} onReview={onReview === undefined ? undefined : () => onReview(commit)} />
      ))}
    </ol>
  )
}

function PreviewCommitItem({ commit, testId, onReview }: { commit: PreviewCommit; testId: string; onReview: (() => void) | undefined }) {
  return (
    <li data-testid={`${testId}-commit`} className="py-4 first:pt-0 last:pb-0">
      <CommitCard
        commit={commit}
        variant="heading"
        addon={
          <>
            {commit.badge}
            {/* the commit in its review, as D on its row opens it */}
            {onReview !== undefined && (
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      size="icon-xs"
                      variant="ghost"
                      className="ml-auto shrink-0 text-muted-foreground"
                      aria-label={`Review Changes of ${formatShortHash(commit.id)}`}
                      data-testid={`${testId}-commit-review`}
                      onClick={onReview}
                    />
                  }
                >
                  <FileDiffIcon />
                </TooltipTrigger>
                <TooltipContent>Review Changes</TooltipContent>
              </Tooltip>
            )}
          </>
        }
      />
    </li>
  )
}
