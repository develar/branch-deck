import { HighlightEngine } from "@/lib/diff/highlight/engine"
import type { GrammarSource } from "@/lib/diff/highlight/engine"
import type { HighlightAnswer, HighlightRequest, PageWant, Priority } from "@/lib/diff/highlight/protocol"
import { TokenCache } from "@/lib/diff/highlight/token-cache"

/*
 * The main-thread side of the highlighting (docs/design/diff-view.md, "Syntax highlighting"): pins each blob to one
 * worker, sends its text once, asks for the pages a view lacks, and keeps the answers in the token cache.
 */

/** A worker, or the engine on the main thread when no worker starts */
export interface HighlightPort {
  post: (request: HighlightRequest) => void
  listen: (listener: (answer: HighlightAnswer) => void) => void
}

/** The pages of a blob a view needs, with what the worker needs to tokenize it */
export interface BlobWant {
  blobId: string
  /** A Shiki language id; "text" is never asked for */
  language: string
  text: string
  pages: readonly number[]
  priority: Priority
}

export class HighlightClient {
  readonly cache: TokenCache
  private readonly ports: HighlightPort[] = []
  /** The port of each blob: its text and grammar states live there */
  private readonly pins = new Map<string, number>()
  /** The blobs whose text their port has */
  private readonly sent = new Set<string>()
  /** The text length pinned to each port: a new blob goes to the least loaded one */
  private readonly load: number[] = []
  private readonly lastWants = new Map<string, readonly BlobWant[]>()
  private readonly failed = new Set<string>()
  /** A port can stand for several, e.g. the main thread for every worker that did not start: it is listened to once */
  private readonly listening = new WeakSet<HighlightPort>()

  constructor(
    ports: readonly HighlightPort[],
    cache = new TokenCache(),
    private readonly onFailure: (blobId: string, reason: string) => void = () => {},
  ) {
    this.cache = cache
    ports.forEach((port, index) => this.replacePort(index, port))
  }

  /** Puts a port in place of another, e.g. the main thread for a worker that did not start; its blobs pin again */
  replacePort(index: number, port: HighlightPort): void {
    this.ports[index] = port
    this.load[index] = 0
    for (const [blobId, pinned] of this.pins) {
      if (pinned === index) {
        this.pins.delete(blobId)
        this.sent.delete(blobId)
      }
    }
    if (!this.listening.has(port)) {
      this.listening.add(port)
      port.listen(answer => this.onAnswer(answer))
    }
    for (const [view, wants] of this.lastWants) {
      this.want(view, wants)
    }
  }

  /** The pages a view needs now; the pages the cache has exact are not asked for. They replace the view's last wants */
  want(view: string, wants: readonly BlobWant[]): void {
    this.lastWants.set(view, wants)
    const byPort = this.ports.map((): PageWant[] => [])
    for (const want of wants) {
      if (want.language === "text" || this.failed.has(want.blobId)) {
        continue
      }
      const pages = want.pages.filter(page => this.cache.get(want.blobId, page)?.exact !== true)
      if (pages.length > 0) {
        byPort[this.pin(want)]!.push({ blobId: want.blobId, pages, priority: want.priority })
      }
    }
    // every port gets the view's list, an empty one too: it drops the work the view asked for before
    this.ports.forEach((port, index) => port.post({ type: "tokens", view, wants: byPort[index]! }))
  }

  /** The view is gone: its work is dropped */
  release(view: string): void {
    this.want(view, [])
    this.lastWants.delete(view)
  }

  private pin(want: BlobWant): number {
    let index = this.pins.get(want.blobId)
    if (index === undefined) {
      index = this.load.indexOf(Math.min(...this.load))
      this.pins.set(want.blobId, index)
      this.load[index]! += want.text.length
    }
    if (!this.sent.has(want.blobId)) {
      this.sent.add(want.blobId)
      this.ports[index]!.post({ type: "text", blobId: want.blobId, language: want.language, text: want.text })
    }
    return index
  }

  private onAnswer(answer: HighlightAnswer): void {
    switch (answer.type) {
      case "page":
        this.cache.put(answer.blobId, answer.page, { exact: answer.exact, lineStarts: answer.lineStarts, runs: answer.runs })
        break
      case "exact":
        this.cache.confirm(answer.blobId, answer.page)
        break
      case "missing":
        // the worker dropped the text: the views that still want the blob send it again
        this.sent.delete(answer.blobId)
        for (const [view, wants] of this.lastWants) {
          if (wants.some(want => want.blobId === answer.blobId)) {
            this.want(view, wants)
          }
        }
        break
      case "failed":
        this.failed.add(answer.blobId)
        this.onFailure(answer.blobId, answer.reason)
        break
    }
  }
}

/** The engine on this thread, as a port: when no worker starts, and in the tests */
export function inProcessPort(source: Promise<GrammarSource>, options: ConstructorParameters<typeof HighlightEngine>[2] = {}): HighlightPort & { idle: () => Promise<void> } {
  const listeners: ((answer: HighlightAnswer) => void)[] = []
  const engine = source.then(grammars => new HighlightEngine(grammars, answer => listeners.forEach(listener => listener(answer)), options))
  return {
    post: request => {
      void engine.then(it => it.handle(request))
    },
    listen: listener => {
      listeners.push(listener)
    },
    idle: async () => {
      await (await engine).idle()
    },
  }
}

/** A worker as a port */
export function workerPort(worker: Worker): HighlightPort {
  return {
    post: request => worker.postMessage(request),
    listen: listener => worker.addEventListener("message", (event: MessageEvent<HighlightAnswer>) => listener(event.data)),
  }
}
