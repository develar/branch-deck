import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { CommitHeader } from "@/components/commit/CommitHeader"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { hasCommitMessageBody } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"

export interface CommitHashPopoverProps {
  hash: string
  subject?: string
  message?: string
  author?: string
  authorTime?: number
}

/** Short hash, a button; hovering or focusing it shows the commit header (subject and meta line) and its body, as in the commit lists. */
export function CommitHashPopover({ hash, subject, message, author, authorTime }: CommitHashPopoverProps) {
  const title = subject || message?.split("\n", 1)[0] || "No message available"
  const hasBody = !!message && hasCommitMessageBody(title, message)

  return (
    <Popover>
      <PopoverTrigger openOnHover className="rounded-sm font-mono outline-none focus-visible:ring-3 focus-visible:ring-ring/30">
        {formatShortHash(hash)}
      </PopoverTrigger>
      <PopoverContent
        collisionPadding={16}
        className="max-h-[min(24rem,var(--available-height))] w-auto max-w-[min(42rem,calc(100vw-2rem))] min-w-75 gap-2 overflow-y-auto overscroll-contain p-3"
      >
        <CommitHeader subject={title} hash={hash} author={author} authorTime={authorTime} />
        {hasBody && <CommitMessageBody message={message} />}
      </PopoverContent>
    </Popover>
  )
}
