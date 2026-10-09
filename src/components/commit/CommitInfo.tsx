import { FileCodeIcon } from "lucide-react"
import { TimeWithPopover } from "@/components/shared/TimeWithPopover"
import { formatShortHash } from "@/lib/hash"

export interface CommitInfoProps {
  hash: string
  author?: string
  authorTime?: number
  committerTime?: number
  fileCount?: number
}

/** One-line commit summary: short hash, author, time and file count. */
export function CommitInfo({ hash, author, authorTime, committerTime, fileCount }: CommitInfoProps) {
  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground">
      <span className="font-mono">{formatShortHash(hash)}</span>
      {author && <span>{`by ${author}`}</span>}
      {!!authorTime && <TimeWithPopover authorTime={authorTime} committerTime={committerTime} />}
      {fileCount !== undefined && (
        <span className="flex items-center gap-1">
          <span>•</span>
          <FileCodeIcon className="size-3" />
          <span>{`${fileCount} ${fileCount === 1 ? "file" : "files"}`}</span>
        </span>
      )}
    </div>
  )
}
