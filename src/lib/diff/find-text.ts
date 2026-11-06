import { findInSegments } from "@/lib/diff/find"
import type { TextFindHit } from "@/lib/diff/find"

const BLOCK_TAGS = new Set([
  "ADDRESS",
  "ARTICLE",
  "ASIDE",
  "BLOCKQUOTE",
  "CAPTION",
  "DD",
  "DETAILS",
  "DIV",
  "DL",
  "DT",
  "FIELDSET",
  "FIGCAPTION",
  "FIGURE",
  "FOOTER",
  "FORM",
  "H1",
  "H2",
  "H3",
  "H4",
  "H5",
  "H6",
  "HEADER",
  "HR",
  "LEGEND",
  "LI",
  "MAIN",
  "MENU",
  "NAV",
  "OL",
  "P",
  "PRE",
  "SECTION",
  "SUMMARY",
  "TABLE",
  "TBODY",
  "TD",
  "TFOOT",
  "TH",
  "THEAD",
  "TR",
  "UL",
])

function isSkipped(element: Element): boolean {
  return element.classList.contains("sr-only") || element.getAttribute("aria-hidden") === "true" || element.hasAttribute("data-find-skip")
}

function textSegments(root: HTMLElement): Text[][] {
  if (isSkipped(root)) {
    return []
  }
  const segments: Text[][] = []
  let current: Text[] = []
  const flush = () => {
    if (current.length > 0) {
      segments.push(current)
      current = []
    }
  }
  const visit = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node as Text
      if (text.data.length > 0) {
        current.push(text)
      }
      return
    }
    if (node.nodeType !== Node.ELEMENT_NODE) {
      return
    }
    const element = node as HTMLElement
    if (isSkipped(element)) {
      flush()
      return
    }
    const block = BLOCK_TAGS.has(element.tagName)
    if (block) {
      flush()
    }
    for (const child of element.childNodes) {
      visit(child)
    }
    if (block) {
      flush()
    }
  }
  for (const child of root.childNodes) {
    visit(child)
  }
  flush()
  return segments
}

function pointAt(nodes: readonly Text[], offset: number): { node: Text; offset: number } {
  let remaining = offset
  for (const node of nodes) {
    if (remaining <= node.data.length) {
      return { node, offset: remaining }
    }
    remaining -= node.data.length
  }
  const last = nodes[nodes.length - 1]!
  return { node: last, offset: last.data.length }
}

function foldsAround(root: HTMLElement, node: Text): HTMLElement[] {
  const folds: HTMLElement[] = []
  for (let element = node.parentElement; element !== null; element = element.parentElement) {
    if (element.hasAttribute("hidden")) {
      folds.push(element)
    }
    if (element === root) {
      break
    }
  }
  return folds.reverse()
}

/** Walks text in document order, skipping excluded content and keeping a segment per block */
export function findTextHits(root: HTMLElement, query: string, file: number): TextFindHit[] {
  if (query === "") {
    return []
  }
  const segments = textSegments(root)
  const matches = findInSegments(
    segments.map(nodes => nodes.map(node => node.data).join("")),
    query,
  )
  return matches.map(({ segment, start, end }) => {
    const nodes = segments[segment]!
    const from = pointAt(nodes, start)
    const to = pointAt(nodes, end)
    const range = root.ownerDocument.createRange()
    range.setStart(from.node, from.offset)
    range.setEnd(to.node, to.offset)
    return { kind: "text", file, range, folds: foldsAround(root, from.node) }
  })
}

/** Opens every hidden fold around a text hit, from the outside in */
export function revealTextHit(hit: TextFindHit): void {
  for (const fold of hit.folds) {
    fold.dispatchEvent(new Event("beforematch", { bubbles: true }))
  }
}
