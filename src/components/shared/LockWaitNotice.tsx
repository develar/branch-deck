import type { ReactNode } from "react"
import { cn } from "cn"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { cancelIntent } from "@/model/intents"
import { presentStopWaiting } from "@/presenters/lock-wait"
import { useSurfaceOperation } from "@/stores/indicators"

/**
 * Stop Waiting of the operation `surface` started, while its command waits for another holder of the repository lock;
 * nothing otherwise. For a surface whose own label already says whom it waits for (an inline form, a button)
 */
export function StopWaitingButton({ surface }: { surface: string }) {
  const item = presentStopWaiting(useSurfaceOperation(surface), cancelIntent)
  return item === null ? null : <MenuItemButton item={item} variant="outline" data-testid="stop-waiting" />
}

/**
 * Whom the operation `surface` started waits for while another holder has the repository lock ("Waiting for `brd pick`
 * (pid 4242, for 12 s)"): the step `waitingForLock` of its operation in the session, worded by `presentLockWait`
 * (`runRewriteIntent`), with its Stop Waiting (`cancel_operation` of the operation); nothing otherwise. For a surface
 * that shows its own busy state (the squash dialog, the commit message editor, the move form, the squash of the fixups,
 * Apply Fix). It comes where it moves no button the pointer pressed: on the line of the buttons, before them, or below
 * them; its Stop Waiting is as high as theirs
 */
export function LockWaitNotice({ surface, className }: { surface: string; className?: string }) {
  const operation = useSurfaceOperation(surface)
  const item = presentStopWaiting(operation, cancelIntent)
  if (item === null) {
    return null
  }
  return (
    <span className={cn("flex min-w-0 items-center gap-2 text-xs text-muted-foreground", className)} data-testid="lock-wait-notice">
      <span className="min-w-0 truncate">{operation?.lockWait}</span>
      <MenuItemButton item={item} variant="outline" data-testid="stop-waiting" />
    </span>
  )
}

/**
 * The lines of the footer of a dialog beside its buttons (its hints), with the lock wait of `surface` over them while
 * it shows: they keep their room, hidden meanwhile, so the footer keeps its height and the buttons their boxes. A line
 * of its own moved the pressed button down, and one in the row of the buttons squeezed the lines into more of them
 */
export function LinesUnderLockWait({ surface, children }: { surface: string; children: ReactNode }) {
  const waits = presentStopWaiting(useSurfaceOperation(surface), cancelIntent) !== null
  return (
    <div className="relative flex min-w-0 flex-1 flex-col gap-0.5">
      <div className={cn("flex min-w-0 flex-col gap-0.5", waits && "invisible")}>{children}</div>
      <LockWaitNotice surface={surface} className="absolute top-1/2 left-0 max-w-full -translate-y-1/2" />
    </div>
  )
}
