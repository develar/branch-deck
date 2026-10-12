import type { MouseEvent } from "react"
import { RectangleEllipsisIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { LinkedText } from "@/components/commit/LinkedText"
import { getCommitMessageBody } from "@/lib/commit"
import { groupCommitBodySections, parseCommitBody } from "@/lib/commit-message"
import type { CommitBodyBlock } from "@/lib/commit-message"

export interface CommitMessageToggleProps {
  expanded: boolean
  onToggle: () => void
}

/** Icon button that shows or hides the full commit message inline; does not change the row selection. */
export function CommitMessageToggle({ expanded, onToggle }: CommitMessageToggleProps) {
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-expanded={expanded}
      aria-label={expanded ? "Hide full commit message" : "Show full commit message"}
      data-testid="commit-message-toggle"
      // `-my-0.5`: the 24px button keeps the 20px line height of the subject
      className="-my-0.5 shrink-0 text-muted-foreground hover:text-foreground aria-expanded:text-foreground"
      onClick={(event: MouseEvent) => {
        event.stopPropagation()
        onToggle()
      }}
    >
      <RectangleEllipsisIcon />
    </Button>
  )
}

/**
 * Body of a commit message (the message without its subject line), rendered only while expanded,
 * so rows don't parse their bodies.
 *
 * The body sits on the row surface and shares the subject's left edge. A lead-in label ("Why:", "What changed:") is a
 * small heading above its blocks. Code tokens are set in the monospace font.
 * The text is selectable, and a click in it does not change the row selection.
 */
export function CommitMessageBody({ message }: { message: string }) {
  const sections = groupCommitBodySections(parseCommitBody(getCommitMessageBody(message)))
  return (
    <div
      data-testid="commit-message-body"
      // ss04: distinct I, l and 1 in identifiers, without ss02's slashed zero in prose numbers
      className="flex max-w-[90ch] cursor-text flex-col gap-4 pb-1 text-[13px] leading-5 text-foreground/85 select-text [font-feature-settings:'ss04']"
      onClick={event => event.stopPropagation()}
    >
      {/* sections of the same message never reorder, so the index is a stable key */}
      {sections.map((section, index) => (
        <div key={index} className="flex flex-col gap-1">
          {section.label != null && (
            <div data-slot="commit-section-label" className="font-medium text-foreground/70">{section.label}</div>
          )}
          <div className="flex min-w-0 flex-col gap-2">
            {section.blocks.map((block, blockIndex) => <CommitBodyBlockView key={blockIndex} block={block} />)}
          </div>
        </div>
      ))}
    </div>
  )
}

function CommitBodyBlockView({ block }: { block: CommitBodyBlock }) {
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
      return block.ordered
        ? <ol start={block.start === 1 ? undefined : block.start} className={`list-decimal ${className}`}>{items}</ol>
        : <ul className={`list-disc ${className}`}>{items}</ul>
    }
    case "preformatted":
      return (
        <pre className="overflow-x-auto rounded-lg bg-muted px-3 py-2 font-mono text-xs leading-relaxed whitespace-pre">
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
