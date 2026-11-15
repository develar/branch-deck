import { useLayoutEffect, useRef } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import type { MenuItem } from "@/lib/menu"
import { cancelIntent } from "@/model/intents"
import { presentStopWaiting } from "@/presenters/lock-wait"
import { useSurfaceOperation } from "@/stores/indicators"

/**
 * Stop Waiting of the operation `surface` started, while its command waits for another holder of the repository lock;
 * nothing otherwise but its live region (`LockWaitStatus`). For a surface whose own label already says whom it waits for
 * (an inline form, a button)
 */
export function StopWaitingButton({ surface }: { surface: string }) {
  const wait = useLockWait(surface)
  return (
    <>
      <LockWaitStatus announcement={wait?.announcement ?? null} />
      {wait !== null && <StopWaiting item={wait.stop} />}
    </>
  )
}

/**
 * Whom the operation `surface` started waits for while another holder has the repository lock ("Waiting for `brd pick`
 * (pid 4242, for 12 s)"): the step `waitingForLock` of its operation in the session, worded by `presentLockWait`
 * (`runRewriteIntent`), with its Stop Waiting (`cancel_operation` of the operation); nothing otherwise but its live region
 * (`LockWaitStatus`). For a surface that shows its own busy state (the squash dialog, the commit message editor, the move
 * form, the squash of the fixups, Apply Fix). It comes where it moves no button the pointer pressed: on the line of the
 * buttons, before them, or below them; its Stop Waiting is as high as theirs
 */
export function LockWaitNotice({ surface, className }: { surface: string; className?: string }) {
  const wait = useLockWait(surface)
  return (
    <>
      <LockWaitStatus announcement={wait?.announcement ?? null} />
      {wait !== null && (
        <span className={cn("flex min-w-0 items-center gap-2 text-xs text-muted-foreground", className)} data-testid="lock-wait-notice">
          {/* its time grows every second: a screen reader hears whom for once, from the live region */}
          <span aria-hidden className="min-w-0 truncate">
            {wait.text}
          </span>
          <StopWaiting item={wait.stop} />
        </span>
      )}
    </>
  )
}

/**
 * The lines of the footer of a dialog beside its buttons (its hints), with the lock wait of `surface` over them while
 * it shows: they keep their room, hidden meanwhile, so the footer keeps its height and the buttons their boxes. A line
 * of its own moved the pressed button down, and one in the row of the buttons squeezed the lines into more of them
 */
export function LinesUnderLockWait({ surface, children }: { surface: string; children: ReactNode }) {
  const waits = useLockWait(surface) !== null
  return (
    <div className="relative flex min-w-0 flex-1 flex-col gap-0.5">
      <div className={cn("flex min-w-0 flex-col gap-0.5", waits && "invisible")}>{children}</div>
      <LockWaitNotice surface={surface} className="absolute top-1/2 left-0 max-w-full -translate-y-1/2" />
    </div>
  )
}

/**
 * The wait of the operation `surface` started for another holder of the repository lock, while it waits: whom for
 * (`presentLockWait`), the same without how long for a screen reader (`presentLockWaitAnnouncement`), and its Stop
 * Waiting; null otherwise
 */
function useLockWait(surface: string): { text: string; announcement: string | null; stop: MenuItem } | null {
  const operation = useSurfaceOperation(surface)
  const stop = presentStopWaiting(operation, cancelIntent)
  return stop === null || operation?.lockWait == null ? null : { text: operation.lockWait, announcement: operation.lockWaitAnnouncement, stop }
}

/**
 * The polite live region of a wait for the repository lock, there before the wait: a screen reader says whom it waits
 * for as it begins and as its holder changes (`presentLockWaitAnnouncement`), where the button says only that it is
 * busy. Out of the flow (`sr-only`), it takes no gap of the line it stands in
 */
function LockWaitStatus({ announcement }: { announcement: string | null }) {
  return (
    <span role="status" className="sr-only" data-testid="lock-wait-status">
      {announcement}
    </span>
  )
}

/**
 * Stop Waiting of a wait the user started on a surface. It takes the focus as the wait shows, from where the press left
 * it: on the pressed button (busy, it stays focusable), elsewhere in its form, dialog or popover, or nowhere (a control
 * that became disabled dropped it), so Enter or Space stops the wait without a Tab. As the wait ends, stopped or over,
 * the focus it held goes back where it came from, else to the button that waited, else to the surface; a focus the user
 * moved elsewhere stays there
 */
function StopWaiting({ item }: { item: MenuItem }) {
  const ref = useRef<HTMLButtonElement>(null)
  // a layout effect: its cleanup runs before the button leaves the document, so the focus goes from it to its target at
  // once. A passive one ran after the button had gone and the focus had fallen to the page, which a popover takes for a
  // focus that left it and closes (the squash of the fixups), under load before the focus came back
  useLayoutEffect(() => {
    const button = ref.current
    if (button === null) {
      return
    }
    const surface = button.closest<HTMLElement>('[role="dialog"]')
    const active = document.activeElement
    const from = active instanceof HTMLElement && active !== document.body && active !== button ? active : null
    const fromPressed = from?.getAttribute("aria-busy") === "true"
    if (from !== null && !fromPressed && surface?.contains(from) !== true) {
      return
    }
    // the button the press left busy, where the focus goes back once it can be pressed again, though the press dropped it
    const pressed = fromPressed ? from : (surface?.querySelector<HTMLElement>('button[aria-busy="true"]') ?? null)
    button.focus()
    return () => {
      // a focus the user moved elsewhere stays there
      if (document.activeElement !== button) {
        return
      }
      for (const target of [from === surface ? null : from, pressed, surface]) {
        if (target?.isConnected === true) {
          target.focus()
          if (document.activeElement === target) {
            return
          }
        }
      }
    }
  }, [])
  return <MenuItemButton ref={ref} item={item} variant="outline" data-testid="stop-waiting" />
}
