import type { MouseEvent } from "react"
import { cn } from "cn"
import { RectangleEllipsisIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LinkedText } from "@/components/commit/LinkedText"
import { getCommitMessageBody } from "@/lib/commit"
import { groupCommitBodySections, parseCommitBody } from "@/lib/commit-message"
import type { CommitBodyBlock } from "@/lib/commit-message"

/** Icon button that opens the full commit message in the commit preview; does not change the row selection. */
export function CommitMessageButton({ onOpen }: { onOpen: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-xs"
            aria-haspopup="dialog"
            aria-label="Show full commit message"
            data-testid="commit-message-toggle"
            // `-my-0.5`: the 24px button keeps the 20px line height of the subject
            className="-my-0.5 shrink-0 text-muted-foreground hover:text-foreground"
            onClick={(event: MouseEvent) => {
              event.stopPropagation()
              onOpen()
            }}
          />
        }
      >
        <RectangleEllipsisIcon />
      </TooltipTrigger>
      <TooltipContent>Show full commit message</TooltipContent>
    </Tooltip>
  )
}

export type CommitMessageBodySize = "compact" | "reading"

const bodySizeClasses: Record<CommitMessageBodySize, { text: string; preformatted: string }> = {
  // the leading follows the size: `cn` drops a leading-* that comes before a text-* size
  compact: { text: "text-sm", preformatted: "text-xs leading-relaxed" },
  reading: { text: "text-md", preformatted: "text-sm leading-relaxed" },
}

/**
 * Body of a commit message (the message without its subject line), shown in the commit preview and the hash popover;
 * rows don't render or parse their bodies.
 *
 * A lead-in label ("Why:", "What changed:") is a small heading above its blocks. Code tokens are set in the monospace font.
 * The text is selectable, and a click in it does not change the row selection.
 *
 * `reading` (the previews) is the 14/22 reading text, `compact` (the hash popover) the 13/20 UI text; a preformatted
 * block is one step below the text. Both keep a line at `max-w-prose` (65ch, about 80 characters).
 */
export function CommitMessageBody({ message, size = "compact" }: { message: string; size?: CommitMessageBodySize }) {
  const sections = groupCommitBodySections(parseCommitBody(getCommitMessageBody(message)))
  return (
    <div
      data-testid="commit-message-body"
      // ss04: distinct I, l and 1 in identifiers, without ss02's slashed zero in prose numbers
      className={cn("flex max-w-prose cursor-text flex-col gap-4 pb-1 [font-feature-settings:'ss04'] text-foreground select-text", bodySizeClasses[size].text)}
      onClick={event => event.stopPropagation()}
    >
      {/* sections of the same message never reorder, so the index is a stable key */}
      {sections.map((section, index) => (
        <div key={index} className="flex flex-col gap-1">
          {section.label != null && (
            <div data-slot="commit-section-label" className="font-medium text-muted-foreground">
              {section.label}
            </div>
          )}
          <div className="flex min-w-0 flex-col gap-2">
            {section.blocks.map((block, blockIndex) => (
              <CommitBodyBlockView key={blockIndex} block={block} size={size} />
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}

function CommitBodyBlockView({ block, size }: { block: CommitBodyBlock; size: CommitMessageBodySize }) {
  switch (block.type) {
    case "paragraph":
      return (
        <p>
          <LinkedText text={block.text} detectCode />
        </p>
      )
    case "list": {
      const items = block.items.map((item, index) => (
        <li key={index}>
          <LinkedText text={item} detectCode />
        </li>
      ))
      const className = "flex flex-col gap-1 pl-5 marker:text-muted-foreground"
      return block.ordered ? (
        <ol start={block.start === 1 ? undefined : block.start} className={`list-decimal ${className}`}>
          {items}
        </ol>
      ) : (
        <ul className={`list-disc ${className}`}>{items}</ul>
      )
    }
    case "preformatted":
      return (
        <pre className={cn("overflow-x-auto rounded-lg bg-muted px-3 py-2 font-mono whitespace-pre", bodySizeClasses[size].preformatted)}>
          <LinkedText text={block.text} />
        </pre>
      )
    case "trailers":
      return (
        <div className="flex flex-col gap-0.5 border-t border-border pt-3 text-xs text-muted-foreground">
          {block.lines.map((line, index) => (
            <LinkedText key={index} text={line} />
          ))}
        </div>
      )
  }
}
