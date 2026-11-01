import { Fragment } from "react"
import type { MouseEvent } from "react"
import { RectangleEllipsisIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LinkedText } from "@/components/commit/LinkedText"
import { useCommitBody } from "@/hooks/useCommitBody"
import type { CommitBodyBlock } from "@/lib/bindings"

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

/** The blocks under a label or a heading, or the ones before the first of them: a gap of 16px apart */
interface CommitBodySection {
  head: Extract<CommitBodyBlock, { type: "label" | "heading" }> | null
  blocks: CommitBodyBlock[]
}

/** The sections of the blocks Rust read, as `CommitBodyBlock` defines them: a label or a heading starts one, a rule is a block of it */
function sectionsOf(blocks: readonly CommitBodyBlock[]): CommitBodySection[] {
  const sections: CommitBodySection[] = []
  let current: CommitBodySection | null = null
  for (const block of blocks) {
    if (block.type === "label" || block.type === "heading") {
      current = { head: block, blocks: [] }
      sections.push(current)
    } else if (current == null) {
      current = { head: null, blocks: [block] }
      sections.push(current)
    } else {
      current.blocks.push(block)
    }
  }
  return sections
}

/**
 * Body of a commit message (the message without its subject line, as Rust reads it: `commit_body`), shown in the commit
 * preview, the branch preview and the review; rows don't render or read their bodies.
 *
 * A label ("Why:", "What changed:") is a small heading above its blocks, a Markdown heading a stronger one. Code tokens
 * are set in the monospace font. The text is selectable, and a click in it does not change the row selection. The
 * 14/22 reading text, a preformatted block one step below it, a line at `max-w-prose` (65ch, about 80 characters).
 */
export function CommitMessageBody({ message }: { message: string }) {
  const body = useCommitBody(message)
  const sections = body === undefined ? [] : sectionsOf(body.blocks)
  return (
    <div
      data-testid="commit-message-body"
      // ss04: distinct I, l and 1 in identifiers, without ss02's slashed zero in prose numbers
      className="flex max-w-prose cursor-text flex-col gap-4 pb-1 [font-feature-settings:'ss04'] text-md text-foreground select-text"
      onClick={event => event.stopPropagation()}
    >
      {/* sections of the same message never reorder, so the index is a stable key */}
      {sections.map((section, index) => (
        <div key={index} className="flex flex-col gap-1">
          {section.head?.type === "label" && (
            <div data-slot="commit-section-label" className="font-medium text-muted-foreground">
              {section.head.text}
            </div>
          )}
          {section.head?.type === "heading" && (
            <div data-slot="commit-section-heading" className="font-semibold">
              <LinkedText text={section.head.text} detectCode />
            </div>
          )}
          {section.blocks.length > 0 && (
            <div className="flex min-w-0 flex-col gap-2">
              {section.blocks.map((block, blockIndex) => (
                <CommitBodyBlockView key={blockIndex} block={block} />
              ))}
            </div>
          )}
        </div>
      ))}
      {body !== undefined && body.trailers.length > 0 && (
        <div className="flex flex-col gap-0.5 border-t border-border pt-3 text-xs text-muted-foreground">
          {body.trailers.map((line, index) => (
            <LinkedText key={index} text={line} />
          ))}
        </div>
      )}
    </div>
  )
}

function CommitBodyBlockView({ block }: { block: CommitBodyBlock }) {
  switch (block.type) {
    case "paragraph":
      // a line of its own (after a hard break, a quote line, a table row) breaks the paragraph there
      return (
        <p>
          {block.lines.map((line, index) => (
            <Fragment key={index}>
              {index > 0 && <br />}
              <LinkedText text={line} detectCode />
            </Fragment>
          ))}
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
        <pre className="overflow-x-auto rounded-lg bg-muted px-3 py-2 font-mono text-sm leading-relaxed whitespace-pre">
          <LinkedText text={block.text} />
        </pre>
      )
    case "rule":
      return <hr className="border-border" />
    // a section holds its label or heading as its head, never as a block
    case "label":
    case "heading":
      return null
  }
}
