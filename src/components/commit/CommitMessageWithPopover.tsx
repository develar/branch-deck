import { RectangleEllipsisIcon } from "lucide-react"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { LinkedText } from "@/components/commit/LinkedText"
import { getCommitMessageBody } from "@/lib/commit"
import { parseCommitBody } from "@/lib/commit-message"
import type { CommitBodyBlock } from "@/lib/commit-message"

export interface CommitMessageWithPopoverProps {
  subject: string
  message: string
  messageClassName?: string
}

/**
 * Commit subject with issue links; for a multi-line message, an icon whose hover popover shows the full message.
 * Moving the pointer into the popover keeps it open (to scroll a long body), clicking the icon pins it.
 */
export function CommitMessageWithPopover({ subject, message, messageClassName = "text-sm text-foreground" }: CommitMessageWithPopoverProps) {
  const hasFullMessage = !!message && message !== subject && message.includes("\n")

  return (
    <div className="flex items-center gap-2">
      <LinkedText text={subject} className={messageClassName} />
      {hasFullMessage && (
        <Popover>
          <PopoverTrigger
            openOnHover
            nativeButton={false}
            aria-label="Show full commit message"
            render={<span className="inline-flex shrink-0 cursor-help" />}
          >
            <RectangleEllipsisIcon className="size-3.5 text-muted-foreground hover:text-foreground" />
          </PopoverTrigger>
          <PopoverContent
            data-testid="commit-message-popover"
            collisionPadding={16}
            className="max-h-(--available-height) w-[min(60ch,calc(100vw-2rem))] max-w-none gap-3 overflow-y-auto overscroll-contain p-4"
          >
            <LinkedText text={subject} className="text-sm font-medium text-foreground" />
            <CommitMessageBody message={message} />
          </PopoverContent>
        </Popover>
      )}
    </div>
  )
}

/** Body blocks of a commit message; rendered only while the popover is open, so rows don't parse their bodies. */
function CommitMessageBody({ message }: { message: string }) {
  const blocks = parseCommitBody(getCommitMessageBody(message))
  // blocks of the same message never reorder, so the index is a stable key
  return blocks.map((block, index) => <CommitBodyBlockView key={index} block={block} />)
}

function CommitBodyBlockView({ block }: { block: CommitBodyBlock }) {
  switch (block.type) {
    case "paragraph":
      return (
        <p className="text-sm leading-relaxed text-foreground/90">
          <LinkedText text={block.text} />
        </p>
      )
    case "list": {
      const items = block.items.map((item, index) => (
        <li key={index}>
          <LinkedText text={item} />
        </li>
      ))
      const className = "flex flex-col gap-1.5 pl-5 text-sm leading-relaxed text-foreground/90 marker:text-muted-foreground"
      return block.ordered
        ? <ol start={block.start === 1 ? undefined : block.start} className={`list-decimal ${className}`}>{items}</ol>
        : <ul className={`list-disc ${className}`}>{items}</ul>
    }
    case "preformatted":
      // `shrink-0`: a scroll container would otherwise shrink in the height-limited flex column of the popup
      return (
        <pre className="shrink-0 overflow-x-auto rounded-lg bg-muted px-3 py-2 font-mono text-xs leading-relaxed whitespace-pre">
          <LinkedText text={block.text} />
        </pre>
      )
    case "trailers":
      return (
        <div className="flex flex-col gap-0.5 border-t border-border pt-3 text-xs text-muted-foreground">
          {block.lines.map((line, index) => <LinkedText key={index} text={line} />)}
        </div>
      )
  }
}
