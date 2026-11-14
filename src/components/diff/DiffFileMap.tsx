import { useCallback, useEffectEvent, useImperativeHandle, useLayoutEffect, useRef, useState } from "react"
import type { MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent, Ref, WheelEvent as ReactWheelEvent } from "react"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import type { FileStatus } from "@/lib/bindings"
import { fileMapBand, fileMapScale, fileMapSectionAt, fileMapThumb, fileMapTicks, PROLOGUE_SECTION } from "@/lib/diff/file-map"
import type { FileMapSection } from "@/lib/diff/file-map"
import type { DiffLayout } from "@/lib/diff/layout"

/** A section of the view as the map shows it and its hover names it */
export interface FileMapEntry extends FileMapSection {
  key: string
  path: string
  oldPath: string | null
  status: FileStatus
  /** The words of the step of the review plan the section is of ("Step 2 · <title>"); null outside the steps */
  stepHeading: string | null
}

export interface DiffFileMapHandle {
  /** The window and the section at the top shown again: after a scroll the view made, which may not scroll at all */
  sync: () => void
}

export interface DiffFileMapProps {
  ref?: Ref<DiffFileMapHandle>
  /** The scroller of the diffs the map stands for */
  scroller: () => HTMLDivElement | null
  /** Scrolls the diffs to a `scrollTop`, as the user does: a drag of the thumb, the wheel over the map */
  scrollTo: (top: number) => void
  layout: DiffLayout
  sections: readonly FileMapEntry[]
  /** The index of the section at the top: the one the view showed last, or the one scrolled to; `PROLOGUE_SECTION` */
  current: () => number
  /** A click on the map: the section under the pointer at the top, as a click in the tree shows it; `PROLOGUE_SECTION` */
  onShowSection: (index: number) => void
  /** The user scrolls with the map (its window dragged, the wheel over it): the view follows as it does a wheel */
  onUserScroll: () => void
}

/** The paler ticks of the files of another owner than the chosen chip's */
const MUTED_TICK_ALPHA = 0.35
/**
 * The thumb is never thinner than this, in CSS px, so a review of 500 files keeps something to see and to drag; a
 * thicker thumb than the window of the view is centred on it
 */
const MIN_THUMB = 16
/** The inset of the tick of a file from the sides of the map, in CSS px; a step crosses it */
const FILE_TICK_INSET = 2
/** A thin section still shows as the current one */
const MIN_CURRENT = 2

/**
 * The map of the files that replaces the scrollbar of the diffs of a review (docs/design/diff-view.md, "The map of
 * files"): the whole document scaled to its height, the ticks of the sections drawn on a canvas, the section at the
 * top filled, the window of the view the thumb that drags. A click shows the section under the pointer, the hover
 * names it; above the first section the prologue is a band, "Overview". It is out of the tab order and hidden from a
 * screen reader: the tree and the keys do the same.
 */
export function DiffFileMap({ ref, scroller, scrollTo, layout, sections, current, onShowSection, onUserScroll }: DiffFileMapProps) {
  const root = useRef<HTMLDivElement>(null)
  const track = useRef<HTMLDivElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const thumb = useRef<HTMLDivElement>(null)
  const shown = useRef<HTMLDivElement>(null)
  const probe = useRef<HTMLSpanElement>(null)
  // the height of the map and of the view, from their observers: a scroll reads no geometry but `scrollTop`
  const [size, setSize] = useState({ width: 0, height: 0, view: 0 })
  const [ratio, setRatio] = useState(() => window.devicePixelRatio || 1)
  const [theme, setTheme] = useState(() => document.documentElement.dataset.theme ?? "")
  const [hover, setHover] = useState<{ index: number; y: number } | null>(null)
  const drag = useRef<{ pointer: number; startY: number; startTop: number } | null>(null)

  const scaleOf = (height: number) => fileMapScale(height, layout.height, size.view)

  /** The thumb at the window of the view, the fill at the section at the top; written to the DOM, no render */
  const sync = useCallback(() => {
    const element = scroller()
    const scale = fileMapScale(size.height, layout.height, size.view)
    if (element === null || thumb.current === null || shown.current === null || root.current === null || scale === 0) {
      return
    }
    const { top, height } = fileMapThumb(element.scrollTop * scale, size.view * scale, size.height, MIN_THUMB)
    thumb.current.style.transform = `translateY(${top}px)`
    thumb.current.style.height = `${height}px`
    const index = current()
    const band = fileMapBand(layout, index)
    if (band === null) {
      shown.current.style.display = "none"
      delete root.current.dataset.current
      return
    }
    shown.current.style.display = ""
    shown.current.style.transform = `translateY(${band.start * scale}px)`
    shown.current.style.height = `${Math.max((band.end - band.start) * scale, MIN_CURRENT)}px`
    root.current.dataset.current = index === PROLOGUE_SECTION ? "prologue" : (sections[index]?.key ?? "")
  }, [scroller, size, layout, sections, current])
  useImperativeHandle(ref, () => ({ sync }))

  // the sizes of the map and of the view, the screen (the zoom of Appearance moves it) and the theme of the ticks, observed
  // once: the view passes a new `scroller` with each layout, and an observer made again for each made a new observation
  // of the view in the frame that laid it out (the heights it measured render at once), a ResizeObserver loop
  const scrollerNow = useEffectEvent(() => scroller())
  useLayoutEffect(() => {
    const element = scrollerNow()
    const area = track.current
    if (element === null || area === null) {
      return
    }
    const measure = () => setSize({ width: area.clientWidth, height: area.clientHeight, view: element.clientHeight })
    measure()
    const resize = new ResizeObserver(measure)
    resize.observe(area)
    resize.observe(element)
    let resolution = matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`)
    const onResolution = () => {
      resolution.removeEventListener("change", onResolution)
      setRatio(window.devicePixelRatio || 1)
      resolution = matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`)
      resolution.addEventListener("change", onResolution)
    }
    resolution.addEventListener("change", onResolution)
    const themes = new MutationObserver(() => setTheme(document.documentElement.dataset.theme ?? ""))
    themes.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] })
    return () => {
      resize.disconnect()
      resolution.removeEventListener("change", onResolution)
      themes.disconnect()
    }
  }, [])

  // the ticks, drawn again for a new layout (a collapse, the row height, the wraps), a size, the screen and the theme;
  // never on a scroll
  useLayoutEffect(() => {
    const surface = canvas.current
    const context = surface?.getContext("2d")
    if (surface === null || context === null || context === undefined || probe.current === null) {
      return
    }
    surface.width = Math.round(size.width * ratio)
    surface.height = Math.round(size.height * ratio)
    context.clearRect(0, 0, surface.width, surface.height)
    const scale = fileMapScale(surface.height, layout.height, size.view)
    if (scale === 0) {
      return
    }
    // the colours of the tokens as the page resolves them: an `oklab()` a canvas of WebKit and of Chromium fills
    const style = getComputedStyle(probe.current)
    const colors = { file: style.color, step: style.backgroundColor }
    const inset = Math.round(FILE_TICK_INSET * ratio)
    for (const tick of fileMapTicks(layout, sections, scale, ratio)) {
      context.globalAlpha = tick.muted ? MUTED_TICK_ALPHA : 1
      context.fillStyle = tick.kind === "step" ? colors.step : colors.file
      const x = tick.kind === "step" ? 0 : inset
      context.fillRect(x, tick.y, surface.width - 2 * x, tick.height)
    }
    sync()
  }, [layout, sections, size, ratio, theme, sync])

  // the thumb follows every scroll of the view: the wheel, the keys, the tree, a drag of it
  useLayoutEffect(() => {
    const element = scroller()
    if (element === null) {
      return
    }
    const onScroll = () => sync()
    element.addEventListener("scroll", onScroll, { passive: true })
    return () => element.removeEventListener("scroll", onScroll)
  }, [scroller, sync])

  /** The section under a point of the map, by the layout: `PROLOGUE_SECTION` in the band of the prologue; null for none */
  const sectionAt = (clientY: number): number | null => {
    const area = track.current
    const scale = scaleOf(size.height)
    if (area === null || scale === 0) {
      return null
    }
    return fileMapSectionAt(layout, (clientY - area.getBoundingClientRect().top) / scale)
  }

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    const element = scroller()
    if (event.button !== 0 || element === null) {
      return
    }
    setHover(null)
    if (event.target === thumb.current) {
      onUserScroll()
      drag.current = { pointer: event.pointerId, startY: event.clientY, startTop: element.scrollTop }
      thumb.current.dataset.dragging = ""
      event.currentTarget.setPointerCapture(event.pointerId)
      return
    }
    const index = sectionAt(event.clientY)
    if (index !== null) {
      onShowSection(index)
    }
  }

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const element = scroller()
    const dragged = drag.current
    if (dragged !== null && dragged.pointer === event.pointerId && element !== null) {
      const scale = scaleOf(size.height)
      if (scale > 0) {
        scrollTo(dragged.startTop + (event.clientY - dragged.startY) / scale)
      }
      return
    }
    // the tip is placed in the map, whose padding insets the track: at the pointer, from the top of the map
    const index = sectionAt(event.clientY)
    setHover(index === null ? null : { index, y: event.clientY - event.currentTarget.getBoundingClientRect().top })
  }

  const endDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (drag.current?.pointer === event.pointerId) {
      drag.current = null
      if (thumb.current !== null) {
        delete thumb.current.dataset.dragging
      }
    }
  }

  // the wheel over the map scrolls the view, as over a scrollbar
  const onWheel = (event: ReactWheelEvent<HTMLDivElement>) => {
    const element = scroller()
    if (element === null) {
      return
    }
    onUserScroll()
    const unit = event.deltaMode === 1 ? layout.metrics.rowHeight : event.deltaMode === 2 ? element.clientHeight : 1
    scrollTo(element.scrollTop + event.deltaY * unit)
  }

  // the press keeps the focus where it is and selects no text: the diffs keep their keys
  const keepFocus = (event: ReactMouseEvent<HTMLDivElement>) => event.preventDefault()

  const hovered = hover === null ? undefined : sections[hover.index]
  const overview = hover?.index === PROLOGUE_SECTION
  const muted = sections.reduce((count, section) => count + (section.muted ? 1 : 0), 0)
  const steps = sections.reduce((count, section) => count + (section.step ? 1 : 0), 0)
  return (
    <div
      ref={root}
      className="dv-map"
      aria-hidden
      data-testid="review-file-map"
      data-sections={sections.length}
      data-steps={steps}
      data-muted={muted}
      onMouseDown={keepFocus}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onPointerLeave={() => setHover(null)}
      onWheel={onWheel}
    >
      <div ref={track} className="dv-map-track" data-testid="review-file-map-track">
        <canvas ref={canvas} className="dv-map-ticks" />
        <div ref={shown} className="dv-map-current" />
        <div ref={thumb} className="dv-map-thumb" data-testid="review-file-map-thumb" />
        {/* the tokens of the ticks, read for the canvas */}
        <span ref={probe} className="dv-map-probe" />
      </div>
      {(hovered !== undefined || overview) && hover !== null && (
        <div
          className="dv-map-tip flex flex-col gap-0.5 rounded-xl bg-popover px-2.5 py-1.5 text-xs text-popover-foreground shadow-md ring-1 ring-foreground/10"
          data-testid="review-file-map-tip"
          style={{ top: hover.y }}
        >
          {hovered === undefined ? (
            <span>Overview</span>
          ) : (
            <>
              <FilePathLabel path={hovered.path} oldPath={hovered.oldPath} status={hovered.status} />
              {hovered.stepHeading !== null && <span className="truncate text-muted-foreground">{hovered.stepHeading}</span>}
            </>
          )}
        </div>
      )}
    </div>
  )
}
