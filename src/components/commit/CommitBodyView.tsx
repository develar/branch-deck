import { Fragment } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { LinkedText } from "@/components/commit/LinkedText"
import { Callout } from "@/components/shared/Callout"
import { useCommitBody } from "@/hooks/useCommitBody"
import type { CommitBody, CommitBodyBlock } from "@/lib/bindings"

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
 * The body of a commit message as Rust reads it (`commit_body`, `sync_utils::commit_body`): the message, whose body
 * Rust reads once and keeps (`useCommitBody`), or the body it read already. One view for every place a body is shown:
 * the card of a commit (`CommitCard`: the card of a row's mark, the previews, the commit a review shows alone) and the
 * body of a pull request; the story a review tells reads its blocks the same way (`CommitBodyBlockView`).
 *
 * A label ("Why:", "What changed:") is a heading above its blocks, as the story of a review shows it, a "Look closely:"
 * an info callout; a Markdown heading is set as strong. Code tokens are set in the monospace font. The text is
 * selectable, and a click in it does not change the row selection. The reading text is 14/22 (`reading`), the card of a
 * row's mark one step smaller (`card`); a preformatted block one step below it, a line at `max-w-prose` (65ch, about 80
 * characters). A body of no blocks shows `empty` ("No description" in the commit preview, nothing by default) and its
 * trailers still; a message of its subject alone shows nothing more.
 */
export function CommitBodyView({ body: source, size = "reading", empty }: { body: string | CommitBody; size?: "card" | "reading"; empty?: ReactNode }) {
  const read = useCommitBody(typeof source === "string" ? source : null)
  const body = typeof source === "string" ? read : source
  const sections = body === undefined ? [] : sectionsOf(body.blocks)
  const isEmpty = body !== undefined && body.blocks.length === 0
  if (isEmpty && body.trailers.length === 0 && empty === undefined) {
    return null
  }
  return (
    <div
      data-testid="commit-message-body"
      data-size={size}
      // ss04: distinct I, l and 1 in identifiers, without ss02's slashed zero in prose numbers
      className={cn(
        "flex max-w-prose cursor-text flex-col pb-1 [font-feature-settings:'ss04'] text-foreground select-text",
        size === "reading" ? "gap-4 text-md" : "gap-3 text-sm [&_pre]:text-xs",
      )}
      onClick={event => event.stopPropagation()}
    >
      {isEmpty && empty}
      {/* sections of the same message never reorder, so the index is a stable key */}
      {sections.map((section, index) => (
        <CommitBodySectionView key={index} section={section} />
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

/** A section of a body: its label or heading above its blocks; a Look closely of the author an info callout, as the story of a review shows it */
function CommitBodySectionView({ section: { head, blocks } }: { section: CommitBodySection }) {
  const isLookClosely = head?.type === "label" && head.kind === "lookClosely"
  const content = blocks.length > 0 && (
    // the details of a callout are muted: the author's words are not
    <div className={cn("flex min-w-0 flex-col gap-2", isLookClosely && "text-foreground")}>
      {blocks.map((block, index) => (
        <CommitBodyBlockView key={index} block={block} />
      ))}
    </div>
  )
  if (isLookClosely) {
    return (
      <Callout tone="info" title={head.text} data-testid="commit-section-look-closely" className="text-foreground">
        {content}
      </Callout>
    )
  }
  return (
    <div className="flex flex-col gap-1">
      {head?.type === "label" && (
        <div data-slot="commit-section-label" className="font-semibold">
          {head.text}
        </div>
      )}
      {head?.type === "heading" && (
        <div data-slot="commit-section-heading" className="font-semibold">
          <LinkedText text={head.text} detectCode />
        </div>
      )}
      {content}
    </div>
  )
}

/** What a preview says of a commit without a body */
export function NoDescription() {
  return <p className="text-sm text-muted-foreground">No description</p>
}

/** The text of a line of a block, by its item (the line of a paragraph, the item of a list): prose with its code and issue links */
export type BodyTextRenderer = (text: string, item: number) => ReactNode

const linkedText: BodyTextRenderer = text => <LinkedText text={text} detectCode />

/**
 * A block of a body as `CommitBody` reads it: a paragraph its lines, a list its items, preformatted text, a rule; the
 * text of each line by `text` (`LinkedText` by default; the review links the code names of a story). A label and a
 * heading head their section, never a block
 */
export function CommitBodyBlockView({ block, text = linkedText }: { block: CommitBodyBlock; text?: BodyTextRenderer }) {
  switch (block.type) {
    case "paragraph":
      // a line of its own (after a hard break, a quote line, a table row) breaks the paragraph there
      return (
        <p>
          {block.lines.map((line, index) => (
            <Fragment key={index}>
              {index > 0 && <br />}
              {text(line, index)}
            </Fragment>
          ))}
        </p>
      )
    case "list": {
      const items = block.items.map((item, index) => <li key={index}>{text(item, index)}</li>)
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
    case "label":
    case "heading":
      return null
  }
}
