import { Fragment, useEffect, useRef } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { LinkedText } from "@/components/commit/LinkedText"
import { Callout } from "@/components/shared/Callout"
import { CodeText } from "@/components/shared/CodeText"
import { DisclosureChevron } from "@/components/shared/Disclosure"
import { useBodyFold } from "@/hooks/useBodyFold"
import type { BodyFoldState } from "@/hooks/useBodyFold"
import { useCommitBody } from "@/hooks/useCommitBody"
import type { ColumnAlign, CommitBody, CommitBodyBlock } from "@/lib/bindings"
import type { BodyPart, BodyParts, PlacedBlock, RemovedBlocks } from "@/presenters/commit-body"
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
  readonly fold: BodyFoldState
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
 * author's words is muted, but what a story's body removed since the review, struck through where Rust placed it
 * beside the blocks it added (`PlacedBlock.added`, `BodyPart.removed`). Code tokens are set in the monospace font; the text is selectable, and a click in it does
 * not change the row selection; a line of a card or a preview at `max-w-prose` (65ch, about 80 characters), of a story
 * at the measure of the column of its document (`ReadingColumn`)
 */
export function BodyPartsView({ parts, size, fold, rendering = {}, trailers = [], empty, testId = "commit-message-body", commit }: BodyPartsViewProps) {
  const text = rendering.text ?? codeTextOf(parts)
  const place = rendering.place ?? inPlace
  const lead = parts.lead
  const storySize = size === "story" || size === "nested"
  const root = useRef<HTMLDivElement>(null)
  const revealSerial = useRef(-1)
  useEffect(() => {
    if (!storySize) {
      return
    }
    const element = root.current
    if (element === null) {
      return
    }
    const onBeforeMatch = (event: Event) => {
      if (event.target instanceof HTMLElement) {
        const key = event.target.dataset.bodyFold
        if (key !== undefined) {
          fold.reveal(key, revealSerial.current--)
        }
      }
    }
    element.addEventListener("beforematch", onBeforeMatch, true)
    return () => element.removeEventListener("beforematch", onBeforeMatch, true)
  }, [fold, storySize])
  const closed = parts.folded.filter(({ part }) => !fold.opened.has(part.key))
  const renderFoldedPart = ({ part, name }: (typeof parts.folded)[number]) => {
    const opened = fold.opened.has(part.key)
    return (
      <BodyPartView
        key={part.key}
        part={part}
        text={text}
        place={place}
        revealed={fold.revealed?.key === part.key ? fold.revealed.serial : null}
        unfolded={opened ? { name, close: () => fold.toggle(part.key) } : undefined}
      />
    )
  }
  return (
    <div
      ref={root}
      data-testid={testId}
      data-size={size}
      data-commit={commit}
      // ss04: distinct I, l and 1 in identifiers, without ss02's slashed zero in prose numbers
      className={cn(
        "flex min-w-0 cursor-text flex-col [font-feature-settings:'ss04'] text-foreground select-text",
        // a story is read in the column of its document, which measures it; a card and a preview measure themselves
        size === "reading" && "max-w-prose gap-4 pb-1 text-md",
        size === "story" && "gap-4 text-md",
        size === "nested" && "gap-2.5 text-md",
        size === "card" && "max-w-prose gap-3 pb-1 text-sm [&_pre]:text-xs",
      )}
      // the opening of a step is no row
      onClick={size === "story" || size === "nested" ? undefined : event => event.stopPropagation()}
    >
      {empty}
      {lead !== null && (
        // a removed lead stands with the lead that replaced it: a column only then, so a body as it was stays as it was
        // the lead of a story a step larger than its text: a title read in its paragraph (`titleInBody`) reads as one
        <div data-testid="commit-body-lead" className={cn(parts.leadRemoved !== undefined && "flex min-w-0 flex-col gap-2", size === "story" && "text-base") || undefined}>
          {removedAt(parts.leadRemoved, lead.index)}
          {place(lead.index, null, <PlacedBlockView placed={lead} text={text(lead.index)} />)}
          {removedAt(parts.leadRemoved, null)}
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
                className="rounded-sm text-foreground underline decoration-muted-foreground/50 underline-offset-2 outline-none hover:decoration-foreground focus-visible:bd-focus"
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
      {parts.folded.map(({ part, name }) => {
        const opened = fold.opened.has(part.key)
        const body = renderFoldedPart({ part, name })
        if (!storySize) {
          return opened ? body : null
        }
        return (
          <div
            key={part.key}
            ref={element => {
              if (element !== null) {
                if (opened) {
                  element.removeAttribute("hidden")
                } else {
                  element.setAttribute("hidden", "until-found")
                }
              }
            }}
            data-body-fold={opened ? undefined : part.key}
            hidden={!opened}
          >
            {body}
          </div>
        )
      })}
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
  const head = part.head
  // removed blocks that stood before a block the part does not show (the label its step is titled by) stand first
  const shownAt = new Set([...(head === null ? [] : [head.index]), ...part.blocks.map(placed => placed.index)])
  const removed = (part.removed ?? []).map(run => (run.before === null || shownAt.has(run.before) ? run : { ...run, before: part.blocks[0]?.index ?? null }))
  // a section new whole is marked once, not each of its blocks again
  const whole = part.added === true
  const blocks = [
    ...part.blocks.map(placed => (
      <Fragment key={placed.index}>
        {removedAt(removed, placed.index)}
        {place(placed.index, revealed, <PlacedBlockView placed={whole ? { index: placed.index, block: placed.block } : placed} text={text(placed.index)} />)}
      </Fragment>
    )),
    <Fragment key="removed-at-end">{removedAt(removed, null)}</Fragment>,
  ]
  // what stood before its label was removed before the section: outside the mark of a new one
  const removedBefore = head === null ? null : removedAt(removed, head.index)
  if (part.tone === "lookClosely" && head !== null) {
    return (
      <>
        {removedBefore}
        {addedMark(
          whole,
          place(
            head.index,
            revealed,
            <Callout tone="info" title={text(head.index)(head.block.text, 0)} data-testid="commit-body-look-closely" className="text-foreground">
              {/* the details of a callout are muted: the author's words are not */}
              <div className="flex flex-col gap-2 text-foreground">{blocks}</div>
            </Callout>,
          ),
        )}
      </>
    )
  }
  const caption =
    unfolded !== undefined ? (
      <button
        type="button"
        aria-expanded
        data-testid="commit-body-unfold"
        data-slot={head?.block.type === "heading" ? "commit-body-heading" : "commit-body-label"}
        className="group flex min-w-0 items-baseline gap-1 self-start rounded-sm text-left font-semibold outline-none hover:underline focus-visible:bd-focus"
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
    <>
      {removedBefore}
      {addedMark(
        whole,
        <div className="flex min-w-0 flex-col gap-1" data-testid="commit-body-part" data-part={part.key}>
          {caption !== false && caption !== null && place(head?.index ?? part.first, revealed, caption)}
          {(part.blocks.length > 0 || removed.some(run => head === null || run.before !== head.index)) && <div className="flex min-w-0 flex-col gap-2">{blocks}</div>}
        </div>,
      )}
    </>
  )
}

/** A block at its place, with the bar of an addition when the reviewed body lacked it (`PlacedBlock.added`) */
function PlacedBlockView({ placed, text }: { placed: PlacedBlock; text: BodyTextRenderer }) {
  return addedMark(placed.added === true, <CommitBodyBlockView block={placed.block} text={text} />)
}

/**
 * What the reviewed body had and the body lacks now, where it stood (spec/branch/review-tour.spec.md, "In the Review
 * Window"): the runs of `removed` that stand before the block `before` (at the end of their part for `null`)
 */
function removedAt(removed: readonly RemovedBlocks[] | undefined, before: number | null): ReactNode {
  return (removed ?? []).filter(run => run.before === before).map(run => <RemovedBlocksView key={run.key} removed={run} />)
}

/**
 * Blocks of the reviewed body removed since the review: muted and struck through, a bar in the tone of a removal in the
 * gutter, so their text stays at the edge of the others; a label or a heading as the caption it was
 */
export function RemovedBlocksView({ removed }: { removed: Pick<RemovedBlocks, "blocks" | "code"> }) {
  return (
    <div
      data-testid="commit-body-removed"
      className="-ml-3 flex min-w-0 flex-col gap-2 border-l-2 border-destructive/70 pl-2.5 text-muted-foreground line-through decoration-muted-foreground/70"
    >
      <span className="sr-only">Removed since your review:</span>
      {removed.blocks.map((block, index) =>
        block.type === "label" || block.type === "heading" ? (
          <div key={index} className="font-semibold">
            <CodeText text={block.text} spans={codeOf(removed.code, index, 0)} />
          </div>
        ) : (
          <CommitBodyBlockView key={index} block={block} text={(line, item) => <CodeText text={line} spans={codeOf(removed.code, index, item)} />} />
        ),
      )}
    </div>
  )
}

/** Blocks added since the review, apart from their body (the overview of what changed): the bar of an addition, each block */
export function AddedBlocksView({ added }: { added: Pick<RemovedBlocks, "blocks" | "code"> }) {
  return addedMark(
    true,
    <div className="flex min-w-0 flex-col gap-2">
      {added.blocks.map((block, index) =>
        block.type === "label" || block.type === "heading" ? (
          <div key={index} className="font-semibold">
            <CodeText text={block.text} spans={codeOf(added.code, index, 0)} />
          </div>
        ) : (
          <CommitBodyBlockView key={index} block={block} text={(line, item) => <CodeText text={line} spans={codeOf(added.code, index, item)} />} />
        ),
      )}
    </div>,
  )
}

/** `node` with the bar of an addition since the review in the gutter, when `added`: its text stays at the edge of the others */
function addedMark(added: boolean, node: ReactNode): ReactNode {
  return added ? (
    <div data-testid="commit-body-added" className="-ml-3 flex min-w-0 flex-col border-l-2 border-success pl-2.5">
      <span className="sr-only">Added since your review:</span>
      {node}
    </div>
  ) : (
    node
  )
}

/** What a preview says of a commit without a body */
export function NoDescription() {
  return <p className="text-sm text-muted-foreground">No description</p>
}

/** The alignment of the cells of a column of a table, by the colons of its delimiter row */
const CELL_ALIGN: Record<ColumnAlign, string> = { none: "text-start", left: "text-left", center: "text-center", right: "text-right" }

/**
 * A block of a body as `CommitBody` reads it: a paragraph its lines, a list its items, preformatted text, a rule, a
 * table its cells (each a line of prose, its item the header row first, then row by row: `LineCode.item`); the text of
 * each line by `text` (its code by `CodeText`; the review links the code names of a story). A label and a heading head
 * their section, never a block
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
    case "table": {
      const width = block.header.length
      const cell = (column: number) => cn("px-3 py-1.5 first:pl-0 last:pr-0", CELL_ALIGN[block.align[column] ?? "none"])
      return (
        // a table wider than its column scrolls in it; its text stays at the edge of the blocks around it
        <div className="max-w-full overflow-x-auto">
          <table className="border-collapse tabular-nums">
            <thead>
              <tr className="border-b border-border">
                {block.header.map((value, column) => (
                  <th key={column} scope="col" className={cn(cell(column), "align-bottom font-semibold")}>
                    {text(value, column)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row, at) => (
                <tr key={at} className="border-b border-border last:border-b-0">
                  {row.map((value, column) => (
                    <td key={column} className={cn(cell(column), "align-top")}>
                      {text(value, (at + 1) * width + column)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )
    }
    case "label":
    case "heading":
      return null
  }
}
