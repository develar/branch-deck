import { Fragment, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BreakableCode, CodeGlue, CodeText } from "@/components/shared/CodeText"
import { InlineCode } from "@/components/shared/InlineCode"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { CodeSpan, ProseDeletion, TextRange } from "@/lib/bindings"
import { glueAfter, glueBefore, isCodeChip } from "@/lib/inline-code"
import type { ContractParagraph, ContractTable, ProseLinkView } from "@/presenters/review-story"

/** A run of a paragraph between two places where its marks change */
interface Run {
  readonly start: number
  readonly end: number
  readonly code: CodeSpan | null
  readonly link: ProseLinkView | null
  readonly inserted: boolean
  readonly substitution: ContractParagraph["substitutions"][number] | null
}

/**
 * The runs of `paragraph` in `from..to`: cut where a span of code, a link, a change or a substitution starts or ends
 */
function runsOf(paragraph: ContractParagraph, inserted: readonly TextRange[], deleted: readonly ProseDeletion[], from: number, to: number): Run[] {
  const cuts = new Set<number>([from, to, ...deleted.map(deletion => deletion.at)])
  for (const range of [...paragraph.code, ...paragraph.links, ...inserted, ...paragraph.substitutions]) {
    cuts.add(range.start)
    cuts.add(range.end)
  }
  const sorted = [...cuts].filter(at => at >= from && at <= to).sort((a, b) => a - b)
  const runs: Run[] = []
  let at = from
  while (at < to) {
    const code = paragraph.code.find(span => span.start <= at && at < span.end) ?? null
    const end = Math.min(code?.end ?? to, sorted.find(cut => cut > at) ?? to)
    runs.push({
      start: at,
      end,
      code,
      link: paragraph.links.find(link => link.start <= at && at < link.end) ?? null,
      inserted: inserted.some(range => range.start <= at && at < range.end),
      substitution: paragraph.substitutions.find(mark => mark.start <= at && at < mark.end) ?? null,
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
 * The words of `paragraph` in `from..to` with their marks: its code set as code, its links, what the change wrote
 * tinted, and the words of `deleted` (each at an offset in `from..to`) struck through where they stood
 */
function ParagraphWords({ paragraph, from, to, deleted }: { paragraph: ContractParagraph; from: number; to: number; deleted: readonly ProseDeletion[] }) {
  const { change, text } = paragraph
  const inserted = change?.kind === "changed" ? change.inserted : []
  const runs = runsOf(paragraph, inserted, deleted, from, to)
  const removed = new Map<number, ProseDeletion[]>()
  for (const deletion of deleted) {
    removed.set(deletion.at, [...(removed.get(deletion.at) ?? []), deletion])
  }
  const removedAt = (at: number) => {
    const deletions = removed.get(at) ?? []
    removed.delete(at)
    return deletions.map(deletion => (
      // a hair apart from the words written in their place: "drops the layouts of takes", never "oftakes"
      <del key={`del:${deletion.at}`} className="me-0.5 text-muted-foreground decoration-destructive/70" data-testid="review-brief-deleted">
        {deletion.text}
      </del>
    ))
  }
  const codeGlue = new Map<number, { readonly before: string; readonly after: string }>()
  paragraph.code.forEach((code, index) => {
    const previousEnd = Math.max(from, paragraph.code[index - 1]?.end ?? from)
    const nextStart = Math.min(to, paragraph.code[index + 1]?.start ?? to)
    codeGlue.set(code.start, {
      before: glueBefore(text.slice(previousEnd, code.start)),
      after: glueAfter(text.slice(code.end, nextStart)),
    })
  })
  const renderRun = (run: Run, raw = false): ReactNode => {
    let piece = text.slice(run.start, run.end)
    if (run.code?.explicit === true) {
      if (run.start === run.code.start) {
        piece = piece.slice(1)
      }
      if (run.end === run.code.end) {
        piece = piece.slice(0, -1)
      }
    }
    let node: ReactNode = raw ? <span>{piece}</span> : run.code !== null ? <BreakableCode text={piece} /> : <CodeText text={piece} spans={[]} />
    if (run.inserted) {
      node = (
        <ins className="rounded-xs bg-success/15 no-underline ring-1 ring-success/15" data-testid="review-brief-inserted">
          {node}
        </ins>
      )
    }
    if (run.substitution !== null) {
      node = (
        <Tooltip>
          <TooltipTrigger
            render={
              <button
                type="button"
                className="inline rounded-sm bg-transparent p-0 text-inherit underline decoration-dotted underline-offset-2 outline-none focus-visible:bd-focus"
                data-testid="review-brief-substitution-word"
              />
            }
          >
            {node}
            <span className="sr-only"> (was {run.substitution.from})</span>
          </TooltipTrigger>
          <TooltipContent>was {run.substitution.from}</TooltipContent>
        </Tooltip>
      )
    }
    if (run.link !== null) {
      node = <LinkRun link={run.link}>{node}</LinkRun>
    }
    return node
  }
  const rendered: ReactNode[] = []
  for (let index = 0; index < runs.length;) {
    const run = runs[index]!
    if (run.code === null) {
      const previousCode = paragraph.code.find(code => code.end === run.start)
      const nextCode = paragraph.code.find(code => code.start === run.end)
      const before = previousCode === undefined ? "" : (codeGlue.get(previousCode.start)?.after ?? "")
      const after = nextCode === undefined ? "" : (codeGlue.get(nextCode.start)?.before ?? "")
      const adjusted = { ...run, start: run.start + before.length, end: Math.max(run.start + before.length, run.end - after.length) }
      rendered.push(
        <Fragment key={run.start}>
          {removedAt(run.start)}
          {renderRun(adjusted, paragraph.form.kind === "code")}
        </Fragment>,
      )
      index += 1
      continue
    }
    const code = run.code
    const segments: ReactNode[] = []
    let last = run
    while (index < runs.length && runs[index]?.code?.start === code.start && runs[index]?.code?.end === code.end) {
      const segment = runs[index]!
      segments.push(
        <Fragment key={segment.start}>
          {removedAt(segment.start)}
          {renderRun(segment)}
        </Fragment>,
      )
      last = segment
      index += 1
    }
    segments.push(removedAt(last.end))
    const glue = codeGlue.get(code.start)
    rendered.push(
      <CodeGlue key={`code:${code.start}`} before={glue?.before} after={glue?.after}>
        <InlineCode plain={!isCodeChip(code)} data-explicit={code.explicit ? "" : undefined} data-testid="review-brief-code-chip">
          {segments}
        </InlineCode>
      </CodeGlue>,
    )
  }
  return (
    <>
      {rendered}
      {removedAt(to)}
    </>
  )
}

/**
 * A paragraph of What Changes (spec/branch/review-tour.spec.md, "In the Review Window"), whole: a paragraph the change
 * adds with the bar of an addition in the gutter, one it removes muted and struck through beside the bar of a removal,
 * one it changes in places with what it wrote tinted and what it removed struck through where it stood; its code set
 * as code, its links showing a changed file or opening a page. Of a paragraph of several sentences changed in a few of
 * them the sentences that hold the changes, an ellipsis that opens it whole on either side
 */
export function ContractParagraphView({ paragraph }: { paragraph: ContractParagraph }) {
  const [whole, setWhole] = useState(false)
  const { change, excerpt, text } = paragraph
  let from = whole || excerpt === null ? 0 : excerpt.start
  let to = whole || excerpt === null ? text.length : excerpt.end
  for (const code of paragraph.code) {
    if (code.start < from && from < code.end) {
      from = code.start
    }
    if (code.start < to && to < code.end) {
      to = code.end
    }
  }
  const deleted = (change?.kind === "changed" ? change.deleted : []).filter(it => it.at >= from && it.at <= to)
  const hiddenLines = text.slice(0, from).split("\n").length - 1 + (to < text.length ? text.slice(to).split("\n").length : 0)
  const more = (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            aria-label="Show the whole paragraph"
            className="rounded-sm px-1 text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:bd-focus"
            data-testid="review-brief-more"
            onClick={() => setWhole(true)}
          />
        }
      >
        {paragraph.form.kind === "code" ? `⋯ ${hiddenLines} more lines` : "…"}
      </TooltipTrigger>
      <TooltipContent>Show the whole paragraph</TooltipContent>
    </Tooltip>
  )
  const content = (
    <>
      {from > 0 && more}
      <ParagraphWords paragraph={paragraph} from={from} to={to} deleted={deleted} />
      {to < text.length && more}
    </>
  )
  if (paragraph.form.kind === "code") {
    const added = change?.kind === "added"
    const removedParagraph = change?.kind === "removed"
    const pre = (
      <pre
        className={cn(
          "rounded-md bg-muted px-3 py-2 font-mono text-sm [overflow-wrap:anywhere] whitespace-pre-wrap",
          removedParagraph && "text-muted-foreground line-through decoration-muted-foreground/70",
          change?.kind === "substituted" && "text-muted-foreground",
        )}
        data-testid="review-brief-code-block"
        data-change={change?.kind}
      >
        {added && <span className="sr-only">Added: </span>}
        {removedParagraph && <span className="sr-only">Removed: </span>}
        {content}
      </pre>
    )
    return added || removedParagraph ? <div className={cn("-ml-[14px] border-l-2 pl-3", added ? "border-success" : "border-destructive/70")}>{pre}</div> : pre
  }
  switch (change?.kind) {
    case "added":
      return (
        <p className="-ml-[14px] border-l-2 border-success pl-3" data-testid="review-brief-paragraph" data-change="added">
          <span className="sr-only">Added: </span>
          {content}
        </p>
      )
    case "removed":
      return (
        <p
          className="-ml-[14px] border-l-2 border-destructive/70 pl-3 text-muted-foreground line-through decoration-muted-foreground/70"
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
    case "substituted":
      return (
        <p className="text-muted-foreground" data-testid="review-brief-paragraph" data-change="substituted">
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

/** The cells of a row of a table (`ProseForm::Row`), none of another paragraph */
function cellsOf(paragraph: ContractParagraph): readonly TextRange[] {
  return paragraph.form.kind === "row" ? paragraph.form.cells : []
}

/** The bar of a change on the quote line of What Changes: an addition, a removal, else none */
function changeBar(change: ContractParagraph["change"]): string {
  return change?.kind === "added" ? "border-success" : change?.kind === "removed" ? "border-destructive/70" : "border-transparent"
}

/**
 * A row of a table of What Changes: each cell its words with their marks, the words the change removed in a cell or
 * after it struck through at its end; the bar of its change on the quote line, a removed row muted and struck through,
 * one changed only by repeated replacements muted
 */
function TableRow({ row, cells, change, header = false }: { row: ContractParagraph; cells: readonly TextRange[]; change: ContractParagraph["change"]; header?: boolean }) {
  const deleted = change?.kind === "changed" ? change.deleted : []
  const Cell = header ? "th" : "td"
  return (
    <tr
      className={cn(
        header ? "text-xs text-muted-foreground" : "border-t border-border",
        change?.kind === "removed" && "text-muted-foreground line-through decoration-muted-foreground/70",
        change?.kind === "substituted" && "text-muted-foreground",
      )}
      data-testid={header ? "review-brief-table-head" : "review-brief-row"}
      data-change={change?.kind}
    >
      {cells.map((cell, index) => {
        // the words removed between this cell and the next stand at its end
        const end = cells[index + 1]?.start ?? Number.MAX_SAFE_INTEGER
        const removed = deleted.filter(it => it.at >= cell.start && it.at < end).map(it => ({ ...it, at: Math.min(it.at, cell.end) }))
        return (
          <Cell
            key={cell.start}
            scope={header ? "col" : undefined}
            className={cn("py-1 pr-4 text-left align-baseline font-normal [overflow-wrap:anywhere]", index === 0 && cn("border-l-2 pl-3", changeBar(change)))}
          >
            {index === 0 && change?.kind === "added" && <span className="sr-only">Added: </span>}
            {index === 0 && change?.kind === "removed" && <span className="sr-only">Removed: </span>}
            <ParagraphWords paragraph={row} from={cell.start} to={cell.end} deleted={removed} />
          </Cell>
        )
      })}
    </tr>
  )
}

/**
 * The rows of one table of What Changes (spec/branch/review-tour.spec.md, "In the Review Window") under their header
 * once, muted: the header row with its marks when the change touched it, else the cells of the header of its first
 * row; then each row the change adds, removes or changes, marked as a paragraph is
 */
export function ContractTableView({ table }: { table: ContractTable }) {
  const first = table.rows[0]
  const head = table.head ?? first
  const headCells = table.head !== null ? cellsOf(table.head) : first?.form.kind === "row" ? first.form.header : []
  return (
    <table className="-ml-[14px] max-w-[calc(100%+14px)] border-collapse self-start text-sm" data-testid="review-brief-table">
      {head !== undefined && headCells.length > 0 && (
        <thead>
          <TableRow row={head} cells={headCells} change={table.head?.change ?? null} header />
        </thead>
      )}
      <tbody>
        {table.rows.map(row => (
          <TableRow key={row.key} row={row} cells={cellsOf(row)} change={row.change} />
        ))}
      </tbody>
    </table>
  )
}
