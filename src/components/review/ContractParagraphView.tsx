import { useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { CodeText } from "@/components/shared/CodeText"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { CodeSpan, ProseDeletion, TextRange } from "@/lib/bindings"
import type { ContractParagraph, ProseLinkView } from "@/presenters/review-story"

/** A run of a paragraph between two places where its marks change: a span of code whole, never split */
interface Run {
  readonly start: number
  readonly end: number
  readonly code: CodeSpan | null
  readonly link: ProseLinkView | null
  readonly inserted: boolean
}

/**
 * The runs of `paragraph` in `from..to`: cut where a span of code, a link or a range the change wrote starts or ends and
 * where it removed words, a span of code one run (written by the change when any of it is)
 */
function runsOf(paragraph: ContractParagraph, inserted: readonly TextRange[], deleted: readonly ProseDeletion[], from: number, to: number): Run[] {
  const cuts = new Set<number>([from, to, ...deleted.map(deletion => deletion.at)])
  for (const range of [...paragraph.code, ...paragraph.links, ...inserted]) {
    cuts.add(range.start)
    cuts.add(range.end)
  }
  const sorted = [...cuts].filter(at => at >= from && at <= to).sort((a, b) => a - b)
  const runs: Run[] = []
  let at = from
  while (at < to) {
    const code = paragraph.code.find(span => span.start <= at && at < span.end) ?? null
    const end = code === null ? (sorted.find(cut => cut > at) ?? to) : Math.min(code.end, to)
    const within = (range: { start: number; end: number }) => range.start < end && at < range.end
    runs.push({
      start: at,
      end,
      code,
      link: paragraph.links.find(link => link.start <= at && at < link.end) ?? null,
      inserted: code === null ? inserted.some(range => range.start <= at && at < range.end) : inserted.some(within),
    })
    at = end
  }
  return runs
}

/** A link of a paragraph: a changed file it shows in the review, a page it opens; text where it goes nowhere */
function LinkRun({ link, children }: { link: ProseLinkView; children: ReactNode }) {
  const navigator = useSelectReviewFile()
  const className = "rounded-sm underline decoration-muted-foreground/50 underline-offset-2 outline-none hover:decoration-foreground focus-visible:bd-focus"
  if (link.href !== null) {
    return (
      <a href={link.href} target="_blank" rel="noopener noreferrer" className={className} data-testid="review-brief-link">
        {children}
      </a>
    )
  }
  const place = link.place
  if (place === null || navigator === null) {
    return children
  }
  return (
    <button type="button" className={cn(className, "text-left")} data-testid="review-brief-link" data-path={place.path} onClick={() => navigator.show(place)}>
      {children}
    </button>
  )
}

/**
 * A paragraph of What Changes (spec/branch/review-tour.spec.md, "In the Review Window"), whole: a paragraph the change
 * adds with the bar of an addition in the gutter, one it removes muted and struck through beside the bar of a removal,
 * one it changes in places with what it wrote tinted and what it removed struck through where it stood; its code set
 * as code, its links showing a changed file or opening a page. Of a long paragraph changed in a few places the
 * sentences that hold the changes, an ellipsis that opens it whole on either side
 */
export function ContractParagraphView({ paragraph }: { paragraph: ContractParagraph }) {
  const [whole, setWhole] = useState(false)
  const { change, excerpt, text } = paragraph
  const inserted = change?.kind === "changed" ? change.inserted : []
  const deleted = change?.kind === "changed" ? change.deleted : []
  const from = whole || excerpt === null ? 0 : excerpt.start
  const to = whole || excerpt === null ? text.length : excerpt.end
  const runs = runsOf(paragraph, inserted, deleted, from, to)
  // the words the change removed, each before the run it stood before (a span of code whole), the rest at the end
  const removed = new Map<number, ProseDeletion[]>()
  for (const deletion of deleted.filter(it => it.at >= from && it.at <= to)) {
    const run = runs.find(it => it.start <= deletion.at && deletion.at < it.end)
    const at = run?.start ?? to
    removed.set(at, [...(removed.get(at) ?? []), deletion])
  }
  const removedAt = (at: number) =>
    (removed.get(at) ?? []).map(deletion => (
      // a hair apart from the words written in their place: "drops the layouts of takes", never "oftakes"
      <del key={`del:${deletion.at}`} className="me-0.5 text-muted-foreground decoration-destructive/70" data-testid="review-brief-deleted">
        {deletion.text}
      </del>
    ))
  const more = (
    <button
      type="button"
      aria-label="Show the whole paragraph"
      className="rounded-sm px-0.5 text-muted-foreground outline-none hover:text-foreground focus-visible:bd-focus"
      data-testid="review-brief-more"
      onClick={() => setWhole(true)}
    >
      …
    </button>
  )
  const content = (
    <>
      {from > 0 && more}
      {runs.map(run => {
        const piece = text.slice(run.start, run.end)
        const code = run.code === null ? [] : [{ ...run.code, start: 0, end: piece.length }]
        let node: ReactNode = <CodeText text={piece} spans={code} />
        if (run.inserted) {
          node = (
            <ins className="rounded-xs bg-success/15 no-underline ring-1 ring-success/15" data-testid="review-brief-inserted">
              {node}
            </ins>
          )
        }
        if (run.link !== null) {
          node = <LinkRun link={run.link}>{node}</LinkRun>
        }
        return (
          <span key={run.start}>
            {removedAt(run.start)}
            {node}
          </span>
        )
      })}
      {removedAt(to)}
      {to < text.length && more}
    </>
  )
  switch (change?.kind) {
    case "added":
      return (
        <p className="-ml-3 border-l-2 border-success pl-2.5" data-testid="review-brief-paragraph" data-change="added">
          <span className="sr-only">Added: </span>
          {content}
        </p>
      )
    case "removed":
      return (
        <p
          className="-ml-3 border-l-2 border-destructive/70 pl-2.5 text-muted-foreground line-through decoration-muted-foreground/70"
          data-testid="review-brief-paragraph"
          data-change="removed"
        >
          <span className="sr-only">Removed: </span>
          {content}
        </p>
      )
    case "changed":
      return (
        <p data-testid="review-brief-paragraph" data-change="changed">
          {content}
        </p>
      )
    default:
      return (
        <p className="text-muted-foreground" data-testid="review-brief-paragraph">
          {content}
        </p>
      )
  }
}
