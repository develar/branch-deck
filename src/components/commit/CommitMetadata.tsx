import { Fragment } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { TimeWithPopover } from "@/components/shared/TimeWithPopover"
import type { CommitFileStats } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { formatTimestamp } from "@/lib/time"
import { fileCount as formatFileCount } from "@/lib/update-state"

export interface CommitMetadataProps {
  hash: string
  /** Hash of the synced copy; shown as `hash → newHash` when it differs */
  newHash?: string
  author?: string
  authorTime?: number
  committerTime?: number
  fileCount?: number
  /** The changed files with their line counts, "3 files · +42 −7"; shown instead of `fileCount` */
  fileStats?: CommitFileStats
  /** Colour the line counts (`success`, `destructive`), as the preview header does; rows keep them muted */
  coloredLineCounts?: boolean
  /** Extra items (e.g. the branch name), each after a `·` separator */
  extra?: readonly ReactNode[]
}

/**
 * The meta line of every commit presentation: short hash (and the synced hash), author, time, file count with the line
 * counts and extras, separated by `·`, in `text-xs text-muted-foreground`.
 */
export function CommitMetadata({ hash, newHash, author, authorTime, committerTime, fileCount, fileStats, coloredLineCounts = false, extra = [] }: CommitMetadataProps) {
  const items: ReactNode[] = []
  if (author) {
    items.push(<span>{author}</span>)
  }
  if (authorTime) {
    items.push(committerTime ? <TimeWithPopover authorTime={authorTime} committerTime={committerTime} /> : <span>{formatTimestamp(authorTime)}</span>)
  }
  const count = fileStats?.count ?? fileCount
  if (count) {
    items.push(<span data-testid="commit-file-count" className="tabular-nums">{formatFileCount(count)}</span>)
  }
  if (fileStats && fileStats.count > 0 && fileStats.additions + fileStats.deletions > 0) {
    items.push(<LineCounts data-testid="commit-line-counts" additions={fileStats.additions} deletions={fileStats.deletions} colored={coloredLineCounts} />)
  }
  items.push(...extra)

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
      <span className="font-mono">{formatShortHash(hash)}</span>
      {newHash && newHash !== hash && (
        <>
          <span>→</span>
          <span className="font-mono">{formatShortHash(newHash)}</span>
        </>
      )}
      {/* items of one commit never reorder, so the index is a stable key */}
      {items.map((item, index) => (
        <Fragment key={index}>
          <span>·</span>
          {item}
        </Fragment>
      ))}
    </div>
  )
}

export interface LineCountsProps {
  "additions": number
  "deletions": number
  "colored": boolean
  "data-testid"?: string
}

/** `+42 −7`: added and deleted lines, with a minus sign rather than a hyphen */
export function LineCounts({ additions, deletions, colored, "data-testid": testId }: LineCountsProps) {
  return (
    <span data-testid={testId} className="tabular-nums">
      <span className={cn(colored && "text-success")}>{`+${additions}`}</span>
      {" "}
      <span className={cn(colored && "text-destructive")}>{`−${deletions}`}</span>
    </span>
  )
}
