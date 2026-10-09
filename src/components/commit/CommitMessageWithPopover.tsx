import { RectangleEllipsisIcon } from "lucide-react"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { LinkedText } from "@/components/commit/LinkedText"
import { getCommitMessageBody } from "@/lib/commit"

export interface CommitMessageWithPopoverProps {
  subject: string
  message: string
  messageClassName?: string
}

/** Commit subject with issue links; for a multi-line message, an icon whose hover popover shows the full message. */
export function CommitMessageWithPopover({ subject, message, messageClassName = "text-sm text-foreground" }: CommitMessageWithPopoverProps) {
  const hasFullMessage = !!message && message !== subject && message.includes("\n")
  const body = hasFullMessage ? getCommitMessageBody(message) : ""

  return (
    <div className="flex items-center gap-2">
      <LinkedText text={subject} className={messageClassName} />
      {hasFullMessage && (
        <Popover>
          <PopoverTrigger openOnHover nativeButton={false} render={<span className="inline-flex" />}>
            <RectangleEllipsisIcon className="size-3 text-muted-foreground hover:text-foreground" />
          </PopoverTrigger>
          <PopoverContent className="w-auto max-w-[500px] gap-2 p-3">
            <LinkedText text={subject} className="text-sm font-medium text-foreground" />
            {body && (
              <div className="text-xs whitespace-pre-wrap text-muted-foreground">
                <LinkedText text={body} />
              </div>
            )}
          </PopoverContent>
        </Popover>
      )}
    </div>
  )
}
