import { TimeWithPopover } from "@/components/shared/TimeWithPopover"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { formatTimestamp } from "@/lib/time"

export interface CommitMetadataProps {
  commit: CommitUnion
  showAuthor?: boolean
  /** `status`: also shows the new hash after sync (`original → new`) */
  variant?: "compact" | "status"
}

/** Inline metadata spans (hash, author, time), meant for a flex row with `•` separators. */
export function CommitMetadata({ commit, showAuthor = false, variant = "compact" }: CommitMetadataProps) {
  const commitHash = getCommitHash(commit)
  const statusHash = "hash" in commit ? commit.hash : undefined
  const showStatusHash = variant === "status" && !!statusHash && commitHash !== statusHash
  const author = commit.author || ""
  const authorTime = commit.authorTime || 0
  const committerTime = commit.committerTime

  return (
    <>
      <span className="font-mono">{formatShortHash(commitHash)}</span>

      {showStatusHash && (
        <>
          <span>→</span>
          <span className="font-mono">{formatShortHash(statusHash)}</span>
        </>
      )}

      {showAuthor && author && (
        <>
          <span>•</span>
          <span>{author}</span>
        </>
      )}

      {!!authorTime && (
        <>
          <span>•</span>
          {committerTime
            ? <TimeWithPopover authorTime={authorTime} committerTime={committerTime} />
            : <span>{formatTimestamp(authorTime)}</span>}
        </>
      )}
    </>
  )
}
