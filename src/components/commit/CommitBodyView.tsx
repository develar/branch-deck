import { Fragment } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { LinkedText } from "@/components/commit/LinkedText"
import { Callout } from "@/components/shared/Callout"
import { CodeText } from "@/components/shared/CodeText"
import { DisclosureChevron } from "@/components/shared/Disclosure"
import { useBodyFold } from "@/hooks/useBodyFold"
import type { BodyFold } from "@/hooks/useBodyFold"
import { useCommitBody } from "@/hooks/useCommitBody"
import type { CommitBody, CommitBodyBlock } from "@/lib/bindings"
import type { BodyPart, BodyParts } from "@/presenters/commit-body"
import { codeOf, presentBodyParts } from "@/presenters/commit-body"

/**
 * The sizes of a body: the reading text of a preview (14/22, `reading`), the card of a row's mark one step smaller
 * (`card`), the opening of a step of a review (`story`: the card's size, its lead a step larger, no trailer), a body
 * inside a disclosure of an opening (`nested`: a story without its larger lead, never larger than its trigger)
 */
export type BodySize = "reading" | "card" | "story" | "nested"

/** The text of a line of a block, by its item (the line of a paragraph, the item of a list): prose with its code and issue links */
export type BodyTextRenderer = (text: string, item: number) => ReactNode

/** How a view of a body sets its words: the text of each block, and each block or head at its place */
export interface BodyRendering {
  /** The text of the lines of the block at `index` of `CommitBody.blocks`; prose with its code (as Rust found it) and issue links by default */
  readonly text?: (index: number) => BodyTextRenderer
  /** A block, or the head of a part, at its place in the body; `revealed` the click that opened the fold it is in */
  readonly place?: (index: number, revealed: number | null, node: ReactNode) => ReactNode
}

/** The text of the lines of a block: prose with the spans of code Rust found in each line and its issue links */
function codeTextOf(parts: BodyParts): NonNullable<BodyRendering["text"]> {
  return index => (text, item) => <CodeText text={text} spans={codeOf(parts.code, index, item)} />
}

const plainText: BodyTextRenderer = text => <LinkedText text={text} />
const inPlace = (_index: number, _revealed: number | null, node: ReactNode) => node

const NO_PARTS: BodyParts = { lead: null, shown: [], folded: [], code: [] }

/**
 * The body of a commit message as Rust reads it (`commit_body`, `sync_utils::commit_body`): the message, whose body
 * Rust reads once and keeps (`useCommitBody`), or the body it read already. The one view of a body
 * (`BodyPartsView`): the card of a commit (`CommitCard`: the card of a row's mark, the previews of a branch, the commit
 * a review shows alone), the commit preview, the body of a pull request; the story a review tells sets the same view
 * with its code names (`StoryBody`). `fold`: the parts Rust folds are named on one line, each its own disclosure (the
 * card); without it the body shows whole (the preview, which shows the full message). A body of no blocks shows
 * `empty` ("No description" in the commit preview, nothing by default) and its trailers still; a message of its
 * subject alone shows nothing more.
 */
export function CommitBodyView({ body: source, size = "reading", empty, fold = false }: { body: string | CommitBody; size?: BodySize; empty?: ReactNode; fold?: boolean }) {
  const read = useCommitBody(typeof source === "string" ? source : null)
  const body = typeof source === "string" ? read : source
  const opened = useBodyFold()
  const isEmpty = body !== undefined && body.blocks.length === 0
  if (isEmpty && body.trailers.length === 0 && empty === undefined) {
    return null
  }
  return (
    <BodyPartsView
      parts={body === undefined ? NO_PARTS : presentBodyParts("body", body, undefined, fold)}
      size={size}
      fold={opened}
      trailers={body?.trailers ?? []}
      empty={isEmpty ? empty : undefined}
    />
  )
}

export interface BodyPartsViewProps {
  readonly parts: BodyParts
  readonly size: BodySize
  readonly fold: BodyFold
  readonly rendering?: BodyRendering
  /** Shown last, below the blocks, without a label; a story shows none */
  readonly trailers?: readonly string[]
  /** In place of the blocks of a body of none */
  readonly empty?: ReactNode
  readonly testId?: string
  /** The commit of the body, of a story (`data-commit`) */
  readonly commit?: string
}

/**
 * A body by its parts (spec/commit/commit-row.spec.md, "Message body"): its lead (a step larger in a story), then its
 * parts before the fold in place, a label the caption of its blocks and a Look closely of the author an info callout;
 * then one line that names the folded parts ("The cause · Tests · Evidence"), each its own disclosure, an open one
 * below the line, so a disclosure grows downward; the trailers last. Rust says which part folds (`CommitBody.parts`):
 * a verification section always, the author's Look closely never. The text is the foreground's: no part of the
 * author's words is muted. Code tokens are set in the monospace font; the text is selectable, and a click in it does
 * not change the row selection; a line at `max-w-prose` (65ch, about 80 characters)
 */
export function BodyPartsView({ parts, size, fold, rendering = {}, trailers = [], empty, testId = "commit-message-body", commit }: BodyPartsViewProps) {
  const text = rendering.text ?? codeTextOf(parts)
  const place = rendering.place ?? inPlace
  const lead = parts.lead
  const closed = parts.folded.filter(({ part }) => !fold.opened.has(part.key))
  return (
    <div
      data-testid={testId}
      data-size={size}
      data-commit={commit}
      // ss04: distinct I, l and 1 in identifiers, without ss02's slashed zero in prose numbers
      className={cn(
        "flex max-w-prose min-w-0 cursor-text flex-col [font-feature-settings:'ss04'] text-foreground select-text",
        size === "reading" ? "gap-4 pb-1 text-md" : "gap-3 text-sm",
        size === "nested" && "gap-2.5",
        size === "card" && "pb-1 [&_pre]:text-xs",
      )}
      // the opening of a step is no row
      onClick={size === "story" || size === "nested" ? undefined : event => event.stopPropagation()}
    >
      {empty}
      {lead !== null && (
        <div data-testid="commit-body-lead" className={size === "story" ? "text-md" : undefined}>
          {place(lead.index, null, <CommitBodyBlockView block={lead.block} text={text(lead.index)} />)}
        </div>
      )}
      {parts.shown.map(part => (
        <BodyPartView key={part.key} part={part} text={text} place={place} revealed={null} />
      ))}
      {closed.length > 0 && (
        // one line of what is still folded, one chevron for it, each name its own toggle: "More: The cause, Tests"
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-1 gap-y-1 text-muted-foreground" data-testid="commit-body-folded">
          <ChevronRightIcon aria-hidden className="size-3.5 shrink-0 self-center" />
          <span>More:</span>
          {closed.map(({ part, name }, index) => (
            // the comma right after its name, as in a sentence
            <span key={part.key}>
              <button
                type="button"
                aria-expanded={false}
                data-testid="commit-body-fold"
                className="rounded-sm text-foreground underline decoration-muted-foreground/50 underline-offset-2 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/30"
                onClick={() => fold.toggle(part.key)}
              >
                {name}
              </button>
              {index < closed.length - 1 && <span aria-hidden>,</span>}
            </span>
          ))}
        </div>
      )}
      {/* an open part in its place after the line, its label the toggle that folds it: its name once */}
      {parts.folded
        .filter(({ part }) => fold.opened.has(part.key))
        .map(({ part, name }) => (
          <BodyPartView
            key={part.key}
            part={part}
            text={text}
            place={place}
            revealed={fold.revealed?.key === part.key ? fold.revealed.serial : null}
            unfolded={{ name, close: () => fold.toggle(part.key) }}
          />
        ))}
      {trailers.length > 0 && (
        <div className="flex flex-col gap-0.5 border-t border-border pt-3 text-xs text-muted-foreground">
          {trailers.map((line, index) => (
            <LinkedText key={index} text={line} />
          ))}
        </div>
      )}
    </div>
  )
}

/**
 * A part of a body: its label or heading above its blocks; a Look closely an info callout, its label the title. A
 * folded part opened (`unfolded`) is headed by the toggle that folds it again, its name once: the label of a labelled
 * part, "The rest" before blocks of no label
 */
export function BodyPartView({
  part,
  text = codeTextOf(NO_PARTS),
  place = inPlace,
  revealed = null,
  unfolded,
}: {
  part: BodyPart
  text?: NonNullable<BodyRendering["text"]>
  place?: NonNullable<BodyRendering["place"]>
  revealed?: number | null
  unfolded?: { name: string; close: () => void }
}) {
  const blocks = part.blocks.map(placed => (
    <Fragment key={placed.index}>{place(placed.index, revealed, <CommitBodyBlockView block={placed.block} text={text(placed.index)} />)}</Fragment>
  ))
  const head = part.head
  if (part.tone === "lookClosely" && head !== null) {
    return place(
      head.index,
      revealed,
      <Callout tone="info" title={text(head.index)(head.block.text, 0)} data-testid="commit-body-look-closely" className="text-foreground">
        {/* the details of a callout are muted: the author's words are not */}
        <div className="flex flex-col gap-2 text-foreground">{blocks}</div>
      </Callout>,
    )
  }
  const caption =
    unfolded !== undefined ? (
      <button
        type="button"
        aria-expanded
        data-testid="commit-body-unfold"
        data-slot={head?.block.type === "heading" ? "commit-body-heading" : "commit-body-label"}
        className="group flex min-w-0 items-baseline gap-1 self-start rounded-sm text-left font-semibold outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
        onClick={unfolded.close}
      >
        <DisclosureChevron />
        {unfolded.name}
      </button>
    ) : (
      head !== null && (
        <div data-slot={head.block.type === "label" ? "commit-body-label" : "commit-body-heading"} className="font-semibold">
          {text(head.index)(head.block.text, 0)}
        </div>
      )
    )
  return (
    <div className="flex min-w-0 flex-col gap-1" data-testid="commit-body-part" data-part={part.key}>
      {caption !== false && caption !== null && place(head?.index ?? part.first, revealed, caption)}
      {blocks.length > 0 && <div className="flex min-w-0 flex-col gap-2">{blocks}</div>}
    </div>
  )
}

/** What a preview says of a commit without a body */
export function NoDescription() {
  return <p className="text-sm text-muted-foreground">No description</p>
}

/**
 * A block of a body as `CommitBody` reads it: a paragraph its lines, a list its items, preformatted text, a rule; the
 * text of each line by `text` (its code by `CodeText`; the review links the code names of a story). A label and a
 * heading head their section, never a block
 */
export function CommitBodyBlockView({ block, text = plainText }: { block: CommitBodyBlock; text?: BodyTextRenderer }) {
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
