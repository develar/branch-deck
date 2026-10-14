import type { DroppedCommits } from "@/hooks/usePushButton"
import { formatShortHash } from "@/lib/hash"
import { formatRelativeTime } from "@/lib/time"

/** The remote commits a force push drops, under the description of the push button: subject, author, id and age */
export function DroppedCommitList({ dropped }: { dropped: DroppedCommits }) {
  return (
    <div data-testid="dropped-commits" className="flex flex-col gap-1.5 border-t border-border pt-2">
      <span className="text-xs font-medium text-muted-foreground">Dropped from the remote</span>
      <ul className="flex flex-col gap-1.5">
        {dropped.commits.map(commit => (
          <li key={commit.id} className="flex min-w-0 flex-col">
            <span className="truncate">{commit.subject}</span>
            <span className="truncate text-xs text-muted-foreground">
              {`${commit.author} · `}
              <span className="font-mono">{formatShortHash(commit.id)}</span>
              {` · ${formatRelativeTime(commit.authorTime * 1000)}`}
            </span>
          </li>
        ))}
      </ul>
      {dropped.more > 0 && <span className="text-xs text-muted-foreground">{`and ${dropped.more} more`}</span>}
    </div>
  )
}
