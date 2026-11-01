import type { ReactNode } from "react"
import { FileDiffIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CommitHeading } from "@/components/commit/CommitHeading"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { useCommitFileStats } from "@/hooks/useCommitFiles"
import { hasCommitMessageBody } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"

/** A commit a preview reads: its subject, its whole message, its times; its copy in a virtual branch */
export interface PreviewCommit {
  readonly hash: string
  /** The copy of the commit in its virtual branch, beside the hash */
  readonly newHash?: string
  readonly subject: string
  readonly message: string
  readonly authorTime: number
  readonly committerTime: number
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
        <PreviewCommitItem key={commit.hash} commit={commit} testId={testId} onReview={onReview === undefined ? undefined : () => onReview(commit)} />
      ))}
    </ol>
  )
}

function PreviewCommitItem({ commit, testId, onReview }: { commit: PreviewCommit; testId: string; onReview: (() => void) | undefined }) {
  const fileStats = useCommitFileStats(commit.hash)
  return (
    <li data-testid={`${testId}-commit`} className="py-4 first:pt-0 last:pb-0">
      <CommitHeading
        subject={commit.subject}
        badge={
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
                      aria-label={`Review Changes of ${formatShortHash(commit.hash)}`}
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
        hash={commit.hash}
        newHash={commit.newHash}
        authorTime={commit.authorTime}
        committerTime={commit.committerTime}
        fileStats={fileStats}
      >
        {hasCommitMessageBody(commit.subject, commit.message) && <CommitMessageBody message={commit.message} />}
      </CommitHeading>
    </li>
  )
}
