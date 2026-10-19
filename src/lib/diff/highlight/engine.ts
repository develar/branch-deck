import { INITIAL } from "shiki/textmate"
import type { IGrammar, StateStack } from "shiki/textmate"
import { PAGE_LINES } from "@/lib/diff/highlight/protocol"
import type { HighlightAnswer, HighlightRequest, PageWant } from "@/lib/diff/highlight/protocol"
import { buildLineTable, lineText } from "@/lib/diff/line-table"
import type { LineTable } from "@/lib/diff/line-table"

/*
 * The tokenizer of the highlight worker (docs/design/diff-view.md, "Syntax highlighting"), free of the worker global, so
 * the main thread runs it too when no worker starts, and the tests drive it directly.
 */

/** A longer line stays plain text, and its state passes on unchanged */
const MAX_TOKENIZED_LINE = 1000
/** The time limit of `tokenizeLine2` for one line, in ms */
const LINE_TIME_LIMIT = 20
/**
 * The time limit of the first line of a grammar that the limit above cuts, in ms: the first lines compile the regexes
 * of the rules they reach (the TypeScript grammar takes longer than 20 ms there), and a cut line stays wrong
 */
const COMPILE_TIME_LIMIT = 1000

/** The grammars whose first cut line had its compile limit */
const compiledGrammars = new WeakSet<IGrammar>()
/** The lines a provisional page looks back for a line without indent to start from */
const PROVISIONAL_LOOKBACK = 100
/** The texts a worker keeps, in UTF-16 code units */
const MAX_TEXT_LENGTH = 64 * 1024 * 1024

export interface GrammarSource {
  /** The grammar of a language id; null when there is none */
  grammar: (language: string) => Promise<IGrammar | null>
  /** The style of a token from its metadata: its role, the font style bits above it */
  styleOf: (metadata: number) => number
}

export interface EngineOptions {
  /** The work between two yields, in ms: 8 in a worker, 4 on the main thread */
  sliceMs?: number
  now?: () => number
  /** Lets the messages of the view in; the main thread waits for an idle period */
  yieldTask?: () => Promise<void>
  /** The time limit of `tokenizeLine2` for one line, in ms; 0 for none, so the runs do not depend on the load (tests) */
  lineTimeLimitMs?: number
}

interface BlobState {
  language: string
  table: LineTable
  /** undefined while it is not loaded */
  grammar: IGrammar | null | undefined
  /** The exact state at the start of each page from the first, without holes: `[0]` is the initial state */
  checkpoints: StateStack[]
  /** The runs of the pages sent as provisional, to compare with the exact ones */
  provisional: Map<number, { lineStarts: Uint32Array; runs: Uint32Array }>
}

interface Job {
  view: string
  blobId: string
  page: number
  priority: number
  order: number
  provisionalSent: boolean
}

interface PageRuns {
  lineStarts: Uint32Array
  runs: Uint32Array
  state: StateStack
}

export class HighlightEngine {
  private readonly blobs = new Map<string, BlobState>()
  private textLength = 0
  private jobs: Job[] = []
  private order = 0
  private pumping: Promise<void> | null = null
  private readonly sliceMs: number
  private readonly now: () => number
  private readonly yieldTask: () => Promise<void>
  private readonly lineTimeLimitMs: number

  constructor(
    private readonly source: GrammarSource,
    private readonly send: (answer: HighlightAnswer, transfer: ArrayBuffer[]) => void,
    { sliceMs = 8, now = () => performance.now(), yieldTask = () => new Promise(resolve => setTimeout(resolve, 0)), lineTimeLimitMs = LINE_TIME_LIMIT }: EngineOptions = {},
  ) {
    this.sliceMs = sliceMs
    this.now = now
    this.yieldTask = yieldTask
    this.lineTimeLimitMs = lineTimeLimitMs
  }

  handle(request: HighlightRequest): void {
    if (request.type === "text") {
      this.setText(request.blobId, request.language, request.text)
    } else {
      this.request(request.view, request.wants)
    }
  }

  /** Resolves when no work is left */
  idle(): Promise<void> {
    return this.pumping ?? Promise.resolve()
  }

  private setText(blobId: string, language: string, text: string): void {
    const known = this.blobs.get(blobId)
    if (known !== undefined) {
      this.blobs.delete(blobId)
      this.blobs.set(blobId, known)
      return
    }
    this.blobs.set(blobId, { language, table: buildLineTable(text), grammar: undefined, checkpoints: [INITIAL], provisional: new Map() })
    this.textLength += text.length
    // the oldest texts without work go first; the one just set stays
    for (const [id, blob] of this.blobs) {
      if (this.textLength <= MAX_TEXT_LENGTH || id === blobId) {
        break
      }
      if (!this.jobs.some(job => job.blobId === id)) {
        this.blobs.delete(id)
        this.textLength -= blob.table.text.length
      }
    }
  }

  /** The pages a view needs now replace the ones it asked for before */
  private request(view: string, wants: readonly PageWant[]): void {
    this.jobs = this.jobs.filter(job => job.view !== view)
    for (const want of wants) {
      for (const page of want.pages) {
        this.jobs.push({ view, blobId: want.blobId, page, priority: want.priority, order: this.order++, provisionalSent: false })
      }
    }
    if (this.jobs.length > 0) {
      this.pumping ??= this.pump().finally(() => {
        this.pumping = null
      })
    }
  }

  private async pump(): Promise<void> {
    // the first slice waits for the other requests of the same task
    await this.yieldTask()
    while (this.jobs.length > 0) {
      const sliceEnd = this.now() + this.sliceMs
      while (this.jobs.length > 0 && this.now() < sliceEnd) {
        await this.step(this.nextJob())
      }
      await this.yieldTask()
    }
  }

  /** The job of the best priority; a provisional page before an exact pass of the same priority */
  private nextJob(): Job {
    let best = this.jobs[0]!
    for (const job of this.jobs) {
      if (
        job.priority < best.priority ||
        (job.priority === best.priority && (Number(job.provisionalSent) < Number(best.provisionalSent) || (job.provisionalSent === best.provisionalSent && job.order < best.order)))
      ) {
        best = job
      }
    }
    return best
  }

  private removeJobs(blobId: string, page?: number): void {
    this.jobs = this.jobs.filter(job => job.blobId !== blobId || (page !== undefined && job.page !== page))
  }

  private async step(job: Job): Promise<void> {
    const blob = this.blobs.get(job.blobId)
    if (blob === undefined) {
      this.removeJobs(job.blobId)
      this.send({ type: "missing", blobId: job.blobId }, [])
      return
    }
    if (blob.grammar === undefined) {
      try {
        blob.grammar = await this.source.grammar(blob.language)
      } catch (error) {
        blob.grammar = null
        this.removeJobs(job.blobId)
        this.send({ type: "failed", blobId: job.blobId, reason: error instanceof Error ? error.message : String(error) }, [])
        return
      }
      if (blob.grammar === null) {
        this.removeJobs(job.blobId)
        this.send({ type: "failed", blobId: job.blobId, reason: `no grammar for ${blob.language}` }, [])
      }
      return
    }
    if (blob.grammar === null || job.page * PAGE_LINES >= Math.max(blob.table.lineCount, 1)) {
      this.removeJobs(job.blobId, job.page)
      return
    }
    const grammar = blob.grammar
    const known = blob.checkpoints.length - 1
    if (job.page <= known) {
      this.sendExact(job.blobId, blob, job.page, this.tokenizePage(grammar, blob, job.page, blob.checkpoints[job.page]!))
      return
    }
    // a page more than one page after the last exact state is provisional first, so the first screen does not wait
    if (!job.provisionalSent && !blob.provisional.has(job.page) && job.page - known > 1) {
      const runs = this.tokenizeProvisional(grammar, blob, job.page)
      blob.provisional.set(job.page, runs)
      for (const other of this.jobs) {
        if (other.blobId === job.blobId && other.page === job.page) {
          other.provisionalSent = true
        }
      }
      this.send({ type: "page", blobId: job.blobId, page: job.page, exact: false, lineStarts: runs.lineStarts.slice(), runs: runs.runs.slice() }, [])
      return
    }
    // the exact pass, one page further
    const runs = this.tokenizePage(grammar, blob, known, blob.checkpoints[known]!)
    if (this.jobs.some(other => other.blobId === job.blobId && other.page === known)) {
      this.sendExact(job.blobId, blob, known, runs)
    } else {
      blob.checkpoints[known + 1] ??= runs.state
    }
  }

  private sendExact(blobId: string, blob: BlobState, page: number, runs: PageRuns): void {
    blob.checkpoints[page + 1] ??= runs.state
    this.removeJobs(blobId, page)
    const provisional = blob.provisional.get(page)
    blob.provisional.delete(page)
    if (provisional !== undefined && sameArray(provisional.lineStarts, runs.lineStarts) && sameArray(provisional.runs, runs.runs)) {
      this.send({ type: "exact", blobId, page }, [])
      return
    }
    this.send({ type: "page", blobId, page, exact: true, lineStarts: runs.lineStarts, runs: runs.runs }, [runs.lineStarts.buffer as ArrayBuffer, runs.runs.buffer as ArrayBuffer])
  }

  /** A page from a guessed state: the initial state at the nearest line above without indent */
  private tokenizeProvisional(grammar: IGrammar, blob: BlobState, page: number): { lineStarts: Uint32Array; runs: Uint32Array } {
    const start = page * PAGE_LINES
    const { text, starts } = blob.table
    const startsWithoutIndent = (line: number) => {
      const first = starts[line]! < text.length ? text.charCodeAt(starts[line]!) : 10
      return first !== 32 && first !== 9 && first !== 10 && first !== 13
    }
    let from = Math.max(start - PROVISIONAL_LOOKBACK, 0)
    for (let line = start; line > from; line--) {
      if (startsWithoutIndent(line)) {
        from = line
        break
      }
    }
    let state = INITIAL
    for (let line = from; line < start; line++) {
      state = this.tokenizeLine(grammar, lineText(blob.table, line), state, []).state
    }
    return this.tokenizePage(grammar, blob, page, state)
  }

  private tokenizePage(grammar: IGrammar, blob: BlobState, page: number, state: StateStack): PageRuns {
    const start = page * PAGE_LINES
    const end = Math.min(start + PAGE_LINES, blob.table.lineCount)
    const lineStarts = new Uint32Array(end - start + 1)
    const runs: number[] = []
    let current = state
    for (let line = start; line < end; line++) {
      lineStarts[line - start] = runs.length / 2
      current = this.tokenizeLine(grammar, lineText(blob.table, line), current, runs).state
    }
    lineStarts[end - start] = runs.length / 2
    return { lineStarts, runs: Uint32Array.from(runs), state: current }
  }

  /** Appends the runs of a line: a pair at each change of the style, none while it is the default foreground */
  private tokenizeLine(grammar: IGrammar, text: string, state: StateStack, runs: number[]): { state: StateStack } {
    if (text.length > MAX_TOKENIZED_LINE) {
      return { state }
    }
    let result = grammar.tokenizeLine2(text, state, this.lineTimeLimitMs)
    if (result.stoppedEarly && !compiledGrammars.has(grammar)) {
      compiledGrammars.add(grammar)
      result = grammar.tokenizeLine2(text, state, COMPILE_TIME_LIMIT)
    }
    const { tokens } = result
    let previous = 0
    for (let index = 0; index < tokens.length; index += 2) {
      const style = this.source.styleOf(tokens[index + 1]!)
      if (style !== previous) {
        runs.push(tokens[index]!, style)
        previous = style
      }
    }
    // a line cut by the time limit keeps the state before it, as VS Code does
    return { state: result.stoppedEarly ? state : result.ruleStack }
  }
}

function sameArray(a: Uint32Array, b: Uint32Array): boolean {
  if (a.length !== b.length) {
    return false
  }
  for (let index = 0; index < a.length; index++) {
    if (a[index] !== b[index]) {
      return false
    }
  }
  return true
}
