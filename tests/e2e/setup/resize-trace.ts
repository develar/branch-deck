/**
 * The trace of the ResizeObservers of a page (`E2E_RO_TRACE=1`, docs/contributing/e2e-tests.md, "Errors are declared"):
 * at a ResizeObserver loop the page logs, besides it, the last deliveries (the frame by `requestAnimationFrame`, the
 * element by an id, its text and its parents, the size, where its observer was made), the end of each callback, the
 * changes of the texts inside observed elements, and the observed elements whose size is not the one they reported last.
 * Which observer delivered what and what changed after it told the loop of the toasts, of the measured heights of a
 * review and of the columns of the branch view. An init script: it runs in the page
 */
export function resizeTrace(): void {
  const log: string[] = []
  const ids = new WeakMap<Element, number>()
  let nextId = 0
  const describe = (element: Element) => {
    if (!ids.has(element)) {
      ids.set(element, ++nextId)
    }
    const text = ((element as HTMLElement).innerText ?? "").replace(/\s+/g, " ").slice(0, 24)
    const parts: string[] = [`#${ids.get(element)}"${text}"`]
    for (let node: Element | null = element; node !== null && parts.length < 5; node = node.parentElement) {
      const id = node.getAttribute("data-testid") ?? node.getAttribute("data-slot") ?? node.getAttribute("role")
      parts.push(`${node.tagName.toLowerCase()}${id ? `[${id}]` : ""}`)
    }
    return parts.join("<")
  }
  let frame = 0
  const tick = () => {
    frame++
    requestAnimationFrame(tick)
  }
  requestAnimationFrame(tick)
  const push = (line: string) => {
    log.push(line)
    if (log.length > 80) {
      log.splice(0, log.length - 80)
    }
  }
  const delivered = new Map<Element, string>()
  const size = (element: Element) => {
    const box = element.getBoundingClientRect()
    return `${Math.round(box.width)}x${Math.round(box.height)}`
  }
  const nodeValue = Object.getOwnPropertyDescriptor(Node.prototype, "nodeValue")!
  Object.defineProperty(Node.prototype, "nodeValue", {
    ...nodeValue,
    set(this: Node, value: string | null) {
      const before = this.nodeValue
      nodeValue.set!.call(this, value)
      const observed = this.parentElement === null ? undefined : [...delivered.keys()].find(element => element.contains(this))
      if (before !== value && observed !== undefined) {
        push(`f${frame} text of #${ids.get(observed)} "${before}" -> "${value}"`)
      }
    },
  })
  const Original = globalThis.ResizeObserver
  globalThis.ResizeObserver = class extends Original {
    constructor(callback: ResizeObserverCallback) {
      const site = (new Error().stack ?? "")
        .split("\n")
        .slice(1, 4)
        .map(line => line.trim())
        .join(" | ")
      super((entries, observer) => {
        for (const entry of entries) {
          delivered.set(entry.target, size(entry.target))
          push(`f${frame} ${describe(entry.target)} ${Math.round(entry.contentRect.width)}x${Math.round(entry.contentRect.height)} @ ${site}`)
        }
        try {
          callback(entries, observer)
        } finally {
          push(`f${frame} end of the callback`)
        }
      })
    }
  }
  window.addEventListener("error", event => {
    if (String(event.message).includes("ResizeObserver loop")) {
      const pending = [...delivered]
        .filter(([element, last]) => element.isConnected && size(element) !== last)
        .map(([element, last]) => `${describe(element)} ${last} -> ${size(element)}`)
      // a line of the error level, which fails the test with it (`expectErrorLog`)
      console.info(`[e2e-log-error] The trace of a ResizeObserver loop at f${frame}: pending ${pending.join("; ")}\n${log.slice(-30).join("\n")}`)
    }
  })
}
