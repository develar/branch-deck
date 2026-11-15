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
 * The boxes of `element` as laid out now, as a `ResizeObserver` reports them (src/lib/observe-size.ts), which no transform
 * changes; null without a box. A scroll bar that takes room (WebKit on macOS with a mouse, "Always show scroll bars")
 * lies between the padding and the border: the content box goes without it, the border box with it. Its size is the
 * rounded border box less the rounded client box and the borders, 0 for an overlay bar or none. Self-contained: an E2E
 * test runs its source in a page (tests/e2e/resize-loops/resize-loops.spec.ts)
 */
export function laidOutBoxes(element: Element): Boxes | null {
  if (!element.isConnected || typeof getComputedStyle === "undefined") {
    return null
  }
  const style = getComputedStyle(element)
  if (style.display === "none") {
    return { content: { inline: 0, block: 0 }, border: { inline: 0, block: 0 } }
  }
  const width = Number.parseFloat(style.width)
  const height = Number.parseFloat(style.height)
  if (!Number.isFinite(width) || !Number.isFinite(height)) {
    return null
  }
  const px = (value: string | undefined) => Number.parseFloat(value ?? "") || 0
  const padding = { inline: px(style.paddingLeft) + px(style.paddingRight), block: px(style.paddingTop) + px(style.paddingBottom) }
  const borders = { inline: px(style.borderLeftWidth) + px(style.borderRightWidth), block: px(style.borderTopWidth) + px(style.borderBottomWidth) }
  const bar = (offset: unknown, client: unknown, border: number) => {
    if (typeof offset !== "number" || typeof client !== "number") {
      return 0
    }
    const size = Math.round(offset - client - border)
    return size >= 2 ? size : 0
  }
  const box = element as HTMLElement
  const scrollBar = { inline: bar(box.offsetWidth, box.clientWidth, borders.inline), block: bar(box.offsetHeight, box.clientHeight, borders.block) }
  return style.boxSizing === "border-box"
    ? {
        content: { inline: width - padding.inline - borders.inline - scrollBar.inline, block: height - padding.block - borders.block - scrollBar.block },
        border: { inline: width, block: height },
      }
    : {
        content: { inline: width, block: height },
        border: { inline: width + padding.inline + borders.inline + scrollBar.inline, block: height + padding.block + borders.block + scrollBar.block },
      }
}
