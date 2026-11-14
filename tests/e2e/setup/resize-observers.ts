/**
 * A test wrapper of the `ResizeObserver` of a page, never in the product: at a ResizeObserver loop it tells the test on
 * the console what it was asked for (an init script with its options, it runs in the page).
 * - `attribute` (the WebKit check of the loops, tests/e2e/resize-loops/resize-loops.spec.ts): `[e2e-resize-loop] {json}`,
 *   the elements whose size is not the one their observer reported last, by owner: the app's (an observer `observeSize`
 *   made, which marks it, src/lib/observe-size.ts) or a library's.
 * - `trace` (`E2E_RO_TRACE=1`, docs/contributing/e2e-tests.md, "Errors are declared"): a line of the error level, which
 *   fails the test with it, of the last deliveries (the frame by `requestAnimationFrame`, the element by an id, its text
 *   and its parents, the size, where its observer was made), the end of each callback, the changes of the texts inside
 *   observed elements, and what was left pending. It found the loops of the toasts, of a review and of the branch columns
 */
export function resizeObservers({ attribute, trace }: { attribute?: boolean; trace?: boolean }): void {
  const mark = Symbol.for("branch-deck.observeSize")
  const log: string[] = []
  const ids = new WeakMap<Element, number>()
  let nextId = 0
  const describe = (element: Element) => {
    if (!ids.has(element)) {
      ids.set(element, ++nextId)
    }
    const parts: string[] = []
    for (let node: Element | null = element; node !== null && parts.length < 4; node = node.parentElement) {
      const id = node.getAttribute("data-testid") ?? node.getAttribute("data-slot") ?? node.getAttribute("role")
      parts.push(`${node.tagName.toLowerCase()}${id ? `[${id}]` : ""}`)
    }
    const text = ((element as HTMLElement).innerText ?? "").replace(/\s+/g, " ").slice(0, 24)
    return `#${ids.get(element)}"${text}" ${parts.join("<")}`
  }
  const size = (element: Element) => {
    const box = element.getBoundingClientRect()
    return `${Math.round(box.width)}x${Math.round(box.height)}`
  }
  let frame = 0
  const tick = () => {
    frame++
    requestAnimationFrame(tick)
  }
  requestAnimationFrame(tick)
  const push = (line: string) => {
    if (trace === true) {
      log.push(line)
      if (log.length > 80) {
        log.splice(0, log.length - 80)
      }
    }
  }
  const observers: { observer: WeakRef<ResizeObserver>; reported: Map<Element, string> }[] = []
  if (trace === true) {
    const nodeValue = Object.getOwnPropertyDescriptor(Node.prototype, "nodeValue")!
    Object.defineProperty(Node.prototype, "nodeValue", {
      ...nodeValue,
      set(this: Node, value: string | null) {
        const before = this.nodeValue
        nodeValue.set!.call(this, value)
        const observed = observers.flatMap(({ reported }) => [...reported.keys()]).find(element => element.contains(this))
        if (before !== value && observed !== undefined) {
          push(`f${frame} text of #${ids.get(observed)} "${before}" -> "${value}"`)
        }
      },
    })
  }
  const Original = globalThis.ResizeObserver
  globalThis.ResizeObserver = class extends Original {
    constructor(callback: ResizeObserverCallback) {
      const reported = new Map<Element, string>()
      const site =
        trace === true
          ? (new Error().stack ?? "")
              .split("\n")
              .slice(1, 4)
              .map(line => line.trim())
              .join(" | ")
          : ""
      super((entries, observer) => {
        for (const entry of entries) {
          reported.set(entry.target, size(entry.target))
          push(`f${frame} ${describe(entry.target)} ${Math.round(entry.contentRect.width)}x${Math.round(entry.contentRect.height)} @ ${site}`)
        }
        try {
          callback(entries, observer)
        } finally {
          push(`f${frame} end of the callback`)
        }
      })
      observers.push({ observer: new WeakRef(this), reported })
    }
  }
  window.addEventListener("error", event => {
    if (!String(event.message).includes("ResizeObserver loop")) {
      return
    }
    const owners: { app: string[]; library: string[] } = { app: [], library: [] }
    for (const { observer, reported } of observers) {
      const instance = observer.deref()
      if (instance === undefined) {
        continue
      }
      const owner = Reflect.get(instance, mark) === true ? owners.app : owners.library
      for (const [element, last] of reported) {
        if (element.isConnected && size(element) !== last) {
          owner.push(`${describe(element)} ${last} -> ${size(element)}`)
        }
      }
    }
    if (attribute === true) {
      console.info(`[e2e-resize-loop] ${JSON.stringify(owners)}`)
    }
    if (trace === true) {
      console.info(`[e2e-log-error] The trace of a ResizeObserver loop at f${frame}: pending ${[...owners.app, ...owners.library].join("; ")}\n${log.slice(-30).join("\n")}`)
    }
  })
}
