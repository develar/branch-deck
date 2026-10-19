import { invoke } from "@tauri-apps/api/core"
import { PAGE_LINES } from "@/lib/diff/highlight/protocol"
import type { Priority } from "@/lib/diff/highlight/protocol"
import type { TokenCache } from "@/lib/diff/highlight/token-cache"

/*
 * The tree-sitter highlighting of the backend (docs/design/diff-view.md, "Tree-sitter highlighting"): Kotlin, Java,
 * Rust, TypeScript and JavaScript are coloured by `get_blob_syntax` as IntelliJ colours them, the other languages by the
 * Shiki worker. A call answers the runs of a range of pages, which go into the token cache as exact pages; a file the
 * backend cannot highlight (`Fallback`) goes to the worker as a whole, both sides, so its colours never mix engines.
 */

/** The Shiki ids of the languages `grammar_of` of `code_symbols::highlight` reads */
export const TREE_SITTER_LANGUAGES: ReadonlySet<string> = new Set(["kotlin", "kts", "java", "rust", "typescript", "tsx", "javascript", "jsx"])

/** The parameters of `get_blob_syntax` (`GetBlobSyntaxParams` of `crates/file-diffs/src/model.rs`) */
export interface BlobSyntaxParams {
  repositoryPath: string
  blob: { id: string; path: string | null }
  filePath: string
  fromLine: number
  toLine: number
  text: string | null
}

/**
 * `get_blob_syntax` answers raw bytes, which the typed bindings cannot say: so it is called here by name, and the
 * backend has a handler of its own for it (`lib.rs` of `src-tauri`)
 */
export type BlobSyntaxInvoke = (params: BlobSyntaxParams) => Promise<ArrayBuffer>

export const invokeBlobSyntax: BlobSyntaxInvoke = params => invoke<ArrayBuffer>("get_blob_syntax", { params })

/** The format and the statuses of an answer (`crates/file-diffs/src/syntax.rs`) */
const SYNTAX_FORMAT = 1
const HEADER = 6
const STATUS_RUNS = 0
const STATUS_MISSING = 2

/** The calls in flight at once: the in-view pages of a window are a few blobs */
const MAX_IN_FLIGHT = 6

/** A blob the backend colours: where it is, what it is and which file it belongs to */
export interface BackendWant {
  blobId: string
  text: string
  pages: readonly number[]
  priority: Priority
  /** The path whose extension tells the grammar: the old path for the old side of a rename */
  filePath: string
  /** The path of the file in the working tree, for a blob of the working tree */
  worktreePath: string | null
  /** The file of the blob: a fallback sends all its sides to the worker */
  group: string
}

interface Job {
  want: BackendWant
  from: number
  to: number
}

/** A blob id that git has: a view sends the text of the others (the result of a merge) with its first call */
function isObjectId(id: string): boolean {
  return /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(id)
}

export class BackendSyntax {
  /** The jobs not started, by view: a new list of a view replaces its old one */
  private readonly queued = new Map<string, Job[]>()
  /** `blobId:page` of the pages a call in flight answers */
  private readonly inFlight = new Set<string>()
  private running = 0
  /** The blobs whose text the backend lacks: their calls send it */
  private readonly needText = new Set<string>()
  private readonly fallbackBlobs = new Set<string>()

  constructor(
    private readonly cache: TokenCache,
    private readonly repositoryPath: () => string | null,
    private readonly onFallback: (group: string) => void,
    private readonly call: BlobSyntaxInvoke = invokeBlobSyntax,
  ) {}

  /** The pages a view needs now, in view first; they replace the view's last wants */
  want(view: string, wants: readonly BackendWant[]): void {
    const jobs: Job[] = []
    for (const want of [...wants].sort((a, b) => a.priority - b.priority)) {
      if (this.fallbackBlobs.has(want.blobId)) {
        continue
      }
      const missing = [...want.pages].sort((a, b) => a - b).filter(page => this.cache.get(want.blobId, page)?.exact !== true && !this.inFlight.has(`${want.blobId}:${page}`))
      // a run of adjacent pages is one call, with the page after it ahead of the scroll
      let start = 0
      for (let index = 1; index <= missing.length; index++) {
        if (index === missing.length || missing[index] !== missing[index - 1]! + 1) {
          jobs.push({ want, from: missing[start]! * PAGE_LINES, to: (missing[index - 1]! + 2) * PAGE_LINES })
          start = index
        }
      }
    }
    if (jobs.length === 0) {
      this.queued.delete(view)
    } else {
      this.queued.set(view, jobs)
    }
    this.pump()
  }

  release(view: string): void {
    this.queued.delete(view)
  }

  private pump(): void {
    while (this.running < MAX_IN_FLIGHT) {
      const job = this.next()
      if (job === undefined) {
        return
      }
      this.start(job)
    }
  }

  /** The queued job of the best priority, its pages not in flight since it was queued */
  private next(): Job | undefined {
    let best: { view: string; index: number; job: Job } | undefined
    for (const [view, jobs] of this.queued) {
      jobs.forEach((job, index) => {
        if (best === undefined || job.want.priority < best.job.want.priority) {
          best = { view, index, job }
        }
      })
    }
    if (best === undefined) {
      return undefined
    }
    const jobs = this.queued.get(best.view)!
    jobs.splice(best.index, 1)
    if (jobs.length === 0) {
      this.queued.delete(best.view)
    }
    return best.job
  }

  private start(job: Job): void {
    const repositoryPath = this.repositoryPath()
    const { want } = job
    if (repositoryPath === null || this.fallbackBlobs.has(want.blobId)) {
      return
    }
    const keys = pagesOf(job).map(page => `${want.blobId}:${page}`)
    keys.forEach(key => this.inFlight.add(key))
    this.running++
    const sendText = this.needText.has(want.blobId) || !isObjectId(want.blobId)
    this.call({
      repositoryPath,
      blob: { id: want.blobId, path: want.worktreePath },
      filePath: want.filePath,
      fromLine: job.from,
      toLine: job.to,
      text: sendText ? want.text : null,
    })
      .then(
        bytes => this.onAnswer(job, bytes, sendText),
        () => this.fallBack(want),
      )
      .finally(() => {
        keys.forEach(key => this.inFlight.delete(key))
        this.running--
        this.pump()
      })
  }

  private onAnswer(job: Job, bytes: ArrayBuffer, sentText: boolean): void {
    const { want } = job
    const words = new Uint32Array(bytes)
    if (words.length < HEADER || words[0] !== SYNTAX_FORMAT) {
      this.fallBack(want)
      return
    }
    const [, status, , from, to, pairs] = words as unknown as [number, number, number, number, number, number]
    if (status === STATUS_MISSING && !sentText) {
      // the backend has no such blob: the next call sends its text
      this.needText.add(want.blobId)
      this.queued.set(`missing:${want.blobId}`, [job])
      return
    }
    if (status !== STATUS_RUNS) {
      this.fallBack(want)
      return
    }
    const starts = words.subarray(HEADER, HEADER + to - from + 1)
    const runs = words.subarray(HEADER + to - from + 1, HEADER + to - from + 1 + pairs * 2)
    // `from` is the start of a page, `to` at most the line count: the backend clamps it
    for (let page = from / PAGE_LINES; page * PAGE_LINES < to; page++) {
      const first = page * PAGE_LINES - from
      const end = Math.min((page + 1) * PAGE_LINES, to) - from
      const base = starts[first]!
      const lineStarts = starts.slice(first, end + 1).map(start => start - base)
      this.cache.put(want.blobId, page, { exact: true, lineStarts, runs: runs.subarray(base * 2, starts[end]! * 2) })
    }
  }

  private fallBack(want: BackendWant): void {
    if (!this.fallbackBlobs.has(want.blobId)) {
      this.fallbackBlobs.add(want.blobId)
      this.onFallback(want.group)
    }
  }
}

function pagesOf(job: Job): number[] {
  const pages: number[] = []
  for (let page = job.from / PAGE_LINES; page * PAGE_LINES < job.to; page++) {
    pages.push(page)
  }
  return pages
}
