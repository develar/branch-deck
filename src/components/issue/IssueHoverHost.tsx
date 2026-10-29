import { lazy, Suspense, useEffect, useEffectEvent, useRef, useState } from "react"
import { Popover, PopoverContent } from "@/components/ui/popover"
import { pointerMoved } from "@/lib/pointer-moves"
import { useYouTrackUrl } from "@/stores/auth"

const IssueDetails = lazy(() => import("@/components/issue/IssueDetails").then(module => ({ default: module.IssueDetails })))

/** The hover waits this long before the popover opens: a pointer that only crosses a list of commits opens none */
const OPEN_DELAY_MS = 500
/** The pointer may cross the gap between the link and the popover */
const CLOSE_DELAY_MS = 200
/** How often an open popover checks that its link is still on the page (a sync, a collapsed branch, a closed toast) */
const ANCHOR_CHECK_MS = 250

/** The layers a link may lie in: the popover goes into the same one, so the layer counts it as its own */
const LAYERS = '[data-slot="popover-content"], [data-slot="dialog-content"], [data-slot="toast-viewport"]'

interface Shown {
  key: string
  anchor: HTMLElement
  container: HTMLElement | undefined
}

/**
 * One issue popover per window (spec/repository/youtrack.spec.md): a hover of half a second, or the keyboard focus, on
 * a link that carries an issue key of the YouTrack signed in to (`LinkedText`) shows the issue; the popover stays
 * while the pointer is over it. One delegated listener instead of a popover per link of every commit list. Signed out,
 * no link carries a key and nothing listens.
 */
export function IssueHoverHost() {
  const youTrackUrl = useYouTrackUrl()
  const [shown, setShown] = useState<Shown | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const popup = useRef<HTMLDivElement>(null)

  const clearTimer = () => {
    if (timer.current !== null) {
      clearTimeout(timer.current)
      timer.current = null
    }
  }
  const schedule = (delay: number, action: () => void) => {
    clearTimer()
    timer.current = setTimeout(() => {
      timer.current = null
      action()
    }, delay)
  }
  const close = () => {
    clearTimer()
    setShown(null)
  }

  // `moved`: the pointer came over the link, or the focus did; a link that came under a still pointer shows nothing
  const onOver = useEffectEvent((link: HTMLElement, moved: boolean) => {
    if (shown?.anchor === link) {
      clearTimer()
      return
    }
    const key = link.dataset.issueKey
    if (key === undefined || !moved) {
      return
    }
    schedule(shown === null ? OPEN_DELAY_MS : CLOSE_DELAY_MS, () => setShown({ key, anchor: link, container: layerOf(link) }))
  })
  const leave = () => schedule(CLOSE_DELAY_MS, () => setShown(null))
  const onLeave = useEffectEvent(leave)

  useEffect(() => {
    if (youTrackUrl === null) {
      return
    }
    const linkOf = (target: EventTarget | null) => (target instanceof Element ? target.closest<HTMLElement>("a[data-issue-key]") : null)
    // the pointer or the focus that goes from a link into the popover keeps it: React tells the popover of the pointer
    // before this listener hears the link left, so a leave scheduled then closed the popover under the pointer
    const intoPopup = (target: EventTarget | null) => target instanceof Node && popup.current?.contains(target) === true
    const over = (event: PointerEvent) => {
      const link = linkOf(event.target)
      if (link !== null) {
        onOver(link, pointerMoved(event))
      }
    }
    const out = (event: PointerEvent) => {
      const link = linkOf(event.target)
      if (link !== null && !link.contains(event.relatedTarget as Node | null) && !intoPopup(event.relatedTarget)) {
        onLeave()
      }
    }
    const focusIn = (event: FocusEvent) => {
      const link = linkOf(event.target)
      if (link?.matches(":focus-visible") === true) {
        onOver(link, true)
      }
    }
    const focusOut = (event: FocusEvent) => {
      if (linkOf(event.target) !== null && !intoPopup(event.relatedTarget)) {
        onLeave()
      }
    }
    document.addEventListener("pointerover", over)
    document.addEventListener("pointerout", out)
    document.addEventListener("focusin", focusIn)
    document.addEventListener("focusout", focusOut)
    return () => {
      document.removeEventListener("pointerover", over)
      document.removeEventListener("pointerout", out)
      document.removeEventListener("focusin", focusIn)
      document.removeEventListener("focusout", focusOut)
    }
  }, [youTrackUrl])

  // Escape closes the popover only: a dialog under it stays
  const onEscape = useEffectEvent((event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.stopPropagation()
      event.preventDefault()
      close()
    }
  })
  useEffect(() => {
    if (shown === null) {
      return
    }
    const escape = (event: KeyboardEvent) => onEscape(event)
    window.addEventListener("keydown", escape, { capture: true })
    const check = setInterval(() => {
      if (!shown.anchor.isConnected) {
        setShown(null)
      }
    }, ANCHOR_CHECK_MS)
    return () => {
      window.removeEventListener("keydown", escape, { capture: true })
      clearInterval(check)
    }
  }, [shown])

  if (shown === null || youTrackUrl === null) {
    return null
  }
  return (
    <Popover open onOpenChange={open => !open && close()}>
      <PopoverContent
        ref={popup}
        anchor={shown.anchor}
        container={shown.container}
        align="start"
        collisionPadding={16}
        initialFocus={false}
        finalFocus={false}
        className="pointer-events-auto w-96 max-w-[calc(100vw-2rem)] gap-3 p-3"
        data-testid="issue-popover"
        data-issue-key={shown.key}
        onPointerEnter={clearTimer}
        onPointerLeave={leave}
      >
        <Suspense fallback={null}>
          <IssueDetails issueKey={shown.key} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}

function layerOf(link: HTMLElement): HTMLElement | undefined {
  return link.closest<HTMLElement>(LAYERS) ?? undefined
}
