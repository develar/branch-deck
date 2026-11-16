/** A box as laid out, inline (the width of the page) and block */
export interface Box {
  readonly inline: number
  readonly block: number
}

/** The content box and the border box of an element, what a `ResizeObserver` reports of it */
export interface Boxes {
  readonly content: Box
  readonly border: Box
}

/**
 * What the boxes of an element are made of as laid out: its computed width and height (its border box in a border box
 * sizing, else its content box, without its scroll bars), its padding, its borders and the room its scroll bars take
 */
export interface Layout {
  readonly borderBox: boolean
  readonly size: Box
  readonly padding: Box
  readonly borders: Box
  /**
   * The room of the scroll bars, the rounded border box less the rounded client box and the borders: 0 under 2 px, an
   * overlay bar or none, whose rounding leaves a pixel at most
   */
  readonly bars: Box
}

/** Two sizes this close are the same: less than a layout unit (1/64 px), more than the rounding of a computed style */
const SAME_SIZE = 0.01

export function sameBox(one: Box, other: Box): boolean {
  return Math.abs(one.inline - other.inline) <= SAME_SIZE && Math.abs(one.block - other.block) <= SAME_SIZE
}

/** The layout of `element` now, which no transform changes; null without a box */
export function layoutOf(element: Element): Layout | null {
  if (!element.isConnected || typeof getComputedStyle === "undefined") {
    return null
  }
  const style = getComputedStyle(element)
  const none = { inline: 0, block: 0 }
  if (style.display === "none") {
    return { borderBox: true, size: none, padding: none, borders: none, bars: none }
  }
  const width = Number.parseFloat(style.width)
  const height = Number.parseFloat(style.height)
  if (!Number.isFinite(width) || !Number.isFinite(height)) {
    return null
  }
  const px = (value: string | undefined) => Number.parseFloat(value ?? "") || 0
  const borders = { inline: px(style.borderLeftWidth) + px(style.borderRightWidth), block: px(style.borderTopWidth) + px(style.borderBottomWidth) }
  const bar = (offset: unknown, client: unknown, border: number) => {
    if (typeof offset !== "number" || typeof client !== "number") {
      return 0
    }
    const size = Math.round(offset - client - border)
    return size >= 2 ? size : 0
  }
  const box = element as HTMLElement
  return {
    borderBox: style.boxSizing === "border-box",
    size: { inline: width, block: height },
    padding: { inline: px(style.paddingLeft) + px(style.paddingRight), block: px(style.paddingTop) + px(style.paddingBottom) },
    borders,
    bars: { inline: bar(box.offsetWidth, box.clientWidth, borders.inline), block: bar(box.offsetHeight, box.clientHeight, borders.block) },
  }
}

/**
 * The boxes of `layout` as a `ResizeObserver` reports them: a scroll bar that takes room (WebKit on macOS with a mouse,
 * "Always show scroll bars") lies between the padding and the border, so the content box goes without it and the border
 * box with it. Exact at a zoom of 100% (in Chromium at any device scale, in WebKit at a whole one), an estimate
 * otherwise: WebKit at another zoom (the zoom of the app) or a fractional scale lays out the borders, the padding and the
 * bars on a grid of its own, which neither the computed style nor the whole pixels of the client box say, and the
 * estimate misses by up to 1.5 px (tests/e2e/resize-loops/resize-loops.spec.ts). So no verdict rests on it alone: a loop
 * is told by the engine's own boxes (src/lib/observe-size.ts), a cycle by two layouts of the element (`sameBoxOf`)
 */
export function boxesOf(layout: Layout): Boxes {
  const { size, padding, borders, bars } = layout
  return layout.borderBox
    ? {
        content: { inline: size.inline - padding.inline - borders.inline - bars.inline, block: size.block - padding.block - borders.block - bars.block },
        border: size,
      }
    : {
        content: size,
        border: { inline: size.inline + padding.inline + borders.inline + bars.inline, block: size.block + padding.block + borders.block + bars.block },
      }
}

/** The boxes of `element` as laid out now (`boxesOf`); null without a box */
export function laidOutBoxes(element: Element): Boxes | null {
  const layout = layoutOf(element)
  return layout === null ? null : boxesOf(layout)
}

/**
 * Whether two layouts of one element have the same `box`: exact at any zoom and scale, as one arithmetic rounds both
 * alike. A bar that takes room in both counts by the size of the first for both: the size of a bar is a constant of the
 * theme, and its estimate of whole pixels can round anew when the element moves by a fraction of a pixel
 */
export function sameBoxOf(one: Layout, other: Layout, box: keyof Boxes): boolean {
  const bar = (first: number, second: number) => (first > 0 === second > 0 ? first : second)
  const bars = { inline: bar(one.bars.inline, other.bars.inline), block: bar(one.bars.block, other.bars.block) }
  return sameBox(boxesOf(one)[box], boxesOf({ ...other, bars })[box])
}

/** The layout units of a CSS pixel: a size a `ResizeObserver` reports is a whole number of them */
const LAYOUT_UNITS = 64

/**
 * The content box of `element`, one without a transform, as its `ResizeObserver` reports it, by the same arithmetic: its
 * border box in the coordinates of the page (`getBoundingClientRect`) over its zoom (`currentCSSZoom`), less its
 * borders, its padding and its bars (`layoutOf`), floored to a layout unit. Under a CSS zoom (`html { zoom }`, which the
 * E2E and unit tests use) the zoom is the float of the engine and the border box whole layout units at that zoom; under
 * the page zoom of the webview (WKWebView `pageZoom`, the zoom of the real app) the zoom is 1 and the border box already
 * carries the fraction, in CSS pixels unzoomed: the division is by 1, the floor does it all. Either way at 110% the
 * computed style says the size before that floor (WebKit 680.894897, Chromium 680.895, for a report of 680.890625), which
 * a read as an element mounts must not take for the report's. Exact where the borders, the padding and the bars are none
 * (the scroller of the diffs); null without a box
 */
export function observedContentBox(element: Element): Box | null {
  const layout = layoutOf(element)
  if (layout === null) {
    return null
  }
  const zoom = (element as Element & { currentCSSZoom?: number }).currentCSSZoom ?? 1
  const rect = element.getBoundingClientRect()
  const content = (border: number, side: keyof Box) => border / zoom - layout.borders[side] - layout.padding[side] - layout.bars[side]
  const floored = (size: number) => Math.floor(size * LAYOUT_UNITS + 1e-6) / LAYOUT_UNITS
  return { inline: floored(content(rect.width, "inline")), block: floored(content(rect.height, "block")) }
}
