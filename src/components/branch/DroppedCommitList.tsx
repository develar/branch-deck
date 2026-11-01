import { FileDiffIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { CommitSubjectLine } from "@/components/commit/CommitSubject"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { formatShortHash } from "@/lib/hash"
import { openCommitIdsReview } from "@/lib/review-window"
import type { DroppedCommits } from "@/presenters/push-button"

/**
 * The remote commits a force push drops, under the description of the push button: subject, author, id and age. Before
 * the push they are reachable (the remote-tracking branch has them): Review Changes reviews them as a list
 * (`reviewOf`, the branch they are dropped from); after it (Activity) they may be gone, and the list only names them.
 */
export function DroppedCommitList({ dropped, reviewOf }: { dropped: DroppedCommits; reviewOf?: string }) {
  return (
    <div data-testid="dropped-commits" className="flex flex-col gap-1.5 border-t border-border pt-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-medium text-muted-foreground">Dropped from the remote</span>
        {reviewOf !== undefined && (
          <Button
            size="xs"
            variant="ghost"
            data-testid="dropped-commits-review"
            onClick={() =>
              void openCommitIdsReview(
                dropped.commits.map(commit => commit.id),
                `${reviewOf} on the remote`,
                0,
              )
            }
          >
            <FileDiffIcon data-icon="inline-start" />
            Review Changes
          </Button>
        )}
      </div>
      <ul className="flex flex-col gap-1.5">
        {dropped.commits.map(commit => (
          <li key={commit.id} className="flex min-w-0 flex-col">
            <CommitSubjectLine subject={commit.subject} />
            <span className="truncate text-xs text-muted-foreground">
              {`${commit.author} · `}
              <span className="font-mono">{formatShortHash(commit.id)}</span>
              {" · "}
              <RelativeTime time={commit.authorTime * 1000} />
            </span>
          </li>
        ))}
      </ul>
      {dropped.more > 0 && <span className="text-xs text-muted-foreground">{`and ${dropped.more} more`}</span>}
    </div>
  )
}
