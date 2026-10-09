import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { CommitInfo } from "@/components/commit/CommitInfo"
import { getCommitMessageBody } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"

export interface CommitHashPopoverProps {
  hash: string
  subject?: string
  message?: string
  author?: string
  authorTime?: number
}

/** Short hash; hovering shows the commit subject, body and info. */
export function CommitHashPopover({ hash, subject, message, author, authorTime }: CommitHashPopoverProps) {
  const hasFullMessage = !!message && !!subject && message !== subject && message.includes("\n")
  const body = hasFullMessage ? getCommitMessageBody(message) : ""

  return (
    <Popover>
      <PopoverTrigger openOnHover nativeButton={false} render={<span className="font-mono" />}>
        {formatShortHash(hash)}
      </PopoverTrigger>
      <PopoverContent className="w-auto min-w-[300px] gap-2 p-3">
        <p className="text-sm font-medium text-foreground">{subject || message || "No message available"}</p>
        {hasFullMessage && <div className="text-xs whitespace-pre-wrap text-muted-foreground">{body}</div>}
        <CommitInfo hash={hash} author={author} authorTime={authorTime} />
      </PopoverContent>
    </Popover>
  )
}
