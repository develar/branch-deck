import { Fragment } from "react"
import type { ReactNode } from "react"
import { TimeWithPopover } from "@/components/shared/TimeWithPopover"
import { formatShortHash } from "@/lib/hash"
import { formatTimestamp } from "@/lib/time"

export interface CommitMetadataProps {
  hash: string
  /** Hash of the synced copy; shown as `hash → newHash` when it differs */
  newHash?: string
  author?: string
  authorTime?: number
  committerTime?: number
  fileCount?: number
  /** Extra items (e.g. the branch name), each after a `·` separator */
  extra?: readonly ReactNode[]
}

/**
 * The meta line of every commit presentation: short hash (and the synced hash), author, time, file count and extras,
 * separated by `·`, in `text-xs text-muted-foreground`.
 */
export function CommitMetadata({ hash, newHash, author, authorTime, committerTime, fileCount, extra = [] }: CommitMetadataProps) {
  const items: ReactNode[] = []
  if (author) {
    items.push(<span>{author}</span>)
  }
  if (authorTime) {
    items.push(committerTime ? <TimeWithPopover authorTime={authorTime} committerTime={committerTime} /> : <span>{formatTimestamp(authorTime)}</span>)
  }
  if (fileCount) {
    items.push(<span>{`${fileCount} ${fileCount === 1 ? "file" : "files"}`}</span>)
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
