/**
 * The changes of a view in the rendering update of a report of a `ResizeObserver`, after its callback: WebKit renders
 * there a commit of React that waited in a microtask, and a flushSync of any callback renders every update pending
 * then. An observed element such a change resizes after its report is an observation that rendering update skips, a
 * ResizeObserver loop, which the page logs as an uncaught error (docs/contributing/frontend.md, "A flushSync in the
 * delivery of a ResizeObserver"). The owner of the observer lets those elements go at once, and measures and observes
 * them anew in the next task (`nextTask`), never in the rendering update, where its own render or write would resize
 * what other observers watch: the column registry of the branch view and the measured heights of a diff view
 */
export class ReportGuard {
  private reporting = false
  private readonly mutations: MutationObserver | null

  /** `recheck`: the nodes the changes of the watched views touched while a report's rendering update ran */
  constructor(recheck: (touched: readonly Node[]) => void) {
    this.mutations =
      typeof MutationObserver === "undefined"
        ? null
        : new MutationObserver(records => {
            if (this.reporting) {
              recheck(records.map(record => record.target))
            }
          })
  }

  /** The observer reported (the end of its callback): until the next task, the changes of the watched nodes are rechecked */
  reported(): void {
    if (this.reporting) {
      return
    }
    this.reporting = true
    nextTask(() => {
      this.reporting = false
    })
  }

  /** Rechecks the changes of `node` and of its subtree, the texts, the children and the classes that size it */
  watch(node: Node): void {
    this.mutations?.observe(node, { subtree: true, childList: true, characterData: true, attributeFilter: ["class", "style", "hidden"] })
  }

  disconnect(): void {
    this.mutations?.disconnect()
  }
}

/** Runs `run` in the next task, after the rendering update: a `MessageChannel`, as the timers of a page can be a test's fake clock */
export function nextTask(run: () => void): void {
  const channel = new MessageChannel()
  channel.port1.onmessage = () => {
    channel.port1.close()
    run()
  }
  channel.port2.postMessage(null)
}

/** The observed elements at or above the nodes `touched` (`observed`: whether an element is one), each once */
export function observedAbove(touched: readonly Node[], observed: (element: Element) => boolean): Set<Element> {
  const found = new Set<Element>()
  for (const node of touched) {
    for (let current: Node | null = node; current !== null && current !== undefined; current = current.parentNode) {
      // the observed are elements: a text or a comment is none of them
      const element = current as Element
      if (observed(element)) {
        found.add(element)
        break
      }
    }
  }
  return found
}
